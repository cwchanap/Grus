//! FIFO production queues on completed producers, derived population, spawn
//! clearance, rally points, and one-time Age 2 advancement. Production is the
//! last system in the canonical fixed-step order, so an Age 2 completion here
//! affects the next economy tick.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::{Component, Entity, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{
    AGE_TWO_COST, AGE_TWO_SECONDS, Age, BuildingKind, Cost, MAX_POPULATION, UnitKind,
    building_spec, unit_spec,
};
use crate::commands::{CommandResult, RejectReason, UnitIndex, assign_move_toward, spawn_unit};
use crate::economy::{Carry, GatherProgress, TeamEconomy, WorkerTask};
use crate::ids::{BuildingId, IdAllocator, TeamId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::gameplay_active;

/// What one production job trains: a catalogue unit or the one-time Age 2
/// research. The Town Center produces both; other producers make one unit kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKind {
    Unit(UnitKind),
    Age2,
}

impl ProductionKind {
    /// Cost charged exactly once, on enqueue acceptance.
    pub(crate) fn cost(self) -> Cost {
        match self {
            Self::Unit(kind) => unit_spec(kind).cost,
            Self::Age2 => AGE_TWO_COST,
        }
    }

    /// Train/research seconds for the queue head.
    pub(crate) fn seconds(self) -> f32 {
        match self {
            Self::Unit(kind) => unit_spec(kind).train_seconds as f32,
            Self::Age2 => AGE_TWO_SECONDS as f32,
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
    matches!(
        kind,
        BuildingKind::TownCenter
            | BuildingKind::Barracks
            | BuildingKind::ArcheryRange
            | BuildingKind::Stable
    )
}

/// Fixed producer compatibility.
pub fn produces(kind: BuildingKind, job: ProductionKind) -> bool {
    match kind {
        BuildingKind::TownCenter => {
            matches!(
                job,
                ProductionKind::Unit(UnitKind::Villager) | ProductionKind::Age2
            )
        }
        BuildingKind::Barracks => job == ProductionKind::Unit(UnitKind::Spearman),
        BuildingKind::ArcheryRange => job == ProductionKind::Unit(UnitKind::Archer),
        BuildingKind::Stable => job == ProductionKind::Unit(UnitKind::Cavalry),
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

/// Completed Town Center/House capacity for the team, clamped to 100.
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
                            BuildingKind::TownCenter | BuildingKind::House
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

    let cost = job.cost();
    let affordable = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .is_some_and(|state| {
            state.stockpile.food >= cost.food
                && state.stockpile.wood >= cost.wood
                && state.stockpile.gold >= cost.gold
        });
    if !affordable {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&issuer)
    {
        state.stockpile.food -= cost.food;
        state.stockpile.wood -= cost.wood;
        state.stockpile.gold -= cost.gold;
    }
    push_job(world, building_entity, ProductionJob { kind: job });
    result
}

/// Applies an accepted `EnqueueAgeUp`: Town Center FIFO, once ever. The
/// one-time lock is set on acceptance so a second command rejects immediately.
pub(crate) fn apply_enqueue_age_up(
    world: &mut World,
    issuer: TeamId,
    building: BuildingId,
) -> CommandResult {
    let mut result = CommandResult::default();
    let building_entity = match validate_enqueue(world, issuer, building, ProductionKind::Age2) {
        Ok(entity) => entity,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };

    // One-time lock plus affordability, after the shared chain above.
    let lock = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .map(|state| state.age_up_started || state.age == Age::Age2)
        .unwrap_or(true);
    if lock {
        result.reject = Some(RejectReason::Locked);
        return result;
    }
    let cost = ProductionKind::Age2.cost();
    let affordable = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .is_some_and(|state| {
            state.stockpile.food >= cost.food
                && state.stockpile.wood >= cost.wood
                && state.stockpile.gold >= cost.gold
        });
    if !affordable {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&issuer)
    {
        state.stockpile.food -= cost.food;
        state.stockpile.wood -= cost.wood;
        state.stockpile.gold -= cost.gold;
        state.age_up_started = true;
    }
    push_job(
        world,
        building_entity,
        ProductionJob {
            kind: ProductionKind::Age2,
        },
    );
    result
}

/// Shared enqueue chain: building exists → owned → completed → producer
/// compatibility → unit unlock (age jobs lock in `apply_enqueue_age_up`).
fn validate_enqueue(
    world: &World,
    issuer: TeamId,
    building: BuildingId,
    job: ProductionKind,
) -> Result<Entity, RejectReason> {
    let building_entity = world
        .get_resource::<BuildingIndex>()
        .and_then(|index| index.entity(building))
        .ok_or(RejectReason::BuildingMissing)?;
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
    if let ProductionKind::Unit(kind) = job {
        let team_age = world
            .get_resource::<TeamEconomy>()
            .and_then(|economy| economy.0.get(&issuer))
            .map(|state| state.age);
        if team_age.is_none_or(|age| age < unit_spec(kind).required_age) {
            return Err(RejectReason::Locked);
        }
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
    let building_entity = world
        .get_resource::<BuildingIndex>()
        .and_then(|index| index.entity(building))
        .ok_or(RejectReason::BuildingMissing);
    let building_entity = match building_entity {
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
            ProductionKind::Age2 => {
                if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
                    && let Some(state) = economy.0.get_mut(&team)
                {
                    state.age = Age::Age2;
                }
                finish_head(world, entity);
            }
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
