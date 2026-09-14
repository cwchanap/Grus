//! Building entities, authoritative placement validation, and construction
//! stepping. One assigned builder advances construction; accepted replacement
//! commands pause or retask it, rejected ones preserve the current work.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::catalog::{BuildingKind, ResourceKind, UnitKind, building_spec};
use crate::commands::{CommandResult, RejectReason, UnitIndex, approach_slots, owned_unit_entity};
use crate::economy::{
    Dropoff, ResourceIndex, ResourceSource, TeamEconomy, WorkerTask, cancel_worker_activity,
};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{ProductionQueue, is_producer};

#[derive(Component, Debug)]
pub struct Building {
    pub id: BuildingId,
    pub team: TeamId,
    pub kind: BuildingKind,
    pub construction: ConstructionState,
}

#[derive(Clone, Copy, Debug)]
pub struct ConstructionState {
    pub progress_seconds: f32,
    pub complete: bool,
    pub active_builder: Option<UnitId>,
}

/// Stable lookup from `BuildingId` to its entity, mirroring `UnitIndex`.
#[derive(Debug, Default, Resource)]
pub struct BuildingIndex(HashMap<BuildingId, Entity>);

impl BuildingIndex {
    pub fn entity(&self, id: BuildingId) -> Option<Entity> {
        self.0.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&BuildingId, &Entity)> {
        self.0.iter()
    }

    pub(crate) fn insert(&mut self, id: BuildingId, entity: Entity) {
        self.0.insert(id, entity);
    }
}

/// Everything an accepted placement needs to apply: the builder entity, the
/// authored footprint, and the immediate-perimeter slot plus route the builder
/// was assigned.
pub struct PlacementPlan {
    pub builder: Entity,
    pub footprint: Footprint,
    pub slot: GridPos,
    pub route: Vec<Vec2>,
}

/// Authoritative placement validation. Checks, in order: owned villager →
/// kind unlocked/buildable → footprint in bounds → footprint cells walkable
/// and free of any live unit's current cell or claimed `MoveOrder` goal →
/// affordability → reachable reserved immediate-perimeter builder slot,
/// evaluated on the post-placement map (footprint cells already blocked).
/// Mutates nothing; apply the returned plan only after every check passes.
pub fn validate_placement(
    world: &World,
    map: &GridMap,
    issuer: TeamId,
    builder: UnitId,
    kind: BuildingKind,
    anchor: GridPos,
) -> Result<PlacementPlan, RejectReason> {
    // 1. owned villager.
    let entity = owned_unit_entity(world, builder, issuer)?;
    if !world
        .get::<Unit>(entity)
        .is_some_and(|unit| unit.kind == UnitKind::Villager)
    {
        return Err(RejectReason::NotVillager);
    }

    // 2. kind unlocked/buildable. Town Centers are seeded, never placed.
    let spec = building_spec(kind);
    let team_age = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .map(|state| state.age);
    if kind == BuildingKind::TownCenter || team_age.is_some_and(|age| age < spec.required_age) {
        return Err(RejectReason::Locked);
    }

    let footprint = Footprint::new(anchor, spec.width, spec.height);

    // 3. footprint in bounds.
    if !footprint.cells().iter().all(|cell| map.in_bounds(*cell)) {
        return Err(RejectReason::OutOfBounds);
    }

    // 4. footprint cells walkable and free of any live unit's current cell or
    // claimed MoveOrder goal: placement would block the cells and permanently
    // entomb a unit standing inside the footprint (find_path needs a walkable
    // start), or strand a unit whose goal lies inside on a preserved order
    // that can never replan onto blocked cells. The builder's own goal is
    // exempt because acceptance cancels that order.
    let mut unit_cells: HashSet<GridPos> = HashSet::new();
    if let Some(index) = world.get_resource::<UnitIndex>() {
        for (_, unit_entity) in index.iter() {
            if let Some(position) = world.get::<SimPosition>(*unit_entity) {
                unit_cells.insert(map.world_to_cell(position.current));
            }
            if *unit_entity != entity
                && let Some(order) = world.get::<MoveOrder>(*unit_entity)
            {
                unit_cells.insert(order.goal);
            }
        }
    }
    if !footprint
        .cells()
        .iter()
        .all(|cell| map.is_walkable(*cell) && !unit_cells.contains(cell))
    {
        return Err(RejectReason::Occupied);
    }

    // 5. affordable.
    let affordable = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .is_some_and(|state| {
            state.stockpile.food >= spec.cost.food
                && state.stockpile.wood >= spec.cost.wood
                && state.stockpile.gold >= spec.cost.gold
        });
    if !affordable {
        return Err(RejectReason::InsufficientResources);
    }

    // 6. reachable immediate-perimeter builder slot, checked against the
    // post-placement map: acceptance blocks the footprint, so a route that
    // only exists through those cells would charge for a site the builder
    // then cannot reach — the first movement replan fails and it idles.
    let mut occupied_map = map.clone();
    for cell in footprint.cells() {
        occupied_map.set_blocked(cell, true);
    }
    let (slot, route) = reachable_builder_slot(world, &occupied_map, entity, footprint, None)?;
    Ok(PlacementPlan {
        builder: entity,
        footprint,
        slot,
        route,
    })
}

/// Applies an accepted `PlaceBuilding`: validates first, then cancels the
/// builder's old activity, deducts the cost once, allocates the `BuildingId`,
/// blocks the footprint, spawns `Building + Footprint`, and routes the builder
/// to its stored approach slot.
pub(crate) fn apply_place_building(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    builder: UnitId,
    kind: BuildingKind,
    anchor: GridPos,
) -> CommandResult {
    let mut result = CommandResult::default();
    let plan = match validate_placement(world, map, issuer, builder, kind, anchor) {
        Ok(plan) => plan,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };

    cancel_worker_activity(world, plan.builder);

    let spec = building_spec(kind);
    {
        let mut economy = world
            .get_resource_mut::<TeamEconomy>()
            .expect("validated economy");
        let state = economy.0.get_mut(&issuer).expect("validated team");
        state.stockpile.food -= spec.cost.food;
        state.stockpile.wood -= spec.cost.wood;
        state.stockpile.gold -= spec.cost.gold;
    }

    let id = world.resource_mut::<IdAllocator>().allocate_building();

    for cell in plan.footprint.cells() {
        map.set_blocked(cell, true);
    }

    let building_entity = world
        .spawn((
            Building {
                id,
                team: issuer,
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: Some(builder),
                },
            },
            plan.footprint,
        ))
        .id();
    let mut index = world.get_resource_or_insert_with(BuildingIndex::default);
    index.insert(id, building_entity);

    world
        .entity_mut(plan.builder)
        .insert(WorkerTask::ToConstruction {
            building: id,
            slot: plan.slot,
        });
    if !plan.route.is_empty() {
        world.entity_mut(plan.builder).insert(MoveOrder {
            waypoints: plan.route,
            next: 0,
            goal: plan.slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }

    result
}

/// Applies an accepted `ResumeConstruction` on an incomplete owned building:
/// validates first, then retasks the builder to an immediate-perimeter slot of
/// the site. Accumulated progress is untouched.
pub(crate) fn apply_resume_construction(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    builder: UnitId,
    building: BuildingId,
) -> CommandResult {
    let mut result = CommandResult::default();
    let (builder_entity, building_entity) = match validate_resume(world, issuer, builder, building)
    {
        Ok(entities) => entities,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };
    let footprint = world
        .get::<Footprint>(building_entity)
        .copied()
        .expect("building footprint");
    // Look up the site's current builder before route validation: an accepted
    // takeover cancels its order below, so its soon-released goal must not
    // reserve a slot the replacement needs, while its current cell is still
    // occupied and stays reserved. Validation remains side-effect free — a
    // rejection here never reached the cancellation.
    let previous_entity = world
        .get::<Building>(building_entity)
        .and_then(|state| state.construction.active_builder)
        .filter(|previous| *previous != builder)
        .and_then(|previous| {
            world
                .get_resource::<UnitIndex>()
                .and_then(|index| index.entity(previous))
        });
    let (slot, route) =
        match reachable_builder_slot(world, map, builder_entity, footprint, previous_entity) {
            Ok(slot) => slot,
            Err(reason) => {
                result.reject = Some(reason);
                return result;
            }
        };

    // The site's previous builder is paused before the new one takes over:
    // one active builder per building, never two.
    if let Some(previous_entity) = previous_entity {
        cancel_worker_activity(world, previous_entity);
    }

    cancel_worker_activity(world, builder_entity);
    world
        .entity_mut(builder_entity)
        .insert(WorkerTask::ToConstruction { building, slot });
    if !route.is_empty() {
        world.entity_mut(builder_entity).insert(MoveOrder {
            waypoints: route,
            next: 0,
            goal: slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }
    if let Some(mut state) = world.get_mut::<Building>(building_entity) {
        state.construction.active_builder = Some(builder);
    }

    result
}

/// Resume validation: owned villager → building exists → owned → incomplete.
fn validate_resume(
    world: &World,
    issuer: TeamId,
    builder: UnitId,
    building: BuildingId,
) -> Result<(Entity, Entity), RejectReason> {
    let entity = owned_unit_entity(world, builder, issuer)?;
    if !world
        .get::<Unit>(entity)
        .is_some_and(|unit| unit.kind == UnitKind::Villager)
    {
        return Err(RejectReason::NotVillager);
    }

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
    if state.construction.complete {
        return Err(RejectReason::Locked);
    }
    Ok((entity, building_entity))
}

/// Picks the builder's approach slot: the first reachable walkable cell on the
/// footprint's immediate perimeter that no live unit stands on or moves to.
/// `goal_released` names a unit whose `MoveOrder` an accepted command cancels
/// (a construction site's previous builder on takeover): its goal is not
/// seeded because acceptance frees it, while its current cell still reserves.
/// Returns the slot and its route waypoints.
fn reachable_builder_slot(
    world: &World,
    map: &GridMap,
    builder: Entity,
    footprint: Footprint,
    goal_released: Option<Entity>,
) -> Result<(GridPos, Vec<Vec2>), RejectReason> {
    let position = world
        .get::<SimPosition>(builder)
        .ok_or(RejectReason::UnknownUnit)?;
    let start = map.world_to_cell(position.current);

    // Same reservation seam as Move: seed every live unit's current cell and
    // existing MoveOrder goal, excluding the builder's own current cell and
    // old goal exactly as Move releases a commanded unit's reservations before
    // reassignment. `goal_released` frees one more goal ahead of acceptance:
    // the order carrying it is cancelled once the command applies, so only
    // that unit's current cell still reserves. When the new order replaces
    // the old one, `cancel_worker_activity` drops the old route (releasing
    // its goal) and the new order claims the slot; on rejection nothing here
    // mutated.
    let mut used = HashSet::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        if entity == builder {
            continue;
        }
        if let Some(position) = world.get::<SimPosition>(entity) {
            used.insert(map.world_to_cell(position.current));
        }
        if Some(entity) != goal_released
            && let Some(order) = world.get::<MoveOrder>(entity)
        {
            used.insert(order.goal);
        }
    }
    for slot in approach_slots(map, footprint, &used, footprint.perimeter_cells().len()) {
        if let Some(path) = map.find_path(start, slot) {
            let route = path
                .into_iter()
                .skip(1)
                .map(|cell| map.cell_center(cell))
                .collect();
            return Ok((slot, route));
        }
    }
    Err(RejectReason::Unreachable)
}

/// Advances construction one fixed tick. A worker transitions
/// `ToConstruction → Constructing` once its current cell equals the stored
/// slot; each building then advances only while its single `active_builder` is
/// the worker in `Constructing` state, so a second builder can never double
/// the rate. Completion clears the assignment, idles the builder, grants a
/// Storehouse its `Dropoff` marker, and grants a Farm its renewable
/// `ResourceSource` under a fresh runtime `ResourceId` — each exactly once.
pub fn step_construction(world: &mut World, seconds: f32) {
    // Arrival transitions happen before advancement so movement completion is
    // visible to construction in the same fixed tick.
    let mut arrivals = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask, &SimPosition)>();
        for (entity, task, position) in query.iter(world) {
            if let WorkerTask::ToConstruction { building, slot } = task
                && worker_at_slot(position, *slot)
            {
                arrivals.push((entity, *building));
            }
        }
    }
    for (entity, building) in arrivals {
        world
            .entity_mut(entity)
            .insert(WorkerTask::Constructing { building });
    }

    let buildings: Vec<(Entity, BuildingId, BuildingKind, TeamId, ConstructionState)> = {
        let mut query = world.query::<(Entity, &Building)>();
        query
            .iter(world)
            .map(|(entity, building)| {
                (
                    entity,
                    building.id,
                    building.kind,
                    building.team,
                    building.construction,
                )
            })
            .collect()
    };

    let mut completions = Vec::new();
    for (building_entity, id, kind, team, state) in buildings {
        if state.complete {
            continue;
        }
        let Some(builder) = state.active_builder else {
            continue;
        };
        let Some(builder_entity) = world
            .get_resource::<UnitIndex>()
            .and_then(|index| index.entity(builder))
        else {
            continue;
        };
        let constructing_this = matches!(
            world.get::<WorkerTask>(builder_entity),
            Some(WorkerTask::Constructing { building }) if *building == id
        );
        if !constructing_this {
            continue;
        }

        let progress = state.progress_seconds + seconds;
        if progress >= building_spec(kind).build_seconds as f32 {
            completions.push((building_entity, builder_entity, kind, team));
        } else if let Some(mut building) = world.get_mut::<Building>(building_entity) {
            building.construction.progress_seconds = progress;
        }
    }

    for (building_entity, builder_entity, kind, team) in completions {
        if let Some(mut building) = world.get_mut::<Building>(building_entity) {
            building.construction.complete = true;
            building.construction.active_builder = None;
        }
        world.entity_mut(builder_entity).insert(WorkerTask::Idle);
        if kind == BuildingKind::Storehouse {
            world.entity_mut(building_entity).insert(Dropoff { team });
        }
        if is_producer(kind) {
            // Completed producers own their FIFO production queue from birth.
            world
                .entity_mut(building_entity)
                .insert(ProductionQueue::default());
        }
        if kind == BuildingKind::Farm {
            // The completed Farm gains a renewable Food source on the same
            // building entity under a fresh runtime `ResourceId`.
            let id = world.resource_mut::<IdAllocator>().allocate_resource();
            world.entity_mut(building_entity).insert(ResourceSource {
                id,
                kind: ResourceKind::Food,
                remaining: None,
                assigned_worker: None,
            });
            world
                .get_resource_or_insert_with(ResourceIndex::default)
                .insert(id, building_entity);
        }
    }
}

/// A worker has reached its stored slot when its current cell equals the slot.
/// Cell equality (not exact-center distance) absorbs post-arrival separation
/// nudges of up to `MAX_SEPARATION_STEP`.
pub(crate) fn worker_at_slot(position: &SimPosition, slot: GridPos) -> bool {
    position.current.x.floor() as i32 == slot.x && position.current.y.floor() as i32 == slot.y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Age, ResourceKind, unit_spec};
    use crate::commands::{
        PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
    };
    use crate::economy::{
        Carry, GatherProgress, ResourceIndex, ResourceSource, ResourceStockpile, TeamEconomy,
    };
    use crate::ids::{IdAllocator, ResourceId};
    use crate::movement::{SIM_STEP_SECONDS, step_movement};

    fn setup_build_test() -> (World, GridMap, Entity) {
        let mut world = World::new();
        let mut economy = TeamEconomy::default();
        economy.insert_team(
            TeamId(1),
            ResourceStockpile {
                food: 500,
                wood: 500,
                gold: 500,
            },
            Age::Age1,
        );
        world.insert_resource(economy);
        world.insert_resource(IdAllocator::new(10, 10, 10));

        let map = GridMap::new(64, 64);
        let villager = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(10.5, 10.5),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(villager).insert((
            Carry::Empty,
            GatherProgress::default(),
            WorkerTask::Idle,
        ));
        (world, map, villager)
    }

    /// Places a House next to the test villager and drives the production API
    /// until the villager is actively constructing it.
    fn setup_active_builder() -> (World, GridMap, Entity, Entity) {
        let (mut world, mut map, villager) = setup_build_test();

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert!(result.reject.is_none(), "placement rejected: {result:?}");
        let building = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("allocated building registered");

        for _ in 0..300 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_construction(&mut world, SIM_STEP_SECONDS);
            if matches!(
                world.get::<WorkerTask>(villager),
                Some(WorkerTask::Constructing { .. })
            ) {
                break;
            }
        }
        assert!(
            matches!(
                world.get::<WorkerTask>(villager),
                Some(WorkerTask::Constructing {
                    building: BuildingId(10)
                })
            ),
            "builder never reached Constructing"
        );
        (world, map, villager, building)
    }

    /// Drives movement + construction until the given worker is Constructing.
    fn run_until_constructing(world: &mut World, map: &GridMap, worker: Entity) {
        for _ in 0..300 {
            step_movement(world, map, SIM_STEP_SECONDS);
            step_construction(world, SIM_STEP_SECONDS);
            if matches!(
                world.get::<WorkerTask>(worker),
                Some(WorkerTask::Constructing { .. })
            ) {
                return;
            }
        }
        panic!("worker never reached Constructing");
    }

    #[test]
    fn rejected_move_preserves_active_builder() {
        let (mut world, mut map, villager, building) = setup_active_builder();
        let old_task = world.get::<WorkerTask>(villager).unwrap().clone();
        let blocked = GridPos::new(20, 20);
        map.set_blocked(blocked, true);
        let target = map.cell_center(blocked);

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(1)],
                kind: UnitCommandKind::Move { target },
            }),
        );

        assert_eq!(
            result.rejected_units,
            vec![(UnitId(1), RejectReason::Unreachable)]
        );
        assert_eq!(world.get::<WorkerTask>(villager), Some(&old_task));
        assert_eq!(
            world
                .get::<Building>(building)
                .unwrap()
                .construction
                .active_builder,
            Some(UnitId(1))
        );
    }

    #[test]
    fn invalid_placement_never_charges_or_reserves() {
        let (mut world, mut map, _villager) = setup_build_test();
        // Not a villager. Parked away from every footprint used below.
        spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Spearman,
            unit_spec(UnitKind::Spearman).speed,
        );

        // Not a villager.
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(2),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::NotVillager));

        // Town Centers are seeded, never placed.
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::TownCenter,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::Locked));

        // Footprint out of bounds (2×2 house at the map edge).
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(63, 63),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::OutOfBounds));

        // Footprint cell blocked.
        map.set_blocked(GridPos::new(13, 10), true);
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::Occupied));

        // Unaffordable.
        world
            .resource_mut::<TeamEconomy>()
            .0
            .get_mut(&TeamId(1))
            .unwrap()
            .stockpile
            .wood = 10;
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(20, 20),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::InsufficientResources));

        // Validation order: a blocked AND unaffordable site rejects as
        // Occupied because walkability precedes affordability.
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::Occupied));

        // Nothing was charged, spawned, or allocated by any rejection.
        let state = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
        assert_eq!(
            state,
            ResourceStockpile {
                food: 500,
                wood: 10,
                gold: 500
            }
        );
        let mut buildings = world.query::<&Building>();
        assert_eq!(buildings.iter(&world).count(), 0);
        assert_eq!(
            world
                .get_resource::<BuildingIndex>()
                .map(|index| index.iter().count())
                .unwrap_or(0),
            0
        );
        assert_eq!(world.resource::<IdAllocator>().next_building, 10);
    }

    #[test]
    fn accepted_house_deducts_fifty_wood_and_blocks_four_cells() {
        let (mut world, mut map, villager) = setup_build_test();
        let anchor = GridPos::new(13, 10);

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor,
            },
        );

        assert_eq!(result.reject, None);

        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        assert_eq!(state.stockpile.wood, 450);
        assert_eq!(state.stockpile.food, 500);
        assert_eq!(state.stockpile.gold, 500);

        let footprint = Footprint::new(anchor, 2, 2);
        for cell in footprint.cells() {
            assert!(!map.is_walkable(cell), "footprint cell {cell:?} unblocked");
        }

        let building_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("building registered");
        let building = world.get::<Building>(building_entity).unwrap();
        assert_eq!(building.id, BuildingId(10));
        assert_eq!(building.team, TeamId(1));
        assert_eq!(building.kind, BuildingKind::House);
        assert_eq!(building.construction.active_builder, Some(UnitId(1)));
        assert!(!building.construction.complete);
        assert_eq!(world.get::<Footprint>(building_entity), Some(&footprint));

        // The builder is tasked to an immediate-perimeter slot with a route.
        let task = world.get::<WorkerTask>(villager).unwrap();
        let WorkerTask::ToConstruction { building: id, slot } = task else {
            panic!("expected ToConstruction, got {task:?}");
        };
        assert_eq!(*id, BuildingId(10));
        assert!(footprint.is_immediately_adjacent(*slot));
        let order = world.get::<MoveOrder>(villager).expect("route installed");
        assert_eq!(order.goal, *slot);
        assert_eq!(
            order.waypoints.last().copied(),
            Some(map.cell_center(*slot))
        );
    }

    #[test]
    fn two_builders_never_double_speed() {
        let (mut world, mut map, first, building) = setup_active_builder();
        let building_id = world.get::<Building>(building).unwrap().id;

        let second = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(second).insert((
            Carry::Empty,
            GatherProgress::default(),
            WorkerTask::Idle,
        ));

        // Resuming with the second builder cancels the first's assignment.
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::ResumeConstruction {
                issuer: TeamId(1),
                builder: UnitId(2),
                building: building_id,
            },
        );
        assert_eq!(result.reject, None);
        assert_eq!(world.get::<WorkerTask>(first), Some(&WorkerTask::Idle));
        assert_eq!(
            world
                .get::<Building>(building)
                .unwrap()
                .construction
                .active_builder,
            Some(UnitId(2))
        );

        run_until_constructing(&mut world, &map, second);

        let start = world
            .get::<Building>(building)
            .unwrap()
            .construction
            .progress_seconds;
        for _ in 0..40 {
            step_construction(&mut world, SIM_STEP_SECONDS);
        }
        let end = world
            .get::<Building>(building)
            .unwrap()
            .construction
            .progress_seconds;
        let expected = 40.0 * SIM_STEP_SECONDS;
        assert!(
            (end - start - expected).abs() < 1e-3,
            "progress advanced {start} → {end}, expected a single builder's {expected}"
        );
        assert_eq!(world.get::<WorkerTask>(first), Some(&WorkerTask::Idle));
    }

    /// A takeover frees the previous builder's en-route goal before the
    /// replacement's slot search runs: on a site with one free perimeter
    /// slot, the dead reservation must not force a spurious Unreachable.
    /// Regression: slot validation used to run before `active_builder` was
    /// looked up, so the about-to-be-cancelled order still reserved its goal.
    #[test]
    fn resume_releases_the_previous_builders_goal_for_the_replacement() {
        let (mut world, mut map, villager) = setup_build_test();
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, None);
        let building = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("building registered");
        let building_id = world.get::<Building>(building).unwrap().id;
        let footprint = Footprint::new(GridPos::new(13, 10), 2, 2);

        // Builder 1 is still en route; its order claims the site's only free
        // approach slot once every other perimeter cell is blocked.
        let claimed = world
            .get::<MoveOrder>(villager)
            .expect("route installed")
            .goal;
        assert!(footprint.is_immediately_adjacent(claimed));
        for cell in footprint.perimeter_cells() {
            if cell != claimed {
                map.set_blocked(cell, true);
            }
        }

        let second = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(9.5, 9.5),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(second).insert((
            Carry::Empty,
            GatherProgress::default(),
            WorkerTask::Idle,
        ));

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::ResumeConstruction {
                issuer: TeamId(1),
                builder: UnitId(2),
                building: building_id,
            },
        );

        assert_eq!(result.reject, None);
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
        assert!(world.get::<MoveOrder>(villager).is_none());
        let task = world.get::<WorkerTask>(second).unwrap().clone();
        let WorkerTask::ToConstruction { building, slot } = task else {
            panic!("expected ToConstruction, got {task:?}");
        };
        assert_eq!(building, building_id);
        assert_eq!(slot, claimed);
        assert_eq!(
            world.get::<MoveOrder>(second).map(|order| order.goal),
            Some(claimed)
        );
    }

    #[test]
    fn resume_keeps_accumulated_progress() {
        let (mut world, mut map, villager, building) = setup_active_builder();
        let building_id = world.get::<Building>(building).unwrap().id;
        // setup_active_builder's arrival tick already advanced one step.
        let baseline = world
            .get::<Building>(building)
            .unwrap()
            .construction
            .progress_seconds;

        for _ in 0..60 {
            step_construction(&mut world, SIM_STEP_SECONDS);
        }
        let accumulated = world
            .get::<Building>(building)
            .unwrap()
            .construction
            .progress_seconds;
        assert!((accumulated - baseline - 60.0 * SIM_STEP_SECONDS).abs() < 1e-3);

        // An accepted move pauses the work immediately.
        let target = map.cell_center(GridPos::new(2, 2));
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(1)],
                kind: UnitCommandKind::Move { target },
            }),
        );
        assert_eq!(result.accepted_units, vec![UnitId(1)]);
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
        assert_eq!(
            world
                .get::<Building>(building)
                .unwrap()
                .construction
                .active_builder,
            None
        );

        // Walk the builder away before resuming so the resume has to route
        // back to the site.
        for _ in 0..10 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
        }

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::ResumeConstruction {
                issuer: TeamId(1),
                builder: UnitId(1),
                building: building_id,
            },
        );
        assert_eq!(result.reject, None);

        run_until_constructing(&mut world, &map, villager);
        // The arrival tick already advanced one step; 59 more completes the
        // same 60-tick batch as before the pause.
        for _ in 0..59 {
            step_construction(&mut world, SIM_STEP_SECONDS);
        }

        let resumed = world
            .get::<Building>(building)
            .unwrap()
            .construction
            .progress_seconds;
        assert!(
            (resumed - baseline - 120.0 * SIM_STEP_SECONDS).abs() < 1e-3,
            "progress was {resumed}, expected the baseline plus 120 ticks' worth"
        );
        assert!(
            !world
                .get::<Building>(building)
                .unwrap()
                .construction
                .complete
        );
    }

    #[test]
    fn completed_storehouse_gains_dropoff_exactly_once() {
        let (mut world, mut map, villager) = setup_build_test();

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::Storehouse,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, None);
        assert_eq!(
            world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood,
            425
        );

        let building_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("building registered");
        for _ in 0..1000 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_construction(&mut world, SIM_STEP_SECONDS);
            if world
                .get::<Building>(building_entity)
                .unwrap()
                .construction
                .complete
            {
                break;
            }
        }
        let state = world.get::<Building>(building_entity).unwrap();
        assert!(state.construction.complete);
        assert_eq!(state.construction.active_builder, None);
        assert_eq!(
            world.get::<Dropoff>(building_entity),
            Some(&Dropoff { team: TeamId(1) })
        );
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));

        // Further ticks never grant a second marker or restart work.
        for _ in 0..10 {
            step_construction(&mut world, SIM_STEP_SECONDS);
        }
        assert_eq!(
            world.get::<Dropoff>(building_entity),
            Some(&Dropoff { team: TeamId(1) })
        );
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
    }

    #[test]
    fn completed_barracks_gains_an_empty_production_queue() {
        let (mut world, mut map, _villager) = setup_build_test();

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::Barracks,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, None);
        let building_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("building registered");
        assert!(world.get::<ProductionQueue>(building_entity).is_none());

        for _ in 0..1000 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_construction(&mut world, SIM_STEP_SECONDS);
            if world
                .get::<Building>(building_entity)
                .unwrap()
                .construction
                .complete
            {
                break;
            }
        }
        assert!(
            world
                .get::<Building>(building_entity)
                .unwrap()
                .construction
                .complete
        );
        let queue = world
            .get::<ProductionQueue>(building_entity)
            .expect("completed producer owns a queue");
        assert!(queue.jobs.is_empty());
        assert_eq!(queue.progress_seconds, 0.0);
        assert_eq!(queue.blocked, None);
    }

    #[test]
    fn completed_farm_gains_a_renewable_food_source_on_the_same_entity() {
        let (mut world, mut map, villager) = setup_build_test();

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::Farm,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, None);
        assert_eq!(
            world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood,
            440
        );

        let farm_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(10))
            .expect("building registered");
        for _ in 0..1000 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
            step_construction(&mut world, SIM_STEP_SECONDS);
            if world
                .get::<Building>(farm_entity)
                .unwrap()
                .construction
                .complete
            {
                break;
            }
        }
        assert!(
            world
                .get::<Building>(farm_entity)
                .unwrap()
                .construction
                .complete
        );
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));

        // The completed Farm building entity carries its renewable source.
        let source = world
            .get::<ResourceSource>(farm_entity)
            .expect("farm source");
        assert_eq!(source.id, ResourceId(10));
        assert_eq!(source.kind, ResourceKind::Food);
        assert_eq!(source.remaining, None);
        assert_eq!(source.assigned_worker, None);
        assert_eq!(
            world.resource::<ResourceIndex>().entity(source.id),
            Some(farm_entity)
        );
        assert_eq!(world.resource::<IdAllocator>().next_resource, 11);

        // Still exactly one entity, carrying both the building and the source.
        let mut buildings = world.query::<&Building>();
        assert_eq!(buildings.iter(&world).count(), 1);
        let mut sources = world.query::<&ResourceSource>();
        assert_eq!(sources.iter(&world).count(), 1);
    }

    #[test]
    fn placement_rejects_a_footprint_under_a_standing_unit() {
        let (mut world, mut map, _villager) = setup_build_test();
        let anchor = GridPos::new(13, 10);
        // A live unit stands inside the would-be footprint.
        spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            map.cell_center(GridPos::new(13, 10)),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor,
            },
        );

        assert_eq!(result.reject, Some(RejectReason::Occupied));

        // No cost was charged and no footprint cell was blocked.
        assert_eq!(
            world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood,
            500
        );
        for cell in Footprint::new(anchor, 2, 2).cells() {
            assert!(map.is_walkable(cell), "footprint cell {cell:?} blocked");
        }
        assert_eq!(
            world
                .get_resource::<BuildingIndex>()
                .map(|index| index.iter().count())
                .unwrap_or(0),
            0
        );
    }

    /// A sibling already moving to a cell inside the footprint has that goal
    /// reserved: accepting the placement would block the goal and strand the
    /// unit on a preserved order that can never replan onto blocked cells.
    #[test]
    fn placement_rejects_a_footprint_over_a_reserved_goal() {
        let (mut world, mut map, _villager) = setup_build_test();
        let anchor = GridPos::new(13, 10);
        let sibling = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            map.cell_center(GridPos::new(8, 8)),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(sibling).insert(MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(13, 10))],
            next: 0,
            goal: GridPos::new(13, 10),
            map_revision: map.revision(),
            last_failed_replan: None,
        });

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor,
            },
        );

        assert_eq!(result.reject, Some(RejectReason::Occupied));
        assert_eq!(
            world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood,
            500
        );
        for cell in Footprint::new(anchor, 2, 2).cells() {
            assert!(map.is_walkable(cell), "footprint cell {cell:?} blocked");
        }
        assert_eq!(
            world.get::<MoveOrder>(sibling).map(|order| order.goal),
            Some(GridPos::new(13, 10)),
            "rejection must leave the sibling's order untouched"
        );
    }

    /// The builder's own old goal is exempt: acceptance cancels that order, so
    /// a footprint over it still validates and the builder is retasked to the
    /// assigned perimeter slot.
    #[test]
    fn placement_ignores_the_builders_own_old_goal() {
        let (mut world, mut map, villager) = setup_build_test();
        let anchor = GridPos::new(13, 10);
        world.entity_mut(villager).insert(MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(13, 10))],
            next: 0,
            goal: GridPos::new(13, 10),
            map_revision: map.revision(),
            last_failed_replan: None,
        });

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor,
            },
        );

        assert_eq!(result.reject, None);
        let order = world.get::<MoveOrder>(villager).expect("route installed");
        assert!(
            Footprint::new(anchor, 2, 2).is_immediately_adjacent(order.goal),
            "builder goal {goal:?} is the assigned slot, not the cancelled one",
            goal = order.goal
        );
    }

    #[test]
    fn placement_slot_is_not_assigned_onto_a_live_unit() {
        let (mut world, mut map, villager) = setup_build_test();
        // An idle unit squats on the footprint's first perimeter candidate,
        // which an empty reservation set would hand to the builder.
        let squatter_cell = GridPos::new(12, 9);
        let squatter = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            map.cell_center(squatter_cell),
            UnitKind::Villager,
            unit_spec(UnitKind::Villager).speed,
        );
        world.entity_mut(squatter).insert(WorkerTask::Idle);

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(13, 10),
            },
        );
        assert_eq!(result.reject, None);

        let task = world.get::<WorkerTask>(villager).unwrap().clone();
        let WorkerTask::ToConstruction {
            building: BuildingId(10),
            slot,
        } = task
        else {
            panic!("expected ToConstruction, got {task:?}");
        };
        assert_ne!(
            slot, squatter_cell,
            "builder was sent onto a live unit's cell"
        );

        // With an unoccupied slot the builder actually reaches the site and
        // starts building instead of being separated off its slot forever.
        run_until_constructing(&mut world, &map, villager);
    }

    /// A footprint that plugs the only corridor between the builder and the
    /// remaining free approach slots must reject instead of charging for a
    /// site the builder can never reach. Regression: reachability used to be
    /// checked on the pre-placement map, so a route through the soon-blocked
    /// footprint was accepted; the first movement replan then failed and the
    /// builder idled next to a paid, unbuildable site.
    #[test]
    fn placement_rejects_when_the_footprint_plugs_the_only_corridor() {
        let (mut world, mut map, villager) = setup_build_test();
        world
            .entity_mut(villager)
            .insert(SimPosition::new(Vec2::new(8.5, 6.5)));

        // Solid wall at x = 10 with a two-cell gap at (10, 5)–(10, 6). The
        // 2×2 House footprint covers the whole gap plus one column past it,
        // so every free approach slot lies on the far side.
        for y in 0..64 {
            if y != 5 && y != 6 {
                map.set_blocked(GridPos::new(10, y), true);
            }
        }

        // Squatters occupy the builder-side perimeter cells so slot selection
        // is forced onto the far side of the footprint.
        for (id, cell) in [
            (2, GridPos::new(9, 4)),
            (3, GridPos::new(9, 5)),
            (4, GridPos::new(9, 6)),
            (5, GridPos::new(9, 7)),
        ] {
            spawn_unit(
                &mut world,
                UnitId(id),
                TeamId(1),
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
        }

        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(1),
                builder: UnitId(1),
                kind: BuildingKind::House,
                anchor: GridPos::new(10, 5),
            },
        );

        assert_eq!(result.reject, Some(RejectReason::Unreachable));

        // Nothing was charged, spawned, blocked, or routed.
        assert_eq!(
            world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood,
            500
        );
        let mut buildings = world.query::<&Building>();
        assert_eq!(buildings.iter(&world).count(), 0);
        assert_eq!(world.resource::<IdAllocator>().next_building, 10);
        for cell in Footprint::new(GridPos::new(10, 5), 2, 2).cells() {
            assert!(map.is_walkable(cell), "footprint cell {cell:?} blocked");
        }
        assert!(world.get::<MoveOrder>(villager).is_none());
        assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
    }
}
