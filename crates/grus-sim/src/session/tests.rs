use std::collections::VecDeque;

use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::{BuildingIndex, step_construction};
use crate::catalog::{UnitKind, unit_spec};
use crate::combat::{AttackCooldown, CombatOrder, CombatTarget, Health, step_combat};
use crate::commands::{
    PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex, apply_player_command,
    spawn_unit,
};
use crate::economy::{TeamEconomy, WorkerTask, step_economy};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, TeamId, UnitId};
use crate::map::{GridMap, GridPos};
use crate::movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, step_movement};
use crate::production::{ProductionJob, ProductionKind, ProductionQueue, step_production};

fn battlefield() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

fn move_command() -> PlayerCommand {
    PlayerCommand::Units(UnitCommand {
        issuer: TeamId(1),
        units: vec![UnitId(1)],
        kind: UnitCommandKind::Move {
            target: Vec2::new(20.5, 44.5),
        },
    })
}

#[test]
fn missing_session_means_playing_and_seed_stays_session_free() {
    let (mut world, mut map) = battlefield();
    // `seed_skirmish` inserts no session: pure-sim tests need no lifecycle
    // boilerplate and the absent session reads as Playing.
    assert!(world.get_resource::<MatchSession>().is_none());
    assert_eq!(active_phase(&world), MatchPhase::Playing);
    assert!(gameplay_active(&world));

    let accepted = apply_player_command(&mut world, &mut map, move_command());
    assert_eq!(accepted.reject, None);
}

#[test]
fn explicit_start_paused_result_reject_commands_with_session_locked() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });

    let locked = apply_player_command(&mut world, &mut map, move_command());
    assert_eq!(locked.reject, Some(RejectReason::SessionLocked));
    assert!(locked.accepted_units.is_empty() && locked.rejected_units.is_empty());

    // Start -> Playing is the explicit start_match transition.
    start_match(&mut world);
    assert_eq!(active_phase(&world), MatchPhase::Playing);
    let accepted = apply_player_command(&mut world, &mut map, move_command());
    assert_eq!(accepted.reject, None);

    set_paused(&mut world, true);
    assert_eq!(active_phase(&world), MatchPhase::Paused);
    let locked = apply_player_command(&mut world, &mut map, move_command());
    assert_eq!(locked.reject, Some(RejectReason::SessionLocked));

    set_paused(&mut world, false);
    resolve_result(&mut world, TeamId(1));
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(1)))
    );
    let locked = apply_player_command(&mut world, &mut map, move_command());
    assert_eq!(locked.reject, Some(RejectReason::SessionLocked));
}

#[test]
fn start_match_and_set_paused_transition_only_their_phases() {
    let mut world = World::new();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    start_match(&mut world);
    assert_eq!(active_phase(&world), MatchPhase::Playing);

    set_paused(&mut world, false);
    assert_eq!(active_phase(&world), MatchPhase::Playing);
    set_paused(&mut world, true);
    assert_eq!(active_phase(&world), MatchPhase::Paused);
    start_match(&mut world);
    assert_eq!(active_phase(&world), MatchPhase::Paused);
    set_paused(&mut world, true);
    assert_eq!(active_phase(&world), MatchPhase::Paused);
    set_paused(&mut world, false);
    assert_eq!(active_phase(&world), MatchPhase::Playing);

    // A settled result survives pause/resume and start_match.
    resolve_result(&mut world, TeamId(2));
    set_paused(&mut world, true);
    set_paused(&mut world, false);
    start_match(&mut world);
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
}

#[test]
fn resolve_result_keeps_the_first_winner() {
    let mut world = World::new();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    resolve_result(&mut world, TeamId(2));
    resolve_result(&mut world, TeamId(1));
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
}

#[test]
fn frozen_movement_collapses_interpolation_and_keeps_the_order() {
    let phases = [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(TeamId(2))),
    ];
    for phase in phases {
        let mut world = World::new();
        let map = GridMap::new(32, 64);
        let unit = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );
        world.entity_mut(unit).insert(MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(20, 2))],
            next: 0,
            goal: GridPos::new(20, 2),
            map_revision: map.revision(),
            last_failed_replan: None,
        });
        world.insert_resource(MatchSession { phase });

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        let position = *world.get::<SimPosition>(unit).unwrap();
        assert_eq!(
            position.previous, position.current,
            "phase {:?}: interpolation must collapse after one gated tick",
            phase
        );
        assert_eq!(
            position.current,
            Vec2::new(2.5, 2.5),
            "phase {:?}: frozen movement must not advance",
            phase
        );
        assert!(
            world.get::<MoveOrder>(unit).is_some(),
            "phase {:?}: the MoveOrder must survive so Resume continues",
            phase
        );

        // Resume continues the kept order normally.
        world.insert_resource(MatchSession {
            phase: MatchPhase::Playing,
        });
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        let position = *world.get::<SimPosition>(unit).unwrap();
        assert!(
            position.current.x > 2.5,
            "phase {:?}: resume must continue the kept MoveOrder",
            phase
        );
        assert_ne!(position.previous, position.current);
    }
}

#[test]
fn frozen_fixed_steps_perform_no_gameplay_mutation() {
    let phases = [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(TeamId(2))),
    ];
    for phase in phases {
        let (mut world, mut map) = battlefield();
        world.insert_resource(MatchSession { phase });

        // A unit-train job mid-flight on the team-1 Town Center.
        let town_center = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .unwrap();
        world.entity_mut(town_center).insert(ProductionQueue {
            jobs: VecDeque::from([ProductionJob {
                kind: ProductionKind::Unit(UnitKind::Villager),
            }]),
            progress_seconds: 1.0,
            blocked: None,
        });
        // A raider mid-cooldown, in range of the Town Center.
        let raider = spawn_unit(
            &mut world,
            UnitId(100),
            TeamId(2),
            Vec2::new(11.5, 49.5),
            UnitKind::Spearman,
            unit_spec(UnitKind::Spearman).speed,
        );
        world.entity_mut(raider).insert((
            CombatOrder::Attack {
                target: CombatTarget::Building(BuildingId(1)),
                last_target_cell: None,
            },
            AttackCooldown(0.4),
        ));
        // A builder standing on its construction slot.
        let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();
        world.entity_mut(builder).insert((
            SimPosition::new(map.cell_center(GridPos::new(20, 40))),
            WorkerTask::ToConstruction {
                building: BuildingId(50),
                slot: GridPos::new(20, 40),
            },
        ));

        let health_before = world.get::<Health>(town_center).unwrap().current;

        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        step_construction(&mut world, SIM_STEP_SECONDS);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);

        assert_eq!(
            world.get::<Health>(town_center).unwrap().current,
            health_before,
            "phase {:?}: combat must not damage outside Playing",
            phase
        );
        assert_eq!(
            world.get::<AttackCooldown>(raider),
            Some(&AttackCooldown(0.4)),
            "phase {:?}: combat must not tick cooldowns outside Playing",
            phase
        );
        let stockpile = &world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
        assert_eq!(
            (stockpile.food, stockpile.wood, stockpile.gold),
            (200, 300, 100),
            "phase {:?}: economy must not mutate outside Playing",
            phase
        );
        assert!(
            matches!(
                world.get::<WorkerTask>(builder),
                Some(WorkerTask::ToConstruction { .. })
            ),
            "phase {:?}: construction must not transition outside Playing",
            phase
        );
        let queue = world.get::<ProductionQueue>(town_center).unwrap();
        assert_eq!(queue.progress_seconds, 1.0);
        assert_eq!(
            queue.jobs.len(),
            1,
            "phase {:?}: production must not complete outside Playing",
            phase
        );
    }
}

#[test]
fn same_step_dual_town_center_destruction_keeps_the_first_result() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    // Ascending UnitId order: the team-2 raider reaches team 1's Town Center
    // first; team 1's counter-raider stands in range of team 2's Town Center
    // in the same tick.
    let raider_two = spawn_unit(
        &mut world,
        UnitId(10),
        TeamId(2),
        Vec2::new(11.5, 49.5),
        UnitKind::Spearman,
        unit_spec(UnitKind::Spearman).speed,
    );
    world.entity_mut(raider_two).insert(CombatOrder::Attack {
        target: CombatTarget::Building(BuildingId(1)),
        last_target_cell: None,
    });
    let raider_one = spawn_unit(
        &mut world,
        UnitId(20),
        TeamId(1),
        Vec2::new(111.5, 49.5),
        UnitKind::Spearman,
        unit_spec(UnitKind::Spearman).speed,
    );
    world.entity_mut(raider_one).insert(CombatOrder::Attack {
        target: CombatTarget::Building(BuildingId(2)),
        last_target_cell: None,
    });

    let town_center_one = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(1))
        .unwrap();
    let town_center_two = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(2))
        .unwrap();
    world.get_mut::<Health>(town_center_one).unwrap().current = 1;
    world.get_mut::<Health>(town_center_two).unwrap().current = 1;

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // First destruction settles the result; the later same-step attacker
    // never strikes, so the second Town Center survives untouched.
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
    assert_eq!(
        world.get::<Health>(town_center_two).unwrap().current,
        1,
        "the later same-step attacker must not act after Result"
    );
    let events = world.resource::<crate::combat::CombatEvents>();
    assert_eq!(events.0.len(), 1);
    assert!(events.0[0].killed);

    // Belt and braces: resolve_result itself never overwrites a settled result.
    resolve_result(&mut world, TeamId(1));
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
}
