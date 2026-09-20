use std::collections::HashSet;

use bevy::prelude::World;

use super::*;
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
};
use crate::fixture::MapFixture;
use crate::map::Footprint;

fn assert_vec2_near(actual: Vec2, expected: Vec2) {
    let error = actual.distance(expected);
    assert!(
        error < 0.0001,
        "expected {expected:?}, got {actual:?}, error {error}"
    );
}

fn test_order(map: &GridMap, waypoint: Vec2) -> MoveOrder {
    MoveOrder {
        waypoints: vec![waypoint],
        next: 0,
        goal: map.world_to_cell(waypoint),
        map_revision: map.revision(),
        last_failed_replan: None,
    }
}

#[test]
fn fixed_step_moves_by_speed_times_delta() {
    let mut world = World::new();
    let map = GridMap::new(16, 16);
    let entity = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        2.0,
    );
    world
        .entity_mut(entity)
        .insert(test_order(&map, Vec2::new(8.5, 1.5)));

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    let position = world.get::<SimPosition>(entity).unwrap();
    assert_vec2_near(position.previous, Vec2::new(1.5, 1.5));
    assert_vec2_near(position.current, Vec2::new(1.6, 1.5));
}

#[test]
fn route_finishes_and_removes_move_order() {
    let mut world = World::new();
    let map = GridMap::new(16, 16);
    let entity = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        20.0,
    );
    world
        .entity_mut(entity)
        .insert(test_order(&map, Vec2::new(2.5, 1.5)));

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    assert_vec2_near(
        world.get::<SimPosition>(entity).unwrap().current,
        Vec2::new(2.5, 1.5),
    );
    assert!(world.get::<MoveOrder>(entity).is_none());
}

#[test]
fn units_do_not_enter_static_blocked_cells() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 16);
    map.set_blocked_rect(GridPos::new(2, 1), GridPos::new(2, 1));
    let entity = spawn_unit(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        20.0,
    );
    world.entity_mut(entity).insert((
        SimPosition {
            previous: Vec2::ZERO,
            current: Vec2::new(1.5, 1.5),
        },
        test_order(&map, Vec2::new(2.5, 1.5)),
    ));

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    let position = world.get::<SimPosition>(entity).unwrap();
    assert_vec2_near(position.previous, Vec2::new(1.5, 1.5));
    assert_vec2_near(position.current, Vec2::new(1.5, 1.5));
    assert!(world.get::<MoveOrder>(entity).is_some());
}

#[test]
fn blocked_cached_step_replans_around_obstacle() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 16);
    map.set_blocked_rect(GridPos::new(2, 1), GridPos::new(2, 1));
    let entity = spawn_unit(
        &mut world,
        UnitId(6),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        12.0,
    );
    // Stale cached route that would step straight through the blocked cell.
    world.entity_mut(entity).insert(MoveOrder {
        waypoints: vec![
            map.cell_center(GridPos::new(2, 1)),
            map.cell_center(GridPos::new(3, 1)),
        ],
        next: 0,
        goal: GridPos::new(3, 1),
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    // The blocked step must trigger a replan around (2, 1) rather than
    // preserving the stale route that walks into it.
    let order = world.get::<MoveOrder>(entity).expect("order preserved");
    assert_ne!(
        order.waypoints.first().copied(),
        Some(map.cell_center(GridPos::new(2, 1))),
        "stale route through blocked cell was not replaced"
    );

    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }

    let position = world.get::<SimPosition>(entity).unwrap();
    assert_vec2_near(position.current, map.cell_center(GridPos::new(3, 1)));
    assert!(world.get::<MoveOrder>(entity).is_none());
}

#[test]
fn failed_forced_replan_waits_for_map_change_instead_of_recomputing() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 16);
    // Enclose the unit's cell (1, 1) so the walkable goal (5, 5) is
    // unreachable from its current position.
    for cell in [
        GridPos::new(2, 1),
        GridPos::new(0, 1),
        GridPos::new(1, 2),
        GridPos::new(1, 0),
    ] {
        map.set_blocked(cell, true);
    }
    let entity = spawn_unit(
        &mut world,
        UnitId(7),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        12.0,
    );
    // Stale cached route whose first step lands on the blocked cell (2, 1),
    // forcing the movement step into the forced-replan branch.
    world.entity_mut(entity).insert(MoveOrder {
        waypoints: vec![
            map.cell_center(GridPos::new(2, 1)),
            map.cell_center(GridPos::new(5, 5)),
        ],
        next: 0,
        goal: GridPos::new(5, 5),
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    let failed_revision = map.revision();

    // First step: the forced replan fails (no path from the enclosed cell),
    // so the order is preserved and the failed revision is recorded.
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    let order = world
        .get::<MoveOrder>(entity)
        .expect("order preserved while stranded");
    assert_eq!(order.last_failed_replan, Some(failed_revision));
    assert_vec2_near(
        world.get::<SimPosition>(entity).unwrap().current,
        Vec2::new(1.5, 1.5),
    );
    let calls_after_first_step = map.path_call_count();

    // Many subsequent steps must not re-run A* for the same revision: the
    // unit stays put, the recorded failed revision is unchanged, and the
    // pathfind call count does not increase.
    for _ in 0..50 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    let order = world
        .get::<MoveOrder>(entity)
        .expect("order still preserved");
    assert_eq!(order.last_failed_replan, Some(failed_revision));
    assert_vec2_near(
        world.get::<SimPosition>(entity).unwrap().current,
        Vec2::new(1.5, 1.5),
    );
    assert_eq!(
        map.path_call_count(),
        calls_after_first_step,
        "A* was re-run on a stranded unit while the map revision was unchanged"
    );

    // When the map changes so the goal becomes reachable, the unit resumes.
    map.set_blocked(GridPos::new(2, 1), false);
    assert!(map.revision() > failed_revision);
    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world.get::<MoveOrder>(entity).is_none(),
        "unit should reach the goal"
    );
    assert_vec2_near(
        world.get::<SimPosition>(entity).unwrap().current,
        map.cell_center(GridPos::new(5, 5)),
    );
}

#[test]
fn failed_revision_replan_preserves_order_until_route_reopens() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 16);
    let goal = GridPos::new(5, 5);
    let entity = spawn_unit(
        &mut world,
        UnitId(8),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        12.0,
    );
    world.entity_mut(entity).insert(MoveOrder {
        waypoints: vec![map.cell_center(GridPos::new(2, 1)), map.cell_center(goal)],
        next: 0,
        goal,
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    // An occupancy change walls the unit in: the revision bumps and the
    // goal becomes unreachable, so the refresh finds no route. The order
    // must be preserved with the failed revision recorded rather than
    // cancelled before the preserved-order branch can run.
    for cell in [
        GridPos::new(2, 1),
        GridPos::new(0, 1),
        GridPos::new(1, 2),
        GridPos::new(1, 0),
    ] {
        map.set_blocked(cell, true);
    }
    let failed_revision = map.revision();

    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    let order = world
        .get::<MoveOrder>(entity)
        .expect("order preserved after failed revision replan");
    assert_eq!(order.map_revision, failed_revision);
    assert_eq!(order.last_failed_replan, Some(failed_revision));

    // While the map is unchanged, no further A* runs and the unit waits.
    let calls = map.path_call_count();
    for _ in 0..50 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert_eq!(
        map.path_call_count(),
        calls,
        "A* re-ran while the failed revision was still current"
    );
    assert!(world.get::<MoveOrder>(entity).is_some());

    // A later revision that reopens the route lets the unit finish.
    map.set_blocked(GridPos::new(2, 1), false);
    assert!(map.revision() > failed_revision);
    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world.get::<MoveOrder>(entity).is_none(),
        "unit should reach the goal once the route reopens"
    );
    assert_vec2_near(
        world.get::<SimPosition>(entity).unwrap().current,
        map.cell_center(goal),
    );
}

#[test]
fn failed_route_idles_an_active_worker_and_releases_its_farm() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 16);
    // A completed Farm whose assignment is held by the traveling worker.
    let farm = world
        .spawn((
            crate::buildings::Building {
                id: crate::ids::BuildingId(1),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::Farm,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(8, 8), 2, 2),
            crate::economy::ResourceSource {
                id: crate::ids::ResourceId(1),
                kind: crate::catalog::ResourceKind::Food,
                remaining: None,
                assigned_worker: Some(UnitId(1)),
            },
        ))
        .id();
    world
        .get_resource_or_insert_with(crate::buildings::BuildingIndex::default)
        .insert(crate::ids::BuildingId(1), farm);
    world
        .get_resource_or_insert_with(crate::economy::ResourceIndex::default)
        .insert(crate::ids::ResourceId(1), farm);

    let worker = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    world.entity_mut(worker).insert((
        crate::economy::WorkerTask::ToDropoff {
            source: crate::ids::ResourceId(1),
            dropoff: crate::ids::BuildingId(1),
            slot: GridPos::new(7, 7),
        },
        crate::economy::Carry::Holding {
            kind: crate::catalog::ResourceKind::Food,
            amount: std::num::NonZeroU32::new(4).unwrap(),
        },
        test_order(&map, Vec2::new(7.5, 7.5)),
    ));

    // Wall the worker in: the revision bumps and the replan finds no
    // route, so the retained-order path fires for an active worker.
    for cell in [
        GridPos::new(0, 0),
        GridPos::new(1, 0),
        GridPos::new(2, 0),
        GridPos::new(0, 1),
        GridPos::new(2, 1),
        GridPos::new(0, 2),
        GridPos::new(1, 2),
        GridPos::new(2, 2),
    ] {
        map.set_blocked(cell, true);
    }

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    assert_eq!(
        world.get::<crate::economy::WorkerTask>(worker),
        Some(&crate::economy::WorkerTask::Idle)
    );
    assert!(world.get::<MoveOrder>(worker).is_none());
    assert_eq!(
        world
            .get::<crate::economy::ResourceSource>(farm)
            .unwrap()
            .assigned_worker,
        None,
        "route failure releases the Farm assignment"
    );
    assert_eq!(
        world
            .get::<crate::economy::Carry>(worker)
            .cloned()
            .unwrap_or(crate::economy::Carry::Empty)
            .amount_or_zero(),
        4,
        "cleanup never discards Carry"
    );
    assert_eq!(
        world
            .resource::<crate::economy::LastRouteReject>()
            .0
            .get(&TeamId(1)),
        Some(&RejectReason::Unreachable)
    );
}

#[test]
fn neighboring_units_separate_instead_of_collapsing() {
    let mut world = World::new();
    let map = GridMap::new(16, 16);
    let first = spawn_unit(
        &mut world,
        UnitId(4),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Villager,
        2.0,
    );
    let second = spawn_unit(
        &mut world,
        UnitId(5),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Villager,
        2.0,
    );
    for entity in [first, second] {
        world
            .entity_mut(entity)
            .insert(test_order(&map, Vec2::new(10.5, 5.5)));
    }

    step_movement(&mut world, &map, SIM_STEP_SECONDS);

    let first_position = world.get::<SimPosition>(first).unwrap().current;
    let second_position = world.get::<SimPosition>(second).unwrap().current;
    assert!(
        first_position.distance(second_position) > 0.01,
        "units collapsed at {first_position:?}"
    );
}

#[test]
fn hundred_unit_group_finishes_representative_battlefield_route() {
    let mut fixture = MapFixture::battlefield();
    let mut world = World::new();
    let mut ids = Vec::new();

    for row in 0..10 {
        for column in 0..10 {
            let id = UnitId((row * 10 + column + 1) as u32);
            ids.push(id);
            spawn_unit(
                &mut world,
                id,
                TeamId(1),
                Vec2::new(10.5 + column as f32, 43.5 + row as f32),
                UnitKind::Villager,
                12.0,
            );
        }
    }

    let outcome = apply_player_command(
        &mut world,
        &mut fixture.map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: ids,
            kind: UnitCommandKind::Move {
                target: fixture.right_spawn,
            },
        }),
    );
    assert_eq!(outcome.accepted_units.len(), 100);
    assert!(outcome.rejected_units.is_empty());

    for _ in 0..600 {
        step_movement(&mut world, &fixture.map, SIM_STEP_SECONDS);
    }

    let mut moving_query = world.query::<&MoveOrder>();
    assert_eq!(moving_query.iter(&world).count(), 0);

    let mut position_query = world.query::<&SimPosition>();
    let final_positions = position_query
        .iter(&world)
        .map(|position| position.current)
        .collect::<Vec<_>>();
    assert!(final_positions.iter().all(|position| {
        fixture
            .map
            .is_walkable(fixture.map.world_to_cell(*position))
    }));
    // Units must actually arrive near the commanded destination, not just
    // shed their MoveOrders: assigned slots cluster within ~7 units of
    // right_spawn, while the spawn block sits ~95 units away.
    assert!(
        final_positions
            .iter()
            .all(|position| position.distance(fixture.right_spawn) < 12.0),
        "100-unit group did not arrive near the commanded destination"
    );
    let final_cells = final_positions
        .into_iter()
        .map(|position| fixture.map.world_to_cell(position))
        .collect::<HashSet<_>>();
    assert_eq!(
        final_cells.len(),
        100,
        "100-unit group collapsed onto shared destination cells"
    );
}
