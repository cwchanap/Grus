use bevy::prelude::{Entity, Or, With, World};

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    ANALYZE_COST, ANALYZE_RESEARCH_GAIN, BuildingKind, MIN_SURFACE_EFFICIENCY, RoomKind,
    building_spec, room_spec,
};
use crate::commands::{
    CommandResult, PlayerCommand, RejectReason, UnitIndex, apply_player_command,
};
use crate::economy::{ResourceIndex, ResourceStockpile, TeamEconomy};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::heat::HeatState;
use crate::ids::{BuildingId, IdAllocator, TeamId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{SIM_STEP_SECONDS, Unit};
use crate::session::{MatchPhase, MatchResult, MatchSession};
use crate::underground::{Room, Underground, UndergroundLevel};

fn battlefield() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

fn points(world: &World) -> f64 {
    world.resource::<ResearchState>().points
}

/// `seed_skirmish` inserts a default `ResearchState` beside the wave clock
/// and heat state; the fresh match starts at zero research.
#[test]
fn seed_skirmish_inserts_a_zero_research_state() {
    let (world, _map) = battlefield();
    assert_eq!(points(&world), 0.0);
    assert_eq!(research_points(&world), 0.0);
}

/// A world without `ResearchState` reads neutral zero points, a full-rate
/// surface source and every zero-percent building gate open — the same
/// neutral-default pattern as a missing `HeatState`. The research-gated
/// towers read locked at the zero read, exactly like a fresh match.
#[test]
fn missing_research_state_reads_zero() {
    let world = World::new();
    assert_eq!(research_points(&world), 0.0);
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND);
    for kind in BuildingKind::ALL {
        let gated = building_spec(kind).research_unlock_percent > 0;
        assert_eq!(
            building_unlocked(&world, kind),
            !gated,
            "{kind:?} at the neutral zero read"
        );
    }
}

/// A warm surface runs the catalogue base rate: 2 points/s at full
/// efficiency, so one 50 ms fixed step banks exactly 0.1.
#[test]
fn warm_world_advances_at_the_base_rate() {
    let (mut world, _map) = battlefield();
    assert_eq!(research_rate(&world), 2.0);

    step_research(&mut world, SIM_STEP_SECONDS);

    let expected = RESEARCH_BASE_PER_SECOND * f64::from(SIM_STEP_SECONDS);
    assert!(
        (points(&world) - expected).abs() < 1e-9,
        "one warm tick must bank ~0.1 points, got {}",
        points(&world)
    );
}

/// Cold scales the same surface term every heat consumer sees: a severe
/// deficit floors efficiency at the catalogue 25% minimum (0.5 points/s),
/// a mid deficit scales linearly between floor and full.
#[test]
fn cold_world_advances_at_the_efficiency_floor() {
    let (mut world, _map) = battlefield();
    world.insert_resource(HeatState {
        supply: 0,
        demand: 10,
        burn_remainder: 0.0,
    });
    assert_eq!(
        research_rate(&world),
        RESEARCH_BASE_PER_SECOND * f64::from(MIN_SURFACE_EFFICIENCY)
    );

    step_research(&mut world, SIM_STEP_SECONDS);
    let expected =
        RESEARCH_BASE_PER_SECOND * f64::from(MIN_SURFACE_EFFICIENCY) * f64::from(SIM_STEP_SECONDS);
    assert!(
        (points(&world) - expected).abs() < 1e-9,
        "one floored tick must bank ~0.025 points, got {}",
        points(&world)
    );

    world.insert_resource(HeatState {
        supply: 5,
        demand: 10,
        burn_remainder: 0.0,
    });
    assert_eq!(research_rate(&world), 1.0);
}

/// Start, Paused and a settled Result freeze passive progress; only
/// Playing — explicit or the session-free fixture default — advances.
#[test]
fn step_research_freezes_outside_playing() {
    for phase in [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(TeamId(2))),
    ] {
        let (mut world, _map) = battlefield();
        world.insert_resource(MatchSession { phase });
        step_research(&mut world, SIM_STEP_SECONDS);
        assert_eq!(points(&world), 0.0, "{phase:?} must freeze research");
    }

    let (mut world, _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    step_research(&mut world, SIM_STEP_SECONDS);
    assert!(points(&world) > 0.0, "Playing must advance research");

    let (mut sessionless, _map) = battlefield();
    step_research(&mut sessionless, SIM_STEP_SECONDS);
    assert!(
        points(&sessionless) > 0.0,
        "a missing session reads as Playing"
    );
}

/// `add_research` clamps at the catalogue target and resolves Team-1
/// Victory through the shared session path — the one helper passive ticks
/// and the Analyze command share. Once settled, the winner never moves.
#[test]
fn add_research_clamps_at_the_target_and_resolves_victory() {
    let (mut world, _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    add_research(&mut world, f64::from(RESEARCH_TARGET_POINTS) - 10.0);
    assert_eq!(points(&world), 8_990.0);
    assert_eq!(world.resource::<MatchSession>().phase, MatchPhase::Playing);

    // The final grant overshoots and lands exactly on the cap.
    add_research(&mut world, 50.0);
    assert_eq!(points(&world), f64::from(RESEARCH_TARGET_POINTS));
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TeamId(1)))
    );

    // A settled Result is never overwritten by a later grant.
    add_research(&mut world, ANALYZE_RESEARCH_GAIN);
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TeamId(1)))
    );
}

/// The overwrite guard must discriminate a settled *Team-2* Result: a
/// Team-1 grant that reaches the target still calls `resolve_result`, but
/// the shared session helper refuses a settled session — the loser can
/// never rewrite the recorded winner post-mortem. (Reaching the target
/// here proves the grant exercised the resolve path; the phase assert is
/// what fails if the guard rewrites.)
#[test]
fn add_research_never_overwrites_a_settled_enemy_victory() {
    let (mut world, _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Result(MatchResult(TeamId(2))),
    });

    add_research(&mut world, f64::from(RESEARCH_TARGET_POINTS));

    assert_eq!(
        points(&world),
        f64::from(RESEARCH_TARGET_POINTS),
        "the grant itself still banks and clamps"
    );
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TeamId(2))),
        "a settled Team-2 Result is never rewritten to Team 1"
    );
}

/// A passive step that crosses the target resolves Victory inside the
/// step — no extra command needed, and the Result gate then freezes
/// further gains.
#[test]
fn passive_step_crossing_the_target_resolves_victory() {
    let (mut world, _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    world.resource_mut::<ResearchState>().points = f64::from(RESEARCH_TARGET_POINTS) - 0.05;

    step_research(&mut world, SIM_STEP_SECONDS); // +0.1 at full rate

    assert_eq!(points(&world), f64::from(RESEARCH_TARGET_POINTS));
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TeamId(1)))
    );

    step_research(&mut world, SIM_STEP_SECONDS);
    assert_eq!(
        points(&world),
        f64::from(RESEARCH_TARGET_POINTS),
        "a settled Result freezes the accumulator"
    );
}

/// Thresholds derive from `RESEARCH_TARGET_POINTS × percent / 100` in the
/// one helper — 25% → 2,250, 50% → 4,500, 75% → 6,750, 100% → 9,000 — and
/// every catalogue kind is always-available except the advanced towers:
/// FlameTower at 25%, CrystalLance at 50%.
#[test]
fn unlock_thresholds_derive_from_the_catalogue_percent() {
    assert_eq!(unlock_points(0), 0);
    assert_eq!(unlock_points(25), 2_250);
    assert_eq!(unlock_points(50), 4_500);
    assert_eq!(unlock_points(75), 6_750);
    assert_eq!(unlock_points(100), RESEARCH_TARGET_POINTS);

    let (world, _map) = battlefield();
    for kind in BuildingKind::ALL {
        let expected = match kind {
            BuildingKind::FlameTower => 25,
            BuildingKind::CrystalLance => 50,
            _ => 0,
        };
        assert_eq!(
            building_spec(kind).research_unlock_percent,
            expected,
            "{kind:?} research gate"
        );
        assert_eq!(
            building_unlocked(&world, kind),
            expected == 0,
            "{kind:?} at zero research"
        );
    }
}

/// Both tower gates flip on live points alone: strictly below each
/// derived threshold the kind stays locked; exactly at it the kind is
/// open. Milestones are monotonic — nothing latches or repeats.
#[test]
fn tower_unlocks_flip_at_their_derived_thresholds() {
    let (mut world, _map) = battlefield();

    // Just below 25% both advanced towers stay locked.
    world.resource_mut::<ResearchState>().points = f64::from(unlock_points(25)) - 0.5;
    assert!(!building_unlocked(&world, BuildingKind::FlameTower));
    assert!(!building_unlocked(&world, BuildingKind::CrystalLance));

    // Exactly at 25% opens the Flame Tower only.
    world.resource_mut::<ResearchState>().points = f64::from(unlock_points(25));
    assert!(building_unlocked(&world, BuildingKind::FlameTower));
    assert!(!building_unlocked(&world, BuildingKind::CrystalLance));

    // Just below 50% the Crystal Lance still stays locked.
    world.resource_mut::<ResearchState>().points = f64::from(unlock_points(50)) - 0.5;
    assert!(!building_unlocked(&world, BuildingKind::CrystalLance));

    // Exactly at 50% both towers are open.
    world.resource_mut::<ResearchState>().points = f64::from(unlock_points(50));
    assert!(building_unlocked(&world, BuildingKind::FlameTower));
    assert!(building_unlocked(&world, BuildingKind::CrystalLance));
}

/// Bridge-style reset: progress banked before the clear can never leak
/// into the reseeded match — the resource is dropped with the rest of the
/// gameplay state and `seed_skirmish` inserts a fresh zero state.
#[test]
fn clear_and_reseed_zeroes_research() {
    let (mut world, _map) = battlefield();
    add_research(&mut world, 500.0);
    assert_eq!(points(&world), 500.0);

    // Mirror `clear_gameplay_world`: gameplay entities and the seed-owned
    // indexes/state die, then `seed_skirmish` re-inserts a fresh world.
    let stale: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Unit>, With<Building>)>>()
        .iter(&world)
        .collect();
    for entity in stale {
        world.despawn(entity);
    }
    world.remove_resource::<UnitIndex>();
    world.remove_resource::<BuildingIndex>();
    world.remove_resource::<ResourceIndex>();
    world.remove_resource::<IdAllocator>();
    world.remove_resource::<ResearchState>();
    assert_eq!(
        research_points(&world),
        0.0,
        "absence must read zero between clear and reseed"
    );

    let fixture = MapFixture::battlefield();
    let mut fresh_map = fixture.map.clone();
    seed_skirmish(&mut world, &mut fresh_map, &fixture);
    assert_eq!(points(&world), 0.0);
    assert_eq!(research_points(&world), 0.0);
}

const TEAM: TeamId = TeamId(1);
const ENEMY: TeamId = TeamId(2);
/// The seeded skirmish core: `seed_skirmish` allocates `BuildingId(1)` to
/// Team 1's completed Research Center.
const RESEARCH_CENTER: BuildingId = BuildingId(1);

fn analyze(world: &mut World, map: &mut GridMap, building: BuildingId) -> CommandResult {
    apply_player_command(
        world,
        map,
        PlayerCommand::Analyze {
            issuer: TEAM,
            building,
        },
    )
}

fn stockpile(world: &World, team: TeamId) -> ResourceStockpile {
    world.resource::<TeamEconomy>().0[&team].stockpile
}

fn set_crystals(world: &mut World, team: TeamId, crystals: u32) {
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&team)
        .unwrap()
        .stockpile
        .crystals = crystals;
}

/// A finished building registered in `BuildingIndex` with its footprint
/// cells blocked — the same helper shape `production::tests` uses to stand
/// up buildings without the placement command path.
fn complete_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
    team: TeamId,
) {
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
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(id, entity);
}

/// An accepted Analyze converts exactly `ANALYZE_COST` (10 Crystals) into
/// `ANALYZE_RESEARCH_GAIN` points in one immediate command — no queue, no
/// timer — spending a stockpile that covers the price down to zero.
#[test]
fn analyze_with_exactly_ten_crystals_succeeds() {
    let (mut world, mut map) = battlefield();
    set_crystals(&mut world, TEAM, ANALYZE_COST.crystals);

    let result = analyze(&mut world, &mut map, RESEARCH_CENTER);

    assert_eq!(result.reject, None);
    assert_eq!(
        stockpile(&world, TEAM),
        ResourceStockpile {
            food: 200,
            scrap: 300,
            coal: 150,
            crystals: 0,
        },
        "only the 10 Crystal leg is spent"
    );
    assert_eq!(research_points(&world), ANALYZE_RESEARCH_GAIN);
}

/// A short stockpile rejects `InsufficientResources` through the atomic
/// `try_spend`: nothing is deducted — not even the affordable currencies —
/// and no points are banked.
#[test]
fn analyze_short_on_crystals_rejects_and_spends_nothing() {
    let (mut world, mut map) = battlefield();
    set_crystals(&mut world, TEAM, ANALYZE_COST.crystals - 1);
    let before = stockpile(&world, TEAM);

    let result = analyze(&mut world, &mut map, RESEARCH_CENTER);

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world, TEAM), before, "a rejected Analyze spent");
    assert_eq!(
        research_points(&world),
        0.0,
        "a rejected Analyze banked points"
    );
}

/// The commandable-building chain answers foreign, wrong-kind and absent
/// targets before any spend: every rejection leaves the stockpile and the
/// point total untouched.
#[test]
fn analyze_rejects_foreign_wrong_kind_and_missing_without_spending() {
    let (mut world, mut map) = battlefield();
    set_crystals(&mut world, TEAM, 100);
    let before = stockpile(&world, TEAM);
    // A foreign core answers NotOwned in the full-information world (a
    // hidden one would answer BuildingMissing through the same gate).
    complete_building(
        &mut world,
        &mut map,
        BuildingId(50),
        BuildingKind::ResearchCenter,
        GridPos::new(10, 10),
        ENEMY,
    );
    // A friendly non-core building is the wrong kind.
    complete_building(
        &mut world,
        &mut map,
        BuildingId(51),
        BuildingKind::Depot,
        GridPos::new(20, 20),
        TEAM,
    );

    let foreign = analyze(&mut world, &mut map, BuildingId(50));
    let wrong_kind = analyze(&mut world, &mut map, BuildingId(51));
    let missing = analyze(&mut world, &mut map, BuildingId(999));

    assert_eq!(foreign.reject, Some(RejectReason::NotOwned));
    assert_eq!(wrong_kind.reject, Some(RejectReason::WrongProducer));
    assert_eq!(missing.reject, Some(RejectReason::BuildingMissing));
    assert_eq!(stockpile(&world, TEAM), before, "a rejected Analyze spent");
    assert_eq!(
        research_points(&world),
        0.0,
        "a rejected Analyze banked points"
    );
}

/// A session outside Playing locks Analyze like every other command — the
/// single `gameplay_active` gate rejects before validation or spend, so
/// nothing is deducted and nothing is banked.
#[test]
fn analyze_rejects_outside_playing_without_spending() {
    for phase in [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(ENEMY)),
    ] {
        let (mut world, mut map) = battlefield();
        set_crystals(&mut world, TEAM, 100);
        let before = stockpile(&world, TEAM);
        world.insert_resource(MatchSession { phase });

        let result = analyze(&mut world, &mut map, RESEARCH_CENTER);

        assert_eq!(
            result.reject,
            Some(RejectReason::SessionLocked),
            "{phase:?} must lock Analyze"
        );
        assert_eq!(stockpile(&world, TEAM), before);
        assert_eq!(research_points(&world), 0.0);
    }
}

/// Sequential Analyze commands each spend and bank independently until the
/// stockpile runs dry; the first unaffordable one rejects while the
/// earlier grants stand — one immediate command at a time, never a job.
#[test]
fn repeated_analyze_spends_and_banks_each_time() {
    let (mut world, mut map) = battlefield();
    set_crystals(&mut world, TEAM, 25); // two full payments, short of three

    for index in 1..=2 {
        let result = analyze(&mut world, &mut map, RESEARCH_CENTER);
        assert_eq!(result.reject, None, "Analyze #{index} rejected");
    }
    assert_eq!(stockpile(&world, TEAM).crystals, 5);
    assert_eq!(research_points(&world), 2.0 * ANALYZE_RESEARCH_GAIN);

    let result = analyze(&mut world, &mut map, RESEARCH_CENTER);
    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world, TEAM).crystals, 5);
    assert_eq!(research_points(&world), 2.0 * ANALYZE_RESEARCH_GAIN);
}

/// One Analyze carries the point total across each milestone threshold —
/// 25% (2,250), 50% (4,500) and 75% (6,750) — seeded just below so the
/// single +90 grant is what crosses the derived boundary.
#[test]
fn analyze_crosses_each_milestone_threshold() {
    for percent in [25, 50, 75] {
        let threshold = f64::from(unlock_points(percent));
        let (mut world, mut map) = battlefield();
        set_crystals(&mut world, TEAM, ANALYZE_COST.crystals);
        world.resource_mut::<ResearchState>().points = threshold - 1.0;
        assert!(research_points(&world) < threshold);

        let result = analyze(&mut world, &mut map, RESEARCH_CENTER);

        assert_eq!(result.reject, None, "{percent}% Analyze rejected");
        assert!(
            research_points(&world) >= threshold,
            "{percent}%: {} must reach {threshold}",
            research_points(&world)
        );
    }
}

/// Analyze at the cap banks exactly `RESEARCH_TARGET_POINTS` — the
/// overshoot clamps, the Playing session resolves Team-1 Victory inside
/// this same command phase, and the next Analyze queued in the batch is
/// already `SessionLocked` with its Crystals unspent.
#[test]
fn analyze_at_the_target_clamps_resolves_victory_and_locks_the_batch() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    set_crystals(&mut world, TEAM, 30);
    world.resource_mut::<ResearchState>().points = f64::from(RESEARCH_TARGET_POINTS) - 10.0;

    let first = analyze(&mut world, &mut map, RESEARCH_CENTER);

    assert_eq!(first.reject, None);
    assert_eq!(stockpile(&world, TEAM).crystals, 20);
    assert_eq!(research_points(&world), f64::from(RESEARCH_TARGET_POINTS));
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TEAM))
    );

    // The queued-behind command sees the settled Result: rejected with
    // nothing spent, and the winner never moves.
    let second = analyze(&mut world, &mut map, RESEARCH_CENTER);
    assert_eq!(second.reject, Some(RejectReason::SessionLocked));
    assert_eq!(stockpile(&world, TEAM).crystals, 20);
    assert_eq!(research_points(&world), f64::from(RESEARCH_TARGET_POINTS));
    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(MatchResult(TEAM))
    );
}

/// Analyze is a Team-1-only command like the Team-1-only placement
/// kinds: a foreign issuer answers `Locked` before any building
/// resolution — a Team-2 core can never spend Team-2 Crystals into
/// Team-1's point total and hand it a Victory.
#[test]
fn analyze_rejects_non_team_one_issuers() {
    let (mut world, mut map) = battlefield();
    world.resource_mut::<TeamEconomy>().insert_team(
        ENEMY,
        ResourceStockpile {
            crystals: 100,
            ..ResourceStockpile::default()
        },
    );
    complete_building(
        &mut world,
        &mut map,
        BuildingId(50),
        BuildingKind::ResearchCenter,
        GridPos::new(10, 10),
        ENEMY,
    );

    let own_core = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Analyze {
            issuer: ENEMY,
            building: BuildingId(50),
        },
    );
    let our_core = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Analyze {
            issuer: ENEMY,
            building: RESEARCH_CENTER,
        },
    );

    assert_eq!(own_core.reject, Some(RejectReason::Locked));
    assert_eq!(our_core.reject, Some(RejectReason::Locked));
    assert_eq!(
        stockpile(&world, ENEMY).crystals,
        100,
        "a foreign Analyze spent"
    );
    assert_eq!(research_points(&world), 0.0, "a foreign Analyze banked");
}

/// Only a finite, positive grant moves the accumulator: a NaN delta
/// would poison the total forever (every threshold read fails closed
/// while the wave boost fails open) and a negative one would drag the
/// monotonic total backward.
#[test]
fn add_research_ignores_nan_and_non_positive_grants() {
    let (mut world, _map) = battlefield();

    add_research(&mut world, f64::NAN);
    add_research(&mut world, -50.0);
    assert_eq!(
        research_points(&world),
        0.0,
        "junk grants must not bank into the seeded zero"
    );

    add_research(&mut world, 10.0);
    add_research(&mut world, f64::NAN);
    add_research(&mut world, -10.0);
    assert_eq!(
        research_points(&world),
        10.0,
        "later junk grants must not corrupt a banked total"
    );
}

/// A pathological fixed-step delta never reaches the accumulator: zero,
/// negative and non-finite `seconds` are refused before `add_research`
/// runs, matching the wave clock's own `advance` guard.
#[test]
fn step_research_refuses_non_finite_or_non_positive_deltas() {
    for seconds in [f32::NAN, f32::INFINITY, 0.0, -0.05] {
        let (mut world, _map) = battlefield();
        world.resource_mut::<ResearchState>().points = 10.0;

        step_research(&mut world, seconds);

        assert_eq!(
            research_points(&world),
            10.0,
            "seconds={seconds} must not move the banked total"
        );
    }
}

/// Passive research lives and dies with the core itself: a session-free
/// world (the benchmark, pure-sim fixtures) has no Result to settle when
/// the Research Center falls, so the step's living-core gate is the only
/// thing that stops accrual after destruction.
#[test]
fn step_research_freezes_once_the_core_is_gone() {
    let (mut world, _map) = battlefield();
    step_research(&mut world, SIM_STEP_SECONDS);
    assert!(research_points(&world) > 0.0);

    // Despawn the core the way combat removal does: index entry, then
    // the entity itself.
    let entity = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .unwrap();
    world
        .resource_mut::<BuildingIndex>()
        .remove(RESEARCH_CENTER);
    world.despawn(entity);
    let banked = research_points(&world);

    step_research(&mut world, SIM_STEP_SECONDS);
    assert_eq!(
        research_points(&world),
        banked,
        "a dead core must freeze passive accrual"
    );
}

/// At the target Analyze has nothing left to buy: session-free worlds
/// have no Result gate locking the command (in-session the settled
/// Victory already rejects `SessionLocked`), so the cap itself rejects
/// `Locked` before the spend — never 10 Crystals for zero points.
#[test]
fn analyze_at_the_cap_rejects_instead_of_spending() {
    let (mut world, mut map) = battlefield();
    set_crystals(&mut world, TEAM, 50);
    world.resource_mut::<ResearchState>().points = f64::from(RESEARCH_TARGET_POINTS);

    let result = analyze(&mut world, &mut map, RESEARCH_CENTER);

    assert_eq!(result.reject, Some(RejectReason::Locked));
    assert_eq!(stockpile(&world, TEAM).crystals, 50, "the cap spent");
    assert_eq!(
        research_points(&world),
        f64::from(RESEARCH_TARGET_POINTS),
        "the cap banked"
    );
}

/// Completed Labs add their catalogue rate as an additive term OUTSIDE the
/// cold-scaled surface product — the one seam HPA-188 left. A Lab still
/// building adds zero; +1 and +2 Labs add exactly their whole rate at full
/// heat, and under the 25% cold floor the surface term floors while the Lab
/// term stays whole (0.5 + 1.0 = 1.5 and 0.5 + 2.0 = 2.5, not the
/// inside-the-product 0.625/0.75).
#[test]
fn completed_labs_add_research_outside_the_cold_multiplier() {
    let (mut world, _map) = battlefield();
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND);

    let completed_lab = || {
        let mut room = Room::new(RoomKind::Lab);
        room.elapsed_seconds = room_spec(RoomKind::Lab).build_seconds as f32;
        Some(room)
    };

    // A Lab still building adds zero.
    world.insert_resource(Underground {
        levels: vec![UndergroundLevel {
            slots: [Some(Room::new(RoomKind::Lab)), None, None, None],
        }],
        excavation: None,
    });
    assert_eq!(
        research_rate(&world),
        RESEARCH_BASE_PER_SECOND,
        "an incomplete Lab adds nothing"
    );

    // One completed Lab: +1 at full heat; a second in another slot: +2.
    world.resource_mut::<Underground>().levels[0].slots[0] = completed_lab();
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND + 1.0);
    world.resource_mut::<Underground>().levels[0].slots[1] = completed_lab();
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND + 2.0);

    // The 25% floor cold: the surface term floors to 0.5 while the Lab
    // term stays whole outside the product — one Lab totals 1.5, two Labs
    // 2.5 (not the inside-the-product 0.625/0.75).
    world.insert_resource(HeatState {
        supply: 0,
        demand: 10,
        burn_remainder: 0.0,
    });
    world.resource_mut::<Underground>().levels[0].slots[1] = None;
    assert_eq!(
        research_rate(&world),
        RESEARCH_BASE_PER_SECOND * f64::from(MIN_SURFACE_EFFICIENCY) + 1.0,
        "one Lab keeps its whole rate at the cold floor"
    );
    world.resource_mut::<Underground>().levels[0].slots[1] = completed_lab();
    assert_eq!(
        research_rate(&world),
        RESEARCH_BASE_PER_SECOND * f64::from(MIN_SURFACE_EFFICIENCY) + 2.0,
        "two Labs keep their whole rates at the cold floor"
    );
}
