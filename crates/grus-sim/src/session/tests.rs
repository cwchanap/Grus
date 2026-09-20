use std::collections::VecDeque;

use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, step_construction};
use crate::catalog::{
    AGE_TWO_COST, Age, BuildingKind, CARRY_LIMIT, ResourceKind, UnitKind, building_spec, unit_spec,
};
use crate::combat::{AttackCooldown, CombatOrder, CombatTarget, Health, step_combat};
use crate::commands::{
    PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex, apply_player_command,
    spawn_unit,
};
use crate::economy::{ResourceSource, ResourceStockpile, TeamEconomy, WorkerTask, step_economy};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, Unit, step_movement};
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

#[test]
fn bounded_defeat_journey_team_two_destroys_team_one_through_the_same_systems() {
    let (mut world, mut map) = live_match();

    run_army_journey(&mut world, &mut map, TeamId(2));
    assert_eq!(
        active_phase(&world),
        MatchPhase::Result(MatchResult(TeamId(2))),
        "team 2's identical journey reads as team 1's defeat"
    );

    assert_result_freezes_and_locks(&mut world, &mut map, TeamId(2));
}

// ---- Bounded passive-opponent journeys -------------------------------------
//
// Shared with combat/tests.rs: the victory journey lives next to combat, the
// defeat journey next to the session lifecycle. Both drive the normal
// authored skirmish through real commands and the real fixed steps — no
// runtime AI, game mode, or debug-grant API — and every wait is a bounded
// event loop that asserts outcomes, never exact tick numbers.

/// Tick budget for every bounded journey loop. A correct journey settles in
/// a few thousand ticks; only a broken system runs out.
const JOURNEY_BUDGET: u32 = 6_000;

/// Full match ticks inspected after Result to prove the freeze holds.
const FREEZE_TICKS: u32 = 40;

/// The authored skirmish under a real session: Start -> start_match().
pub(crate) fn live_match() -> (World, GridMap) {
    let (mut world, map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    start_match(&mut world);
    assert_eq!(active_phase(&world), MatchPhase::Playing);
    (world, map)
}

/// One canonical fixed-step match tick in the production order: combat,
/// movement, economy, construction, production.
fn step_match(world: &mut World, map: &mut GridMap) {
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

/// Bounded event loop for a mid-journey outcome condition.
fn run_until(world: &mut World, map: &mut GridMap, what: &str, condition: impl Fn(&World) -> bool) {
    for _ in 0..JOURNEY_BUDGET {
        if condition(world) {
            return;
        }
        step_match(world, map);
    }
    panic!("{what} did not happen inside {JOURNEY_BUDGET} ticks");
}

fn other_team(team: TeamId) -> TeamId {
    if team == TeamId(1) {
        TeamId(2)
    } else {
        TeamId(1)
    }
}

fn town_center_of(world: &World, team: TeamId) -> BuildingId {
    world
        .resource::<BuildingIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Building>(*entity)
                .is_some_and(|building| {
                    building.team == team && building.kind == BuildingKind::TownCenter
                })
                .then_some(*id)
        })
        .min()
        .unwrap_or_else(|| panic!("team {team:?} has no Town Center"))
}

pub(crate) fn units_of_kind(world: &World, team: TeamId, kind: UnitKind) -> Vec<UnitId> {
    let mut ids: Vec<UnitId> = world
        .resource::<UnitIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Unit>(*entity)
                .is_some_and(|unit| unit.team == team && unit.kind == kind)
                .then_some(*id)
        })
        .collect();
    ids.sort_unstable();
    ids
}

fn villagers_of(world: &World, team: TeamId) -> Vec<UnitId> {
    units_of_kind(world, team, UnitKind::Villager)
}

/// Authored source of `kind` nearest the team's Town Center.
fn nearest_source(world: &World, team: TeamId, kind: ResourceKind) -> ResourceId {
    let town_center_entity = world
        .resource::<BuildingIndex>()
        .entity(town_center_of(world, team))
        .expect("indexed Town Center");
    let home = world
        .get::<Footprint>(town_center_entity)
        .expect("Town Center footprint")
        .center();
    world
        .resource::<crate::economy::ResourceIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            let source = world.get::<ResourceSource>(*entity)?;
            let footprint = world.get::<Footprint>(*entity)?;
            (source.kind == kind).then_some((footprint.center().distance_squared(home), *id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(_, id)| id)
        .expect("an authored source of the requested kind")
}

/// The authored mirrored Barracks anchor from the one base-coordinate table
/// (`MapFixture::team_plan`): clear of every authored start cell, resource
/// node, footprint, and gather slot on both starts.
fn barracks_anchor(team: TeamId) -> GridPos {
    MapFixture::team_plan(team).barracks_anchor
}

fn barracks_of(world: &World, team: TeamId) -> BuildingId {
    world
        .resource::<BuildingIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Building>(*entity)
                .is_some_and(|building| {
                    building.team == team && building.kind == BuildingKind::Barracks
                })
                .then_some(*id)
        })
        .min()
        .expect("a placed Barracks")
}

fn barracks_complete(world: &World, id: BuildingId) -> bool {
    world
        .resource::<BuildingIndex>()
        .entity(id)
        .and_then(|entity| world.get::<Building>(entity))
        .is_some_and(|building| building.construction.complete)
}

/// The full passive-opponent journey for `attacker`: a real gather wave feeds
/// real production; the trained army marches through the combat step and
/// destroys the enemy Town Center, settling the Result. The opponent is
/// passive — nobody drives its units; there is no AI.
pub(crate) fn run_army_journey(world: &mut World, map: &mut GridMap, attacker: TeamId) {
    let defender = other_team(attacker);
    let villagers = villagers_of(world, attacker);
    assert_eq!(villagers.len(), 4, "authored villager complement");

    // Economy: every villager gathers Food until real deposits have grown
    // the stockpile by one full carry wave.
    let food_before = world.resource::<TeamEconomy>().0[&attacker].stockpile.food;
    let food = nearest_source(world, attacker, ResourceKind::Food);
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::Gather {
            issuer: attacker,
            workers: villagers.clone(),
            source: food,
        },
    );
    assert_eq!(outcome.reject, None, "gather accepted");
    assert_eq!(outcome.accepted_units, villagers);
    run_until(world, map, "the gather wave", |world| {
        world.resource::<TeamEconomy>().0[&attacker].stockpile.food
            >= food_before + CARRY_LIMIT * villagers.len() as u32
    });

    // Production: the authored starting wood stockpile pays for a Barracks;
    // one villager builds it while the rest keep gathering.
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::PlaceBuilding {
            issuer: attacker,
            builder: villagers[0],
            kind: BuildingKind::Barracks,
            anchor: barracks_anchor(attacker),
        },
    );
    assert_eq!(outcome.reject, None, "Barracks placement accepted");
    let barracks = barracks_of(world, attacker);
    assert_eq!(
        world.resource::<TeamEconomy>().0[&attacker].stockpile.wood,
        300 - building_spec(BuildingKind::Barracks).cost.wood,
        "placement charges the real catalogue cost once"
    );
    run_until(world, map, "the Barracks construction", |world| {
        barracks_complete(world, barracks)
    });

    // Production: four Spearmen through the real FIFO queue.
    for _ in 0..4 {
        let outcome = apply_player_command(
            world,
            map,
            PlayerCommand::EnqueueUnit {
                issuer: attacker,
                building: barracks,
                kind: UnitKind::Spearman,
            },
        );
        assert_eq!(outcome.reject, None, "Spearman enqueue accepted");
    }
    run_until(world, map, "the Spearman training", |world| {
        units_of_kind(world, attacker, UnitKind::Spearman).len() == 4
    });

    // The last production leg — Age 2. Three villagers switch to Gold while
    // the freed builder returns to Food until the one-time research is
    // affordable; it then runs alongside the march below, so the Result
    // freeze has real age-up progress to hold still.
    let gold = nearest_source(world, attacker, ResourceKind::Gold);
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::Gather {
            issuer: attacker,
            workers: villagers[1..].to_vec(),
            source: gold,
        },
    );
    assert_eq!(outcome.reject, None, "gold retask accepted");
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::Gather {
            issuer: attacker,
            workers: vec![villagers[0]],
            source: food,
        },
    );
    assert_eq!(outcome.reject, None, "food retask accepted");
    run_until(world, map, "the Age 2 affordability", |world| {
        let stockpile = &world.resource::<TeamEconomy>().0[&attacker].stockpile;
        stockpile.food >= AGE_TWO_COST.food && stockpile.gold >= AGE_TWO_COST.gold
    });
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::EnqueueAgeUp {
            issuer: attacker,
            building: town_center_of(world, attacker),
        },
    );
    assert_eq!(outcome.reject, None, "Age 2 research accepted");

    // March + attack: the army attacks the enemy Town Center through the
    // combat step's pursuit and strikes; its destruction settles the Result.
    let enemy_town_center = town_center_of(world, defender);
    let outcome = apply_player_command(
        world,
        map,
        PlayerCommand::Attack {
            issuer: attacker,
            units: units_of_kind(world, attacker, UnitKind::Spearman),
            target: CombatTarget::Building(enemy_town_center),
        },
    );
    assert_eq!(outcome.accepted_units.len(), 4, "the whole army attacks");

    run_until_result(world, map, "the journey");
    assert_eq!(
        active_phase(world),
        MatchPhase::Result(MatchResult(attacker)),
        "the enemy Town Center destruction wins the match"
    );
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(enemy_town_center)
            .is_none(),
        "the enemy Town Center is destroyed atomically"
    );
}

/// Everything the Result freeze contract names, captured for equality.
#[derive(Debug, PartialEq)]
struct FreezeSnapshot {
    economies: Vec<(u8, ResourceStockpile, Age, bool)>,
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
            .map(|(team, state)| (team.0, state.stockpile, state.age, state.age_up_started))
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
            economies,
            positions,
            cooldowns,
            constructions,
            production,
        }
    }
}

/// Post-Result freeze + lock battery: stockpiles, positions, construction,
/// production, cooldowns and age-up progress survive subsequent full match
/// ticks unchanged, and every player command kind rejects `SessionLocked`
/// without mutating anything.
pub(crate) fn assert_result_freezes_and_locks(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
) {
    let frozen = FreezeSnapshot::of(world);

    for _ in 0..FREEZE_TICKS {
        step_match(world, map);
    }
    assert_eq!(
        FreezeSnapshot::of(world),
        frozen,
        "Result must freeze stockpiles, positions, construction, production, cooldowns and age-up progress"
    );

    // Every command kind hits the same session gate: rejected wholesale,
    // no per-unit outcomes, no mutation.
    let worker = *villagers_of(world, issuer)
        .first()
        .expect("villagers survive");
    let town_center = town_center_of(world, issuer);
    let source = nearest_source(world, issuer, ResourceKind::Food);
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
                building: town_center,
                kind: UnitKind::Villager,
            },
        ),
        (
            "attack",
            PlayerCommand::Attack {
                issuer,
                units: units_of_kind(world, issuer, UnitKind::Spearman),
                target: CombatTarget::Building(town_center),
            },
        ),
        (
            "place",
            PlayerCommand::PlaceBuilding {
                issuer,
                builder: worker,
                kind: BuildingKind::Barracks,
                anchor: barracks_anchor(issuer),
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
