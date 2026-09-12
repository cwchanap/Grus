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
use crate::commands::{CommandResult, RejectReason, UnitIndex, spawn_unit};
use crate::economy::{Carry, GatherProgress, TeamEconomy, WorkerTask};
use crate::ids::{BuildingId, IdAllocator, TeamId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};

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
        Some(state) if state.team == issuer => {}
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
    if let Some(rally) = world.get::<RallyPoint>(entity).copied()
        && let Some(path) = map.find_path(slot, rally.0)
    {
        let waypoints = path
            .into_iter()
            .skip(1)
            .map(|cell| map.cell_center(cell))
            .collect::<Vec<_>>();
        if !waypoints.is_empty() {
            world.entity_mut(spawned).insert(MoveOrder {
                waypoints,
                next: 0,
                goal: rally.0,
                map_revision: map.revision(),
                last_failed_replan: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::*;
    use crate::buildings::ConstructionState;
    use crate::catalog::{Age, ResourceKind, building_spec};
    use crate::commands::{PlayerCommand, apply_player_command};
    use crate::economy::{ResourceStockpile, gather_rate_for_age, spawn_resource_source};
    use crate::ids::{IdAllocator, ResourceId, UnitId};
    use crate::movement::{SIM_STEP_SECONDS, step_movement};

    const TEAM: TeamId = TeamId(1);
    const ENEMY: TeamId = TeamId(2);

    fn open_world() -> (World, GridMap) {
        let mut world = World::new();
        let mut economy = TeamEconomy::default();
        for team in [TEAM, ENEMY] {
            economy.insert_team(
                team,
                ResourceStockpile {
                    food: 1000,
                    wood: 1000,
                    gold: 1000,
                },
                Age::Age1,
            );
        }
        world.insert_resource(economy);
        world.insert_resource(IdAllocator::new(100, 100, 100));
        (world, GridMap::new(64, 64))
    }

    fn complete_building(
        world: &mut World,
        map: &mut GridMap,
        id: BuildingId,
        kind: BuildingKind,
        anchor: GridPos,
        team: TeamId,
    ) -> Entity {
        let spec = building_spec(kind);
        let footprint = Footprint::new(anchor, spec.width, spec.height);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id,
                    team,
                    kind,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                footprint,
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(id, entity);
        entity
    }

    fn incomplete_building(
        world: &mut World,
        map: &mut GridMap,
        id: BuildingId,
        kind: BuildingKind,
        anchor: GridPos,
        team: TeamId,
    ) -> Entity {
        let spec = building_spec(kind);
        let footprint = Footprint::new(anchor, spec.width, spec.height);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id,
                    team,
                    kind,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: false,
                        active_builder: None,
                    },
                },
                footprint,
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(id, entity);
        entity
    }

    fn enqueue(
        world: &mut World,
        map: &mut GridMap,
        building: BuildingId,
        kind: UnitKind,
    ) -> CommandResult {
        apply_player_command(
            world,
            map,
            PlayerCommand::EnqueueUnit {
                issuer: TEAM,
                building,
                kind,
            },
        )
    }

    fn enqueue_age_up(world: &mut World, map: &mut GridMap, building: BuildingId) -> CommandResult {
        apply_player_command(
            world,
            map,
            PlayerCommand::EnqueueAgeUp {
                issuer: TEAM,
                building,
            },
        )
    }

    fn stockpile(world: &World, team: TeamId) -> ResourceStockpile {
        world.resource::<TeamEconomy>().0[&team].stockpile
    }

    fn queue_of(world: &World, entity: Entity) -> ProductionQueue {
        world.get::<ProductionQueue>(entity).cloned().unwrap()
    }

    /// Regression world: two Barracks queues hold ready Spearman jobs at
    /// population 9 of 10, built entirely through the production APIs.
    fn setup_two_ready_barracks_at_pop_9_of_10() -> (World, GridMap, Entity, Entity) {
        let (mut world, mut map) = open_world();
        complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(8, 8),
            TEAM,
        );
        for index in 1..=9i32 {
            let cell = GridPos::new(2 + (index % 3) * 2, 2 + (index / 3) * 2);
            spawn_unit(
                &mut world,
                UnitId(index as u32),
                TEAM,
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
        }
        let first = complete_building(
            &mut world,
            &mut map,
            BuildingId(101),
            BuildingKind::Barracks,
            GridPos::new(20, 20),
            TEAM,
        );
        let second = complete_building(
            &mut world,
            &mut map,
            BuildingId(102),
            BuildingKind::Barracks,
            GridPos::new(32, 32),
            TEAM,
        );

        let first_result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Spearman);
        assert_eq!(first_result.reject, None, "first enqueue rejected");
        let second_result = enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Spearman);
        assert_eq!(second_result.reject, None, "second enqueue rejected");
        // Both charged once at acceptance: 2 × 60 Food.
        assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);

        // Advance to just below readiness; nothing spawns while not ready.
        for _ in 0..399 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert_eq!(population_used(&world, TEAM), 9);
        assert!((queue_of(&world, first).progress_seconds - 19.95).abs() < 1e-3);
        assert!((queue_of(&world, second).progress_seconds - 19.95).abs() < 1e-3);

        (world, map, first, second)
    }

    fn step_economy_once(world: &mut World, map: &mut GridMap) {
        crate::economy::step_economy(world, map, SIM_STEP_SECONDS);
    }

    #[test]
    fn age_two_gather_rate_is_two_point_two() {
        assert_eq!(
            gather_rate_for_age(Age::Age1),
            crate::catalog::BASE_GATHER_RATE
        );
        assert_eq!(gather_rate_for_age(Age::Age2), 2.2);
    }

    #[test]
    fn villager_enqueue_charges_fifty_food_exactly_once() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );

        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, None);
        assert_eq!(stockpile(&world, TEAM).food, 950);

        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }

        // Spawned exactly once, charged exactly once.
        assert_eq!(stockpile(&world, TEAM).food, 950);
        assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
        assert_eq!(queue_of(&world, town_center).jobs.len(), 0);
        assert_eq!(population_used(&world, TEAM), 1);
        assert_eq!(population_cap(&world, TEAM), 10);
    }

    #[test]
    fn enqueue_with_insufficient_stockpile_rejects_and_never_charges() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        // 49 Food cannot afford the 50-Food Villager.
        world
            .resource_mut::<TeamEconomy>()
            .0
            .get_mut(&TEAM)
            .unwrap()
            .stockpile
            .food = 49;

        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
        assert_eq!(stockpile(&world, TEAM).food, 49, "rejected enqueue charged");
        assert!(
            world.get::<ProductionQueue>(town_center).is_none(),
            "rejected enqueue created a queue or pushed a job"
        );
    }

    #[test]
    fn archer_into_barracks_is_wrong_producer_and_never_charged() {
        let (mut world, mut map) = open_world();
        let barracks = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::Barracks,
            GridPos::new(30, 30),
            TEAM,
        );

        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Archer);

        assert_eq!(result.reject, Some(RejectReason::WrongProducer));
        assert_eq!(
            stockpile(&world, TEAM),
            ResourceStockpile {
                food: 1000,
                wood: 1000,
                gold: 1000
            }
        );
        assert!(world.get::<ProductionQueue>(barracks).is_none());
    }

    #[test]
    fn cavalry_into_stable_is_locked_before_age_two() {
        let (mut world, mut map) = open_world();
        let stable = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::Stable,
            GridPos::new(30, 30),
            TEAM,
        );

        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Cavalry);

        assert_eq!(result.reject, Some(RejectReason::Locked));
        assert_eq!(
            stockpile(&world, TEAM),
            ResourceStockpile {
                food: 1000,
                wood: 1000,
                gold: 1000
            }
        );
        assert!(world.get::<ProductionQueue>(stable).is_none());
    }

    #[test]
    fn enqueue_validates_missing_unowned_incomplete_and_producer() {
        let (mut world, mut map) = open_world();
        complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        complete_building(
            &mut world,
            &mut map,
            BuildingId(101),
            BuildingKind::Barracks,
            GridPos::new(40, 40),
            ENEMY,
        );
        incomplete_building(
            &mut world,
            &mut map,
            BuildingId(102),
            BuildingKind::Barracks,
            GridPos::new(50, 50),
            TEAM,
        );
        complete_building(
            &mut world,
            &mut map,
            BuildingId(103),
            BuildingKind::House,
            GridPos::new(8, 8),
            TEAM,
        );

        // Nonexistent building.
        let result = enqueue(&mut world, &mut map, BuildingId(999), UnitKind::Villager);
        assert_eq!(result.reject, Some(RejectReason::BuildingMissing));

        // Enemy building.
        let result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Spearman);
        assert_eq!(result.reject, Some(RejectReason::NotOwned));

        // Incomplete producer: its queue is locked until completion.
        let result = enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Spearman);
        assert_eq!(result.reject, Some(RejectReason::Locked));

        // Wrong producer on two fronts: units belong to specific buildings.
        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Spearman);
        assert_eq!(result.reject, Some(RejectReason::WrongProducer));
        let result = enqueue(&mut world, &mut map, BuildingId(103), UnitKind::Villager);
        assert_eq!(result.reject, Some(RejectReason::WrongProducer));

        // No rejection charged anything or created a queue.
        assert_eq!(
            stockpile(&world, TEAM),
            ResourceStockpile {
                food: 1000,
                wood: 1000,
                gold: 1000
            }
        );
        let mut queues = world.query::<&ProductionQueue>();
        assert_eq!(queues.iter(&world).count(), 0);
    }

    #[test]
    fn age_up_charges_once_locks_immediately_and_completes_to_age_two() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );

        let result = enqueue_age_up(&mut world, &mut map, BuildingId(100));
        assert_eq!(result.reject, None);
        let state = &world.resource::<TeamEconomy>().0[&TEAM];
        assert_eq!(state.stockpile.food, 700);
        assert_eq!(state.stockpile.gold, 800);
        assert_eq!(state.stockpile.wood, 1000);
        assert!(state.age_up_started);

        // Second Age 2 command rejects immediately, without charging.
        let result = enqueue_age_up(&mut world, &mut map, BuildingId(100));
        assert_eq!(result.reject, Some(RejectReason::Locked));
        let state = &world.resource::<TeamEconomy>().0[&TEAM];
        assert_eq!(state.stockpile.food, 700);
        assert_eq!(state.stockpile.gold, 800);

        for _ in 0..901 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }

        assert_eq!(world.resource::<TeamEconomy>().0[&TEAM].age, Age::Age2);
        assert_eq!(queue_of(&world, town_center).jobs.len(), 0);

        // Still exactly once: even a fresh-looking command stays locked.
        let result = enqueue_age_up(&mut world, &mut map, BuildingId(100));
        assert_eq!(result.reject, Some(RejectReason::Locked));
    }

    #[test]
    fn age_two_gather_rate_applies_from_the_next_economy_tick() {
        let (mut world, mut map) = open_world();
        complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        spawn_resource_source(
            &mut world,
            &mut map,
            ResourceId(1),
            ResourceKind::Food,
            GridPos::new(20, 20),
            600,
        );
        let villager = spawn_unit(
            &mut world,
            UnitId(1),
            TEAM,
            map.cell_center(GridPos::new(21, 20)),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(villager).insert((
            Carry::Empty,
            GatherProgress::default(),
            WorkerTask::Idle,
        ));

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Gather {
                issuer: TEAM,
                workers: vec![UnitId(1)],
                source: ResourceId(1),
            },
        );
        assert_eq!(result.accepted_units, vec![UnitId(1)]);
        for _ in 0..50 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            if world.get::<MoveOrder>(villager).is_none() {
                break;
            }
        }
        step_economy_once(&mut world, &mut map);
        let age_one_progress = world.get::<GatherProgress>(villager).unwrap().0;
        assert!(
            (age_one_progress - 0.1).abs() < 1e-6,
            "Age 1 rate tick gathered {age_one_progress}"
        );
        assert_eq!(world.resource::<TeamEconomy>().0[&TEAM].age, Age::Age1);

        let result = enqueue_age_up(&mut world, &mut map, BuildingId(100));
        assert_eq!(result.reject, None);

        for _ in 0..901 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert_eq!(world.resource::<TeamEconomy>().0[&TEAM].age, Age::Age2);

        step_economy_once(&mut world, &mut map);
        let age_two_progress = world.get::<GatherProgress>(villager).unwrap().0;
        let delta = age_two_progress - age_one_progress;
        assert!(
            delta > 0.105 && (delta - 0.11).abs() < 1e-4,
            "the next economy tick after the flip must gather at 2.2/s: {delta}"
        );
    }

    #[test]
    fn two_ready_barracks_spawn_one_then_block_population_full() {
        let (mut world, mut map, first, second) = setup_two_ready_barracks_at_pop_9_of_10();

        step_production(&mut world, &mut map, SIM_STEP_SECONDS);

        // Ascending BuildingId: Barracks 101 wins the final slot.
        assert_eq!(population_used(&world, TEAM), 10);
        assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
        let first_queue = queue_of(&world, first);
        assert!(first_queue.jobs.is_empty());
        assert_eq!(first_queue.blocked, None);
        let second_queue = queue_of(&world, second);
        assert_eq!(second_queue.jobs.len(), 1);
        assert_eq!(second_queue.progress_seconds, 20.0);
        assert_eq!(second_queue.blocked, Some(RejectReason::PopulationFull));
        assert_eq!(world.resource::<IdAllocator>().next_unit, 101);
    }

    #[test]
    fn blocked_ready_job_sits_at_full_and_never_recharges() {
        let (mut world, mut map, _first, second) = setup_two_ready_barracks_at_pop_9_of_10();
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);

        // Waiting at 100% neither restarts training nor charges again.
        for _ in 0..100 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert_eq!(queue_of(&world, second).progress_seconds, 20.0);
        assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);

        // A freed slot spawns the retained job immediately, still uncharged.
        let filler = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        world.despawn(filler);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);

        assert_eq!(population_used(&world, TEAM), 10);
        assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_some());
        assert_eq!(queue_of(&world, second).jobs.len(), 0);
        assert_eq!(queue_of(&world, second).blocked, None);
        assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);
    }

    #[test]
    fn walled_producer_keeps_ready_job_with_no_spawn_space() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, None);

        // Wall in the whole immediate perimeter.
        map.set_blocked_rect(GridPos::new(29, 29), GridPos::new(34, 34));
        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        let queue = queue_of(&world, town_center);
        assert_eq!(queue.progress_seconds, 15.0);
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.blocked, Some(RejectReason::NoSpawnSpace));
        assert_eq!(population_used(&world, TEAM), 0);
        assert_eq!(world.resource::<IdAllocator>().next_unit, 100);

        // Reopening one perimeter cell spawns the retained job next tick.
        map.set_blocked(GridPos::new(29, 29), false);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        assert_eq!(population_used(&world, TEAM), 1);
        assert_eq!(world.resource::<IdAllocator>().next_unit, 101);
        let queue = queue_of(&world, town_center);
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.blocked, None);
        let spawned = world.resource::<UnitIndex>().entity(UnitId(100)).unwrap();
        let position = world.get::<SimPosition>(spawned).unwrap().current;
        assert_eq!(map.world_to_cell(position), GridPos::new(29, 29));
    }

    #[test]
    fn occupied_perimeter_cells_block_spawn() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, None);

        // Squatters on every immediate-perimeter cell: walkable but occupied.
        // They are enemy units so the team's own population stays below cap —
        // this isolates the spawn-clearance reject.
        for (index, cell) in Footprint::new(GridPos::new(30, 30), 4, 4)
            .perimeter_cells()
            .into_iter()
            .enumerate()
        {
            spawn_unit(
                &mut world,
                UnitId(index as u32 + 1),
                ENEMY,
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
        }
        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert_eq!(
            queue_of(&world, town_center).blocked,
            Some(RejectReason::NoSpawnSpace)
        );
        assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_none());
    }

    #[test]
    fn reserved_goal_on_last_free_perimeter_cell_blocks_spawn() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, None);

        // Block every perimeter cell except (29, 29), then reserve exactly that
        // cell as another unit's MoveOrder goal — the reservation seam counts
        // it as occupied.
        for cell in Footprint::new(GridPos::new(30, 30), 4, 4).perimeter_cells() {
            if cell != GridPos::new(29, 29) {
                map.set_blocked(cell, true);
            }
        }
        let traveler = spawn_unit(
            &mut world,
            UnitId(1),
            TEAM,
            map.cell_center(GridPos::new(2, 2)),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(traveler).insert(MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(29, 29))],
            next: 0,
            goal: GridPos::new(29, 29),
            map_revision: map.revision(),
            last_failed_replan: None,
        });

        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert_eq!(
            queue_of(&world, town_center).blocked,
            Some(RejectReason::NoSpawnSpace)
        );
        assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_none());
    }

    #[test]
    fn rally_routes_spawned_units_and_failed_rally_leaves_them_idle() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        let _barracks = complete_building(
            &mut world,
            &mut map,
            BuildingId(101),
            BuildingKind::Barracks,
            GridPos::new(40, 40),
            TEAM,
        );

        // Reachable rally: the spawned villager gets a normal Move.
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::SetRally {
                issuer: TEAM,
                building: BuildingId(100),
                target: GridPos::new(10, 10),
            },
        );
        assert_eq!(result.reject, None);
        assert_eq!(
            world.get::<RallyPoint>(town_center),
            Some(&RallyPoint(GridPos::new(10, 10)))
        );
        let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager);
        assert_eq!(result.reject, None);
        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        let villager = world.resource::<UnitIndex>().entity(UnitId(100)).unwrap();
        let order = world
            .get::<MoveOrder>(villager)
            .expect("rallied unit moves");
        assert_eq!(order.goal, GridPos::new(10, 10));
        assert_eq!(
            order.waypoints.last().copied(),
            Some(map.cell_center(GridPos::new(10, 10)))
        );

        // Unreachable rally: the spearman still spawns, but idles.
        map.set_blocked_rect(GridPos::new(0, 0), GridPos::new(8, 8));
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::SetRally {
                issuer: TEAM,
                building: BuildingId(101),
                target: GridPos::new(5, 5),
            },
        );
        assert_eq!(result.reject, None);
        let result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Spearman);
        assert_eq!(result.reject, None);
        for _ in 0..401 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        let spearman = world.resource::<UnitIndex>().entity(UnitId(101)).unwrap();
        assert!(world.get::<MoveOrder>(spearman).is_none());
        assert_eq!(world.get::<WorkerTask>(spearman), Some(&WorkerTask::Idle));
        let position = world.get::<SimPosition>(spearman).unwrap().current;
        assert!(
            Footprint::new(GridPos::new(40, 40), 3, 3)
                .is_immediately_adjacent(map.world_to_cell(position)),
            "spawned at the producer perimeter, got {position:?}"
        );
    }

    #[test]
    fn set_rally_validates_missing_and_unowned_buildings() {
        let (mut world, mut map) = open_world();
        complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        let enemy = complete_building(
            &mut world,
            &mut map,
            BuildingId(101),
            BuildingKind::TownCenter,
            GridPos::new(50, 50),
            ENEMY,
        );

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::SetRally {
                issuer: TEAM,
                building: BuildingId(999),
                target: GridPos::new(10, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::BuildingMissing));

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::SetRally {
                issuer: TEAM,
                building: BuildingId(101),
                target: GridPos::new(10, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::NotOwned));
        assert_eq!(world.get::<RallyPoint>(enemy), None);
    }

    #[test]
    fn queue_is_fifo_and_only_the_head_advances() {
        let (mut world, mut map) = open_world();
        let town_center = complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(30, 30),
            TEAM,
        );
        assert_eq!(
            enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager).reject,
            None
        );
        assert_eq!(
            enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Villager).reject,
            None
        );
        assert_eq!(stockpile(&world, TEAM).food, 900);

        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        // Exactly one spawn; the second job is now the advancing head.
        assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
        assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_none());
        let queue = queue_of(&world, town_center);
        assert_eq!(queue.jobs.len(), 1);
        assert!(queue.progress_seconds > 0.0);

        for _ in 0..301 {
            step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_some());
        assert_eq!(queue_of(&world, town_center).jobs.len(), 0);
        assert_eq!(population_used(&world, TEAM), 2);
        assert_eq!(stockpile(&world, TEAM).food, 900);
    }

    #[test]
    fn population_cap_sums_completed_tc_and_house_and_clamps() {
        let (mut world, mut map) = open_world();
        complete_building(
            &mut world,
            &mut map,
            BuildingId(100),
            BuildingKind::TownCenter,
            GridPos::new(8, 8),
            TEAM,
        );
        assert_eq!(population_cap(&world, TEAM), 10);

        incomplete_building(
            &mut world,
            &mut map,
            BuildingId(101),
            BuildingKind::House,
            GridPos::new(20, 20),
            TEAM,
        );
        assert_eq!(
            population_cap(&world, TEAM),
            10,
            "incomplete House must not count"
        );

        complete_building(
            &mut world,
            &mut map,
            BuildingId(102),
            BuildingKind::House,
            GridPos::new(30, 30),
            TEAM,
        );
        assert_eq!(population_cap(&world, TEAM), 20);

        for index in 0..9i32 {
            complete_building(
                &mut world,
                &mut map,
                BuildingId((103 + index) as u32),
                BuildingKind::House,
                GridPos::new(40 + (index % 3) * 6, 8 + (index / 3) * 6),
                TEAM,
            );
        }
        // 10 TC + 10 Houses would exceed the clamp.
        assert_eq!(population_cap(&world, TEAM), 100);

        // Population is per team: the enemy's units and buildings don't count.
        spawn_unit(
            &mut world,
            UnitId(1),
            ENEMY,
            map.cell_center(GridPos::new(2, 2)),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        complete_building(
            &mut world,
            &mut map,
            BuildingId(200),
            BuildingKind::House,
            GridPos::new(56, 56),
            ENEMY,
        );
        assert_eq!(population_used(&world, ENEMY), 1);
        assert_eq!(population_used(&world, TEAM), 0);
        assert_eq!(population_cap(&world, ENEMY), 10);
    }
}
