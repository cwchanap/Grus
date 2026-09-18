//! Authoritative combat contracts and the combat fixed step: health, combat
//! orders with their cached pursuit cell, attack cooldowns, target
//! eligibility, damage, pursuit, and atomic unit destruction.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{ATTACK_MOVE_RADIUS, BuildingKind, CombatSpec, unit_spec};
use crate::commands::{UnitIndex, approach_slots, assign_move_toward, reserve_slot};
use crate::economy::{
    ResourceIndex, ResourceSource, WorkerTask, cancel_unit_activity, reroute_dropoff_worker,
};
use crate::ids::{BuildingId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::{MatchPhase, active_phase, gameplay_active, resolve_result};

/// Live and maximum hit points. Spawned units and seeded/placed buildings
/// start at full health from their catalogue spec.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

/// Combat identity of an attackable target, stable across pursuit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatTarget {
    Unit(UnitId),
    Building(BuildingId),
}

/// Current combat intent of a unit. The last known target cell is cached
/// here — there is no separate pursuit component.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum CombatOrder {
    Attack {
        target: CombatTarget,
        last_target_cell: Option<GridPos>,
    },
    AttackMove {
        destination: GridPos,
        target: Option<CombatTarget>,
        last_target_cell: Option<GridPos>,
    },
}

/// Seconds until the next attack is ready; zero means ready.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackCooldown(pub f32);

/// One hit recorded during a single combat tick: attacker/target identity,
/// damage, hit position, ranged/melee, and whether the target died.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatEvent {
    pub attacker: UnitId,
    pub target: CombatTarget,
    pub damage: u32,
    pub position: Vec2,
    pub ranged: bool,
    pub killed: bool,
}

/// Sim-owned drain buffer of current-tick combat events, cleared at the
/// start of every combat step so headless runs cannot accumulate stale
/// events — Result ticks excepted: they hold the settle tick's killing
/// blow until the bridge drains it during presentation.
#[derive(Debug, Default, Resource)]
pub struct CombatEvents(pub Vec<CombatEvent>);

/// Stable lookup of one attackable target: entity, owning team, and health.
pub(crate) fn resolve_target(
    world: &World,
    target: CombatTarget,
) -> Option<(Entity, TeamId, Health)> {
    match target {
        CombatTarget::Unit(id) => {
            let entity = world.get_resource::<UnitIndex>()?.entity(id)?;
            let team = world.get::<Unit>(entity)?.team;
            let health = *world.get::<Health>(entity)?;
            Some((entity, team, health))
        }
        CombatTarget::Building(id) => {
            let entity = world.get_resource::<BuildingIndex>()?.entity(id)?;
            let team = world.get::<Building>(entity)?.team;
            let health = *world.get::<Health>(entity)?;
            Some((entity, team, health))
        }
    }
}

/// The single eligibility seam for every direct attack and attack-move
/// acquisition: the target exists in its index, has live Health, and belongs
/// to another team. Buildings included; visibility lands in HPA-473 here.
pub fn target_eligible(world: &World, attacker_team: TeamId, target: CombatTarget) -> bool {
    resolve_target(world, target)
        .is_some_and(|(_, team, health)| team != attacker_team && health.current > 0)
}

/// Advances combat one fixed tick; runs before movement. Clears
/// `CombatEvents` (Result ticks preserve the settle tick's buffer for the
/// bridge drain), decrements cooldowns, then lets every attacker act in
/// ascending stable-ID order: refresh/validate its target, acquire one for
/// AttackMove, strike when in range and cooldown-ready, or refresh pursuit.
/// An attacker destroyed earlier in the same step is skipped via its
/// `UnitIndex` re-resolution; dead units and buildings are destroyed
/// atomically.
pub fn step_combat(world: &mut World, map: &mut GridMap, seconds: f32) {
    // Draining the presentation-feedback buffer is bookkeeping, not
    // gameplay: it must happen even on frozen ticks, or the last Playing
    // tick's events survive a pause and replay on the first resumed frame.
    // Result ticks are the exception: `strike()` records the killing blow
    // and resolves the match in the same fixed tick, and at high sim speed
    // several more frozen ticks can run before the bridge's next `_process`
    // drain — clearing here would erase the settle tick's events before
    // they ever present.
    if world.get_resource::<CombatEvents>().is_none() {
        world.init_resource::<CombatEvents>();
    }
    if !matches!(active_phase(world), MatchPhase::Result(_)) {
        world.resource_mut::<CombatEvents>().0.clear();
    }
    if !gameplay_active(world) {
        return;
    }

    let mut cooldowns = world.query::<&mut AttackCooldown>();
    for mut cooldown in cooldowns.iter_mut(world) {
        cooldown.0 = (cooldown.0 - seconds).max(0.0);
    }

    let mut attacker_ids: Vec<UnitId> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(id, _)| *id).collect())
        .unwrap_or_default();
    attacker_ids.sort_unstable();

    for id in attacker_ids {
        // A Town Center death mid-step settles the Result and freezes combat
        // for every later attacker in this same tick — first destruction wins.
        if !gameplay_active(world) {
            break;
        }
        // Re-resolve before acting: an earlier attacker may have destroyed
        // this one's entity. Never keep a stale handle and blindly mutate.
        let Some(entity) = world
            .get_resource::<UnitIndex>()
            .and_then(|index| index.entity(id))
        else {
            continue;
        };
        let Some(unit) = world.get::<Unit>(entity).copied() else {
            continue;
        };
        let Some(spec) = unit_spec(unit.kind).combat else {
            continue;
        };
        let Some(order) = world.get::<CombatOrder>(entity).cloned() else {
            continue;
        };
        let Some(position) = world.get::<SimPosition>(entity).copied() else {
            continue;
        };
        let cooldown_ready = world
            .get::<AttackCooldown>(entity)
            .is_some_and(|cooldown| cooldown.0 <= 0.0);

        let order = match refresh_target(world, unit.team, position.current, order) {
            Some(order) => order,
            None => {
                // Direct Attack: the target died or vanished, the order ends.
                let mut entity = world.entity_mut(entity);
                entity.remove::<CombatOrder>();
                entity.remove::<MoveOrder>();
                continue;
            }
        };
        world.entity_mut(entity).insert(order.clone());

        let Some(target) = current_target(&order) else {
            // AttackMove without a target keeps moving toward its destination.
            if let CombatOrder::AttackMove { destination, .. } = order {
                resume_destination(world, map, entity, &position, destination);
            }
            continue;
        };
        let Some(point) = target_position(world, position.current, target) else {
            continue;
        };

        if position.current.distance(point) <= spec.attack_range {
            // Already in range: never path, and drop whatever leg is active.
            // A pursuit leg ends here so a pursuer stops closing once it can
            // strike; an AttackMove destination leg ends too — keeping it
            // would march the unit out of range mid-fight and force a
            // pursuit path back. Once the target dies or clears,
            // resume_destination re-paths toward the destination.
            world.entity_mut(entity).remove::<MoveOrder>();
            if cooldown_ready {
                strike(world, map, entity, &unit, spec, target, point);
            }
        } else {
            pursue(world, map, entity, &position, &order, target);
        }
    }
}

/// Target of the order, if it holds one.
fn current_target(order: &CombatOrder) -> Option<CombatTarget> {
    match order {
        CombatOrder::Attack { target, .. } => Some(*target),
        CombatOrder::AttackMove { target, .. } => *target,
    }
}

/// Validates/refreshes the order's target: a dead or vanished target ends a
/// Direct Attack (`None`) and is cleared from an AttackMove, which then
/// acquires the nearest eligible target within `ATTACK_MOVE_RADIUS`. The
/// pursuit cache of a retained target is left untouched so pursuit can
/// detect target movement; a cleared or newly acquired target resets it to
/// `None` so the first `pursue` assigns a route immediately — the
/// still-active destination route must not read as an active pursuit leg.
fn refresh_target(
    world: &World,
    attacker_team: TeamId,
    attacker_position: Vec2,
    order: CombatOrder,
) -> Option<CombatOrder> {
    match order {
        CombatOrder::Attack { target, .. } => {
            target_eligible(world, attacker_team, target).then_some(order)
        }
        CombatOrder::AttackMove {
            destination,
            target: input_target,
            last_target_cell,
        } => {
            let target = match input_target {
                Some(target) if target_eligible(world, attacker_team, target) => Some(target),
                _ => None,
            };
            let target = match target {
                Some(target) => Some(target),
                None => acquire_target(world, attacker_team, attacker_position),
            };
            let retained = input_target.is_some() && input_target == target;
            let last_target_cell = if retained { last_target_cell } else { None };
            Some(CombatOrder::AttackMove {
                destination,
                target,
                last_target_cell,
            })
        }
    }
}

/// Nearest eligible target within `ATTACK_MOVE_RADIUS`: distance ties break
/// by stable identity, units before buildings.
fn acquire_target(world: &World, attacker_team: TeamId, position: Vec2) -> Option<CombatTarget> {
    let mut candidates: Vec<(f32, u8, u32, CombatTarget)> = Vec::new();
    if let Some(index) = world.get_resource::<UnitIndex>() {
        for (id, _) in index.iter() {
            let target = CombatTarget::Unit(*id);
            if let Some(point) = target_position(world, position, target) {
                candidates.push((position.distance(point), 0, id.0, target));
            }
        }
    }
    if let Some(index) = world.get_resource::<BuildingIndex>() {
        for (id, entity) in index.iter() {
            let Some(footprint) = world.get::<Footprint>(*entity) else {
                continue;
            };
            let point = footprint.closest_point(position);
            candidates.push((
                position.distance(point),
                1,
                id.0,
                CombatTarget::Building(*id),
            ));
        }
    }
    candidates
        .into_iter()
        .filter(|(_, _, _, target)| target_eligible(world, attacker_team, *target))
        .filter(|(distance, ..)| *distance <= ATTACK_MOVE_RADIUS)
        .min_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        })
        .map(|(_, _, _, target)| target)
}

/// The point of the target that range and acquisition distance are measured
/// against: a unit's `SimPosition`, or the nearest point of a building's
/// footprint — never a footprint center.
fn target_position(world: &World, attacker_position: Vec2, target: CombatTarget) -> Option<Vec2> {
    match target {
        CombatTarget::Unit(id) => world
            .get_resource::<UnitIndex>()?
            .entity(id)
            .and_then(|entity| world.get::<SimPosition>(entity))
            .map(|position| position.current),
        CombatTarget::Building(id) => world
            .get_resource::<BuildingIndex>()?
            .entity(id)
            .and_then(|entity| world.get::<Footprint>(entity))
            .map(|footprint| footprint.closest_point(attacker_position)),
    }
}

/// Pursues the target: unit pursuit reuses the route until the target
/// changes grid cell or the route ends; a static building's pursuit reuses
/// its active leg (the route whose goal is the cached pursuit slot) and
/// otherwise — fresh assignment or route end — picks the
/// nearest-perimeter walkable slot (sorted by attacker distance here, at
/// the combat call site only).
fn pursue(
    world: &mut World,
    map: &GridMap,
    attacker: Entity,
    position: &SimPosition,
    order: &CombatOrder,
    target: CombatTarget,
) {
    match target {
        CombatTarget::Unit(_) => {
            let Some(target_cell) =
                target_position(world, position.current, target).map(world_to_cell)
            else {
                return;
            };
            let last_target_cell = last_target_cell(order);
            let route_active = world.get::<MoveOrder>(attacker).is_some();
            if last_target_cell == Some(target_cell) && route_active {
                return;
            }
            assign_move_toward(
                world,
                map,
                attacker,
                world_to_cell(position.current),
                target_cell,
            );
            world
                .entity_mut(attacker)
                .insert(with_last_target_cell(order, Some(target_cell)));
        }
        CombatTarget::Building(id) => {
            // A leg whose goal is the cached pursuit slot is still en route
            // to this building — keep it. Any other active route is the
            // AttackMove destination (or a stale leg) and must not block
            // engaging the target.
            let route_goal = world.get::<MoveOrder>(attacker).map(|order| order.goal);
            if route_goal.is_some() && last_target_cell(order) == route_goal {
                return;
            }
            let Some(footprint) = world
                .get_resource::<BuildingIndex>()
                .and_then(|index| index.entity(id))
                .and_then(|entity| world.get::<Footprint>(entity))
                .copied()
            else {
                return;
            };
            let start = world_to_cell(position.current);
            let mut used: HashMap<GridPos, usize> = HashMap::new();
            let others: Vec<Entity> = world
                .get_resource::<UnitIndex>()
                .map(|index| index.iter().map(|(_, entity)| *entity).collect())
                .unwrap_or_default();
            for other in others {
                if other == attacker {
                    continue;
                }
                if let Some(position) = world.get::<SimPosition>(other) {
                    reserve_slot(&mut used, world_to_cell(position.current));
                }
                if let Some(order) = world.get::<MoveOrder>(other) {
                    reserve_slot(&mut used, order.goal);
                }
            }
            let keys: HashSet<GridPos> = used.keys().copied().collect();
            let mut candidates =
                approach_slots(map, footprint, &keys, footprint.perimeter_cells().len());
            // Combat-site-only ordering: nearest perimeter cell to the
            // attacker first. Stable sort keeps approach_slots' ring order on
            // distance ties. The global approach_slots ordering is unchanged.
            candidates.sort_by(|a, b| {
                let distance_a = map.cell_center(*a).distance_squared(position.current);
                let distance_b = map.cell_center(*b).distance_squared(position.current);
                distance_a.total_cmp(&distance_b)
            });
            for slot in candidates {
                if let Some(path) = map.find_path(start, slot) {
                    let waypoints: Vec<Vec2> = path
                        .into_iter()
                        .skip(1)
                        .map(|cell| map.cell_center(cell))
                        .collect();
                    if !waypoints.is_empty() {
                        world.entity_mut(attacker).insert(MoveOrder {
                            waypoints,
                            next: 0,
                            goal: slot,
                            map_revision: map.revision(),
                            last_failed_replan: None,
                        });
                        world
                            .entity_mut(attacker)
                            .insert(with_last_target_cell(order, Some(slot)));
                    }
                    break;
                }
            }
        }
    }
}

fn world_to_cell(point: Vec2) -> GridPos {
    GridPos::new(point.x.floor() as i32, point.y.floor() as i32)
}

fn last_target_cell(order: &CombatOrder) -> Option<GridPos> {
    match order {
        CombatOrder::Attack {
            last_target_cell, ..
        }
        | CombatOrder::AttackMove {
            last_target_cell, ..
        } => *last_target_cell,
    }
}

fn with_last_target_cell(order: &CombatOrder, cell: Option<GridPos>) -> CombatOrder {
    match *order {
        CombatOrder::Attack { target, .. } => CombatOrder::Attack {
            target,
            last_target_cell: cell,
        },
        CombatOrder::AttackMove {
            destination,
            target,
            ..
        } => CombatOrder::AttackMove {
            destination,
            target,
            last_target_cell: cell,
        },
    }
}

/// Reassigns the AttackMove route toward its destination. A leg already
/// bound for the destination is kept; any other active leg is a stale
/// pursuit route to a dead target's cell and is replaced now instead of
/// being waited out.
fn resume_destination(
    world: &mut World,
    map: &GridMap,
    attacker: Entity,
    position: &SimPosition,
    destination: GridPos,
) {
    if world
        .get::<MoveOrder>(attacker)
        .is_some_and(|order| order.goal == destination)
    {
        return;
    }
    assign_move_toward(
        world,
        map,
        attacker,
        world_to_cell(position.current),
        destination,
    );
}

/// Lands one validated hit: counter bonus only against the named unit kind,
/// base damage only against buildings, instant application, cooldown
/// installed on strike, event recorded, and atomic destruction of a dead
/// unit or building.
fn strike(
    world: &mut World,
    map: &mut GridMap,
    attacker: Entity,
    unit: &Unit,
    spec: CombatSpec,
    target: CombatTarget,
    position: Vec2,
) {
    let Some((target_entity, _, mut health)) = resolve_target(world, target) else {
        return;
    };
    let damage = match target {
        CombatTarget::Unit(_) => {
            let defender_kind = world.get::<Unit>(target_entity).map(|unit| unit.kind);
            spec.damage + u32::from(defender_kind == Some(spec.counter_target)) * spec.counter_bonus
        }
        CombatTarget::Building(_) => spec.damage,
    };
    health.current = health.current.saturating_sub(damage);
    let killed = health.current == 0;
    if let Some(mut state) = world.get_mut::<Health>(target_entity) {
        *state = health;
    }
    if let Some(mut cooldown) = world.get_mut::<AttackCooldown>(attacker) {
        cooldown.0 = spec.cooldown_seconds;
    }
    if let Some(mut events) = world.get_resource_mut::<CombatEvents>() {
        events.0.push(CombatEvent {
            attacker: unit.id,
            target,
            damage,
            position,
            ranged: spec.ranged,
            killed,
        });
    }
    if killed {
        match target {
            CombatTarget::Unit(_) => destroy_unit(world, target_entity),
            CombatTarget::Building(_) => {
                // Result resolves immediately on Town Center destruction.
                let town_center = world
                    .get::<Building>(target_entity)
                    .is_some_and(|building| building.kind == BuildingKind::TownCenter);
                destroy_building(world, map, target_entity);
                if town_center {
                    resolve_result(world, unit.team);
                }
            }
        }
    }
}

/// Atomic unit destruction: releases every worker/building/Farm assignment
/// through the shared cleanup helper (carried resources die with the
/// entity), removes the stable index entry, and despawns.
pub(crate) fn destroy_unit(world: &mut World, entity: Entity) {
    cancel_unit_activity(world, entity);
    if let Some(id) = world.get::<Unit>(entity).map(|unit| unit.id)
        && let Some(mut index) = world.get_resource_mut::<UnitIndex>()
    {
        index.remove(id);
    }
    world.despawn(entity);
}

/// Atomic building/site destruction (completed buildings and construction
/// sites share one transaction; sites simply lack production/farm extras):
/// capture footprint and Farm resource identity, free the footprint cells,
/// remove the `BuildingIndex` and Farm `ResourceIndex` entries, retask every
/// live worker whose task references the destroyed identity, then despawn —
/// the production queue and derived population capacity fall out with the
/// entity, and living units above a reduced cap are never deleted.
///
/// Worker retasking: `ToConstruction`/`Constructing` cancel to Idle through
/// the existing cancellation; a destroyed Farm idles its workers with Carry
/// preserved in every phase (never standalone `deplete_source()`); a
/// destroyed drop-off reroutes carrying workers immediately through the
/// economy's narrow routing seam, else idles them preserving Carry.
pub(crate) fn destroy_building(world: &mut World, map: &mut GridMap, entity: Entity) {
    let Some(footprint) = world.get::<Footprint>(entity).copied() else {
        return;
    };
    let building_id = world.get::<Building>(entity).map(|building| building.id);
    let farm_source = world.get::<ResourceSource>(entity).map(|source| source.id);

    for cell in footprint.cells() {
        map.set_blocked(cell, false);
    }
    if let Some(id) = building_id
        && let Some(mut index) = world.get_resource_mut::<BuildingIndex>()
    {
        index.remove(id);
    }
    if let Some(source) = farm_source
        && let Some(mut index) = world.get_resource_mut::<ResourceIndex>()
    {
        index.remove(source);
    }

    // Scan every live WorkerTask referencing the destroyed identity — not
    // just active-builder or assigned-worker bookkeeping. Collect first so
    // the retasking mutations never fight the query borrow.
    let mut affected: Vec<(Entity, WorkerTask)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask)>();
        for (worker, task) in query.iter(world) {
            let references_destroyed = match *task {
                WorkerTask::ToConstruction { building, .. }
                | WorkerTask::Constructing { building } => Some(building) == building_id,
                WorkerTask::ToDropoff {
                    source, dropoff, ..
                } => Some(dropoff) == building_id || Some(source) == farm_source,
                WorkerTask::ToSource { source, .. } | WorkerTask::Gathering { source } => {
                    Some(source) == farm_source
                }
                WorkerTask::Idle => false,
            };
            if references_destroyed {
                affected.push((worker, task.clone()));
            }
        }
    }
    for (worker, task) in affected {
        match task {
            WorkerTask::ToConstruction { .. } | WorkerTask::Constructing { .. } => {
                cancel_unit_activity(world, worker);
            }
            WorkerTask::ToDropoff { source, .. } if Some(source) == farm_source => {
                // Returning a destroyed Farm's load: idle, Carry preserved.
                cancel_unit_activity(world, worker);
            }
            WorkerTask::ToDropoff { source, .. } => {
                reroute_dropoff_worker(world, map, worker, source);
            }
            WorkerTask::ToSource { .. } | WorkerTask::Gathering { .. } => {
                cancel_unit_activity(world, worker);
            }
            WorkerTask::Idle => {}
        }
    }

    world.despawn(entity);
}

#[cfg(test)]
mod tests;
