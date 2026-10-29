use bevy::math::Vec2;
use bevy::prelude::World;
use grus_sim::{
    GridMap, GridPos, MapFixture, MoveOrder, PlayerCommand, ResourceId, SIM_STEP_SECONDS,
    SimPosition, TeamId, UnitCommand, UnitCommandKind, UnitId, UnitIndex, UnitKind, WorkerTask,
    apply_player_command, seed_skirmish, spawn_unit, step_economy, step_movement,
};

#[test]
fn active_move_replans_when_new_obstacle_intersects_route() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 8);
    let unit = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 3.5),
        UnitKind::Worker,
        4.0,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::Move {
                target: Vec2::new(12.5, 3.5),
            },
        }),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    for _ in 0..5 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    map.set_blocked_rect(GridPos::new(4, 3), GridPos::new(4, 3));

    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }

    let position = world.get::<SimPosition>(unit).unwrap().current;
    assert!(
        world.get::<MoveOrder>(unit).is_none(),
        "route stalled instead of replanning at {position:?}"
    );
    assert!(
        position.x > 10.0,
        "unit did not make it around the obstacle: {position:?}"
    );
    assert!(map.is_walkable(map.world_to_cell(position)));
}

/// A worker whose task flips on cell arrival while its order still has the
/// centre waypoint pending must not be idled when a placement bumps the map
/// revision: the fulfilled order is dropped with the transition instead of
/// re-planning a zero-length route into a route failure.
#[test]
fn arrived_worker_is_not_idled_by_a_revision_bump() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let source = ResourceId(3); // salvage at (56,52)
    let slot = GridPos::new(55, 51);

    // The arrival window the defense smoke hit: the worker has crossed the
    // slot-cell boundary while the order's centre waypoint is still pending.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(55.5, 51.2)));
    world
        .entity_mut(worker)
        .insert(WorkerTask::ToSource { source, slot });
    world.entity_mut(worker).insert(MoveOrder {
        waypoints: vec![map.cell_center(slot)],
        next: 0,
        goal: slot,
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(matches!(
        world.get::<WorkerTask>(worker),
        Some(WorkerTask::Gathering { .. })
    ));
    assert!(
        world.get::<MoveOrder>(worker).is_none(),
        "arrival must drop the fulfilled order"
    );

    // A wall placed elsewhere bumps the map revision; the arrived gatherer
    // must keep gathering instead of being treated as a failed route.
    map.set_blocked(GridPos::new(60, 44), true);
    for _ in 0..4 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Gathering { .. })
        ),
        "revision bump idled an arrived gatherer: {:?}",
        world.get::<WorkerTask>(worker)
    );
}
