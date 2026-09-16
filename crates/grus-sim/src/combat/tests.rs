use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{BuildingKind, ResourceKind, UnitKind, building_spec, unit_spec};
use crate::commands::{
    CommandResult, PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
};
use crate::economy::{Carry, ResourceSource, WorkerTask};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::movement::{SIM_STEP_SECONDS, SimPosition, step_movement};

fn open_map() -> GridMap {
    GridMap::new(32, 64)
}

fn spawn_combatant(
    world: &mut World,
    id: UnitId,
    team: TeamId,
    position: Vec2,
    kind: UnitKind,
) -> Entity {
    spawn_unit(world, id, team, position, kind, unit_spec(kind).speed)
}

fn issue(world: &mut World, map: &mut GridMap, command: PlayerCommand) -> CommandResult {
    apply_player_command(world, map, command)
}

fn attack_command(issuer: TeamId, units: &[UnitId], target: CombatTarget) -> PlayerCommand {
    PlayerCommand::Attack {
        issuer,
        units: units.to_vec(),
        target,
    }
}

fn attack_move_command(issuer: TeamId, units: &[UnitId], target: Vec2) -> PlayerCommand {
    PlayerCommand::Units(UnitCommand {
        issuer,
        units: units.to_vec(),
        kind: UnitCommandKind::AttackMove { target },
    })
}

fn spawn_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    team: TeamId,
    kind: BuildingKind,
    anchor: GridPos,
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
            Health {
                current: spec.max_health,
                max: spec.max_health,
            },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(id, entity);
    entity
}

fn events(world: &World) -> &[CombatEvent] {
    &world.resource::<CombatEvents>().0
}

#[test]
fn melee_attacker_in_range_hits_without_pathing() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );

    let outcome = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);
    assert_eq!(
        world.get::<AttackCooldown>(attacker).unwrap().0,
        1.0,
        "the catalogue cooldown is installed on strike"
    );
    assert!(
        world.get::<MoveOrder>(attacker).is_none(),
        "an in-range attacker never paths"
    );
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0],
        CombatEvent {
            attacker: UnitId(1),
            target: CombatTarget::Unit(UnitId(2)),
            damage: 10,
            position: Vec2::new(6.5, 5.5),
            ranged: false,
            killed: false,
        }
    );
}

#[test]
fn melee_attacker_out_of_range_pursues_without_striking() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    assert!(events(&world).is_empty(), "out of range means no hit");
    assert_eq!(world.get::<Health>(defender).unwrap().current, 100);
    let order = world.get::<MoveOrder>(attacker).expect("pursuit route");
    assert_eq!(order.goal, GridPos::new(7, 4), "pursues the target's cell");
}

#[test]
fn archer_strikes_at_ranged_range_without_moving() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Archer,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(10.5, 5.5),
        UnitKind::Cavalry,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    // Cavalry takes the archer's base damage: its counter bonus names Spearman.
    assert_eq!(world.get::<Health>(defender).unwrap().current, 132);
    assert!(world.get::<MoveOrder>(attacker).is_none());
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].ranged, "archer hits are ranged");
    assert_eq!(recorded[0].damage, 8);
}

#[test]
fn counter_bonus_applies_only_to_the_named_kind() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let cavalry = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Cavalry,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    // Spearman base 10 + counter bonus 10 vs Cavalry.
    assert_eq!(world.get::<Health>(cavalry).unwrap().current, 120);
    assert_eq!(events(&world)[0].damage, 20);
}

#[test]
fn cooldown_gates_repeat_strikes_until_elapsed() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);

    // Events are current-tick-only: the next step is empty and the target is
    // still on cooldown.
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(events(&world).is_empty());
    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);

    let mut restrike_at = None;
    for tick in 1..=40 {
        step_combat(&mut world, &map, SIM_STEP_SECONDS);
        if !events(&world).is_empty() {
            restrike_at = Some(tick);
            break;
        }
    }
    let restrike_at = restrike_at.expect("the cooldown must elapse within ~1s");
    assert!(
        (15..=25).contains(&restrike_at),
        "second strike waits roughly one cooldown, got tick {restrike_at}"
    );
    assert_eq!(world.get::<Health>(defender).unwrap().current, 80);
}

#[test]
fn buildings_take_base_damage_and_stand_at_zero() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(11.5, 48.5),
        UnitKind::Spearman,
    );
    let town_center = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(1),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    // Base damage only: buildings never take a counter bonus.
    assert_eq!(world.get::<Health>(town_center).unwrap().current, 790);
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].target, CombatTarget::Building(BuildingId(1)));
    assert_eq!(recorded[0].damage, 10);
    assert_eq!(recorded[0].position, Vec2::new(12.0, 48.5));
    assert!(!recorded[0].killed);

    // The killing blow depletes Health but the building keeps standing until
    // the destruction transaction (a later task) claims it. The attacker's
    // cooldown from the first strike is cleared so the blow lands now.
    let attacker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.get_mut::<AttackCooldown>(attacker).unwrap().0 = 0.0;
    world.get_mut::<Health>(town_center).unwrap().current = 10;
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(world.get::<Health>(town_center).unwrap().current, 0);
    assert!(events(&world)[0].killed);
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .is_some(),
        "a building at 0 HP is not despawned by combat"
    );
}

#[test]
fn attack_rejects_missing_friendly_and_dead_targets() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    // A friendly teammate exists for the friendly-fire rejection below.
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );
    let dead = spawn_combatant(
        &mut world,
        UnitId(4),
        TeamId(2),
        Vec2::new(7.5, 5.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(dead).unwrap().current = 0;

    let missing = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(9))),
    );
    let enemy_own_team = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(3))),
    );
    let dead_target = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(4))),
    );

    assert_eq!(
        missing.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::TargetMissing)]
    );
    assert_eq!(
        enemy_own_team.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::InvalidTarget)]
    );
    assert_eq!(
        dead_target.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::InvalidTarget)]
    );
    assert!(world.get::<CombatOrder>(attacker).is_none());
}

#[test]
fn attack_move_rejects_unreachable_destinations() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    map.set_blocked(GridPos::new(10, 5), true);

    let blocked = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(10.5, 5.5)),
    );
    let out_of_bounds = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(500.5, 500.5)),
    );

    assert_eq!(
        blocked.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::Unreachable)]
    );
    assert_eq!(
        out_of_bounds.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::Unreachable)]
    );
    assert!(world.get::<CombatOrder>(attacker).is_none());
}

#[test]
fn attack_move_targets_the_nearest_enemy_within_radius() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(7.5, 12.5),
        UnitKind::Villager,
    );
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(2),
        Vec2::new(12.5, 12.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 12),
            target: Some(CombatTarget::Unit(UnitId(2))),
            last_target_cell: Some(GridPos::new(7, 12)),
        }),
        "the nearest enemy in radius is acquired first"
    );
}

#[test]
fn attack_move_tie_breaks_by_lower_stable_id() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(2),
        Vec2::new(8.5, 14.5),
        UnitKind::Villager,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 10.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    match world.get::<CombatOrder>(attacker) {
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Unit(id)),
            ..
        }) => assert_eq!(
            *id,
            UnitId(2),
            "equidistant ties resolve to the lower stable id"
        ),
        other => panic!("expected an acquired unit target, got {other:?}"),
    }
}

#[test]
fn attack_move_ignores_enemies_beyond_the_radius() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(13.5, 12.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    assert!(events(&world).is_empty());
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 12),
            target: None,
            last_target_cell: None,
        }),
        "an enemy beyond ATTACK_MOVE_RADIUS is never acquired"
    );
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("resumes destination");
    assert_eq!(order.goal, GridPos::new(20, 12));
}

#[test]
fn attack_move_acquires_buildings() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(16.5, 48.5),
        UnitKind::Spearman,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 52.5)),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    // Already inside attack range of the footprint: strike immediately, on
    // the footprint's closest point — never its center.
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].target, CombatTarget::Building(BuildingId(1)));
    assert_eq!(recorded[0].position, Vec2::new(16.0, 48.5));
}

fn run_symmetric_duel(left: UnitKind, right: UnitKind) -> Option<UnitKind> {
    let mut world = World::new();
    let mut map = open_map();
    let left_position = Vec2::new(7.5, 16.5);
    let right_position = Vec2::new(14.5, 16.5);
    spawn_combatant(&mut world, UnitId(1), TeamId(1), left_position, left);
    spawn_combatant(&mut world, UnitId(2), TeamId(2), right_position, right);
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_move_command(TeamId(1), &[UnitId(1)], right_position)
        )
        .rejected_units
        .is_empty()
    );
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_move_command(TeamId(2), &[UnitId(2)], left_position)
        )
        .rejected_units
        .is_empty()
    );

    for _ in 0..2_000 {
        step_combat(&mut world, &map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        let left_alive = world.resource::<UnitIndex>().entity(UnitId(1)).is_some();
        let right_alive = world.resource::<UnitIndex>().entity(UnitId(2)).is_some();
        match (left_alive, right_alive) {
            (true, false) => return Some(left),
            (false, true) => return Some(right),
            _ => {}
        }
    }
    None
}

#[test]
fn spearman_survives_the_symmetric_cavalry_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Spearman, UnitKind::Cavalry),
        Some(UnitKind::Spearman)
    );
}

#[test]
fn archer_survives_the_symmetric_spearman_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Archer, UnitKind::Spearman),
        Some(UnitKind::Archer)
    );
}

#[test]
fn cavalry_survives_the_symmetric_archer_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Cavalry, UnitKind::Archer),
        Some(UnitKind::Cavalry)
    );
}

#[test]
fn dead_later_attacker_is_skipped_in_the_same_combat_step() {
    let mut world = World::new();
    let mut map = open_map();
    let left = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let right = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(left).unwrap().current = 10;
    world.get_mut::<Health>(right).unwrap().current = 10;
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2)))
        )
        .accepted_units
            == vec![UnitId(1)]
    );
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_command(TeamId(2), &[UnitId(2)], CombatTarget::Unit(UnitId(1)))
        )
        .accepted_units
            == vec![UnitId(2)]
    );

    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    // The lower stable id acts first and atomically destroys the later
    // attacker, which is then skipped instead of acting on a stale entity.
    assert!(world.resource::<UnitIndex>().entity(UnitId(2)).is_none());
    assert!(world.get::<Health>(right).is_none(), "destroyed atomically");
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1, "the dead later attacker never acted");
    assert_eq!(recorded[0].attacker, UnitId(1));
    assert!(recorded[0].killed);
    assert_eq!(
        world.get::<Health>(left).unwrap().current,
        10,
        "the survivor took no damage this step"
    );

    // Next step the survivor's direct order ends on its dead target.
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(world.get::<CombatOrder>(left).is_none());
}

#[test]
fn direct_attack_ends_when_the_target_dies() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    world.get_mut::<Health>(defender).unwrap().current = 10;

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);

    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        world.get::<CombatOrder>(attacker).is_none(),
        "a direct attack ends on its dead target"
    );
}

#[test]
fn attack_move_clears_the_dead_target_and_resumes_destination() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Archer,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    world.get_mut::<Health>(defender).unwrap().current = 8;

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 5.5)),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);

    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 5),
            target: None,
            last_target_cell: None,
        }),
        "the dead target is cleared"
    );
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("resumes destination");
    assert_eq!(order.goal, GridPos::new(20, 5));
}

#[test]
fn destroy_unit_releases_worker_state_and_index_entry() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let worker = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    let source = crate::economy::spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Wood,
        GridPos::new(9, 5),
        100,
    );
    world
        .get_mut::<ResourceSource>(source)
        .unwrap()
        .assigned_worker = Some(UnitId(2));
    world.entity_mut(worker).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        Carry::Holding {
            kind: crate::catalog::ResourceKind::Wood,
            amount: std::num::NonZeroU32::new(5).unwrap(),
        },
    ));
    world.get_mut::<Health>(worker).unwrap().current = 10;

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    assert!(events(&world)[0].killed);
    assert!(
        world.resource::<UnitIndex>().entity(UnitId(2)).is_none(),
        "the dead worker leaves the stable index"
    );
    assert!(
        world.get::<Health>(worker).is_none(),
        "the entity is despawned"
    );
    assert_eq!(
        world.get::<ResourceSource>(source).unwrap().assigned_worker,
        None,
        "the source reservation dies with the worker"
    );
}

#[test]
fn unit_pursuit_refreshes_only_when_the_target_moves_or_the_route_ends() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 5.5),
        UnitKind::Spearman,
    );
    let target = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    let initial_paths = map.path_call_count();
    assert!(initial_paths > 0, "the first pursuit paths once");
    let goal = world.get::<MoveOrder>(attacker).unwrap().goal;

    // Same target cell and an active route: pursuit is reused, no new A*.
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(map.path_call_count(), initial_paths);
    assert_eq!(world.get::<MoveOrder>(attacker).unwrap().goal, goal);

    // A target cell change triggers a replan.
    world
        .entity_mut(target)
        .insert(SimPosition::new(Vec2::new(8.5, 9.5)));
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        map.path_call_count() > initial_paths,
        "a moved target replans pursuit"
    );

    // A route that ended is reassigned even against a stationary target.
    let paths_before_end = map.path_call_count();
    world.entity_mut(attacker).remove::<MoveOrder>();
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(
        map.path_call_count() > paths_before_end,
        "an ended pursuit route is reassigned"
    );
}

#[test]
fn building_pursuit_repaths_only_when_the_route_ends_and_picks_the_nearest_perimeter() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(8.5, 48.5),
        UnitKind::Spearman,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(1),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );
    step_combat(&mut world, &map, SIM_STEP_SECONDS);

    let goal = world
        .get::<MoveOrder>(attacker)
        .expect("pursuit route")
        .goal;
    assert_eq!(
        goal,
        GridPos::new(11, 48),
        "the nearest walkable immediate-perimeter cell wins at the combat call site"
    );
    let paths = map.path_call_count();
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(
        map.path_call_count(),
        paths,
        "a static building never triggers a pursuit replan while the route is active"
    );

    world.entity_mut(attacker).remove::<MoveOrder>();
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
    assert!(map.path_call_count() > paths, "a ended route is reassigned");
    assert_eq!(world.get::<MoveOrder>(attacker).unwrap().goal, goal);
}
