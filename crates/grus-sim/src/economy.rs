//! Team stockpiles, worker task state, resource sources, drop-off contracts,
//! the single worker-cancellation helper, Gather assignment, and the
//! gather/carry/deposit fixed step.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::buildings::{Building, BuildingIndex, worker_at_slot};
use crate::catalog::{
    AGE_TWO_GATHER_RATE, Age, BASE_GATHER_RATE, CARRY_LIMIT, ResourceKind, UnitKind,
};
use crate::combat::CombatOrder;
use crate::commands::{
    CommandResult, RejectReason, UnitIndex, approach_slots, owned_unit_entity, release_slot,
    reserve_slot,
};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::gameplay_active;
use crate::visibility::{explored_by, visible_to};

/// Carried load of a worker. Invariant: never empty while `Holding`, never
/// mixes resource kinds, never holds zero.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub enum Carry {
    Empty,
    Holding {
        kind: ResourceKind,
        amount: NonZeroU32,
    },
}

impl Carry {
    /// Carried units; zero when empty.
    pub fn amount_or_zero(&self) -> u32 {
        match self {
            Carry::Empty => 0,
            Carry::Holding { amount, .. } => amount.get(),
        }
    }
}

/// One gatherable node: a standalone 1×1 berry bush, tree, or gold deposit, or
/// the renewable Food source added to a completed Farm building.
#[derive(Component, Debug)]
pub struct ResourceSource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub remaining: Option<u32>,
    pub assigned_worker: Option<UnitId>,
}

/// Stable lookup from `ResourceId` to its entity, mirroring `UnitIndex`.
#[derive(Debug, Default, Resource)]
pub struct ResourceIndex(HashMap<ResourceId, Entity>);

impl ResourceIndex {
    pub fn entity(&self, id: ResourceId) -> Option<Entity> {
        self.0.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceId, &Entity)> {
        self.0.iter()
    }

    pub(crate) fn insert(&mut self, id: ResourceId, entity: Entity) {
        self.0.insert(id, entity);
    }

    pub(crate) fn remove(&mut self, id: ResourceId) {
        self.0.remove(&id);
    }
}

/// Spawns one standalone finite source with a 1×1 blocked footprint and
/// registers it in the `ResourceIndex`. Shared by authored seeding and tests.
pub(crate) fn spawn_resource_source(
    world: &mut World,
    map: &mut GridMap,
    id: ResourceId,
    kind: ResourceKind,
    cell: GridPos,
    amount: u32,
) -> Entity {
    map.set_blocked(cell, true);
    let entity = world
        .spawn((
            ResourceSource {
                id,
                kind,
                remaining: Some(amount),
                assigned_worker: None,
            },
            Footprint::new(cell, 1, 1),
        ))
        .id();
    world
        .get_resource_or_insert_with(ResourceIndex::default)
        .insert(id, entity);
    entity
}

/// Per-worker fractional gather accumulator; whole resources transfer when
/// progress crosses 1.0.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GatherProgress(pub f32);

/// Current activity of a worker. Arrival is positive: transitions happen when
/// the worker's cell equals the stored slot.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub enum WorkerTask {
    Idle,
    ToSource {
        source: ResourceId,
        slot: GridPos,
    },
    Gathering {
        source: ResourceId,
    },
    ToDropoff {
        source: ResourceId,
        dropoff: BuildingId,
        slot: GridPos,
    },
    ToConstruction {
        building: BuildingId,
        slot: GridPos,
    },
    Constructing {
        building: BuildingId,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceStockpile {
    pub food: u32,
    pub wood: u32,
    pub gold: u32,
}

#[derive(Clone, Debug)]
pub struct TeamState {
    pub stockpile: ResourceStockpile,
    pub age: Age,
    pub age_up_started: bool,
}

#[derive(Debug, Default, Resource)]
pub struct TeamEconomy(pub HashMap<TeamId, TeamState>);

impl TeamEconomy {
    pub fn insert_team(&mut self, team: TeamId, stockpile: ResourceStockpile, age: Age) {
        self.0.insert(
            team,
            TeamState {
                stockpile,
                age,
                age_up_started: false,
            },
        );
    }
}

/// Marker on buildings that accept deposits; carries only team ownership.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dropoff {
    pub team: TeamId,
}

/// The one unit-activity cleanup path: clears any construction assignment
/// and Farm reservation the unit held, resets its gather progress, returns
/// it to `Idle`, and drops its route and any combat order. Replacement
/// commands cancel only after validating, and rejected commands never reach
/// this. `Carry` is never touched.
pub(crate) fn cancel_unit_activity(world: &mut World, entity: Entity) {
    let building = match world.get::<WorkerTask>(entity) {
        Some(WorkerTask::ToConstruction { building, .. })
        | Some(WorkerTask::Constructing { building }) => Some(*building),
        _ => None,
    };
    if let Some(building) = building {
        let unit_id = world.get::<Unit>(entity).map(|unit| unit.id);
        if let Some(building_entity) = world
            .get_resource::<BuildingIndex>()
            .and_then(|index| index.entity(building))
            && let Some(mut state) = world.get_mut::<Building>(building_entity)
            && state.construction.active_builder == unit_id
        {
            state.construction.active_builder = None;
        }
    }
    let source = match world.get::<WorkerTask>(entity) {
        Some(WorkerTask::ToSource { source, .. })
        | Some(WorkerTask::Gathering { source })
        | Some(WorkerTask::ToDropoff { source, .. }) => Some(*source),
        _ => None,
    };
    if let Some(source) = source {
        let unit_id = world.get::<Unit>(entity).map(|unit| unit.id);
        if let Some(source_entity) = world
            .get_resource::<ResourceIndex>()
            .and_then(|index| index.entity(source))
            && let Some(unit_id) = unit_id
            && let Some(mut state) = world.get_mut::<ResourceSource>(source_entity)
            && state.assigned_worker == Some(unit_id)
        {
            state.assigned_worker = None;
        }
    }
    world.entity_mut(entity).insert(WorkerTask::Idle);
    world.entity_mut(entity).insert(GatherProgress::default());
    world.entity_mut(entity).remove::<MoveOrder>();
    world.entity_mut(entity).remove::<CombatOrder>();
}

/// Most recent route-failure reject per team, drained by the bridge into its
/// feedback channel (Team 1 only — a Team-2 AI worker's failure can never
/// surface as, or overwrite, the human player's feedback). One latest slot
/// per team. Lives in the sim so the cleanup helper stays Godot-free.
#[derive(Debug, Default, Resource)]
pub struct LastRouteReject(pub HashMap<TeamId, RejectReason>);

/// Terminal cleanup when a worker's required route becomes impossible: the
/// full `cancel_unit_activity` semantics (Farm assignment released,
/// active-builder cleared, progress reset, order and combat intent dropped,
/// `Idle`) plus the
/// typed reject recorded for bridge feedback. `Carry` is never touched.
pub(crate) fn idle_worker_on_route_failure(
    world: &mut World,
    entity: Entity,
    reason: RejectReason,
) {
    // Capture the worker's team before cancellation touches the entity.
    let team = world.get::<Unit>(entity).map(|unit| unit.team);
    cancel_unit_activity(world, entity);
    if let Some(team) = team {
        world
            .get_resource_or_insert_with(LastRouteReject::default)
            .0
            .insert(team, reason);
    }
}

/// Narrow combat-destruction seam over the existing drop-off routing: a
/// worker whose drop-off was destroyed immediately tries another reachable
/// same-team drop-off, preserving `Carry` and the task's source; with no
/// route it takes the full idle cleanup (also preserving `Carry`) with typed
/// feedback. Returns whether a reroute was found.
pub(crate) fn reroute_dropoff_worker(
    world: &mut World,
    map: &GridMap,
    worker: Entity,
    source: ResourceId,
) -> bool {
    let used = seed_used_excluding(world, map, worker);
    match nearest_reachable_dropoff(world, map, worker, &used) {
        Ok((dropoff, slot, route)) => {
            // Same as every other ToDropoff installation: partial gather
            // progress never survives a task change.
            world.entity_mut(worker).insert(GatherProgress::default());
            world.entity_mut(worker).insert(WorkerTask::ToDropoff {
                source,
                dropoff,
                slot,
            });
            // The stale route toward the destroyed drop-off must not
            // survive an empty reroute: with the worker already standing on
            // the replacement slot, a lingering MoveOrder would walk it
            // away before the deposit lands.
            world.entity_mut(worker).remove::<MoveOrder>();
            if !route.is_empty() {
                world.entity_mut(worker).insert(MoveOrder {
                    waypoints: route,
                    next: 0,
                    goal: slot,
                    map_revision: map.revision(),
                    last_failed_replan: None,
                });
            }
            true
        }
        Err(reason) => {
            idle_worker_on_route_failure(world, worker, reason);
            false
        }
    }
}

/// The one idle-villager definition: Villager + `WorkerTask::Idle` + no
/// `MoveOrder`. Lifted from the bridge HUD so the HUD idle count and the AI
/// worker pool read the same truth.
pub fn is_idle_worker(world: &World, entity: Entity) -> bool {
    world
        .get::<Unit>(entity)
        .is_some_and(|unit| unit.kind == UnitKind::Villager)
        && world.get::<WorkerTask>(entity) == Some(&WorkerTask::Idle)
        && world.get::<MoveOrder>(entity).is_none()
}

/// Stable-ID-sorted idle villagers of one team; consumed by the bridge HUD
/// and the AI alike.
pub fn idle_worker_ids(world: &World, team: TeamId) -> Vec<UnitId> {
    let mut ids: Vec<UnitId> = world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    (world.get::<Unit>(*entity)?.team == team && is_idle_worker(world, *entity))
                        .then_some(*id)
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

pub fn gather_rate_for_age(age: Age) -> f32 {
    match age {
        Age::Age1 => BASE_GATHER_RATE,
        Age::Age2 => AGE_TWO_GATHER_RATE,
    }
}

/// Applies an accepted `Gather`: validates source knowledge (explored
/// standalone source or own completed Farm) then source ownership — a
/// foreign Farm, even while currently visible, is never an own economic
/// source — plus owned villagers, Farm availability, and shared
/// reservation state; then assigns unique immediate-perimeter slots. A worker carrying resources
/// routes to a reachable same-team Dropoff first (depositing) and only then
/// to the requested source, so Carry never mixes kinds. Validation precedes
/// any cancellation: a rejected worker keeps its old task and order.
pub(crate) fn apply_gather(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    mut workers: Vec<UnitId>,
    source: ResourceId,
) -> CommandResult {
    workers.sort_unstable();
    workers.dedup();
    let mut outcome = CommandResult::default();

    let Some(source_entity) = world
        .get_resource::<ResourceIndex>()
        .and_then(|index| index.entity(source))
    else {
        outcome.reject = Some(RejectReason::SourceMissing);
        return outcome;
    };
    let footprint = world
        .get::<Footprint>(source_entity)
        .copied()
        .expect("registered resource source footprint");
    let is_farm = world.get::<Building>(source_entity).is_some();
    // Knowledge gate, called unconditionally (both predicates pass through
    // when no VisibilityMap exists, so pure-sim full information is kept):
    // standalone sources are known once their cell was explored and stay
    // known after vision is lost; own completed Farms are always valid
    // knowledge; an enemy Farm is an enemy building and needs current
    // visibility merely to be *known* — it is never admitted merely by
    // sitting in the `ResourceIndex`.
    let source_known = if is_farm {
        world
            .get::<Building>(source_entity)
            .is_some_and(|building| building.team == issuer)
            || visible_to(world, issuer, footprint)
    } else {
        explored_by(world, issuer, footprint)
    };
    if !source_known {
        outcome.reject = Some(RejectReason::Unexplored);
        return outcome;
    }
    // Ownership: a foreign Farm stays `Unexplored` while hidden (fog
    // privacy), but once visible the command authority refuses it — the
    // Godot picker never offers enemy Farm views, and the Rust rule is the
    // same: an enemy Farm is never gathered as an own economic source.
    if is_farm
        && !world
            .get::<Building>(source_entity)
            .is_some_and(|building| building.team == issuer)
    {
        outcome.reject = Some(RejectReason::NotOwned);
        return outcome;
    }

    // Same reservation seam as Move and building placement: seed every live
    // unit's current cell and MoveOrder goal. A commanded worker's own current
    // cell and old goal are released only while it is being reassigned and
    // restored if the reassignment fails, so rejected siblings never free a
    // cell they still hold.
    let mut used: HashMap<GridPos, usize> = HashMap::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        if let Some(position) = world.get::<SimPosition>(entity) {
            reserve_slot(&mut used, map.world_to_cell(position.current));
        }
        if let Some(order) = world.get::<MoveOrder>(entity) {
            reserve_slot(&mut used, order.goal);
        }
    }

    for id in workers {
        let entity = match owned_unit_entity(world, id, issuer) {
            Ok(entity) => entity,
            Err(reason) => {
                outcome.rejected_units.push((id, reason));
                continue;
            }
        };
        if !world
            .get::<Unit>(entity)
            .is_some_and(|unit| unit.kind == UnitKind::Villager)
        {
            outcome.rejected_units.push((id, RejectReason::NotVillager));
            continue;
        }
        // A Farm serves exactly one worker: the first accepted command
        // reserves it, later commands reject until a retask or Stop releases.
        if is_farm {
            let assigned = world
                .get::<ResourceSource>(source_entity)
                .and_then(|state| state.assigned_worker);
            if assigned.is_some_and(|holder| holder != id) {
                outcome
                    .rejected_units
                    .push((id, RejectReason::FarmOccupied));
                continue;
            }
        }
        let Some(position) = world.get::<SimPosition>(entity).copied() else {
            outcome.rejected_units.push((id, RejectReason::UnknownUnit));
            continue;
        };
        let start = map.world_to_cell(position.current);
        let old_goal = world.get::<MoveOrder>(entity).map(|order| order.goal);
        release_slot(&mut used, start);
        if let Some(goal) = old_goal {
            release_slot(&mut used, goal);
        }

        let holding = world
            .get::<Carry>(entity)
            .is_some_and(|carry| carry.amount_or_zero() > 0);
        let assignment: Result<(WorkerTask, GridPos, Vec<Vec2>), RejectReason> = if holding {
            nearest_reachable_dropoff(world, map, entity, &used).map(|(dropoff, slot, route)| {
                (
                    WorkerTask::ToDropoff {
                        source,
                        dropoff,
                        slot,
                    },
                    slot,
                    route,
                )
            })
        } else {
            let keys: HashSet<GridPos> = used.keys().copied().collect();
            let candidates =
                approach_slots(map, footprint, &keys, footprint.perimeter_cells().len());
            pick_reachable_slot(map, start, &candidates)
                .map(|(slot, route)| (WorkerTask::ToSource { source, slot }, slot, route))
        };

        let (task, slot, route) = match assignment {
            Ok(assignment) => assignment,
            Err(reason) => {
                reserve_slot(&mut used, start);
                if let Some(goal) = old_goal {
                    reserve_slot(&mut used, goal);
                }
                outcome.rejected_units.push((id, reason));
                continue;
            }
        };

        // Fully validated: cancel the old activity (releasing any Farm
        // reservation it held) before installing the gather task.
        cancel_unit_activity(world, entity);
        world.entity_mut(entity).insert(task);
        if !route.is_empty() {
            world.entity_mut(entity).insert(MoveOrder {
                waypoints: route,
                next: 0,
                goal: slot,
                map_revision: map.revision(),
                last_failed_replan: None,
            });
        }
        reserve_slot(&mut used, slot);
        if is_farm && let Some(mut state) = world.get_mut::<ResourceSource>(source_entity) {
            state.assigned_worker = Some(id);
        }
        outcome.accepted_units.push(id);
    }

    outcome
}

/// Picks the worker's slot: the first reachable walkable cell on the given
/// candidates list, with its route waypoints. `Err(Crowded)` when no candidate
/// slots exist at all, `Err(Unreachable)` when none of them is pathable.
fn pick_reachable_slot(
    map: &GridMap,
    start: GridPos,
    candidates: &[GridPos],
) -> Result<(GridPos, Vec<Vec2>), RejectReason> {
    candidates
        .iter()
        .copied()
        .find_map(|slot| {
            map.find_path(start, slot).map(|path| {
                (
                    slot,
                    path.into_iter()
                        .skip(1)
                        .map(|cell| map.cell_center(cell))
                        .collect(),
                )
            })
        })
        .ok_or(if candidates.is_empty() {
            RejectReason::Crowded
        } else {
            RejectReason::Unreachable
        })
}

/// Picks the nearest same-team Dropoff by straight-line distance to the
/// footprint's geometric center whose immediate perimeter still has a free,
/// reachable slot. Runs once per decision point (command acceptance, full
/// carry, depletion) — never per simulation tick.
fn nearest_reachable_dropoff(
    world: &World,
    map: &GridMap,
    worker: Entity,
    used: &HashMap<GridPos, usize>,
) -> Result<(BuildingId, GridPos, Vec<Vec2>), RejectReason> {
    let position = world
        .get::<SimPosition>(worker)
        .ok_or(RejectReason::UnknownUnit)?;
    let start = map.world_to_cell(position.current);
    let team = world
        .get::<Unit>(worker)
        .map(|unit| unit.team)
        .ok_or(RejectReason::UnknownUnit)?;

    let mut dropoffs: Vec<(BuildingId, Footprint, f32)> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    if world.get::<Dropoff>(*entity)?.team != team {
                        return None;
                    }
                    let footprint = *world.get::<Footprint>(*entity)?;
                    Some((
                        *id,
                        footprint,
                        position.current.distance_squared(footprint.center()),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    // BuildingIndex iterates in arbitrary HashMap order; distance ties must
    // still resolve deterministically, so the lower BuildingId wins them.
    dropoffs.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)));

    let keys: HashSet<GridPos> = used.keys().copied().collect();
    let mut any_free_slot = false;
    for (id, footprint, _) in dropoffs {
        for slot in approach_slots(map, footprint, &keys, footprint.perimeter_cells().len()) {
            any_free_slot = true;
            if let Some(path) = map.find_path(start, slot) {
                let route = path
                    .into_iter()
                    .skip(1)
                    .map(|cell| map.cell_center(cell))
                    .collect();
                return Ok((id, slot, route));
            }
        }
    }
    Err(if any_free_slot {
        RejectReason::Unreachable
    } else {
        RejectReason::Crowded
    })
}

/// Advances the gather/carry/deposit loop one fixed tick. Arrival is positive:
/// workers transition when their current cell equals the stored slot. A worker
/// leaving the gathering phase (full carry, depleted source, or vanished
/// source) banks no partial progress and routes once to the nearest reachable
/// same-team Dropoff. Stockpiles change only on deposit at the stored drop-off
/// slot.
pub fn step_economy(world: &mut World, map: &mut GridMap, seconds: f32) {
    if !gameplay_active(world) {
        return;
    }
    let mut source_arrivals: Vec<(Entity, ResourceId)> = Vec::new();
    let mut dropoff_arrivals: Vec<(Entity, ResourceId)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask, &SimPosition)>();
        for (entity, task, position) in query.iter(world) {
            match task {
                WorkerTask::ToSource { source, slot } if worker_at_slot(position, *slot) => {
                    source_arrivals.push((entity, *source));
                }
                WorkerTask::ToDropoff { source, slot, .. } if worker_at_slot(position, *slot) => {
                    dropoff_arrivals.push((entity, *source));
                }
                _ => {}
            }
        }
    }

    for (entity, source) in source_arrivals {
        let exists = world
            .get_resource::<ResourceIndex>()
            .is_some_and(|index| index.entity(source).is_some());
        if exists {
            world
                .entity_mut(entity)
                .insert(WorkerTask::Gathering { source });
        } else {
            world.entity_mut(entity).insert(WorkerTask::Idle);
        }
    }

    for (entity, source) in dropoff_arrivals {
        deposit_carry(world, entity);
        match route_back_to_source(world, map, entity, source) {
            Ok((slot, route)) => {
                world
                    .entity_mut(entity)
                    .insert(WorkerTask::ToSource { source, slot });
                if !route.is_empty() {
                    world.entity_mut(entity).insert(MoveOrder {
                        waypoints: route,
                        next: 0,
                        goal: slot,
                        map_revision: map.revision(),
                        last_failed_replan: None,
                    });
                }
            }
            Err(reason) => {
                // The route back is impossible: full cleanup with typed
                // feedback instead of idling raw and retrying every tick.
                idle_worker_on_route_failure(world, entity, reason);
            }
        }
    }

    let mut gatherers: Vec<(Entity, ResourceId, Carry, TeamId)> = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask, &Carry, &Unit)>();
        for (entity, task, carry, unit) in query.iter(world) {
            if let WorkerTask::Gathering { source } = task {
                gatherers.push((entity, *source, carry.clone(), unit.team));
            }
        }
    }
    for (entity, source, carry, team) in gatherers {
        let team_age = world
            .get_resource::<TeamEconomy>()
            .and_then(|economy| economy.0.get(&team))
            .map(|state| state.age)
            .unwrap_or(Age::Age1);

        let source_entity = world
            .get_resource::<ResourceIndex>()
            .and_then(|index| index.entity(source));
        let Some(source_entity) = source_entity else {
            leave_gathering(world, map, entity, source, carry);
            continue;
        };

        // Read phase: current progress, carry, and source state.
        let (progress, remaining, kind) = match (
            world.get::<GatherProgress>(entity).map(|state| state.0),
            world
                .get::<ResourceSource>(source_entity)
                .map(|state| (state.remaining, state.kind)),
        ) {
            (Some(progress), Some((remaining, kind))) => (progress, remaining, kind),
            _ => {
                leave_gathering(world, map, entity, source, carry);
                continue;
            }
        };

        let mut progress = progress + gather_rate_for_age(team_age) * seconds;
        let whole = progress.floor() as u32;
        let carry_space = CARRY_LIMIT - carry.amount_or_zero();
        let source_available = remaining.unwrap_or(u32::MAX);
        let transferred = whole.min(carry_space).min(source_available);

        let mut carried = carry;
        if transferred > 0 {
            carried = match carried {
                Carry::Empty => Carry::Holding {
                    kind,
                    amount: NonZeroU32::new(transferred).expect("transferred is positive"),
                },
                Carry::Holding { kind, amount } => Carry::Holding {
                    kind,
                    amount: NonZeroU32::new(amount.get() + transferred)
                        .expect("carry stays positive"),
                },
            };
            progress -= transferred as f32;
        }
        let depleted = remaining.is_some_and(|remaining| transferred >= remaining);

        // Write phase: progress, carry, and remaining all move together.
        if let Some(mut state) = world.get_mut::<GatherProgress>(entity) {
            state.0 = progress;
        }
        if transferred > 0 {
            if let Some(mut state) = world.get_mut::<Carry>(entity) {
                *state = carried.clone();
            }
            if let Some(remaining) = remaining
                && let Some(mut state) = world.get_mut::<ResourceSource>(source_entity)
            {
                state.remaining = Some(remaining - transferred);
            }
        }

        if depleted {
            deplete_source(world, map, source_entity);
        }

        let carried_now = world.get::<Carry>(entity).cloned().unwrap_or(Carry::Empty);
        if depleted || carried_now.amount_or_zero() >= CARRY_LIMIT {
            leave_gathering(world, map, entity, source, carried_now);
        }
    }
}

/// Leaves the active gathering phase: partial progress is not banked. A worker
/// still holding resources routes once to the nearest reachable same-team
/// Dropoff; with nothing to deliver it idles. A failed route search never
/// loops A* per tick — the worker idles instead.
fn leave_gathering(
    world: &mut World,
    map: &mut GridMap,
    worker: Entity,
    source: ResourceId,
    carry: Carry,
) {
    world.entity_mut(worker).insert(GatherProgress::default());
    if carry.amount_or_zero() == 0 {
        world.entity_mut(worker).insert(WorkerTask::Idle);
        return;
    }
    reroute_dropoff_worker(world, map, worker, source);
}

/// Routes a worker that just deposited back to its requested source, if the
/// source still exists and a reachable immediate-perimeter slot is free.
fn route_back_to_source(
    world: &mut World,
    map: &GridMap,
    worker: Entity,
    source: ResourceId,
) -> Result<(GridPos, Vec<Vec2>), RejectReason> {
    let source_entity = world
        .get_resource::<ResourceIndex>()
        .and_then(|index| index.entity(source))
        .ok_or(RejectReason::Unreachable)?;
    let footprint = world
        .get::<Footprint>(source_entity)
        .copied()
        .ok_or(RejectReason::Unreachable)?;
    // A Farm keeps serving its assigned worker only.
    if world.get::<Building>(source_entity).is_some() {
        let unit_id = world
            .get::<Unit>(worker)
            .map(|unit| unit.id)
            .ok_or(RejectReason::Unreachable)?;
        let Some(mut state) = world.get_mut::<ResourceSource>(source_entity) else {
            return Err(RejectReason::Unreachable);
        };
        match state.assigned_worker {
            Some(holder) if holder != unit_id => return Err(RejectReason::Unreachable),
            _ => state.assigned_worker = Some(unit_id),
        }
    }
    let position = world
        .get::<SimPosition>(worker)
        .ok_or(RejectReason::Unreachable)?;
    let start = map.world_to_cell(position.current);
    let used = seed_used_excluding(world, map, worker);
    let keys: HashSet<GridPos> = used.keys().copied().collect();
    let candidates = approach_slots(map, footprint, &keys, footprint.perimeter_cells().len());
    pick_reachable_slot(map, start, &candidates)
}

/// Seeds reference-counted reservations from every live unit's current cell
/// and MoveOrder goal, except the given worker's.
fn seed_used_excluding(world: &World, map: &GridMap, worker: Entity) -> HashMap<GridPos, usize> {
    let mut used = HashMap::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        if entity == worker {
            continue;
        }
        if let Some(position) = world.get::<SimPosition>(entity) {
            reserve_slot(&mut used, map.world_to_cell(position.current));
        }
        if let Some(order) = world.get::<MoveOrder>(entity) {
            reserve_slot(&mut used, order.goal);
        }
    }
    used
}

/// Removes a fully depleted source: index entry, entity, and its blocked 1×1
/// footprint cells through `GridMap::set_blocked`.
fn deplete_source(world: &mut World, map: &mut GridMap, source_entity: Entity) {
    let Some(footprint) = world.get::<Footprint>(source_entity).copied() else {
        return;
    };
    let Some(source) = world.get::<ResourceSource>(source_entity) else {
        return;
    };
    let id = source.id;
    for cell in footprint.cells() {
        map.set_blocked(cell, false);
    }
    if let Some(mut index) = world.get_resource_mut::<ResourceIndex>() {
        index.remove(id);
    }
    world.despawn(source_entity);
}

/// Atomically moves the worker's whole carry into its team stockpile and
/// empties the carry.
fn deposit_carry(world: &mut World, worker: Entity) {
    let Some(carry) = world.get::<Carry>(worker) else {
        return;
    };
    let Carry::Holding { kind, amount } = *carry else {
        return;
    };
    let amount = amount.get();
    let Some(team) = world.get::<Unit>(worker).map(|unit| unit.team) else {
        return;
    };
    if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&team)
    {
        match kind {
            ResourceKind::Food => state.stockpile.food += amount,
            ResourceKind::Wood => state.stockpile.wood += amount,
            ResourceKind::Gold => state.stockpile.gold += amount,
        }
    }
    world.entity_mut(worker).insert(Carry::Empty);
}

#[cfg(test)]
mod tests;
