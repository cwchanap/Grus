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
use crate::commands::{
    CommandResult, RejectReason, UnitIndex, approach_slots, owned_unit_entity, release_slot,
    reserve_slot,
};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};

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

/// The one worker-activity cleanup path: clears any construction assignment
/// and Farm reservation the worker held, resets its gather progress, returns
/// it to `Idle`, and drops its route. Replacement commands cancel only after
/// validating, and rejected commands never reach this. `Carry` is never
/// touched.
pub(crate) fn cancel_worker_activity(world: &mut World, entity: Entity) {
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
}

/// Most recent route-failure reject, drained by the bridge into its feedback
/// channel. Single slot — the latest failure in a tick wins. Lives in the sim
/// so the cleanup helper stays Godot-free.
#[derive(Debug, Default, Resource)]
pub struct LastRouteReject(pub Option<RejectReason>);

/// Terminal cleanup when a worker's required route becomes impossible: the
/// full `cancel_worker_activity` semantics (Farm assignment released,
/// active-builder cleared, progress reset, order dropped, `Idle`) plus the
/// typed reject recorded for bridge feedback. `Carry` is never touched.
pub(crate) fn idle_worker_on_route_failure(
    world: &mut World,
    entity: Entity,
    reason: RejectReason,
) {
    cancel_worker_activity(world, entity);
    world.insert_resource(LastRouteReject(Some(reason)));
}

pub fn gather_rate_for_age(age: Age) -> f32 {
    match age {
        Age::Age1 => BASE_GATHER_RATE,
        Age::Age2 => AGE_TWO_GATHER_RATE,
    }
}

/// Applies an accepted `Gather`: validates source, owned villagers, Farm
/// availability, and shared reservation state; then assigns unique
/// immediate-perimeter slots. A worker carrying resources routes to a
/// reachable same-team Dropoff first (depositing) and only then to the
/// requested source, so Carry never mixes kinds. Validation precedes any
/// cancellation: a rejected worker keeps its old task and order.
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
        cancel_worker_activity(world, entity);
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
    let used = seed_used_excluding(world, map, worker);
    match nearest_reachable_dropoff(world, map, worker, &used) {
        Ok((dropoff, slot, route)) => {
            world.entity_mut(worker).insert(WorkerTask::ToDropoff {
                source,
                dropoff,
                slot,
            });
            if !route.is_empty() {
                world.entity_mut(worker).insert(MoveOrder {
                    waypoints: route,
                    next: 0,
                    goal: slot,
                    map_revision: map.revision(),
                    last_failed_replan: None,
                });
            }
        }
        Err(reason) => {
            // Route impossible: full cleanup with typed feedback instead of
            // idling raw and leaking the source/farm assignment.
            idle_worker_on_route_failure(world, worker, reason);
        }
    }
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
mod tests {
    use std::collections::HashSet;

    use bevy::math::Vec2;

    use super::*;
    use crate::buildings::step_construction;
    use crate::catalog::unit_spec;
    use crate::commands::{
        PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
    };
    use crate::fixture::{MapFixture, seed_skirmish};
    use crate::ids::IdAllocator;
    use crate::movement::{SIM_STEP_SECONDS, step_movement};

    fn test_economy(world: &mut World) {
        let mut economy = TeamEconomy::default();
        economy.insert_team(
            TeamId(1),
            ResourceStockpile {
                food: 200,
                wood: 300,
                gold: 100,
            },
            Age::Age1,
        );
        world.insert_resource(economy);
    }

    fn spawn_villager(world: &mut World, id: UnitId, position: Vec2) -> Entity {
        let entity = spawn_unit(
            world,
            id,
            TeamId(1),
            position,
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(entity).insert((
            Carry::Empty,
            GatherProgress::default(),
            WorkerTask::Idle,
        ));
        entity
    }

    fn gather_command(issuer: TeamId, workers: Vec<UnitId>, source: ResourceId) -> PlayerCommand {
        PlayerCommand::Gather {
            issuer,
            workers,
            source,
        }
    }

    fn skirmish_world() -> (World, GridMap) {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);
        (world, map)
    }

    #[test]
    fn gather_assigns_four_villagers_unique_immediate_slots() {
        let mut world = World::new();
        let mut map = GridMap::new(24, 24);
        test_economy(&mut world);
        world.insert_resource(IdAllocator::new(5, 1, 2));
        for id in 1..=4 {
            let offset = id - 1;
            spawn_villager(
                &mut world,
                UnitId(id),
                Vec2::new(
                    10.5 + (offset % 2) as f32 * 4.0,
                    10.5 + (offset / 2) as f32 * 4.0,
                ),
            );
        }
        let source = spawn_resource_source(
            &mut world,
            &mut map,
            ResourceId(1),
            ResourceKind::Wood,
            GridPos::new(12, 12),
            400,
        );
        let footprint = world.get::<Footprint>(source).copied().unwrap();

        // A normal Move onto the blocked source center stays Unreachable.
        let source_center = map.cell_center(GridPos::new(12, 12));
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)],
                kind: UnitCommandKind::Move {
                    target: source_center,
                },
            }),
        );
        assert_eq!(outcome.accepted_units, Vec::<UnitId>::new());
        assert_eq!(outcome.rejected_units.len(), 4);
        assert!(
            outcome
                .rejected_units
                .iter()
                .all(|(_, reason)| *reason == RejectReason::Unreachable)
        );

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(
                TeamId(1),
                vec![UnitId(4), UnitId(3), UnitId(2), UnitId(1)],
                ResourceId(1),
            ),
        );
        assert_eq!(
            outcome.accepted_units,
            vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)]
        );
        assert!(outcome.rejected_units.is_empty());

        let index = world.resource::<UnitIndex>();
        let mut slots = HashSet::new();
        for id in 1..=4 {
            let entity = index.entity(UnitId(id)).unwrap();
            let WorkerTask::ToSource { source, slot } = world.get::<WorkerTask>(entity).unwrap()
            else {
                panic!("expected ToSource for {id:?}");
            };
            assert_eq!(*source, ResourceId(1));
            assert!(map.is_walkable(*slot), "slot {slot:?} is not walkable");
            assert!(footprint.is_immediately_adjacent(*slot));
            slots.insert(*slot);
        }
        assert_eq!(slots.len(), 4, "all four ToSource slots must be unique");
    }

    #[test]
    fn gather_beyond_perimeter_capacity_rejects_as_crowded() {
        let mut world = World::new();
        let mut map = GridMap::new(24, 24);
        test_economy(&mut world);
        world.insert_resource(IdAllocator::new(10, 1, 2));
        let footprint = Footprint::new(GridPos::new(12, 12), 1, 1);
        for (index, cell) in footprint.perimeter_cells().into_iter().enumerate() {
            spawn_villager(&mut world, UnitId(index as u32 + 1), map.cell_center(cell));
        }
        let outsider = spawn_villager(&mut world, UnitId(9), Vec2::new(2.5, 2.5));
        spawn_resource_source(
            &mut world,
            &mut map,
            ResourceId(1),
            ResourceKind::Wood,
            GridPos::new(12, 12),
            400,
        );

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), (1..=9).map(UnitId).collect(), ResourceId(1)),
        );

        assert_eq!(outcome.accepted_units.len(), 8);
        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(9), RejectReason::Crowded)]
        );
        let slots: HashSet<GridPos> = (1..=8)
            .map(|id| {
                let entity = world.resource::<UnitIndex>().entity(UnitId(id)).unwrap();
                match world.get::<WorkerTask>(entity).unwrap() {
                    WorkerTask::ToSource { slot, .. } => *slot,
                    other => panic!("expected ToSource, got {other:?}"),
                }
            })
            .collect();
        assert_eq!(slots.len(), 8, "accepted villagers keep unique slots");
        assert_eq!(world.get::<WorkerTask>(outsider), Some(&WorkerTask::Idle));
    }

    #[test]
    fn gather_while_holding_deposits_first_then_gathers_the_requested_source() {
        let (mut world, mut map) = skirmish_world();

        let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        // Park the villager away from both the berries and the Town Center so
        // the deposit leg is a real walk.
        world
            .entity_mut(villager)
            .insert(SimPosition::new(Vec2::new(20.5, 42.5)));
        world.entity_mut(villager).insert(Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(6).unwrap(),
        });

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        match world.get::<WorkerTask>(villager).unwrap() {
            WorkerTask::ToDropoff {
                source, dropoff, ..
            } => {
                assert_eq!(*source, ResourceId(1));
                assert_eq!(
                    *dropoff,
                    BuildingId(1),
                    "carrying workers route to the starting Town Center first"
                );
            }
            other => panic!("expected ToDropoff, got {other:?}"),
        }

        // Nothing is deposited while the worker is still walking.
        for _ in 0..10 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        assert_eq!(state.stockpile.wood, 300);

        for _ in 0..600 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
            if world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood == 306 {
                break;
            }
        }
        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        assert_eq!(state.stockpile.wood, 306);
        assert_eq!(state.stockpile.food, 200, "no Food before gathering begins");
        assert_eq!(world.get::<Carry>(villager), Some(&Carry::Empty));
        match world.get::<WorkerTask>(villager).unwrap() {
            WorkerTask::ToSource { source, slot } => {
                assert_eq!(*source, ResourceId(1));
                assert!(
                    Footprint::new(GridPos::new(22, 42), 1, 1).is_immediately_adjacent(*slot),
                    "worker routes to the berry slot after depositing"
                );
            }
            other => panic!("expected ToSource after deposit, got {other:?}"),
        }
    }

    #[test]
    fn ten_gather_ticks_transfer_one_whole_food() {
        let (mut world, mut map) = skirmish_world();

        let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        // Park the villager next to the 600-Food berry bush at (22, 42).
        world
            .entity_mut(villager)
            .insert(SimPosition::new(map.cell_center(GridPos::new(21, 42))));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

        for _ in 0..400 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            if world.get::<MoveOrder>(villager).is_none() {
                break;
            }
        }
        assert!(world.get::<MoveOrder>(villager).is_none(), "never arrived");

        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        assert!(
            matches!(
                world.get::<WorkerTask>(villager),
                Some(WorkerTask::Gathering { .. })
            ),
            "arrival must transition into Gathering"
        );

        for _ in 0..10 {
            step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        }

        assert_eq!(
            world.get::<Carry>(villager),
            Some(&Carry::Holding {
                kind: ResourceKind::Food,
                amount: NonZeroU32::new(1).unwrap(),
            })
        );
        let source_entity = world
            .resource::<ResourceIndex>()
            .entity(ResourceId(1))
            .unwrap();
        assert_eq!(
            world
                .get::<ResourceSource>(source_entity)
                .unwrap()
                .remaining,
            Some(599)
        );
    }

    #[test]
    fn rejected_gather_preserves_the_previous_task_and_route() {
        let (mut world, mut map) = skirmish_world();

        let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1)], ResourceId(3)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        let old_task = world.get::<WorkerTask>(villager).unwrap().clone();
        let old_route = world.get::<MoveOrder>(villager).unwrap().waypoints.clone();

        // Unknown source: the whole command rejects before any cancellation.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1)], ResourceId(99)),
        );
        assert_eq!(outcome.reject, Some(RejectReason::SourceMissing));
        assert_eq!(world.get::<WorkerTask>(villager), Some(&old_task));
        assert_eq!(
            world.get::<MoveOrder>(villager).unwrap().waypoints,
            old_route
        );
    }

    #[test]
    fn depleted_source_despawns_unblocks_and_the_worker_deposits_then_idles() {
        let (mut world, mut map) = skirmish_world();

        let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        // A tiny finite wood source next to the villager's parked spot.
        let source = spawn_resource_source(
            &mut world,
            &mut map,
            ResourceId(50),
            ResourceKind::Wood,
            GridPos::new(13, 43),
            2,
        );
        world
            .entity_mut(villager)
            .insert(SimPosition::new(map.cell_center(GridPos::new(13, 42))));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1)], ResourceId(50)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

        for _ in 0..400 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            if world.get::<MoveOrder>(villager).is_none() {
                break;
            }
        }
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        for _ in 0..30 {
            step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
            if matches!(
                world.get::<WorkerTask>(villager),
                Some(WorkerTask::ToDropoff { .. })
            ) {
                break;
            }
        }
        // The last unit drained the source: despawned, unblocked, and the
        // worker delivers the final carry.
        assert_eq!(
            world.resource::<ResourceIndex>().entity(ResourceId(50)),
            None,
            "depleted source must leave the index"
        );
        assert!(
            world.get::<ResourceSource>(source).is_none(),
            "entity despawned"
        );
        assert!(map.is_walkable(GridPos::new(13, 43)), "footprint unblocked");
        assert_eq!(
            world.get::<Carry>(villager),
            Some(&Carry::Holding {
                kind: ResourceKind::Wood,
                amount: NonZeroU32::new(2).unwrap(),
            })
        );

        for _ in 0..400 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
            if world.get::<WorkerTask>(villager) == Some(&WorkerTask::Idle) {
                break;
            }
        }
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
        assert_eq!(world.get::<Carry>(villager), Some(&Carry::Empty));
        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        assert_eq!(
            state.stockpile,
            ResourceStockpile {
                food: 200,
                wood: 302,
                gold: 100,
            }
        );
    }

    #[test]
    fn farm_allows_one_worker_until_retasked_or_stopped() {
        let (mut world, mut map) = skirmish_world();

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(2),
                kind: crate::catalog::BuildingKind::Farm,
                anchor: GridPos::new(11, 41),
            },
        );
        assert_eq!(outcome.reject, None, "farm placement rejected: {outcome:?}");
        let farm = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(3))
            .unwrap();
        for _ in 0..2000 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_construction(&mut world, SIM_STEP_SECONDS);
            if world.get::<Building>(farm).unwrap().construction.complete {
                break;
            }
        }
        assert!(world.get::<Building>(farm).unwrap().construction.complete);
        let farm_source = world.get::<ResourceSource>(farm).unwrap();
        let farm_source_id = farm_source.id;
        assert_eq!(farm_source.kind, ResourceKind::Food);
        assert_eq!(farm_source.remaining, None, "farms are renewable");
        assert_eq!(farm_source.assigned_worker, None);

        // First worker reserves; the second rejects FarmOccupied.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(1), UnitId(3)], farm_source_id),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(3), RejectReason::FarmOccupied)]
        );
        assert_eq!(
            world.get::<ResourceSource>(farm).unwrap().assigned_worker,
            Some(UnitId(1))
        );

        // Stop releases the assignment.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(1)],
                kind: UnitCommandKind::Stop,
            }),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        assert_eq!(
            world.get::<ResourceSource>(farm).unwrap().assigned_worker,
            None
        );

        // The freed farm accepts the next worker; an accepted retask releases.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            gather_command(TeamId(1), vec![UnitId(3)], farm_source_id),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(3)]);
        assert_eq!(
            world.get::<ResourceSource>(farm).unwrap().assigned_worker,
            Some(UnitId(3))
        );
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(3)],
                kind: UnitCommandKind::Move {
                    target: Vec2::new(2.5, 2.5),
                },
            }),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(3)]);
        assert_eq!(
            world.get::<ResourceSource>(farm).unwrap().assigned_worker,
            None
        );
    }

    #[test]
    fn walled_in_gatherer_idles_through_cleanup_and_releases_the_farm() {
        let mut world = World::new();
        let mut map = GridMap::new(24, 24);
        test_economy(&mut world);

        // A completed Farm serving villager 1, plus a Town Center drop-off.
        let farm = world
            .spawn((
                Building {
                    id: BuildingId(1),
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::Farm,
                    construction: crate::buildings::ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(GridPos::new(4, 16), 2, 2),
                ResourceSource {
                    id: ResourceId(1),
                    kind: ResourceKind::Food,
                    remaining: None,
                    assigned_worker: Some(UnitId(1)),
                },
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(BuildingId(1), farm);
        world
            .get_resource_or_insert_with(ResourceIndex::default)
            .insert(ResourceId(1), farm);
        let town_center = world
            .spawn((
                Building {
                    id: BuildingId(2),
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::TownCenter,
                    construction: crate::buildings::ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(GridPos::new(16, 16), 4, 4),
                Dropoff { team: TeamId(1) },
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(BuildingId(2), town_center);

        // The assigned gatherer sits walled in with a full carry, so its
        // drop-off route is impossible.
        let worker = spawn_villager(&mut world, UnitId(1), map.cell_center(GridPos::new(4, 4)));
        for cell in [
            GridPos::new(3, 3),
            GridPos::new(4, 3),
            GridPos::new(5, 3),
            GridPos::new(3, 4),
            GridPos::new(5, 4),
            GridPos::new(3, 5),
            GridPos::new(4, 5),
            GridPos::new(5, 5),
        ] {
            map.set_blocked(cell, true);
        }
        world.entity_mut(worker).insert((
            WorkerTask::Gathering {
                source: ResourceId(1),
            },
            Carry::Holding {
                kind: ResourceKind::Food,
                amount: NonZeroU32::new(10).unwrap(),
            },
            GatherProgress::default(),
        ));

        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

        assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
        assert!(world.get::<MoveOrder>(worker).is_none());
        assert_eq!(
            world.get::<ResourceSource>(farm).unwrap().assigned_worker,
            None,
            "route failure must release the Farm assignment"
        );
        assert_eq!(
            world.get::<Carry>(worker),
            Some(&Carry::Holding {
                kind: ResourceKind::Food,
                amount: NonZeroU32::new(10).unwrap(),
            }),
            "cleanup never discards Carry"
        );
        assert_eq!(
            world.resource::<LastRouteReject>().0,
            Some(RejectReason::Unreachable),
            "typed code recorded for bridge feedback"
        );
    }

    /// Ordering must measure each drop-off's geometric center, not its anchor
    /// cell. Worker at (10.5, 44.5): the 4×4 Town Center's anchor cell center
    /// (12.5, 46.5) is 8 units² away while the 2×2 Storehouse's anchor cell
    /// center (13.5, 44.5) is 9 — anchor-based sorting picks the Town Center.
    /// Geometric centers are (14, 48) at 24.5 vs (14, 45) at 12.5: the
    /// Storehouse is truly nearer and must win.
    #[test]
    fn nearest_dropoff_orders_by_geometric_center_not_anchor() {
        let mut world = World::new();
        let mut map = GridMap::new(24, 64);
        test_economy(&mut world);

        let town_center = world
            .spawn((
                Building {
                    id: BuildingId(1),
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::TownCenter,
                    construction: crate::buildings::ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(GridPos::new(12, 46), 4, 4),
                Dropoff { team: TeamId(1) },
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(BuildingId(1), town_center);
        let storehouse = world
            .spawn((
                Building {
                    id: BuildingId(2),
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::Storehouse,
                    construction: crate::buildings::ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(GridPos::new(13, 44), 2, 2),
                Dropoff { team: TeamId(1) },
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(BuildingId(2), storehouse);
        for cell in Footprint::new(GridPos::new(12, 46), 4, 4)
            .cells()
            .into_iter()
            .chain(Footprint::new(GridPos::new(13, 44), 2, 2).cells())
        {
            map.set_blocked(cell, true);
        }

        let worker = spawn_villager(&mut world, UnitId(1), Vec2::new(10.5, 44.5));
        let used = seed_used_excluding(&world, &map, worker);

        let (dropoff, slot, _) = nearest_reachable_dropoff(&world, &map, worker, &used)
            .expect("a reachable drop-off exists");

        assert_eq!(
            dropoff,
            BuildingId(2),
            "the truly nearer Storehouse must win over the anchor-nearer Town Center"
        );
        assert!(
            Footprint::new(GridPos::new(13, 44), 2, 2).is_immediately_adjacent(slot),
            "assigned slot {slot:?} is on the Storehouse perimeter"
        );
    }

    /// Exact distance ties break on `BuildingId`, never on `BuildingIndex`'s
    /// arbitrary `HashMap` iteration order. Worker at (10.5, 10.5): the left
    /// Storehouse's geometric center (9, 10) and the right one's (12, 10) are
    /// both exactly 2.5 units² away, so the lower id must win every run. The
    /// higher id is inserted first so insertion order alone cannot explain
    /// the outcome.
    #[test]
    fn nearest_dropoff_breaks_distance_ties_on_building_id() {
        let mut world = World::new();
        let mut map = GridMap::new(24, 64);
        test_economy(&mut world);

        for (id, anchor) in [
            (BuildingId(2), GridPos::new(8, 9)),
            (BuildingId(1), GridPos::new(11, 9)),
        ] {
            let storehouse = world
                .spawn((
                    Building {
                        id,
                        team: TeamId(1),
                        kind: crate::catalog::BuildingKind::Storehouse,
                        construction: crate::buildings::ConstructionState {
                            progress_seconds: 0.0,
                            complete: true,
                            active_builder: None,
                        },
                    },
                    Footprint::new(anchor, 2, 2),
                    Dropoff { team: TeamId(1) },
                ))
                .id();
            world
                .get_resource_or_insert_with(BuildingIndex::default)
                .insert(id, storehouse);
            for cell in Footprint::new(anchor, 2, 2).cells() {
                map.set_blocked(cell, true);
            }
        }

        let worker = spawn_villager(&mut world, UnitId(1), Vec2::new(10.5, 10.5));
        let used = seed_used_excluding(&world, &map, worker);

        let (dropoff, slot, _) = nearest_reachable_dropoff(&world, &map, worker, &used)
            .expect("a reachable drop-off exists");

        assert_eq!(
            dropoff,
            BuildingId(1),
            "the lower BuildingId must win an exact distance tie"
        );
        assert!(
            Footprint::new(GridPos::new(11, 9), 2, 2).is_immediately_adjacent(slot),
            "assigned slot {slot:?} is on the winning Storehouse perimeter"
        );
    }
}
