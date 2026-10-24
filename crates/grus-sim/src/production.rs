//! FIFO production queues on completed producers, derived population, spawn
//! clearance, and rally points. Production is the last system in the
//! canonical fixed-step order.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::{Component, Entity, World};

use crate::buildings::{Building, BuildingIndex, commandable_building_entity};
use crate::catalog::{BuildingKind, Cost, MAX_POPULATION, UnitKind, building_spec, unit_spec};
use crate::commands::{CommandResult, RejectReason, UnitIndex, assign_move_toward, spawn_unit};
use crate::economy::{Carry, GatherProgress, TeamEconomy, WorkerTask};
use crate::ids::{BuildingId, IdAllocator, TeamId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::gameplay_active;

/// What one production job trains: a catalogue unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKind {
    Unit(UnitKind),
}

impl ProductionKind {
    /// Cost charged exactly once, on enqueue acceptance.
    pub(crate) fn cost(self) -> Cost {
        match self {
            Self::Unit(kind) => unit_spec(kind).cost,
        }
    }

    /// Train seconds for the queue head.
    pub(crate) fn seconds(self) -> f32 {
        match self {
            Self::Unit(kind) => unit_spec(kind).train_seconds as f32,
        }
    }
}

/// One queued production job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionJob {
    pub kind: ProductionKind,
}

/// FIFO queue owned by a completed producer. Only the head job advances; a
/// ready head blocked by population or spawn clearance sits at 100% until it
/// can complete, and `blocked` names why it is waiting.
#[derive(Component, Clone, Debug, Default)]
pub struct ProductionQueue {
    pub jobs: VecDeque<ProductionJob>,
    pub progress_seconds: f32,
    pub blocked: Option<RejectReason>,
}

/// Stored rally target for a producer; spawned units Move toward it.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct RallyPoint(pub GridPos);

/// Building kinds that own a production queue on completion.
pub(crate) fn is_producer(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::ResearchCenter | BuildingKind::Barracks)
}

/// Fixed producer compatibility: the Research Center trains Workers; the
/// Barracks trains Guards and Riflemen.
pub fn produces(kind: BuildingKind, job: ProductionKind) -> bool {
    match kind {
        BuildingKind::ResearchCenter => job == ProductionKind::Unit(UnitKind::Worker),
        BuildingKind::Barracks => matches!(
            job,
            ProductionKind::Unit(UnitKind::Guard) | ProductionKind::Unit(UnitKind::Rifleman)
        ),
        _ => false,
    }
}

/// Live team units.
pub fn population_used(world: &World, team: TeamId) -> u32 {
    world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter(|(_, entity)| {
                    world
                        .get::<Unit>(**entity)
                        .is_some_and(|unit| unit.team == team)
                })
                .count() as u32
        })
        .unwrap_or(0)
}

/// Completed Research Center/Bunkhouse capacity for the team, clamped to 100.
pub fn population_cap(world: &World, team: TeamId) -> u32 {
    let capacity = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(_, entity)| world.get::<Building>(*entity))
                .filter(|building| {
                    building.team == team
                        && building.construction.complete
                        && matches!(
                            building.kind,
                            BuildingKind::ResearchCenter | BuildingKind::Bunkhouse
                        )
                })
                .map(|building| building_spec(building.kind).population_capacity)
                .sum()
        })
        .unwrap_or(0);
    capacity.min(MAX_POPULATION)
}

/// Applies an accepted `EnqueueUnit`: ownership → completion → producer
/// compatibility → unlock → affordability. Charges once on acceptance.
pub(crate) fn apply_enqueue_unit(
    world: &mut World,
    issuer: TeamId,
    building: BuildingId,
    kind: UnitKind,
) -> CommandResult {
    let mut result = CommandResult::default();
    let job = ProductionKind::Unit(kind);
    let building_entity = match validate_enqueue(world, issuer, building, job) {
        Ok(entity) => entity,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };

    // Atomic spend before the queue push: a failed `try_spend` leaves every
    // currency field unchanged and nothing enqueued.
    let cost = job.cost();
    let spent = world
        .get_resource_mut::<TeamEconomy>()
        .map(|mut economy| {
            economy
                .0
                .get_mut(&issuer)
                .is_some_and(|state| state.stockpile.try_spend(cost))
        })
        .unwrap_or(false);
    if !spent {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    push_job(world, building_entity, ProductionJob { kind: job });
    result
}

/// Shared enqueue chain: building exists (hidden enemy indistinguishable
/// from absent, fog privacy) → owned → completed → producer compatibility.
fn validate_enqueue(
    world: &World,
    issuer: TeamId,
    building: BuildingId,
    job: ProductionKind,
) -> Result<Entity, RejectReason> {
    let building_entity = commandable_building_entity(world, issuer, building)?;
    let state = world
        .get::<Building>(building_entity)
        .ok_or(RejectReason::BuildingMissing)?;
    if state.team != issuer {
        return Err(RejectReason::NotOwned);
    }
    // An incomplete building's queue is locked until completion.
    if !state.construction.complete {
        return Err(RejectReason::Locked);
    }
    if !produces(state.kind, job) {
        return Err(RejectReason::WrongProducer);
    }
    Ok(building_entity)
}

/// Applies an accepted `SetRally` on an owned building: stores the map target.
pub(crate) fn apply_set_rally(
    world: &mut World,
    issuer: TeamId,
    building: BuildingId,
    target: GridPos,
) -> CommandResult {
    let mut result = CommandResult::default();
    // A hidden enemy building answers `BuildingMissing` like an absent id:
    // distinct rejects would let a caller probe ids to count enemy
    // producers.
    let building_entity = match commandable_building_entity(world, issuer, building) {
        Ok(entity) => entity,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };
    match world.get::<Building>(building_entity) {
        Some(state) if state.team == issuer && is_producer(state.kind) => {}
        Some(state) if state.team == issuer => {
            result.reject = Some(RejectReason::WrongProducer);
            return result;
        }
        _ => {
            result.reject = Some(RejectReason::NotOwned);
            return result;
        }
    }
    world.entity_mut(building_entity).insert(RallyPoint(target));
    result
}

fn push_job(world: &mut World, building_entity: Entity, job: ProductionJob) {
    match world.get_mut::<ProductionQueue>(building_entity) {
        Some(mut queue) => queue.jobs.push_back(job),
        None => {
            world.entity_mut(building_entity).insert(ProductionQueue {
                jobs: VecDeque::from([job]),
                ..ProductionQueue::default()
            });
        }
    }
}

/// Completeness tolerance for queue-head progress: float accumulation drifts
/// a few thousandths of a second over a 45s job, so a tick that reaches the
/// nominal duration snaps the head to exactly 100% and completes it in that
/// same tick. Far below one 50ms simulation step.
const PROGRESS_EPSILON: f32 = 0.01;

/// Advances production one fixed tick. Every queue head advances (clamped at
/// 100%); ready producers then complete in ascending `BuildingId` order,
/// recomputing population after each successful spawn so same-tick completions
/// cannot both consume the final slot. Runs LAST in the fixed-step chain.
pub fn step_production(world: &mut World, map: &mut GridMap, seconds: f32) {
    if !gameplay_active(world) {
        return;
    }
    let mut producers: Vec<(Entity, BuildingId, TeamId)> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let queue = world.get::<ProductionQueue>(*entity)?;
                    if queue.jobs.is_empty() {
                        return None;
                    }
                    let team = world.get::<Building>(*entity)?.team;
                    Some((*entity, *id, team))
                })
                .collect()
        })
        .unwrap_or_default();
    producers.sort_unstable_by_key(|(_, id, _)| *id);

    for (entity, _, team) in producers {
        let Some(job) = world
            .get::<ProductionQueue>(entity)
            .and_then(|queue| queue.jobs.front().copied())
        else {
            continue;
        };
        let required = job.kind.seconds();

        // Only the head advances, and never past 100%: a blocked ready job
        // sits at full progress instead of recharging or restarting.
        let progress = {
            let mut queue = world
                .get_mut::<ProductionQueue>(entity)
                .expect("producer queue collected above");
            if queue.progress_seconds < required {
                let next = queue.progress_seconds + seconds;
                queue.progress_seconds = if next + PROGRESS_EPSILON >= required {
                    required
                } else {
                    next
                };
            }
            queue.progress_seconds
        };
        if progress < required {
            continue;
        }

        match job.kind {
            ProductionKind::Unit(kind) => {
                if population_used(world, team) >= population_cap(world, team) {
                    if let Some(mut queue) = world.get_mut::<ProductionQueue>(entity) {
                        queue.blocked = Some(RejectReason::PopulationFull);
                    }
                    continue;
                }
                let Some(slot) = spawn_slot(world, map, entity) else {
                    if let Some(mut queue) = world.get_mut::<ProductionQueue>(entity) {
                        queue.blocked = Some(RejectReason::NoSpawnSpace);
                    }
                    continue;
                };
                spawn_trained_unit(world, map, entity, team, kind, slot);
                finish_head(world, entity);
            }
        }
    }
}

/// Pops the finished head job and resets progress for the next one.
fn finish_head(world: &mut World, entity: Entity) {
    if let Some(mut queue) = world.get_mut::<ProductionQueue>(entity) {
        queue.jobs.pop_front();
        queue.progress_seconds = 0.0;
        queue.blocked = None;
    }
}

/// First walkable, unoccupied immediate-perimeter slot: no live unit stands on
/// it and no reserved `MoveOrder` goal claims it (same seam as Move/build/gather).
fn spawn_slot(world: &World, map: &GridMap, entity: Entity) -> Option<GridPos> {
    let footprint = world.get::<Footprint>(entity).copied()?;
    let mut used = HashSet::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for unit_entity in entities {
        if let Some(position) = world.get::<SimPosition>(unit_entity) {
            used.insert(map.world_to_cell(position.current));
        }
        if let Some(order) = world.get::<MoveOrder>(unit_entity) {
            used.insert(order.goal);
        }
    }
    footprint
        .perimeter_cells()
        .into_iter()
        .find(|cell| map.is_walkable(*cell) && !used.contains(cell))
}

/// Spawns the trained unit through the allocator and assigns its rally Move.
/// A failed rally route leaves the unit spawned and idle.
fn spawn_trained_unit(
    world: &mut World,
    map: &mut GridMap,
    entity: Entity,
    team: TeamId,
    kind: UnitKind,
    slot: GridPos,
) {
    let unit_id = world.resource_mut::<IdAllocator>().allocate_unit();
    let spec = unit_spec(kind);
    let spawned = spawn_unit(
        world,
        unit_id,
        team,
        map.cell_center(slot),
        kind,
        spec.speed,
    );
    world
        .entity_mut(spawned)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    // Rally uses the normal Move assignment path (destination generation plus
    // reservation seeding), so a rallied unit cannot claim an already-reserved
    // goal. A failed route leaves the unit spawned and idle.
    if let Some(rally) = world.get::<RallyPoint>(entity).copied() {
        assign_move_toward(world, map, spawned, slot, rally.0);
    }
}

#[cfg(test)]
mod tests;
