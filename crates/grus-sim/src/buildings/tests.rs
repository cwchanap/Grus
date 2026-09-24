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
use crate::visibility::{VisibilityMap, explored_by, refresh_visibility};

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
    world
        .entity_mut(villager)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    (world, map, villager)
}

/// Opts the world into runtime fog and stamps the initial reveal.
fn reveal(world: &mut World, map: &GridMap) {
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(world, map);
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
    world
        .entity_mut(second)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));

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
    world
        .entity_mut(second)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));

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

/// An unexplored footprint that overlaps a hidden enemy unit and its move
/// goal must reject `Unexplored` before the occupancy scan runs — never
/// `Occupied`, which would leak the hidden unit/goal through the preview.
#[test]
fn unexplored_footprint_rejects_unexplored_never_occupied() {
    let (mut world, mut map, _villager) = setup_build_test();
    let scout = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(24.5, 10.5),
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
    );
    world.entity_mut(scout).insert(MoveOrder {
        waypoints: vec![],
        next: 0,
        goal: GridPos::new(25, 11),
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    reveal(&mut world, &map);
    assert!(
        !explored_by(&world, TeamId(1), GridPos::new(24, 10)),
        "the planned footprint is unexplored ground"
    );

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::House,
            anchor: GridPos::new(24, 10),
        },
    );

    assert_eq!(
        result.reject,
        Some(RejectReason::Unexplored),
        "unexplored ground rejects before the occupancy scan can leak the hidden unit or goal"
    );
}

/// Explored-but-not-visible ground must not leak a hidden enemy occupant
/// through `Occupied` either: the preview/validation scan skips occupants
/// the issuer cannot see, and acceptance displaces the unit clear of the
/// footprint instead of entombing it under blocked cells.
#[test]
fn hidden_enemy_inside_footprint_does_not_block_and_is_displaced() {
    let (mut world, mut map, villager) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), anchor));

    // Vision withdraws: the footprint stays explored but leaves current
    // visibility, so an enemy standing on it is genuinely hidden.
    world
        .entity_mut(villager)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(
        explored_by(&world, TeamId(1), anchor) && !visible_to(&world, TeamId(1), anchor),
        "the footprint is explored fog, not current vision"
    );

    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(14, 11)),
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
    );

    assert!(
        validate_placement(
            &world,
            &map,
            TeamId(1),
            UnitId(1),
            BuildingKind::House,
            anchor
        )
        .is_ok(),
        "a hidden occupant must not surface as `Occupied` through the preview seam"
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
    assert_eq!(result.reject, None);

    // The hidden unit survives — displaced to open ground, not entombed.
    let cell = map.world_to_cell(world.get::<SimPosition>(enemy).unwrap().current);
    assert!(
        !Footprint::new(anchor, 2, 2).cells().contains(&cell) && map.is_walkable(cell),
        "the displaced unit stands on open ground outside the footprint: {cell:?}"
    );
}

/// A hidden enemy move goal inside the footprint is likewise invisible to
/// the preview seam; acceptance retargets the order to open ground so the
/// preserved route can never strand on blocked cells.
#[test]
fn hidden_enemy_goal_inside_footprint_is_retargeted_on_acceptance() {
    let (mut world, mut map, villager) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    world
        .entity_mut(villager)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(!visible_to(&world, TeamId(1), anchor));

    // The hidden enemy walks toward a cell the footprint will cover.
    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(40, 40)),
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
    );
    world.entity_mut(enemy).insert(MoveOrder {
        waypoints: vec![],
        next: 0,
        goal: GridPos::new(14, 11),
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    assert!(
        validate_placement(
            &world,
            &map,
            TeamId(1),
            UnitId(1),
            BuildingKind::House,
            anchor
        )
        .is_ok()
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
    assert_eq!(result.reject, None);

    let order = world.get::<MoveOrder>(enemy).expect("order preserved");
    assert_ne!(
        order.goal,
        GridPos::new(14, 11),
        "the covered goal is retargeted off the footprint"
    );
    assert!(
        map.is_walkable(order.goal) && !Footprint::new(anchor, 2, 2).cells().contains(&order.goal),
        "the new goal {goal:?} is open ground outside the footprint",
        goal = order.goal
    );
}

/// Current vision restores the honest rejection: a *visible* enemy unit
/// inside the footprint still surfaces `Occupied` — the seam only hides
/// what the issuer genuinely cannot see.
#[test]
fn visible_enemy_inside_footprint_still_rejects_occupied() {
    let (mut world, mut map, _villager) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(14, 11)),
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
    );
    reveal(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), GridPos::new(14, 11)));

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

#[test]
fn abandoned_site_never_progresses_without_a_builder() {
    let (mut world, mut map, _villager) = setup_build_test();
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

    // Stop cancels the builder's task before he ever reaches the site.
    let stop = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::Stop,
        }),
    );
    assert_eq!(stop.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        world
            .get::<Building>(building)
            .unwrap()
            .construction
            .active_builder,
        None,
        "Stop must release the construction claim"
    );
    let progress_before = world
        .get::<Building>(building)
        .unwrap()
        .construction
        .progress_seconds;

    for _ in 0..400 {
        step_construction(&mut world, SIM_STEP_SECONDS);
    }

    let state = &world.get::<Building>(building).unwrap().construction;
    assert!(!state.complete, "an abandoned site must never complete");
    assert_eq!(
        state.progress_seconds, progress_before,
        "a site with no active builder must not advance"
    );
}

#[test]
fn placement_ignores_the_builders_own_move_goal() {
    let (mut world, mut map, villager) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    // The villager is en route to a cell the new House will cover; his own
    // goal is exempt from the occupancy check because acceptance cancels it.
    world.entity_mut(villager).insert(MoveOrder {
        waypoints: vec![map.cell_center(anchor)],
        next: 0,
        goal: anchor,
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

    assert!(
        result.reject.is_none(),
        "the builder's own goal must not reject his placement: {result:?}"
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(villager),
            Some(WorkerTask::ToConstruction { .. })
        ),
        "an accepted placement retasks the builder to the site"
    );
    let order = world.get::<MoveOrder>(villager).expect("route to the site");
    assert_ne!(
        order.goal, anchor,
        "the cancelled route is replaced by the approach route"
    );
}

#[test]
fn resume_construction_reject_codes_are_deterministic() {
    let (mut world, mut map, villager, building) = setup_active_builder();

    // A non-villager worker cannot resume.
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(8.5, 8.5),
        UnitKind::Spearman,
        6.0,
    );
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(2),
            building: BuildingId(10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::NotVillager));

    // A building owned by another team cannot be resumed.
    let enemy_site = world
        .spawn((
            Building {
                id: BuildingId(11),
                team: TeamId(2),
                kind: BuildingKind::House,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(20, 20), 2, 2),
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(11), enemy_site);
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(1),
            building: BuildingId(11),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::NotOwned));

    // An unknown building id rejects without touching the builder.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(1),
            building: BuildingId(999),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::BuildingMissing));
    assert!(
        matches!(
            world.get::<WorkerTask>(villager),
            Some(WorkerTask::Constructing { .. })
        ),
        "every rejection leaves the active builder untouched"
    );

    // Once the site completes, resuming it is locked.
    for _ in 0..400 {
        step_construction(&mut world, SIM_STEP_SECONDS);
    }
    assert!(
        world
            .get::<Building>(building)
            .unwrap()
            .construction
            .complete,
        "the actively built House should finish within 20 simulated seconds"
    );
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(1),
            building: BuildingId(10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Locked));
}

#[test]
fn placed_buildings_start_at_full_catalogue_health() {
    let (mut world, mut map, _) = setup_build_test();

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

    // Sites use full owning-building health from placement; construction
    // progress does not scale it.
    let health = world.get::<Health>(building).expect("building health");
    assert_eq!(
        health,
        &Health {
            current: 250,
            max: 250
        }
    );
}
