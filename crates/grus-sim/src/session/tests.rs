use std::collections::VecDeque;

use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, step_construction};
use crate::catalog::{BuildingKind, UnitKind, unit_spec};
use crate::combat::{AttackCooldown, CombatOrder, CombatTarget, Health, step_combat};
use crate::commands::{
    PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex, apply_player_command,
    spawn_unit,
};
use crate::economy::{ResourceStockpile, TeamEconomy, WorkerTask, step_economy};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{GridMap, GridPos};
use crate::movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, step_movement};
use crate::production::{ProductionJob, ProductionKind, ProductionQueue, step_production};
use crate::waves::{WaveState, step_waves};

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
            UnitKind::Worker,
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

        // A unit-train job mid-flight on the team-1 Research Center.
        let research_center = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .unwrap();
        world.entity_mut(research_center).insert(ProductionQueue {
            jobs: VecDeque::from([ProductionJob {
                kind: ProductionKind::Unit(UnitKind::Worker),
            }]),
            progress_seconds: 1.0,
            blocked: None,
        });
        // A raider mid-cooldown, in range of the Research Center.
        let raider = spawn_unit(
            &mut world,
            UnitId(100),
            TeamId(2),
            Vec2::new(11.5, 49.5),
            UnitKind::Guard,
            unit_spec(UnitKind::Guard).speed,
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

        let health_before = world.get::<Health>(research_center).unwrap().current;

        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        step_construction(&mut world, SIM_STEP_SECONDS);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);

        assert_eq!(
            world.get::<Health>(research_center).unwrap().current,
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
            (stockpile.food, stockpile.scrap, stockpile.coal),
            (200, 300, 150),
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
        let queue = world.get::<ProductionQueue>(research_center).unwrap();
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
fn same_step_core_destruction_keeps_the_first_result_and_stops_later_combat() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    let raider = spawn_unit(
        &mut world,
        UnitId(10),
        TeamId(2),
        Vec2::new(61.5, 49.5),
        UnitKind::Guard,
        unit_spec(UnitKind::Guard).speed,
    );
    world.entity_mut(raider).insert(CombatOrder::Attack {
        target: CombatTarget::Building(BuildingId(1)),
        last_target_cell: None,
    });
    let later_attacker = spawn_unit(
        &mut world,
        UnitId(20),
        TeamId(1),
        Vec2::new(30.5, 30.5),
        UnitKind::Guard,
        unit_spec(UnitKind::Guard).speed,
    );
    world
        .entity_mut(later_attacker)
        .insert(CombatOrder::Attack {
            target: CombatTarget::Unit(UnitId(30)),
            last_target_cell: None,
        });
    let later_target = spawn_unit(
        &mut world,
        UnitId(30),
        TeamId(2),
        Vec2::new(30.5, 30.5),
        UnitKind::Swarmer,
        unit_spec(UnitKind::Swarmer).speed,
    );
    let core = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(1))
        .unwrap();
    world.get_mut::<Health>(core).unwrap().current = 1;
    world.get_mut::<Health>(later_target).unwrap().current = 1;

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
    assert_eq!(
        world.get::<Health>(later_target).unwrap().current,
        1,
        "the later same-step attacker must not act after Result"
    );
    let events = world.resource::<crate::combat::CombatEvents>();
    assert_eq!(events.0.len(), 1);
    assert!(events.0[0].killed);
    assert_eq!(events.0[0].target, CombatTarget::Building(BuildingId(1)));

    resolve_result(&mut world, TeamId(1));
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2)))
    );
}

#[test]
fn bounded_wave_journey_destroys_the_only_core_and_freezes_the_session() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    world.get_resource_mut::<WaveState>().unwrap().elapsed = 299.95;
    run_until_result(&mut world, &mut map, "the alien wave");

    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2))),
        "Team-2 aliens defeat the human by destroying the only core"
    );
    assert_eq!(
        world.resource::<BuildingIndex>().iter().count(),
        0,
        "the only Research Center was destroyed"
    );
    assert!(world.resource::<WaveState>().next_wave >= 2);
    assert!(
        world
            .resource::<crate::combat::CombatEvents>()
            .0
            .iter()
            .any(|event| { event.target == CombatTarget::Building(BuildingId(1)) && event.killed })
    );
    assert_result_freezes_and_locks(&mut world, &mut map, TeamId(1));
}

// ---- Bounded wave journey --------------------------------------------------

/// Tick budget for every bounded journey loop. A correct journey settles in
/// a few thousand ticks; only a broken system runs out.
const JOURNEY_BUDGET: u32 = 6_000;

/// Full match ticks inspected after Result to prove the freeze holds.
const FREEZE_TICKS: u32 = 40;

/// One canonical fixed-step match tick in the production order: waves,
/// combat, movement, economy, construction, production.
fn step_match(world: &mut World, map: &mut GridMap) {
    step_waves(world, map, SIM_STEP_SECONDS);
    step_combat(world, map, SIM_STEP_SECONDS);
    step_movement(world, map, SIM_STEP_SECONDS);
    step_economy(world, map, SIM_STEP_SECONDS);
    step_construction(world, SIM_STEP_SECONDS);
    step_production(world, map, SIM_STEP_SECONDS);
}

fn is_result(world: &World) -> bool {
    matches!(active_phase(world), MatchPhase::Result(_))
}

/// Bounded event loop: full match ticks until the session settles on a
/// Result. Returns the resolving tick; panics outside the budget.
fn run_until_result(world: &mut World, map: &mut GridMap, what: &str) -> u32 {
    let mut resolved_at = None;
    for tick in 0..JOURNEY_BUDGET {
        step_match(world, map);
        if is_result(world) {
            resolved_at = Some(tick);
            break;
        }
    }
    resolved_at.unwrap_or_else(|| panic!("{what} did not resolve inside {JOURNEY_BUDGET} ticks"))
}

/// Everything the Result freeze contract names, captured for equality.
#[derive(Debug, PartialEq)]
struct FreezeSnapshot {
    wave: WaveState,
    economies: Vec<(u8, ResourceStockpile)>,
    positions: Vec<(UnitId, Vec2, Vec2)>,
    cooldowns: Vec<(UnitId, f32)>,
    constructions: Vec<(BuildingId, f32, bool, Option<UnitId>)>,
    production: Vec<(BuildingId, usize, f32, Option<RejectReason>)>,
}

impl FreezeSnapshot {
    fn of(world: &World) -> Self {
        let mut economies: Vec<_> = world
            .resource::<TeamEconomy>()
            .0
            .iter()
            .map(|(team, state)| (team.0, state.stockpile))
            .collect();
        economies.sort_by_key(|entry| entry.0);

        let mut positions: Vec<_> = world
            .resource::<UnitIndex>()
            .iter()
            .filter_map(|(id, entity)| {
                world
                    .get::<SimPosition>(*entity)
                    .map(|position| (*id, position.previous, position.current))
            })
            .collect();
        positions.sort_by_key(|entry| entry.0);

        let mut cooldowns: Vec<_> = world
            .resource::<UnitIndex>()
            .iter()
            .filter_map(|(id, entity)| {
                world
                    .get::<AttackCooldown>(*entity)
                    .map(|cooldown| (*id, cooldown.0))
            })
            .collect();
        cooldowns.sort_by_key(|entry| entry.0);

        let mut constructions: Vec<_> = world
            .resource::<BuildingIndex>()
            .iter()
            .filter_map(|(id, entity)| {
                world.get::<Building>(*entity).map(|building| {
                    (
                        *id,
                        building.construction.progress_seconds,
                        building.construction.complete,
                        building.construction.active_builder,
                    )
                })
            })
            .collect();
        constructions.sort_by_key(|entry| entry.0);

        let mut production: Vec<_> = world
            .resource::<BuildingIndex>()
            .iter()
            .filter_map(|(id, entity)| {
                world
                    .get::<ProductionQueue>(*entity)
                    .map(|queue| (*id, queue.jobs.len(), queue.progress_seconds, queue.blocked))
            })
            .collect();
        production.sort_by_key(|entry| entry.0);

        Self {
            wave: *world.resource::<WaveState>(),
            economies,
            positions,
            cooldowns,
            constructions,
            production,
        }
    }
}

/// Post-Result freeze + lock battery: stockpiles, positions, construction,
/// production and cooldowns survive subsequent full match
/// ticks unchanged, and every player command kind rejects `SessionLocked`
/// without mutating anything.
pub(crate) fn assert_result_freezes_and_locks(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
) {
    let frozen = FreezeSnapshot::of(world);

    // The surviving live wave and its elapsed clock make the freeze proof
    // non-vacuous; destroying the sole core also removes its production queue.
    assert!(
        frozen.wave.elapsed >= 300.0 && !frozen.positions.is_empty(),
        "the wave clock or surviving units are absent at Result"
    );

    for _ in 0..FREEZE_TICKS {
        step_match(world, map);
    }
    assert_eq!(
        FreezeSnapshot::of(world),
        frozen,
        "Result must freeze stockpiles, positions, construction, production and cooldowns"
    );

    // Every command kind hits the same session gate: rejected wholesale,
    // no per-unit outcomes, no mutation.
    let worker = UnitId(1);
    let research_center = BuildingId(1);
    let source = ResourceId(1);
    let commands: [(&str, PlayerCommand); 5] = [
        (
            "move",
            PlayerCommand::Units(UnitCommand {
                issuer,
                units: vec![worker],
                kind: UnitCommandKind::Move {
                    target: Vec2::new(64.5, 48.5),
                },
            }),
        ),
        (
            "gather",
            PlayerCommand::Gather {
                issuer,
                workers: vec![worker],
                source,
            },
        ),
        (
            "enqueue",
            PlayerCommand::EnqueueUnit {
                issuer,
                building: research_center,
                kind: UnitKind::Worker,
            },
        ),
        (
            "attack",
            PlayerCommand::Attack {
                issuer,
                units: vec![worker],
                target: CombatTarget::Building(research_center),
            },
        ),
        (
            "place",
            PlayerCommand::PlaceBuilding {
                issuer,
                builder: worker,
                kind: BuildingKind::Barracks,
                anchor: MapFixture::barracks_anchor(issuer),
            },
        ),
    ];
    for (name, command) in commands {
        let outcome = apply_player_command(world, map, command);
        assert_eq!(
            outcome.reject,
            Some(RejectReason::SessionLocked),
            "{name} after Result must be session-locked"
        );
        assert!(
            outcome.accepted_units.is_empty() && outcome.rejected_units.is_empty(),
            "{name} after Result must not produce per-unit outcomes"
        );
    }
    assert_eq!(
        FreezeSnapshot::of(world),
        frozen,
        "session-locked commands must not mutate anything"
    );
}
