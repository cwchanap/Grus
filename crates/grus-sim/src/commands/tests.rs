use bevy::prelude::World;

use super::*;
use crate::combat::CombatOrder;
use crate::economy::WorkerTask;
use crate::movement::{SIM_STEP_SECONDS, step_movement};

fn open_map() -> GridMap {
    GridMap::new(24, 24)
}

fn move_command(issuer: TeamId, units: Vec<UnitId>, target: Vec2) -> PlayerCommand {
    PlayerCommand::Units(UnitCommand {
        issuer,
        units,
        kind: UnitCommandKind::Move { target },
    })
}

fn stop_command(issuer: TeamId, units: Vec<UnitId>) -> PlayerCommand {
    PlayerCommand::Units(UnitCommand {
        issuer,
        units,
        kind: UnitCommandKind::Stop,
    })
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
fn enemy_units_are_rejected_without_mutating_their_order() {
    let mut world = World::new();
    let mut map = open_map();
    let enemy = spawn_unit(
        &mut world,
        UnitId(9),
        TeamId(2),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(enemy)
        .insert(test_order(&map, Vec2::new(3.5, 2.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(9)], Vec2::new(18.5, 18.5)),
    );

    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(9), RejectReason::NotOwned)]
    );
    assert_eq!(
        world.get::<MoveOrder>(enemy).unwrap().waypoints,
        vec![Vec2::new(3.5, 2.5)]
    );
}

#[test]
fn replacement_move_discards_the_previous_route() {
    let mut world = World::new();
    let mut map = open_map();
    let unit = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(unit)
        .insert(test_order(&map, Vec2::new(3.5, 1.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1)], Vec2::new(19.5, 19.5)),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    let order = world.get::<MoveOrder>(unit).expect("replacement order");
    assert_eq!(order.waypoints.last().copied(), Some(Vec2::new(19.5, 19.5)));
    assert_ne!(order.waypoints, vec![Vec2::new(3.5, 1.5)]);
}

#[test]
fn stop_removes_an_active_move_order() {
    let mut world = World::new();
    let mut map = open_map();
    let unit = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(unit)
        .insert(test_order(&map, Vec2::new(8.5, 1.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        stop_command(TeamId(1), vec![UnitId(2)]),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(2)]);
    assert!(world.get::<MoveOrder>(unit).is_none());
}

#[test]
fn unreachable_units_return_terminal_rejection_without_an_order() {
    let mut world = World::new();
    let mut map = open_map();
    map.set_blocked_rect(GridPos::new(10, 10), GridPos::new(12, 12));
    let unit = spawn_unit(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(3)], Vec2::new(11.5, 11.5)),
    );

    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(3), RejectReason::Unreachable)]
    );
    assert!(world.get::<MoveOrder>(unit).is_none());
}

#[test]
fn group_targets_distinct_walkable_destination_slots() {
    let mut world = World::new();
    let mut map = open_map();
    for id in 1..=4 {
        spawn_unit(
            &mut world,
            UnitId(id),
            TeamId(1),
            Vec2::new(2.5, id as f32 + 1.5),
            UnitKind::Villager,
            6.0,
        );
    }

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(
            TeamId(1),
            vec![UnitId(4), UnitId(2), UnitId(1), UnitId(3)],
            Vec2::new(18.5, 18.5),
        ),
    );

    assert_eq!(
        outcome.accepted_units,
        vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)]
    );
    let index = world.resource::<UnitIndex>();
    let entities = (1..=4)
        .map(|id| index.entity(UnitId(id)).unwrap())
        .collect::<Vec<_>>();
    let destinations = entities
        .into_iter()
        .map(|entity| {
            let point = world
                .get::<MoveOrder>(entity)
                .unwrap()
                .waypoints
                .last()
                .copied()
                .unwrap();
            map.world_to_cell(point)
        })
        .collect::<HashSet<_>>();

    assert_eq!(destinations.len(), 4);
    assert!(destinations.iter().all(|cell| map.is_walkable(*cell)));
}

#[test]
fn sequential_commands_do_not_collapse_onto_occupied_destination() {
    let mut world = World::new();
    let mut map = open_map();
    let first = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        20.0,
    );
    let second = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(2.5, 4.5),
        UnitKind::Villager,
        20.0,
    );

    // Command 1: move the first unit to the target.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world.get::<MoveOrder>(first).is_none(),
        "first unit should have arrived"
    );

    // Command 2: move the second unit to the SAME target.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(2)], Vec2::new(18.5, 18.5)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(2)]);
    for _ in 0..200 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(
        world.get::<MoveOrder>(second).is_none(),
        "second unit should have arrived"
    );

    let first_pos = world.get::<SimPosition>(first).unwrap().current;
    let second_pos = world.get::<SimPosition>(second).unwrap().current;
    assert!(
        first_pos.distance(second_pos) >= 0.9,
        "units collapsed at first={first_pos:?} second={second_pos:?}"
    );
}

#[test]
fn destination_slots_keep_scanning_past_reserved_candidates() {
    let mut world = World::new();
    let mut map = open_map();
    // Reserve the four closest destination candidates (the target cell and
    // the first three ring-1 cells in slot order) with units that are not part
    // of the command, so a fixed 4*units.len() slot list would be exhausted
    // and the one-unit command would be rejected as unreachable.
    let target_cell = map.world_to_cell(Vec2::new(18.5, 18.5));
    let reserved = [
        target_cell,
        GridPos::new(17, 17),
        GridPos::new(18, 17),
        GridPos::new(19, 17),
    ];
    for (i, cell) in reserved.iter().enumerate() {
        spawn_unit(
            &mut world,
            UnitId(100 + i as u32),
            TeamId(2),
            map.cell_center(*cell),
            UnitKind::Villager,
            6.0,
        );
    }
    let unit = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    let order = world.get::<MoveOrder>(unit).expect("order accepted");
    assert!(
        !reserved.contains(&order.goal),
        "unit settled on a reserved candidate {:?}",
        order.goal
    );
    assert!(map.is_walkable(order.goal));
}

#[test]
fn unreachable_move_preserves_an_active_order() {
    let mut world = World::new();
    let mut map = open_map();
    map.set_blocked_rect(GridPos::new(10, 10), GridPos::new(12, 12));
    let unit = spawn_unit(
        &mut world,
        UnitId(4),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(unit)
        .insert(test_order(&map, Vec2::new(3.5, 2.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(4)], Vec2::new(11.5, 11.5)),
    );

    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(4), RejectReason::Unreachable)]
    );
    assert_eq!(
        world.get::<MoveOrder>(unit).unwrap().waypoints,
        vec![Vec2::new(3.5, 2.5)]
    );
}

#[test]
fn unreachable_route_preserves_an_active_order() {
    let mut world = World::new();
    let mut map = open_map();
    // Wall off the unit from the target region so the target is walkable
    // but no destination slot is reachable.
    map.set_blocked_rect(GridPos::new(0, 3), GridPos::new(23, 3));
    let unit = spawn_unit(
        &mut world,
        UnitId(5),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(unit)
        .insert(test_order(&map, Vec2::new(1.5, 2.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(5)], Vec2::new(5.5, 5.5)),
    );

    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(5), RejectReason::Unreachable)]
    );
    assert_eq!(
        world.get::<MoveOrder>(unit).unwrap().waypoints,
        vec![Vec2::new(1.5, 2.5)]
    );
}

#[test]
fn rejected_unit_keeps_its_destination_reserved_from_accepted_sibling() {
    let mut world = World::new();
    let mut map = open_map();
    let target = Vec2::new(18.5, 18.5);
    let target_cell = map.world_to_cell(target);

    // Unit A is on the right side and can reach the target.
    let a = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(18.5, 2.5),
        UnitKind::Villager,
        12.0,
    );
    // Unit B is walled into a pocket on the left so it is Unreachable, but
    // it already holds a MoveOrder to the target cell. B must keep that goal
    // reserved so the accepted A is not sent to the same cell.
    for cell in [
        GridPos::new(3, 2),
        GridPos::new(2, 3),
        GridPos::new(1, 2),
        GridPos::new(2, 1),
    ] {
        map.set_blocked(cell, true);
    }
    let b = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        12.0,
    );
    world.entity_mut(b).insert(MoveOrder {
        waypoints: vec![map.cell_center(target_cell)],
        next: 0,
        goal: target_cell,
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1), UnitId(2)], target),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(2), RejectReason::Unreachable)]
    );

    // A must not have been assigned the target cell B is already moving to.
    let a_goal = world.get::<MoveOrder>(a).expect("A accepted an order").goal;
    assert_ne!(
        a_goal, target_cell,
        "accepted unit took the rejected unit's goal"
    );

    // B keeps its existing order to the target cell unchanged.
    let b_order = world.get::<MoveOrder>(b).expect("B keeps its order");
    assert_eq!(b_order.goal, target_cell);
    assert_eq!(b_order.waypoints, vec![map.cell_center(target_cell)]);
}

#[test]
fn approach_slots_stay_on_the_immediate_perimeter() {
    let mut map = GridMap::new(8, 8);
    let footprint = Footprint::new(GridPos::new(3, 3), 2, 2);
    for cell in footprint.cells() {
        map.set_blocked(cell, true);
    }

    let used = std::collections::HashSet::new();
    let slots = approach_slots(&map, footprint, &used, 8);

    assert!(!slots.is_empty());
    assert!(slots.iter().all(|slot| map.is_walkable(*slot)));
    assert!(
        slots
            .iter()
            .all(|slot| footprint.is_immediately_adjacent(*slot))
    );
    assert!(slots.iter().all(|slot| !footprint.cells().contains(slot)));
}

#[test]
fn stop_rejects_foreign_units_and_preserves_their_orders() {
    let mut world = World::new();
    let mut map = open_map();
    let enemy = spawn_unit(
        &mut world,
        UnitId(9),
        TeamId(2),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );
    world
        .entity_mut(enemy)
        .insert(test_order(&map, Vec2::new(3.5, 2.5)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        stop_command(TeamId(1), vec![UnitId(9)]),
    );

    assert_eq!(outcome.accepted_units, Vec::<UnitId>::new());
    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(9), RejectReason::NotOwned)]
    );
    assert!(
        world.get::<MoveOrder>(enemy).is_some(),
        "a rejected Stop must not cancel the unit's activity"
    );
}

#[test]
fn move_never_assigns_a_non_commanded_units_existing_goal() {
    let mut world = World::new();
    let mut map = open_map();
    let mover = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    let other = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(4.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    let claimed = GridPos::new(12, 12);
    world
        .entity_mut(other)
        .insert(test_order(&map, map.cell_center(claimed)));
    let claimed_center = map.cell_center(claimed);

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1)], claimed_center),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    let order = world.get::<MoveOrder>(mover).expect("replacement order");
    assert_ne!(
        order.goal, claimed,
        "the mover must not be sent onto a goal a non-commanded unit already claims"
    );
    assert_eq!(
        world.get::<MoveOrder>(other).unwrap().goal,
        claimed,
        "the uncommanded unit keeps its own route"
    );
}

#[test]
fn group_move_never_targets_a_cell_another_commanded_unit_stands_on() {
    let mut world = World::new();
    let mut map = open_map();
    let a = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(10.5, 10.5),
        UnitKind::Villager,
        6.0,
    );
    let b = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(12.5, 10.5),
        UnitKind::Villager,
        6.0,
    );
    let b_cell = GridPos::new(12, 10);

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        move_command(TeamId(1), vec![UnitId(1), UnitId(2)], Vec2::new(12.5, 10.5)),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1), UnitId(2)]);
    assert!(outcome.rejected_units.is_empty());
    let goal_a = world
        .get::<MoveOrder>(a)
        .expect("the displaced unit moves")
        .goal;
    assert_ne!(
        goal_a, b_cell,
        "the mover is never sent onto a cell a commanded sibling still holds"
    );
    assert!(
        world.get::<MoveOrder>(b).is_none(),
        "the unit already standing on the target keeps its place"
    );
}

#[test]
fn spawn_unit_attaches_catalogue_health_and_ready_cooldown() {
    let mut world = World::new();
    let villager = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(1.5, 1.5),
        UnitKind::Villager,
        6.0,
    );
    let spearman = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(2.5, 1.5),
        UnitKind::Spearman,
        6.0,
    );

    let villager_health = world.get::<Health>(villager).expect("villager health");
    assert_eq!(
        villager_health,
        &Health {
            current: 50,
            max: 50
        }
    );
    let spearman_health = world.get::<Health>(spearman).expect("spearman health");
    assert_eq!(
        spearman_health,
        &Health {
            current: 100,
            max: 100
        }
    );
    assert_eq!(
        world.get::<AttackCooldown>(villager),
        Some(&AttackCooldown::default())
    );
    assert_eq!(
        world.get::<AttackCooldown>(spearman),
        Some(&AttackCooldown::default())
    );
}

#[test]
fn reject_reason_variants_stay_append_ordered() {
    // `RejectReason` is append-only and its discriminants leak to Godot as
    // numeric reject codes (`reason as i32`). `SessionLocked` (the newest
    // variant) must keep a higher discriminant than every pre-existing
    // variant and must not reuse `Locked`.
    assert!(RejectReason::SessionLocked as i32 > RejectReason::NoSpawnSpace as i32);
    assert!(RejectReason::SessionLocked as i32 > RejectReason::UnknownUnit as i32);
    assert!(RejectReason::SessionLocked as i32 > RejectReason::NotCombatant as i32);
    assert!(RejectReason::SessionLocked as i32 > RejectReason::TargetMissing as i32);
    assert!(RejectReason::SessionLocked as i32 > RejectReason::InvalidTarget as i32);
    assert_ne!(RejectReason::SessionLocked, RejectReason::Locked);
}

#[test]
fn attack_commands_reject_without_touching_unit_state() {
    // Combat intent installation lands with the combat step; until then
    // Attack/AttackMove are typed rejections that preserve the unit's whole
    // worker/movement/combat state.
    let mut world = World::new();
    let mut map = open_map();
    let spearman = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(2.5, 2.5),
        UnitKind::Spearman,
        6.0,
    );
    let enemy = spawn_unit(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(10.5, 10.5),
        UnitKind::Spearman,
        6.0,
    );
    world.entity_mut(spearman).insert((
        WorkerTask::Idle,
        CombatOrder::Attack {
            target: CombatTarget::Unit(UnitId(2)),
            last_target_cell: None,
        },
        test_order(&map, Vec2::new(3.5, 2.5)),
    ));

    let attack_move = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::AttackMove {
                target: Vec2::new(12.5, 12.5),
            },
        }),
    );
    let attack = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Attack {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            target: CombatTarget::Unit(UnitId(2)),
        },
    );

    assert_eq!(
        attack_move.rejected_units,
        vec![(UnitId(1), RejectReason::InvalidTarget)]
    );
    assert_eq!(attack_move.accepted_units, Vec::<UnitId>::new());
    assert_eq!(
        attack.rejected_units,
        vec![(UnitId(1), RejectReason::InvalidTarget)]
    );
    assert_eq!(attack.accepted_units, Vec::<UnitId>::new());
    assert!(
        world.get::<WorkerTask>(spearman).is_some(),
        "a rejected attack must not clear the task"
    );
    assert!(
        world.get::<MoveOrder>(spearman).is_some(),
        "a rejected attack must not clear the route"
    );
    assert!(
        world.get::<CombatOrder>(spearman).is_some(),
        "a rejected attack must not clear combat intent"
    );
    assert!(world.get::<MoveOrder>(enemy).is_none());
}
