use bevy::prelude::{Entity, Or, With, World};

use super::*;
use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{ANALYZE_RESEARCH_GAIN, BuildingKind, MIN_SURFACE_EFFICIENCY, building_spec};
use crate::commands::UnitIndex;
use crate::economy::ResourceIndex;
use crate::fixture::{MapFixture, seed_skirmish};
use crate::heat::HeatState;
use crate::ids::{IdAllocator, TeamId};
use crate::map::GridMap;
use crate::movement::{SIM_STEP_SECONDS, Unit};
use crate::session::{MatchPhase, MatchResult, MatchSession};

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
/// neutral-default pattern as a missing `HeatState`.
#[test]
fn missing_research_state_reads_zero() {
    let world = World::new();
    assert_eq!(research_points(&world), 0.0);
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND);
    for kind in BuildingKind::ALL {
        assert!(
            building_unlocked(&world, kind),
            "{kind:?} must stay unlocked"
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
/// one helper — 25% → 2,250, 50% → 4,500, 75% → 6,750, 100% → 9,000 —
/// and every current catalogue kind stays at 0% (always unlocked) until
/// the advanced towers land.
#[test]
fn unlock_thresholds_derive_from_the_catalogue_percent() {
    assert_eq!(unlock_points(0), 0);
    assert_eq!(unlock_points(25), 2_250);
    assert_eq!(unlock_points(50), 4_500);
    assert_eq!(unlock_points(75), 6_750);
    assert_eq!(unlock_points(100), RESEARCH_TARGET_POINTS);

    let (world, _map) = battlefield();
    for kind in BuildingKind::ALL {
        assert_eq!(building_spec(kind).research_unlock_percent, 0);
        assert!(
            building_unlocked(&world, kind),
            "{kind:?} must stay unlocked"
        );
    }
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
