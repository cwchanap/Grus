use bevy::math::Vec2;
use bevy::prelude::World;
use grus_sim::{
    GridMap, GridPos, MoveOrder, SIM_STEP_SECONDS, SimPosition, TeamId, UnitCommand,
    UnitCommandKind, UnitId, apply_command, spawn_unit, step_movement,
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
        4.0,
    );

    let outcome = apply_command(
        &mut world,
        &map,
        UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::Move {
                target: Vec2::new(12.5, 3.5),
            },
        },
    );
    assert_eq!(outcome.accepted, vec![UnitId(1)]);

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
    assert!(position.x > 10.0, "unit did not make it around the obstacle: {position:?}");
    assert!(map.is_walkable(map.world_to_cell(position)));
}
