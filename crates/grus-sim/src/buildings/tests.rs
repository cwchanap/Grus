use super::*;
use crate::catalog::unit_spec;
use crate::combat::{destroy_building, step_combat};
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
};
use crate::economy::{
    Carry, GatherProgress, ResourceIndex, ResourceSource, ResourceStockpile, TeamEconomy,
};
use crate::ids::IdAllocator;
use crate::movement::{SIM_STEP_SECONDS, step_movement};
use crate::session::{MatchPhase, MatchSession};
use crate::visibility::{VisibilityMap, explored_by, refresh_visibility};

fn setup_build_test() -> (World, GridMap, Entity) {
    let mut world = World::new();
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 500,
            scrap: 500,
            coal: 500,
            crystals: 0,
        },
    );
    world.insert_resource(economy);
    world.insert_resource(IdAllocator::new(10, 10, 10));

    let map = GridMap::new(64, 64);
    let worker = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(10.5, 10.5),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world
        .entity_mut(worker)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    (world, map, worker)
}

/// Opts the world into runtime fog and stamps the initial reveal.
fn reveal(world: &mut World, map: &GridMap) {
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(world, map);
}

/// Placement preview (`validate_placement`, the same function the bridge's
/// `placement_preview` calls on mouse motion) is read-only: a fully valid
/// validation leaves every stockpile field untouched. Spending belongs to
/// `apply_place_building` alone.
#[test]
fn placement_validation_never_spends() {
    let (world, map, _) = setup_build_test();
    let before = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
    let plan = validate_placement(
        &world,
        &map,
        TeamId(1),
        UnitId(1),
        BuildingKind::Bunkhouse,
        GridPos::new(13, 10),
    );
    match plan {
        Ok(_) => {}
        Err(reason) => panic!("validation rejected: {reason:?}"),
    }
    let after = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
    assert_eq!(before, after, "read-only validation moved the stockpile");
}

#[test]
fn gate_placement_routes_humans_and_replans_aliens_until_destruction() {
    let (mut world, _initial_map, builder) = setup_build_test();
    let mut map = GridMap::new(12, 8);
    let gate = GridPos::new(6, 3);
    for y in 0..map.height() {
        if y != gate.y {
            map.set_blocked(GridPos::new(gate.x, y), true);
        }
    }
    let revision_before_gate = map.revision();
    world
        .entity_mut(builder)
        .insert(SimPosition::new(Vec2::new(8.5, 3.5)));

    let alien = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(1.5, 3.5),
        UnitKind::Swarmer,
        6.0,
    );
    let move_alien = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(2),
            units: vec![UnitId(2)],
            kind: UnitCommandKind::Move {
                target: Vec2::new(10.5, 3.5),
            },
        }),
    );
    assert_eq!(move_alien.accepted_units, vec![UnitId(2)]);
    assert!(
        world
            .get::<MoveOrder>(alien)
            .unwrap()
            .waypoints
            .contains(&map.cell_center(gate)),
        "the pre-placement alien route crossed the future Gate cell"
    );

    // Reserve every free approach slot on the east side without putting
    // bodies in the builder's route. Validation must use the Gate flag in
    // its post-placement clone to find a reachable west-side builder slot.
    let reserved_slots = [GridPos::new(7, 2), GridPos::new(7, 3), GridPos::new(7, 4)];
    let reservations = reserved_slots
        .into_iter()
        .enumerate()
        .map(|(i, slot)| {
            let id = UnitId(3 + i as u32);
            let entity = spawn_unit(
                &mut world,
                id,
                TeamId(1),
                Vec2::new(9.5 + i as f32, 1.5),
                UnitKind::Worker,
                6.0,
            );
            world.entity_mut(entity).insert(MoveOrder {
                waypoints: Vec::new(),
                next: 0,
                goal: slot,
                map_revision: map.revision(),
                last_failed_replan: None,
            });
            entity
        })
        .collect::<Vec<_>>();

    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Gate,
            anchor: gate,
        },
    );
    assert_eq!(placed.reject, None, "Gate placement rejected: {placed:?}");
    assert_eq!(map.revision(), revision_before_gate + 1);
    assert!(!map.is_walkable(gate), "a Gate still reserves free ground");
    assert!(map.is_traversable_for(TeamId(1), gate));
    assert!(!map.is_traversable_for(TeamId(2), gate));
    let stockpile_after_gate = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;

    let building = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(10))
        .expect("Gate registered");
    let WorkerTask::ToConstruction { slot, .. } = world.get::<WorkerTask>(builder).unwrap() else {
        panic!("builder was not assigned to the Gate site");
    };
    assert_eq!(slot.x, 5, "the east-side slots were all reserved");
    let builder_order = world.get::<MoveOrder>(builder).expect("builder route");
    assert!(
        builder_order.waypoints.contains(&map.cell_center(gate)),
        "the builder's post-placement route must cross the new Gate"
    );

    // A placement that overlaps the Gate rejects without spending, changing
    // its route or bumping occupancy a second time.
    let task_before_reject = world.get::<WorkerTask>(builder).unwrap().clone();
    let overlapping = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Wall,
            anchor: gate,
        },
    );
    assert_eq!(overlapping.reject, Some(RejectReason::Occupied));
    assert_eq!(world.get::<WorkerTask>(builder), Some(&task_before_reject));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile,
        stockpile_after_gate
    );
    assert_eq!(map.revision(), revision_before_gate + 1);
    for entity in reservations {
        world.entity_mut(entity).remove::<MoveOrder>();
    }

    let failed_revision = map.revision();
    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
        if world
            .get::<Building>(building)
            .unwrap()
            .construction
            .complete
        {
            break;
        }
    }
    assert!(
        world
            .get::<Building>(building)
            .unwrap()
            .construction
            .complete,
        "Gate builder did not finish"
    );
    let builder_position = world.get::<SimPosition>(builder).unwrap().current;
    assert!(
        map.world_to_cell(builder_position).x < gate.x,
        "the Worker did not walk through the newly placed Gate: {builder_position:?}"
    );
    let alien_position = world.get::<SimPosition>(alien).unwrap().current;
    assert_ne!(
        map.world_to_cell(alien_position),
        gate,
        "the alien followed its pre-placement route through the Gate"
    );
    assert_eq!(
        map.revision(),
        failed_revision,
        "Gate completion must not mutate occupancy a second time"
    );
    assert_eq!(
        world.get::<MoveOrder>(alien).unwrap().last_failed_replan,
        Some(failed_revision),
        "the alien's prior order remains pending at the sealed revision"
    );

    destroy_building(&mut world, &mut map, building);
    assert_eq!(map.revision(), failed_revision + 1);
    assert!(map.is_walkable(gate));
    assert!(map.is_traversable_for(TeamId(2), gate));
    let position_before_reopen = world.get::<SimPosition>(alien).unwrap().current;
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        world.get::<SimPosition>(alien).unwrap().current.x > position_before_reopen.x,
        "the alien did not recover its order after Gate destruction"
    );
    let recovered = world
        .get::<MoveOrder>(alien)
        .expect("alien order recovered");
    assert_eq!(recovered.map_revision, map.revision());
    assert_eq!(recovered.last_failed_replan, None);
}

/// An insufficient-resources placement rejects before ANY effect: no
/// deduction on any currency field, the builder keeps its current task
/// (the atomic spend happens before cancellation), and no building is
/// allocated.
#[test]
fn unaffordable_placement_preserves_builder_state_and_stockpile() {
    let (mut world, mut map, worker, _site) = setup_active_builder();
    let task = world.get::<WorkerTask>(worker).unwrap().clone();
    {
        let mut economy = world.resource_mut::<TeamEconomy>();
        economy.0.get_mut(&TeamId(1)).unwrap().stockpile = ResourceStockpile::default();
    }
    let building_count = world.resource::<BuildingIndex>().iter().count();

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Barracks,
            anchor: GridPos::new(20, 10),
        },
    );

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(
        world.get::<WorkerTask>(worker),
        Some(&task),
        "rejected placement must not cancel the builder's task"
    );
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile,
        ResourceStockpile::default(),
        "rejected placement spent resources"
    );
    assert_eq!(
        world.resource::<BuildingIndex>().iter().count(),
        building_count,
        "rejected placement allocated a building"
    );
}

/// Places a Bunkhouse next to the test worker and drives the production API
/// until the worker is actively constructing it.
fn setup_active_builder() -> (World, GridMap, Entity, Entity) {
    let (mut world, mut map, worker) = setup_build_test();

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
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
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Constructing { .. })
        ) {
            break;
        }
    }
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Constructing {
                building: BuildingId(10)
            })
        ),
        "builder never reached Constructing"
    );
    (world, map, worker, building)
}

fn place_test_wall(world: &mut World, map: &mut GridMap, anchor: GridPos) -> BuildingId {
    let result = apply_player_command(
        world,
        map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Wall,
            anchor,
        },
    );
    assert_eq!(result.reject, None, "Wall placement rejected: {result:?}");
    *world
        .resource::<BuildingIndex>()
        .iter()
        .map(|(id, _)| id)
        .max()
        .expect("placed Wall is indexed")
}

fn make_active_site_complete_next_tick(
    world: &mut World,
    map: &GridMap,
    builder: Entity,
    building: BuildingId,
) {
    let slot = match world.get::<WorkerTask>(builder).expect("builder task") {
        WorkerTask::ToConstruction {
            building: assigned,
            slot,
        } if *assigned == building => *slot,
        task => panic!("builder is not assigned to {building:?}: {task:?}"),
    };
    world.entity_mut(builder).remove::<MoveOrder>();
    world.entity_mut(builder).insert((
        SimPosition::new(map.cell_center(slot)),
        WorkerTask::Constructing { building },
    ));
    let entity = world
        .resource::<BuildingIndex>()
        .entity(building)
        .expect("active site is indexed");
    world
        .get_mut::<Building>(entity)
        .unwrap()
        .construction
        .progress_seconds =
        building_spec(BuildingKind::Wall).build_seconds as f32 - SIM_STEP_SECONDS;
}

#[test]
fn completed_wall_resumes_the_nearest_reachable_old_site_after_a_freeze() {
    let (mut world, mut map, builder) = setup_build_test();
    let far = place_test_wall(&mut world, &mut map, GridPos::new(22, 10));
    let near = place_test_wall(&mut world, &mut map, GridPos::new(17, 10));
    let active = place_test_wall(&mut world, &mut map, GridPos::new(12, 10));
    assert_eq!(
        world
            .get::<WorkerTask>(builder)
            .and_then(|task| match task {
                WorkerTask::ToConstruction { building, .. } => Some(*building),
                _ => None,
            }),
        Some(active),
        "the last-placed site keeps the builder"
    );
    make_active_site_complete_next_tick(&mut world, &map, builder, active);

    world.insert_resource(MatchSession {
        phase: MatchPhase::Paused,
    });
    let active_entity = world.resource::<BuildingIndex>().entity(active).unwrap();
    step_construction(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        !world
            .get::<Building>(active_entity)
            .unwrap()
            .construction
            .complete
    );
    assert_eq!(
        world.get::<WorkerTask>(builder),
        Some(&WorkerTask::Constructing { building: active })
    );
    assert_eq!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(near).unwrap())
            .unwrap()
            .construction
            .active_builder,
        None,
        "a frozen tick must not hand off the builder"
    );

    world.resource_mut::<MatchSession>().phase = MatchPhase::Playing;
    step_construction(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        world
            .get::<Building>(active_entity)
            .unwrap()
            .construction
            .complete
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(builder),
            Some(WorkerTask::ToConstruction { building, .. }) if *building == near
        ),
        "handoff chooses the closest reachable unfinished site"
    );
    assert_eq!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(near).unwrap())
            .unwrap()
            .construction
            .active_builder,
        Some(UnitId(1))
    );
    assert_eq!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(far).unwrap())
            .unwrap()
            .construction
            .active_builder,
        None,
        "the farther old site remains unassigned"
    );
}

#[test]
fn completed_wall_skips_busy_and_unreachable_sites_during_handoff() {
    let (mut world, mut map, builder) = setup_build_test();
    let busy = place_test_wall(&mut world, &mut map, GridPos::new(14, 10));
    let blocked = place_test_wall(&mut world, &mut map, GridPos::new(16, 10));
    let reachable = place_test_wall(&mut world, &mut map, GridPos::new(22, 10));
    let active = place_test_wall(&mut world, &mut map, GridPos::new(12, 10));

    let second_builder = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(14.5, 13.5),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world.entity_mut(second_builder).insert((
        Carry::Empty,
        GatherProgress::default(),
        WorkerTask::Idle,
    ));
    let assignment = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(2),
            building: busy,
        },
    );
    assert_eq!(assignment.reject, None, "busy site assignment rejected");

    let blocked_entity = world.resource::<BuildingIndex>().entity(blocked).unwrap();
    let blocked_footprint = *world.get::<Footprint>(blocked_entity).unwrap();
    for cell in blocked_footprint.perimeter_cells() {
        map.set_blocked(cell, true);
    }
    make_active_site_complete_next_tick(&mut world, &map, builder, active);
    step_construction(&mut world, &map, SIM_STEP_SECONDS);

    assert!(
        matches!(
            world.get::<WorkerTask>(builder),
            Some(WorkerTask::ToConstruction { building, .. }) if *building == reachable
        ),
        "handoff skips the blocked nearer site and tries the farther site"
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(second_builder),
            Some(WorkerTask::ToConstruction { building, .. }) if *building == busy
        ),
        "handoff leaves an already assigned site and builder alone"
    );
    assert_eq!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(reachable).unwrap())
            .unwrap()
            .construction
            .active_builder,
        Some(UnitId(1))
    );
    assert_eq!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(busy).unwrap())
            .unwrap()
            .construction
            .active_builder,
        Some(UnitId(2))
    );
    assert_eq!(
        world
            .get::<Building>(blocked_entity)
            .unwrap()
            .construction
            .active_builder,
        None
    );
}

/// Drives movement + construction until the given worker is Constructing.
fn run_until_constructing(world: &mut World, map: &GridMap, worker: Entity) {
    for _ in 0..300 {
        step_movement(world, map, SIM_STEP_SECONDS);
        step_construction(world, map, SIM_STEP_SECONDS);
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
    let (mut world, mut map, worker, building) = setup_active_builder();
    let old_task = world.get::<WorkerTask>(worker).unwrap().clone();
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
    assert_eq!(world.get::<WorkerTask>(worker), Some(&old_task));
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
    let (mut world, mut map, _worker) = setup_build_test();
    // Not a worker. Parked away from every footprint used below.
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Guard,
        unit_spec(UnitKind::Guard).speed,
    );

    // Not a worker.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(2),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(13, 10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::NotWorker));

    // Research Centers are seeded, never placed.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::ResearchCenter,
            anchor: GridPos::new(13, 10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Locked));

    // Footprint out of bounds (2×2 bunkhouse at the map edge).
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
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
            kind: BuildingKind::Bunkhouse,
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
        .scrap = 10;
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
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
            kind: BuildingKind::Bunkhouse,
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
            scrap: 10,
            coal: 500,
            crystals: 0,
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

/// A Team-2 Gate would seal its own team while passing Team 1, a Team-2
/// GunTurret never joins the Team-1-only combat pass, and the one-player
/// winter fixture has no Team-2 furnace economy — none of these kinds is
/// placeable by a non-Team-1 issuer. Walls stay legal for any team.
#[test]
fn gate_gun_turret_and_furnace_reject_non_team_one_issuers() {
    let (mut world, mut map, _worker) = setup_build_test();
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(30.5, 30.5),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );

    for kind in [
        BuildingKind::Gate,
        BuildingKind::GunTurret,
        BuildingKind::CoalFurnace,
    ] {
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TeamId(2),
                builder: UnitId(2),
                kind,
                anchor: GridPos::new(32, 30),
            },
        );
        assert_eq!(result.reject, Some(RejectReason::Locked), "{kind:?}");
    }
    assert_eq!(
        world
            .get_resource::<BuildingIndex>()
            .map(|index| index.iter().count())
            .unwrap_or(0),
        0
    );
}

#[test]
fn accepted_bunkhouse_deducts_fifty_scrap_and_blocks_four_cells() {
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );

    assert_eq!(result.reject, None);

    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.scrap, 450);
    assert_eq!(state.stockpile.food, 500);
    assert_eq!(state.stockpile.coal, 500);

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
    assert_eq!(building.kind, BuildingKind::Bunkhouse);
    assert_eq!(building.construction.active_builder, Some(UnitId(1)));
    assert!(!building.construction.complete);
    assert_eq!(world.get::<Footprint>(building_entity), Some(&footprint));

    // The builder is tasked to an immediate-perimeter slot with a route.
    let task = world.get::<WorkerTask>(worker).unwrap();
    let WorkerTask::ToConstruction { building: id, slot } = task else {
        panic!("expected ToConstruction, got {task:?}");
    };
    assert_eq!(*id, BuildingId(10));
    assert!(footprint.is_immediately_adjacent(*slot));
    let order = world.get::<MoveOrder>(worker).expect("route installed");
    assert_eq!(order.goal, *slot);
    assert_eq!(
        order.waypoints.last().copied(),
        Some(map.cell_center(*slot))
    );
}

#[test]
fn gun_turret_gets_attack_cooldown_only_when_construction_completes() {
    let (mut world, mut map, worker) = setup_build_test();
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::GunTurret,
            anchor: GridPos::new(13, 10),
        },
    );
    assert_eq!(result.reject, None);

    let entity = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(10))
        .expect("turret site is indexed");
    assert!(world.get::<AttackCooldown>(entity).is_none());
    assert!(!world.get::<Building>(entity).unwrap().construction.complete);

    world.entity_mut(worker).insert(WorkerTask::Constructing {
        building: BuildingId(10),
    });
    step_construction(&mut world, &map, 25.0);

    assert!(world.get::<Building>(entity).unwrap().construction.complete);
    assert_eq!(
        world.get::<AttackCooldown>(entity),
        Some(&AttackCooldown::default())
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
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
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
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
    let (mut world, mut map, worker) = setup_build_test();
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
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
        .get::<MoveOrder>(worker)
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
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
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
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
    assert!(world.get::<MoveOrder>(worker).is_none());
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
    let (mut world, mut map, worker, building) = setup_active_builder();
    let building_id = world.get::<Building>(building).unwrap().id;
    // setup_active_builder's arrival tick already advanced one step.
    let baseline = world
        .get::<Building>(building)
        .unwrap()
        .construction
        .progress_seconds;

    for _ in 0..60 {
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
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

    run_until_constructing(&mut world, &map, worker);
    // The arrival tick already advanced one step; 59 more completes the
    // same 60-tick batch as before the pause.
    for _ in 0..59 {
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
fn completed_depot_gains_dropoff_exactly_once() {
    let (mut world, mut map, worker) = setup_build_test();

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Depot,
            anchor: GridPos::new(13, 10),
        },
    );
    assert_eq!(result.reject, None);
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap,
        425
    );

    let building_entity = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(10))
        .expect("building registered");
    for _ in 0..1000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));

    // Further ticks never grant a second marker or restart work.
    for _ in 0..10 {
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert_eq!(
        world.get::<Dropoff>(building_entity),
        Some(&Dropoff { team: TeamId(1) })
    );
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
}

#[test]
fn completed_barracks_gains_an_empty_production_queue() {
    let (mut world, mut map, _worker) = setup_build_test();

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
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
fn completed_building_never_gains_a_resource_source() {
    // Completion grants are Dropoff (Depot) and the producer queue —
    // the Farm resource grant is deleted with the Farm kind, so a real
    // completion must never leave a ResourceSource on the building entity
    // or a new entry in the ResourceIndex.
    let (mut world, mut map, _worker) = setup_build_test();

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
    assert_eq!(result.reject, None, "Barracks placement rejected");
    let barracks = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(10))
        .expect("barracks registered");
    for _ in 0..2000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
        if world
            .get::<Building>(barracks)
            .unwrap()
            .construction
            .complete
        {
            break;
        }
    }

    assert!(
        world
            .get::<Building>(barracks)
            .unwrap()
            .construction
            .complete,
        "Barracks never completed"
    );
    assert!(world.get::<ResourceSource>(barracks).is_none());
    let mut sources = world.query::<&ResourceSource>();
    assert_eq!(sources.iter(&world).count(), 0);
    assert_eq!(
        world
            .get_resource::<ResourceIndex>()
            .map(|index| index.iter().count())
            .unwrap_or(0),
        0
    );
}

#[test]
fn placement_rejects_a_footprint_under_a_standing_unit() {
    let (mut world, mut map, _worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    // A live unit stands inside the would-be footprint.
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        map.cell_center(GridPos::new(13, 10)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );

    assert_eq!(result.reject, Some(RejectReason::Occupied));

    // No cost was charged and no footprint cell was blocked.
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap,
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
    let (mut world, mut map, _worker) = setup_build_test();
    let scout = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(24.5, 10.5),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
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
            kind: BuildingKind::Bunkhouse,
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
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), anchor));

    // Vision withdraws: the footprint stays explored but leaves current
    // visibility, so an enemy standing on it is genuinely hidden.
    world
        .entity_mut(worker)
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
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );

    assert!(
        validate_placement(
            &world,
            &map,
            TeamId(1),
            UnitId(1),
            BuildingKind::Bunkhouse,
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
            kind: BuildingKind::Bunkhouse,
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
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(!visible_to(&world, TeamId(1), anchor));

    // The hidden enemy walks toward a cell the footprint will cover.
    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(40, 40)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
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
            BuildingKind::Bunkhouse,
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
            kind: BuildingKind::Bunkhouse,
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

/// Review item: a hidden enemy's `MoveOrder` goal is order intent no foreign
/// team can observe — admitting it because the goal *cell* sits on currently
/// visible ground would seed an `Occupied` reject that maps the unseen
/// unit's destination. The footprint must validate and accept like
/// identical empty ground; the covered goal still retargets on acceptance.
#[test]
fn hidden_enemy_goal_on_visible_ground_does_not_reject_placement() {
    let (mut world, mut map, _worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), GridPos::new(14, 11)));

    // The enemy itself stands far outside Team 1's vision (hidden); only
    // its reserved goal lies on seen ground inside the footprint.
    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(50, 50)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world.entity_mut(enemy).insert(MoveOrder {
        waypoints: vec![],
        next: 0,
        goal: GridPos::new(14, 11),
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    assert!(!visible_to(&world, TeamId(1), GridPos::new(50, 50)));

    assert!(
        validate_placement(
            &world,
            &map,
            TeamId(1),
            UnitId(1),
            BuildingKind::Bunkhouse,
            anchor
        )
        .is_ok(),
        "an unseen enemy's goal must not surface as `Occupied` on visible ground"
    );
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );
    assert_eq!(result.reject, None);

    // Acceptance still protects the hidden unit's order: the covered goal
    // retargets off the footprint exactly as on hidden ground.
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

/// A hidden attack-move destination inside the footprint retargets in
/// lockstep with the route goal: a stale destination would make the next
/// combat tick's `resume_destination` discard the fresh route and fail
/// re-pathing onto the now-blocked cell — the unit stops while its order
/// lives forever.
#[test]
fn hidden_attack_move_destination_retargets_with_the_route() {
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);

    // The hidden enemy attack-moves toward a cell the footprint will
    // cover. A Guard, not a Worker: `step_combat` skips units whose
    // kind has no combat spec, so only a combatant exercises the
    // `resume_destination` interaction under test.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(40, 40)),
        UnitKind::Guard,
        unit_spec(UnitKind::Guard).speed,
    );
    world.entity_mut(enemy).insert(MoveOrder {
        waypoints: vec![],
        next: 0,
        goal: GridPos::new(14, 11),
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    world.entity_mut(enemy).insert(CombatOrder::AttackMove {
        destination: GridPos::new(14, 11),
        target: None,
        last_target_cell: None,
    });

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );
    assert_eq!(result.reject, None);

    let goal = world.get::<MoveOrder>(enemy).expect("order preserved").goal;
    let combat = world.get::<CombatOrder>(enemy).unwrap();
    let CombatOrder::AttackMove { destination, .. } = *combat else {
        panic!("combat order must stay an attack-move");
    };
    assert_eq!(
        destination, goal,
        "the attack-move destination must move in lockstep with the route goal"
    );
    assert!(
        map.is_walkable(goal) && !Footprint::new(anchor, 2, 2).cells().contains(&goal),
        "the shared new goal {goal:?} is open ground outside the footprint"
    );

    // The strand: one combat tick must not discard the retargeted route
    // (a stale destination made `resume_destination` drop it and fail to
    // re-path onto the blocked cell), and the unit must keep marching.
    // Twenty ticks (1 s) cannot cover the ~25-cell march, so the route
    // must still be live — arrival legitimately drops it.
    let before = world.get::<SimPosition>(enemy).unwrap().current;
    for _ in 0..20 {
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world.get::<MoveOrder>(enemy).is_some(),
        "the combat tick must keep a live route toward the re-anchored destination"
    );
    let after = world.get::<SimPosition>(enemy).unwrap().current;
    assert_ne!(
        before, after,
        "the retargeted attack-mover must still march, not strand"
    );
}

/// Current vision restores the honest rejection: a *visible* enemy unit
/// inside the footprint still surfaces `Occupied` — the seam only hides
/// what the issuer genuinely cannot see.
#[test]
fn visible_enemy_inside_footprint_still_rejects_occupied() {
    let (mut world, mut map, _worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        map.cell_center(GridPos::new(14, 11)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    reveal(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), GridPos::new(14, 11)));

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Occupied));
}

/// A completed 2×2 Bunkhouse of `team` standing on `anchor`, fully registered
/// (cells blocked, index updated) — the manual shape of an applied
/// placement, for knowledge-gate tests.
fn spawn_enemy_building(world: &mut World, map: &mut GridMap, team: TeamId, anchor: GridPos) {
    let footprint = Footprint::new(anchor, 2, 2);
    for cell in footprint.cells() {
        map.set_blocked(cell, true);
    }
    let id = BuildingId(77);
    let entity = world
        .spawn((
            Building {
                id,
                team,
                kind: BuildingKind::Bunkhouse,
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
}

fn spawn_enemy_wall(world: &mut World, map: &mut GridMap, cell: GridPos) {
    map.set_blocked(cell, true);
    let id = BuildingId(77);
    let entity = world
        .spawn((
            Building {
                id,
                team: TeamId(2),
                kind: BuildingKind::Wall,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(cell, 1, 1),
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(id, entity);
}

/// A previously scouted, now hidden enemy building footprint must answer
/// the preview exactly like identical empty explored ground — `Occupied`
/// from walkability would map the hidden building directly. The committed
/// command stays honest: two real buildings can never overlap.
#[test]
fn seen_then_hidden_enemy_building_footprint_never_rejects_preview_occupied() {
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);

    // Team 1 scouts the enemy building standing on the anchor.
    spawn_enemy_building(&mut world, &mut map, TeamId(2), anchor);
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), anchor));

    // Vision withdraws: the footprint stays explored but leaves current
    // visibility, so the building is genuinely hidden.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(
        explored_by(&world, TeamId(1), anchor) && !visible_to(&world, TeamId(1), anchor),
        "the footprint is explored fog, not current vision"
    );

    assert!(
        validate_placement(
            &world,
            &map,
            TeamId(1),
            UnitId(1),
            BuildingKind::Bunkhouse,
            anchor
        )
        .is_ok(),
        "a hidden enemy building must not surface as `Occupied` through the preview seam"
    );

    // The committed command re-checks real occupancy: no overlap, no charge.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Occupied));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap,
        500
    );
}

#[test]
fn hidden_enemy_wall_does_not_change_gate_preview_but_commit_rejects_overlap() {
    let (mut world, mut map, worker) = setup_build_test();
    let gate = GridPos::new(13, 10);
    for y in 0..map.height() {
        if y != gate.y {
            map.set_blocked(GridPos::new(gate.x, y), true);
        }
    }
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(16.5, 10.5)));

    // Reserve all approach slots on the builder's side. The only remaining
    // slot requires a route through the Gate cell in the cloned post-placement map.
    for (index, slot) in [
        GridPos::new(14, 9),
        GridPos::new(14, 10),
        GridPos::new(14, 11),
    ]
    .into_iter()
    .enumerate()
    {
        let entity = spawn_unit(
            &mut world,
            UnitId(2 + index as u32),
            TeamId(1),
            Vec2::new(1.5 + index as f32, 50.5),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
        world.entity_mut(entity).insert(MoveOrder {
            waypoints: Vec::new(),
            next: 0,
            goal: slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }

    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(
        explored_by(&world, TeamId(1), gate) && !visible_to(&world, TeamId(1), gate),
        "the empty ground should be explored but hidden"
    );

    let empty_ground_plan =
        validate_placement(&world, &map, TeamId(1), UnitId(1), BuildingKind::Gate, gate)
            .expect("empty ground Gate preview should find the west-side slot");
    assert!(empty_ground_plan.route.contains(&map.cell_center(gate)));

    spawn_enemy_wall(&mut world, &mut map, gate);
    refresh_visibility(&mut world, &map);
    assert!(
        explored_by(&world, TeamId(1), gate) && !visible_to(&world, TeamId(1), gate),
        "the enemy Wall should be hidden in explored fog"
    );

    let hidden_plan =
        validate_placement(&world, &map, TeamId(1), UnitId(1), BuildingKind::Gate, gate)
            .expect("hidden occupancy must preview like identical empty ground");
    assert_eq!(hidden_plan.slot, empty_ground_plan.slot);
    assert_eq!(hidden_plan.route, empty_ground_plan.route);

    let stockpile_before = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
    let revision_before = map.revision();
    let committed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Gate,
            anchor: gate,
        },
    );
    assert_eq!(committed.reject, Some(RejectReason::Occupied));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile,
        stockpile_before
    );
    assert_eq!(map.revision(), revision_before);
}

#[test]
fn hidden_blocker_outside_gate_footprint_keeps_builder_route_unreachable() {
    let (mut world, mut map, worker) = setup_build_test();
    let hidden_blocker = GridPos::new(13, 10);
    let gate = GridPos::new(12, 10);
    for y in 0..map.height() {
        if y != hidden_blocker.y {
            map.set_blocked(GridPos::new(hidden_blocker.x, y), true);
        }
    }

    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(50.5, 50.5)));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), hidden_blocker));
    assert!(!visible_to(&world, TeamId(1), hidden_blocker));

    let empty_ground_plan =
        validate_placement(&world, &map, TeamId(1), UnitId(1), BuildingKind::Gate, gate)
            .expect("empty chokepoint should be reachable");
    assert!(
        empty_ground_plan
            .route
            .contains(&map.cell_center(hidden_blocker))
    );

    spawn_enemy_wall(&mut world, &mut map, hidden_blocker);
    refresh_visibility(&mut world, &map);
    assert!(!visible_to(&world, TeamId(1), hidden_blocker));
    let stockpile_before = world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
    let revision_before = map.revision();

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Gate,
            anchor: gate,
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Unreachable));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile,
        stockpile_before
    );
    assert_eq!(map.revision(), revision_before);
    assert_eq!(world.resource::<BuildingIndex>().iter().count(), 1);
}

/// Current vision keeps the honest rejection: a *visible* enemy building
/// footprint still surfaces `Occupied` — the seam only hides what the
/// issuer genuinely cannot see.
#[test]
fn visible_enemy_building_footprint_still_rejects_occupied() {
    let (mut world, mut map, _worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    reveal(&mut world, &map);
    spawn_enemy_building(&mut world, &mut map, TeamId(2), anchor);
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), anchor));

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Occupied));
}

/// Building visibility is classified once per footprint — the same
/// any-cell rule presentation/combat use: with only one edge cell in
/// current vision the whole building is already on screen and pickable,
/// so its far-side cells must answer `Occupied` honestly instead of
/// pretending to be free ground that flips to `Occupied` at apply time.
#[test]
fn partially_visible_enemy_building_exposes_whole_footprint_as_occupied() {
    let (mut world, mut map, worker) = setup_build_test();
    let building_anchor = GridPos::new(18, 16);

    // Scout the ground so every relevant cell is explored.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(map.cell_center(GridPos::new(17, 12))));
    reveal(&mut world, &map);
    spawn_enemy_building(&mut world, &mut map, TeamId(2), building_anchor);
    refresh_visibility(&mut world, &map);

    // Withdraw to where only the near corner stays in current vision:
    // the building is on screen (the predicate is any-cell) while its
    // far cells are explored fog.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(10.5, 10.5)));
    refresh_visibility(&mut world, &map);
    assert!(visible_to(
        &world,
        TeamId(1),
        Footprint::new(building_anchor, 2, 2)
    ));
    assert!(visible_to(&world, TeamId(1), GridPos::new(18, 16)));
    assert!(!visible_to(&world, TeamId(1), GridPos::new(19, 17)));
    assert!(explored_by(&world, TeamId(1), GridPos::new(19, 17)));

    // A proposed footprint overlapping only the hidden far cells must
    // reject `Occupied` at validation, matching the committed command.
    assert!(
        matches!(
            validate_placement(
                &world,
                &map,
                TeamId(1),
                UnitId(1),
                BuildingKind::Bunkhouse,
                GridPos::new(19, 16)
            ),
            Err(RejectReason::Occupied)
        ),
        "a partially visible building's hidden cells are not free ground"
    );
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(19, 16),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::Occupied));
}

/// A sibling already moving to a cell inside the footprint has that goal
/// reserved: accepting the placement would block the goal and strand the
/// unit on a preserved order that can never replan onto blocked cells.
#[test]
fn placement_rejects_a_footprint_over_a_reserved_goal() {
    let (mut world, mut map, _worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    let sibling = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        map.cell_center(GridPos::new(8, 8)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
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
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );

    assert_eq!(result.reject, Some(RejectReason::Occupied));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap,
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
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    world.entity_mut(worker).insert(MoveOrder {
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
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );

    assert_eq!(result.reject, None);
    let order = world.get::<MoveOrder>(worker).expect("route installed");
    assert!(
        Footprint::new(anchor, 2, 2).is_immediately_adjacent(order.goal),
        "builder goal {goal:?} is the assigned slot, not the cancelled one",
        goal = order.goal
    );
}

#[test]
fn placement_slot_is_not_assigned_onto_a_live_unit() {
    let (mut world, mut map, worker) = setup_build_test();
    // An idle unit squats on the footprint's first perimeter candidate,
    // which an empty reservation set would hand to the builder.
    let squatter_cell = GridPos::new(12, 9);
    let squatter = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        map.cell_center(squatter_cell),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world.entity_mut(squatter).insert(WorkerTask::Idle);

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(13, 10),
        },
    );
    assert_eq!(result.reject, None);

    let task = world.get::<WorkerTask>(worker).unwrap().clone();
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
    run_until_constructing(&mut world, &map, worker);
}

/// A footprint that plugs the only corridor between the builder and the
/// remaining free approach slots must reject instead of charging for a
/// site the builder can never reach. Regression: reachability used to be
/// checked on the pre-placement map, so a route through the soon-blocked
/// footprint was accepted; the first movement replan then failed and the
/// builder idled next to a paid, unbuildable site.
#[test]
fn placement_rejects_when_the_footprint_plugs_the_only_corridor() {
    let (mut world, mut map, worker) = setup_build_test();
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(8.5, 6.5)));

    // Solid wall at x = 10 with a two-cell gap at (10, 5)–(10, 6). The
    // 2×2 Bunkhouse footprint covers the whole gap plus one column past it,
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
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(10, 5),
        },
    );

    assert_eq!(result.reject, Some(RejectReason::Unreachable));

    // Nothing was charged, spawned, blocked, or routed.
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap,
        500
    );
    let mut buildings = world.query::<&Building>();
    assert_eq!(buildings.iter(&world).count(), 0);
    assert_eq!(world.resource::<IdAllocator>().next_building, 10);
    for cell in Footprint::new(GridPos::new(10, 5), 2, 2).cells() {
        assert!(map.is_walkable(cell), "footprint cell {cell:?} blocked");
    }
    assert!(world.get::<MoveOrder>(worker).is_none());
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
}

#[test]
fn abandoned_site_never_progresses_without_a_builder() {
    let (mut world, mut map, _worker) = setup_build_test();
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
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
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
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
    let (mut world, mut map, worker) = setup_build_test();
    let anchor = GridPos::new(13, 10);
    // The worker is en route to a cell the new Bunkhouse will cover; his own
    // goal is exempt from the occupancy check because acceptance cancels it.
    world.entity_mut(worker).insert(MoveOrder {
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
            kind: BuildingKind::Bunkhouse,
            anchor,
        },
    );

    assert!(
        result.reject.is_none(),
        "the builder's own goal must not reject his placement: {result:?}"
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::ToConstruction { .. })
        ),
        "an accepted placement retasks the builder to the site"
    );
    let order = world.get::<MoveOrder>(worker).expect("route to the site");
    assert_ne!(
        order.goal, anchor,
        "the cancelled route is replaced by the approach route"
    );
}

#[test]
fn resume_construction_reject_codes_are_deterministic() {
    let (mut world, mut map, worker, building) = setup_active_builder();

    // A non-worker worker cannot resume.
    spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(8.5, 8.5),
        UnitKind::Guard,
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
    assert_eq!(result.reject, Some(RejectReason::NotWorker));

    // A building owned by another team cannot be resumed.
    let enemy_site = world
        .spawn((
            Building {
                id: BuildingId(11),
                team: TeamId(2),
                kind: BuildingKind::Bunkhouse,
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
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Constructing { .. })
        ),
        "every rejection leaves the active builder untouched"
    );

    // Once the site completes, resuming it is locked.
    for _ in 0..400 {
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world
            .get::<Building>(building)
            .unwrap()
            .construction
            .complete,
        "the actively built Bunkhouse should finish within 20 simulated seconds"
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
            kind: BuildingKind::Bunkhouse,
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

/// Spawns a hidden enemy worker (no team-1 vision where it stands) with an
/// optional initial task/order, plus its own incomplete site at `anchor`.
fn hidden_enemy_worker_with_site(
    world: &mut World,
    _map: &mut GridMap,
    site_anchor: GridPos,
    task: WorkerTask,
    order: Option<MoveOrder>,
) -> Entity {
    let worker = spawn_unit(
        world,
        UnitId(5),
        TeamId(2),
        Vec2::new(30.5, 30.5),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    let site_footprint = Footprint::new(site_anchor, 2, 2);
    let site = world
        .spawn((
            Building {
                id: BuildingId(50),
                team: TeamId(2),
                kind: BuildingKind::Bunkhouse,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: Some(UnitId(5)),
                },
            },
            site_footprint,
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(50), site);
    world
        .entity_mut(worker)
        .insert((Carry::Empty, GatherProgress::default(), task));
    if let Some(order) = order {
        world.entity_mut(worker).insert(order);
    }
    worker
}

/// Review item: a hidden enemy worker whose route ends inside an accepted
/// footprint must not be lost forever. The paired task slot retargets with
/// the goal onto the site's perimeter, so arrival can fire again and the
/// worker reaches `Constructing`.
#[test]
fn placement_retargets_a_hidden_workers_task_slot_off_the_footprint() {
    let (mut world, mut map, worker) = setup_build_test();
    // Walk the builder over the future anchor first so the ground is
    // explored, then park it far away: the anchor must be explored fog, not
    // current vision, or the placement would honestly see the enemy goal.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(13.5, 10.5)));
    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(4.5, 40.5)));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), GridPos::new(13, 10)));
    assert!(!crate::visibility::visible_to(
        &world,
        TeamId(1),
        GridPos::new(14, 11)
    ));

    // The enemy site at (15,10) has its 2x2 footprint beside the placement;
    // the worker's approach slot (14,10) is on that site's perimeter AND
    // inside the Bunkhouse footprint about to block (13..14, 10..11).
    let site_footprint = Footprint::new(GridPos::new(15, 10), 2, 2);
    let slot = GridPos::new(14, 10);
    let revision = map.revision();
    let route = vec![map.cell_center(slot)];
    let worker = hidden_enemy_worker_with_site(
        &mut world,
        &mut map,
        GridPos::new(15, 10),
        WorkerTask::ToConstruction {
            building: BuildingId(50),
            slot,
        },
        Some(MoveOrder {
            waypoints: route,
            next: 0,
            goal: slot,
            map_revision: revision,
            last_failed_replan: None,
        }),
    );
    assert!(site_footprint.is_immediately_adjacent(slot));

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(13, 10),
        },
    );
    assert!(result.reject.is_none(), "placement rejected: {result:?}");

    let bunkhouse_cells = Footprint::new(GridPos::new(13, 10), 2, 2).cells();
    let WorkerTask::ToConstruction { slot: new_slot, .. } =
        world.get::<WorkerTask>(worker).unwrap().clone()
    else {
        panic!("the hidden worker must stay tasked toward its site");
    };
    let goal = world.get::<MoveOrder>(worker).unwrap().goal;
    assert_eq!(
        goal, new_slot,
        "the retargeted goal and task slot must stay in lockstep"
    );
    assert!(
        !bunkhouse_cells.contains(&new_slot),
        "the task slot must leave the blocked footprint"
    );
    assert!(
        site_footprint.is_immediately_adjacent(new_slot),
        "the retargeted slot stays on the site's perimeter: {new_slot:?}"
    );

    // The worker walks and counts as arrived again — no more lost-for-the-
    // match worker or stalled site.
    for _ in 0..600 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Constructing { .. })
        ) {
            break;
        }
    }
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Constructing {
                building: BuildingId(50)
            })
        ),
        "the retargeted worker must reach Constructing again"
    );
}

/// Review item: a hidden worker teleported off its construction site by a
/// later placement is rerouted to a fresh adjacent slot instead of building
/// from up to seven cells away.
#[test]
fn displacement_reroutes_a_constructing_worker_to_an_adjacent_slot() {
    let (mut world, mut map, worker) = setup_build_test();
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(13.5, 10.5)));
    reveal(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(4.5, 40.5)));
    refresh_visibility(&mut world, &map);

    // The enemy worker is mid-build, standing on its site's perimeter cell
    // (14,10) — inside the footprint about to block.
    let worker = hidden_enemy_worker_with_site(
        &mut world,
        &mut map,
        GridPos::new(15, 10),
        WorkerTask::Constructing {
            building: BuildingId(50),
        },
        None,
    );
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(14.5, 10.5)));
    let site_entity = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(50))
        .unwrap();
    world
        .get_mut::<Building>(site_entity)
        .unwrap()
        .construction
        .progress_seconds = 3.0;

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(1),
            kind: BuildingKind::Bunkhouse,
            anchor: GridPos::new(13, 10),
        },
    );
    assert!(result.reject.is_none(), "placement rejected: {result:?}");

    let WorkerTask::ToConstruction { slot, .. } = world.get::<WorkerTask>(worker).unwrap().clone()
    else {
        panic!(
            "a displaced builder must walk back, not keep Constructing from afar: {:?}",
            world.get::<WorkerTask>(worker)
        );
    };
    let site_footprint = Footprint::new(GridPos::new(15, 10), 2, 2);
    assert!(
        site_footprint.is_immediately_adjacent(slot),
        "the fresh slot is on the site's perimeter: {slot:?}"
    );
    assert!(
        world.get::<MoveOrder>(worker).is_some(),
        "the reroute installs a real route"
    );

    // While walking back, construction does not advance.
    let before = world
        .get::<Building>(site_entity)
        .unwrap()
        .construction
        .progress_seconds;
    step_construction(&mut world, &map, SIM_STEP_SECONDS);
    let after = world
        .get::<Building>(site_entity)
        .unwrap()
        .construction
        .progress_seconds;
    assert_eq!(before, after, "no building from a distance");
}

/// Review item: resume on a hidden enemy building must answer
/// `BuildingMissing` exactly like an absent id, so probing ids cannot map
/// hidden enemy construction; a *visible* enemy site stays `NotOwned`.
#[test]
fn resume_on_a_hidden_enemy_building_answers_building_missing() {
    let (mut world, mut map, _) = setup_build_test();
    reveal(&mut world, &map);
    let enemy_site = world
        .spawn((
            Building {
                id: BuildingId(60),
                team: TeamId(2),
                kind: BuildingKind::Bunkhouse,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(40, 40), 2, 2),
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(60), enemy_site);
    refresh_visibility(&mut world, &map);
    assert!(
        !crate::visibility::visible_to(&world, TeamId(1), GridPos::new(40, 40)),
        "premise: the enemy site starts hidden"
    );

    let hidden = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(1),
            building: BuildingId(60),
        },
    );
    assert_eq!(hidden.reject, Some(RejectReason::BuildingMissing));

    // A genuinely seen enemy site stays honest.
    world
        .entity_mut(world.resource::<UnitIndex>().entity(UnitId(1)).unwrap())
        .insert(SimPosition::new(Vec2::new(40.5, 43.5)));
    refresh_visibility(&mut world, &map);
    assert!(crate::visibility::visible_to(
        &world,
        TeamId(1),
        GridPos::new(40, 40)
    ));
    let visible = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(1),
            building: BuildingId(60),
        },
    );
    assert_eq!(visible.reject, Some(RejectReason::NotOwned));
}
