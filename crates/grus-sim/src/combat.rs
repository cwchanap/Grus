//! Authoritative combat contracts and the combat fixed step: health, combat
//! orders with their cached pursuit cell, attack cooldowns, target
//! eligibility, damage, pursuit, and atomic unit destruction.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{ATTACK_MOVE_RADIUS, BuildingKind, CombatSpec, building_spec, unit_spec};
use crate::commands::{
    UnitIndex, approach_slots, assign_move_toward, destination_slots, reserve_slot,
};
use crate::economy::{TeamEconomy, WorkerTask, cancel_unit_activity, reroute_dropoff_worker};
use crate::ids::{BuildingId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::{MatchPhase, active_phase, gameplay_active, resolve_result};
use crate::visibility::{VisibilityMap, VisibilitySubject, visible_to};

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
        /// The assigned destination leg's goal: the cell the
        /// reservation-aware planner accepted, which may neighbor the
        /// clicked cell. Compared against the active `MoveOrder`'s goal to
        /// recognize the leg as its own.
        destination: GridPos,
        target: Option<CombatTarget>,
        last_target_cell: Option<GridPos>,
    },
}

/// Seconds until the next attack is ready; zero means ready.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackCooldown(pub f32);

/// Records that pursuit of `target` found no reachable route on the static
/// map at `revision` — every approach slot for a building target, or every
/// destination slot for the target unit's current cell (`cell` anchors the
/// check). While the stamp holds, `pursue` skips its pathing instead of
/// re-running a doomed A* every tick; a map-revision change or a move to a
/// different cell reopens the attempt. Waves retry treats the stamp like a
/// failed route so a blocked alien is re-tasked to another structure.
/// `cancel_unit_activity` clears it so a new command starts fresh.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct PursuitBlocked {
    pub target: CombatTarget,
    pub cell: GridPos,
    pub revision: u64,
}

/// One hit recorded during a single combat tick: attacker/target identity,
/// damage, hit position, ranged/melee, and whether the target died.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatEvent {
    pub attacker: CombatTarget,
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

/// Monotonic count of strikes recorded in this world: unlike the per-tick
/// `CombatEvents` buffer (cleared every non-Result tick, then consumed by
/// the presentation drain), this never rewinds, so a second reader can
/// detect combat activity without consuming the buffer another reader
/// owns. Reset paths drop it with `CombatEvents` so a reseeded world starts
/// at zero.
#[derive(Debug, Default, Resource)]
pub struct CombatEventTotal(pub u64);

#[derive(Clone, Copy)]
struct AttackerContext {
    entity: Entity,
    identity: CombatTarget,
    team: TeamId,
    spec: CombatSpec,
}

/// Stable lookup of one attackable target: entity, owning team, and health.
/// The companion of `target_eligible`: callers that already resolved a
/// target reuse this rather than re-walking the indexes.
pub fn resolve_target(world: &World, target: CombatTarget) -> Option<(Entity, TeamId, Health)> {
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

/// The one eligibility rule every combat target passes: foreign to the
/// attacker, live, and currently visible. `target_eligible` applies it to a
/// stored target resolved through the world; `acquire_target` applies the
/// same rule to `AcquisitionSnapshot` rows, so a rule change lands on both
/// consumers at once.
fn target_passes(attacker_team: TeamId, team: TeamId, alive: bool, visible: bool) -> bool {
    team != attacker_team && alive && visible
}

/// The eligibility check for a stored combat target: the target exists in
/// its index and satisfies `target_passes`, with visibility resolved
/// through the one `visible_to` predicate, unconditionally (a unit by its
/// current cell, a building by its footprint). Without a `VisibilityMap`
/// the predicate passes everything, so pure-sim tests keep full
/// information.
pub fn target_eligible(world: &World, attacker_team: TeamId, target: CombatTarget) -> bool {
    resolve_target(world, target).is_some_and(|(entity, team, health)| {
        target_passes(
            attacker_team,
            team,
            health.current > 0,
            target_visible(world, attacker_team, target, entity),
        )
    })
}

/// Combat visibility through the one public predicate; no consumer picks a
/// second visibility rule.
fn target_visible(world: &World, team: TeamId, target: CombatTarget, entity: Entity) -> bool {
    match target {
        CombatTarget::Unit(_) => {
            let position = world
                .get::<SimPosition>(entity)
                .expect("indexed unit carries a SimPosition");
            visible_to(world, team, world_to_cell(position.current))
        }
        CombatTarget::Building(_) => {
            let footprint = world
                .get::<Footprint>(entity)
                .expect("indexed building carries a Footprint");
            visible_to(world, team, *footprint)
        }
    }
}

/// Advances combat one fixed tick; runs before movement. Clears
/// `CombatEvents` (Result ticks preserve the settle tick's buffer for the
/// bridge drain), decrements cooldowns, then lets every attacker act in
/// ascending stable-ID order: units first, then completed Team-1 Gun
/// Turrets. Units refresh/validate their orders, acquire one for AttackMove,
/// strike when in range and cooldown-ready, or refresh pursuit. Turrets use
/// the same acquisition snapshot and strike path without an order or route.
/// An attacker destroyed earlier in the step is skipped through its stable
/// index; dead units and buildings are destroyed atomically.
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
    if world.get_resource::<CombatEventTotal>().is_none() {
        world.init_resource::<CombatEventTotal>();
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
    // One acquisition table for the whole step: re-resolving every
    // candidate through the world per scanning attacker was the benchmark's
    // dominant per-tick cost. Positions and footprints cannot change
    // mid-step; liveness changes only through `strike`, which re-resolves
    // the struck entry afterward.
    let mut acquisition = AcquisitionSnapshot::build(world);

    for id in attacker_ids {
        // A Research Center death mid-step settles the Result and freezes combat
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

        let order = match refresh_target(world, &acquisition, unit.team, position.current, order) {
            Some(order) => order,
            None => {
                // Direct Attack: the target died, vanished, or left vision —
                // the order and its pursuit route end here.
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
                resume_destination(world, map, entity, &position, &order, destination);
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
                strike(
                    world,
                    map,
                    AttackerContext {
                        entity,
                        identity: CombatTarget::Unit(unit.id),
                        team: unit.team,
                        spec,
                    },
                    target,
                    point,
                );
                acquisition.refresh_struck(world, target);
            }
        } else {
            pursue(world, map, entity, &position, &order, target);
        }
    }

    // Turrets have no CombatOrder or movement route. Keep the existing unit
    // attacker pass untouched, then process completed Team-1 combat
    // buildings in stable building-ID order against the same per-tick
    // acquisition table. Filter on the combat spec before sorting so walls
    // and other passive buildings never join the pass.
    let mut building_ids: Vec<BuildingId> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter(|(_, entity)| {
                    world.get::<Building>(**entity).is_some_and(|building| {
                        building.team == TeamId(1)
                            && building.construction.complete
                            && building_spec(building.kind).combat.is_some()
                    })
                })
                .map(|(id, _)| *id)
                .collect()
        })
        .unwrap_or_default();
    building_ids.sort_unstable();
    for id in building_ids {
        if !gameplay_active(world) {
            break;
        }
        let Some(entity) = world
            .get_resource::<BuildingIndex>()
            .and_then(|index| index.entity(id))
        else {
            continue;
        };
        let Some(building) = world.get::<Building>(entity) else {
            continue;
        };
        if building.team != TeamId(1) || !building.construction.complete {
            continue;
        }
        let Some(spec) = building_spec(building.kind).combat else {
            continue;
        };
        let team = building.team;
        let Some(footprint) = world.get::<Footprint>(entity).copied() else {
            continue;
        };
        if !world
            .get::<AttackCooldown>(entity)
            .is_some_and(|cooldown| cooldown.0 <= 0.0)
        {
            continue;
        }
        let target = acquire_target(
            world,
            &acquisition,
            team,
            footprint.center(),
            spec.attack_range,
            Some(footprint),
            true,
        );
        let Some(target) = target else {
            continue;
        };
        let Some(point) = target_position(world, footprint.center(), target) else {
            continue;
        };
        strike(
            world,
            map,
            AttackerContext {
                entity,
                identity: CombatTarget::Building(id),
                team,
                spec,
            },
            target,
            point,
        );
        acquisition.refresh_struck(world, target);
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
    acquisition: &AcquisitionSnapshot,
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
                None => acquire_target(
                    world,
                    acquisition,
                    attacker_team,
                    attacker_position,
                    ATTACK_MOVE_RADIUS,
                    None,
                    false,
                ),
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

/// Per-tick table of every acquirable target, collected in one pass over
/// the live indices at step start. `acquire_target` scans this pure data —
/// the shared `target_passes` eligibility rule, distance key, and `min_by`
/// ordering — instead of re-resolving each candidate through `World` for
/// every scanning attacker. `strike` is the step's only target-state
/// mutation, so the struck entry alone is refreshed from live state after
/// each strike.
#[derive(Default)]
struct AcquisitionSnapshot {
    units: Vec<UnitCandidate>,
    buildings: Vec<BuildingCandidate>,
}

struct UnitCandidate {
    id: UnitId,
    team: TeamId,
    alien: bool,
    position: Vec2,
    cell: GridPos,
    alive: bool,
}

struct BuildingCandidate {
    id: BuildingId,
    team: TeamId,
    footprint: Footprint,
    alive: bool,
}

impl AcquisitionSnapshot {
    /// Indexed entries only, mirroring `resolve_target`/`target_position`:
    /// a unit missing `Unit`, `SimPosition`, or `Health` — or a building
    /// missing `Building`, `Footprint`, or `Health` — never enters the
    /// table, exactly as the live scans skipped it.
    fn build(world: &World) -> Self {
        let mut snapshot = Self::default();
        if let Some(index) = world.get_resource::<UnitIndex>() {
            for (id, entity) in index.iter() {
                let (Some(unit), Some(position), Some(health)) = (
                    world.get::<Unit>(*entity),
                    world.get::<SimPosition>(*entity),
                    world.get::<Health>(*entity),
                ) else {
                    continue;
                };
                snapshot.units.push(UnitCandidate {
                    id: *id,
                    team: unit.team,
                    alien: unit_spec(unit.kind).alien.is_some(),
                    position: position.current,
                    cell: world_to_cell(position.current),
                    alive: health.current > 0,
                });
            }
        }
        if let Some(index) = world.get_resource::<BuildingIndex>() {
            for (id, entity) in index.iter() {
                let (Some(building), Some(footprint), Some(health)) = (
                    world.get::<Building>(*entity),
                    world.get::<Footprint>(*entity),
                    world.get::<Health>(*entity),
                ) else {
                    continue;
                };
                snapshot.buildings.push(BuildingCandidate {
                    id: *id,
                    team: building.team,
                    footprint: *footprint,
                    alive: health.current > 0,
                });
            }
        }
        snapshot
    }

    /// Re-resolves one struck target against the live world: a destroyed or
    /// zero-health entry is marked dead so no later attacker this step
    /// acquires it — the same mid-step view the live-index scan gave.
    fn refresh_struck(&mut self, world: &World, target: CombatTarget) {
        let alive = resolve_target(world, target).is_some_and(|(_, _, health)| health.current > 0);
        let entry = match target {
            CombatTarget::Unit(id) => self
                .units
                .iter_mut()
                .find(|unit| unit.id == id)
                .map(|unit| &mut unit.alive),
            CombatTarget::Building(id) => self
                .buildings
                .iter_mut()
                .find(|building| building.id == id)
                .map(|building| &mut building.alive),
        };
        if let Some(alive_flag) = entry {
            *alive_flag = alive;
        }
    }
}

/// Nearest eligible target within `range`: distance ties break
/// by stable identity, units before buildings. Reads the per-tick
/// `AcquisitionSnapshot`; the `VisibilityMap` resource fetch is hoisted to
/// once per call, and the visibility check is the same `visible_to`
/// contract (no map means everything is visible). A building attacker uses
/// its closest footprint point to each candidate; `alien_units_only` keeps
/// turrets on the enemy unit kinds and out of building targets.
fn acquire_target(
    world: &World,
    acquisition: &AcquisitionSnapshot,
    attacker_team: TeamId,
    position: Vec2,
    range: f32,
    attacker_footprint: Option<Footprint>,
    alien_units_only: bool,
) -> Option<CombatTarget> {
    let visibility = world.get_resource::<VisibilityMap>();
    let visible = |subject: VisibilitySubject| match visibility {
        None => true,
        Some(map) => map.subject_visible(attacker_team, &subject),
    };
    let mut candidates: Vec<(f32, u8, u32, CombatTarget)> = Vec::new();
    // Radius first: the visibility lookup only runs for the few candidates
    // actually in reach.
    for unit in &acquisition.units {
        if alien_units_only && !unit.alien {
            continue;
        }
        let origin =
            attacker_footprint.map_or(position, |footprint| footprint.closest_point(unit.position));
        let distance = origin.distance(unit.position);
        if distance <= range
            && target_passes(
                attacker_team,
                unit.team,
                unit.alive,
                visible(unit.cell.into()),
            )
        {
            candidates.push((distance, 0, unit.id.0, CombatTarget::Unit(unit.id)));
        }
    }
    if !alien_units_only {
        for building in &acquisition.buildings {
            let target_point = building.footprint.closest_point(position);
            let origin = attacker_footprint
                .map_or(position, |footprint| footprint.closest_point(target_point));
            let distance = origin.distance(target_point);
            if distance <= range
                && target_passes(
                    attacker_team,
                    building.team,
                    building.alive,
                    visible(building.footprint.into()),
                )
            {
                candidates.push((
                    distance,
                    1,
                    building.id.0,
                    CombatTarget::Building(building.id),
                ));
            }
        }
    }
    candidates
        .into_iter()
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
            if pursuit_blocked(world, map, attacker, target, target_cell) {
                return;
            }
            let start = world_to_cell(position.current);
            let assigned = assign_move_toward(world, map, attacker, start, target_cell).is_some();
            if assigned {
                world.entity_mut(attacker).remove::<PursuitBlocked>();
            } else {
                // Stamp only a static seal — every reservation-free
                // destination cell unreachable. Reservation-only failures
                // can clear when other units move, without a map change, so
                // they stay transient and retry next tick.
                let Some(team) = world.get::<Unit>(attacker).map(|unit| unit.team) else {
                    return;
                };
                let static_destinations: HashSet<GridPos> =
                    destination_slots(map, target_cell, 1, &HashSet::new())
                        .into_iter()
                        .collect();
                let mut pending = static_destinations.clone();
                let parents = map.reachable_until(team, start, |cell| {
                    pending.remove(cell);
                    pending.is_empty()
                });
                if static_destinations
                    .iter()
                    .all(|slot| *slot != start && !parents.contains_key(slot))
                {
                    world.entity_mut(attacker).insert(PursuitBlocked {
                        target,
                        cell: target_cell,
                        revision: map.revision(),
                    });
                }
            }
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
            let Some(team) = world.get::<Unit>(attacker).map(|unit| unit.team) else {
                return;
            };
            // A pursuit proven sealed at this map revision waits for the
            // next map change instead of re-running the flood every tick.
            if pursuit_blocked(world, map, attacker, target, footprint.anchor) {
                return;
            }
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
            // One flood from the attacker answers every candidate's
            // reachability — per-slot A* would search the same unreachable
            // region once per ring cell. It stops as soon as the full static
            // ring (candidates ignoring reservations) has been examined, so
            // the seal check below is decided against every possible slot,
            // not only the unreserved ones.
            let static_slots: HashSet<GridPos> =
                approach_slots(map, footprint, &HashSet::new(), usize::MAX)
                    .into_iter()
                    .collect();
            let mut pending = static_slots.clone();
            let parents = map.reachable_until(team, start, |cell| {
                pending.remove(cell);
                pending.is_empty()
            });
            let reached = |slot: GridPos| slot == start || parents.contains_key(&slot);
            if let Some(slot) = candidates.into_iter().find(|slot| reached(*slot)) {
                if let Some(path) = map.flood_path(start, &parents, slot) {
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
                }
                world.entity_mut(attacker).remove::<PursuitBlocked>();
                return;
            }
            // Stamp only a static seal — no approach cell reachable even
            // ignoring reservations. Unreserved-failures from crowding stay
            // transient and retry next tick like before.
            if static_slots.iter().all(|slot| !reached(*slot)) {
                world.entity_mut(attacker).insert(PursuitBlocked {
                    target,
                    cell: footprint.anchor,
                    revision: map.revision(),
                });
            }
        }
    }
}

/// Whether `attacker`'s recorded pursuit failure still holds: same target,
/// same cell anchor, same static-map revision.
fn pursuit_blocked(
    world: &World,
    map: &GridMap,
    attacker: Entity,
    target: CombatTarget,
    cell: GridPos,
) -> bool {
    world
        .get::<PursuitBlocked>(attacker)
        .is_some_and(|blocked| {
            blocked.target == target && blocked.cell == cell && blocked.revision == map.revision()
        })
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

/// Returns the order re-anchored to a different assigned destination goal.
/// Direct Attack has no destination and is returned unchanged.
pub(crate) fn with_destination(order: &CombatOrder, destination: GridPos) -> CombatOrder {
    match *order {
        CombatOrder::Attack { .. } => order.clone(),
        CombatOrder::AttackMove {
            target,
            last_target_cell,
            ..
        } => CombatOrder::AttackMove {
            destination,
            target,
            last_target_cell,
        },
    }
}

/// Reassigns the AttackMove route toward its assigned destination. A leg
/// already bound for that goal — including one still finishing while the
/// unit stands on the destination cell — is kept; any other active leg is
/// a stale pursuit route to a dead target's cell and is removed now
/// instead of being waited out. The stale leg is dropped before the
/// re-plan so a failed assignment — or one that accepts the unit's own
/// cell and installs no route — cannot leave the old pursuit route
/// marching. A re-plan that lands on a neighbor cell re-anchors the stored
/// destination to the accepted goal, or the leg would fail the exact-goal
/// check again next tick and re-path forever.
fn resume_destination(
    world: &mut World,
    map: &GridMap,
    attacker: Entity,
    position: &SimPosition,
    order: &CombatOrder,
    destination: GridPos,
) {
    let start = world_to_cell(position.current);
    let route_goal = world.get::<MoveOrder>(attacker).map(|order| order.goal);
    if start == destination {
        if route_goal.is_some_and(|goal| goal != destination) {
            world.entity_mut(attacker).remove::<MoveOrder>();
        }
        return;
    }
    if route_goal == Some(destination) {
        return;
    }
    world.entity_mut(attacker).remove::<MoveOrder>();
    if let Some(goal) = assign_move_toward(world, map, attacker, start, destination)
        && goal != destination
    {
        world
            .entity_mut(attacker)
            .insert(with_destination(order, goal));
    }
}

/// Lands one validated hit: counter bonus only against the named unit kind,
/// base damage only against buildings, instant application, cooldown
/// installed on strike, event recorded, and atomic destruction of a dead
/// unit or building.
fn strike(
    world: &mut World,
    map: &mut GridMap,
    attacker: AttackerContext,
    target: CombatTarget,
    position: Vec2,
) {
    let AttackerContext {
        entity: attacker_entity,
        identity,
        team: attacker_team,
        spec,
    } = attacker;
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
    if let Some(mut cooldown) = world.get_mut::<AttackCooldown>(attacker_entity) {
        cooldown.0 = spec.cooldown_seconds;
    }
    if let Some(mut events) = world.get_resource_mut::<CombatEvents>() {
        events.0.push(CombatEvent {
            attacker: identity,
            target,
            damage,
            position,
            ranged: spec.ranged,
            killed,
        });
    }
    if let Some(mut total) = world.get_resource_mut::<CombatEventTotal>() {
        total.0 += 1;
    }
    if killed {
        match target {
            CombatTarget::Unit(_) => {
                if attacker_team == TeamId(1)
                    && let Some(reward) = world
                        .get::<Unit>(target_entity)
                        .filter(|victim| victim.team == TeamId(2))
                        .and_then(|victim| unit_spec(victim.kind).alien)
                        .map(|alien| alien.crystal_reward)
                    && let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
                    && let Some(team) = economy.0.get_mut(&TeamId(1))
                {
                    team.stockpile.crystals = team.stockpile.crystals.saturating_add(reward);
                }
                destroy_unit(world, target_entity);
            }
            CombatTarget::Building(_) => {
                // Result resolves immediately on Research Center destruction.
                let research_center = world
                    .get::<Building>(target_entity)
                    .is_some_and(|building| building.kind == BuildingKind::ResearchCenter);
                destroy_building(world, map, target_entity);
                if research_center {
                    resolve_result(world, attacker_team);
                }
            }
        }
    }
}

/// Atomic unit destruction: releases every worker/building assignment
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
/// sites share one transaction; sites simply lack production extras):
/// capture the footprint, free its cells, remove the `BuildingIndex` entry,
/// retask every live worker whose task references the destroyed identity,
/// then despawn — the production queue and derived population capacity fall
/// out with the entity, and living units above a reduced cap are never
/// deleted.
///
/// Worker retasking: `ToConstruction`/`Constructing` cancel to Idle through
/// the existing cancellation; a destroyed drop-off reroutes carrying
/// workers immediately through the economy's narrow routing seam, else
/// idles them preserving Carry.
pub(crate) fn destroy_building(world: &mut World, map: &mut GridMap, entity: Entity) {
    let Some(footprint) = world.get::<Footprint>(entity).copied() else {
        return;
    };
    let building_id = world.get::<Building>(entity).map(|building| building.id);

    for cell in footprint.cells() {
        map.set_blocked(cell, false);
    }
    if let Some(id) = building_id
        && let Some(mut index) = world.get_resource_mut::<BuildingIndex>()
    {
        index.remove(id);
    }

    // Scan every live WorkerTask referencing the destroyed identity — not
    // just active-builder bookkeeping. Collect first so the retasking
    // mutations never fight the query borrow.
    let mut affected: Vec<(Entity, WorkerTask)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask)>();
        for (worker, task) in query.iter(world) {
            let references_destroyed = match *task {
                WorkerTask::ToConstruction { building, .. }
                | WorkerTask::Constructing { building } => Some(building) == building_id,
                WorkerTask::ToDropoff { dropoff, .. } => Some(dropoff) == building_id,
                WorkerTask::ToSource { .. } | WorkerTask::Gathering { .. } | WorkerTask::Idle => {
                    false
                }
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
            WorkerTask::ToDropoff { source, .. } => {
                reroute_dropoff_worker(world, map, worker, source);
            }
            WorkerTask::ToSource { .. } | WorkerTask::Gathering { .. } | WorkerTask::Idle => {}
        }
    }

    world.despawn(entity);
}

#[cfg(test)]
mod tests;
