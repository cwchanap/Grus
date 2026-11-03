//! The HPA-288 pre-wave feasibility gate: a fully paid L1–L3 excavation
//! plus L3 Geothermal Tap and Lab, funded only by real Worker gathering
//! through the canonical `step_tick`, must complete strictly before wave 1
//! launches at `WAVE_PREPARATION_SECONDS` (300 s).
//!
//! No stockpile injection and no timer skipping: the seeded 300 Scrap /
//! 150 Coal treasury must be stretched by real deposits — the journey
//! costs 786 Scrap / 258 Coal (three digs 516 + Tap 120 + Lab 150), so at
//! least 486 Scrap and 108 Coal are gathered — while the serial dig
//! timers (30 + 45 + 60 s) plus the two concurrent 30 s room builds
//! impose the 165 s floor against the 300 s first-wave deadline. The
//! margin this test pins is the budget the later Godot smoke (and its
//! 45 s/90 s CI timeout) will spend; run with `--nocapture` for the
//! timeline evidence.

use bevy::prelude::World;
use grus_sim::catalog::building_spec;
use grus_sim::{
    BuildingKind, GridMap, HeatState, MapFixture, PlayerCommand, RESEARCH_BASE_PER_SECOND,
    RejectReason, ResourceId, ResourceKind, RoomKind, SIM_STEP_SECONDS, TeamEconomy, TeamId,
    Underground, UnitKind, WAVE_PREPARATION_SECONDS, WaveState, apply_player_command,
    excavation_cost_and_seconds, live_alien_ids, research_rate, room_spec, seed_skirmish,
    sorted_unit_ids, step_tick,
};

const TEAM: TeamId = TeamId(1);
/// The journey digs exactly three levels and builds the two L3 rooms.
const TARGET_DEPTH: u32 = 3;
const TAP_SLOT: u8 = 0;
const LAB_SLOT: u8 = 1;
/// One f32 step (0.05 s) accumulates tiny drift over thousands of
/// additions; timeline floors compare with this slack.
const TIME_TOLERANCE: f64 = 1e-3;

fn stockpile(world: &World) -> (u32, u32, u32) {
    let state = &world.resource::<TeamEconomy>().0[&TEAM];
    (
        state.stockpile.food,
        state.stockpile.scrap,
        state.stockpile.coal,
    )
}

/// Total L1–`TARGET_DEPTH` dig spend, derived from the catalogue table —
/// never a copied constant.
fn total_dig_spend() -> (u32, u32) {
    (1..=TARGET_DEPTH)
        .map(excavation_cost_and_seconds)
        .fold((0, 0), |(scrap, coal), (cost, _)| {
            (scrap + cost.scrap, coal + cost.coal)
        })
}

/// The one safe source of `kind` (or the ordered list for Scrap), read
/// from the authored fixture tables — never coordinates or id guesses.
fn safe_sources(fixture: &MapFixture, kind: ResourceKind) -> Vec<ResourceId> {
    let safe = MapFixture::safe_source_ids(TEAM);
    fixture
        .resources
        .iter()
        .filter(|spawn| spawn.kind == kind && safe.contains(&spawn.id))
        .map(|spawn| spawn.id)
        .collect()
}

/// Issues one room build; `false` only for the gather-funded
/// `InsufficientResources` retry — anything else is a bug in the journey.
fn issue_build_room(
    world: &mut World,
    map: &mut GridMap,
    level: u32,
    slot: u8,
    kind: RoomKind,
) -> bool {
    let result = apply_player_command(
        world,
        map,
        PlayerCommand::BuildRoom {
            issuer: TEAM,
            level,
            slot,
            kind,
        },
    );
    match result.reject {
        None => true,
        Some(RejectReason::InsufficientResources) => false,
        Some(other) => panic!("unexpected BuildRoom({kind:?}) rejection: {other:?}"),
    }
}

/// Whether the L3 room in `slot` is placed and finished.
fn room_complete(world: &World, slot: u8) -> bool {
    world.resource::<Underground>().levels[(TARGET_DEPTH - 1) as usize].slots[slot as usize]
        .is_some_and(|room| room.complete())
}

#[test]
fn gather_funded_l3_tap_and_lab_complete_before_wave_1() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    // The seeded survival treasury is the only funding: 200 Food /
    // 300 Scrap / 150 Coal. The journey spends only Scrap and Coal.
    let start = stockpile(&world);
    assert_eq!(start, (200, 300, 150));
    let (dig_scrap, dig_coal) = total_dig_spend();
    let total_scrap = dig_scrap
        + room_spec(RoomKind::GeothermalTap).cost.scrap
        + room_spec(RoomKind::Lab).cost.scrap;
    let total_coal = dig_coal;
    // The locked feasibility premise: 786 Scrap / 258 Coal against the
    // seeded treasury, i.e. real gathering of 486 Scrap and 108 Coal.
    assert_eq!((total_scrap, total_coal), (786, 258));

    // Assign the four starting Workers through real Gather commands: one
    // on the single safe Coal node, three on the three safe Scrap nodes.
    let workers = sorted_unit_ids(&world, TEAM, |unit| unit.kind == UnitKind::Worker);
    assert_eq!(workers.len(), 4, "the seed provides four starting Workers");
    let coal_sources = safe_sources(&fixture, ResourceKind::Coal);
    let scrap_sources = safe_sources(&fixture, ResourceKind::Scrap);
    assert_eq!(coal_sources.len(), 1, "one safe Coal node rings the start");
    assert_eq!(
        scrap_sources.len(),
        3,
        "three safe Scrap nodes ring the start"
    );
    let mut assignment = vec![(workers[0], coal_sources[0])];
    assignment.extend(workers[1..].iter().copied().zip(scrap_sources));
    for (worker, source) in assignment {
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::Gather {
                issuer: TEAM,
                workers: vec![worker],
                source,
            },
        );
        assert_eq!(
            outcome.accepted_units,
            vec![worker],
            "gather assignment rejected"
        );
    }

    // Drive the canonical tick. The controller is the player: it starts
    // the next dig as soon as the treasury affords it and places both L3
    // rooms the moment L3 completes and Scrap allows, retrying while a
    // gather-funded balance is short — never injecting anything.
    let tick_cap = (f64::from(WAVE_PREPARATION_SECONDS) / f64::from(SIM_STEP_SECONDS)) as usize;
    let mut dig_start: Vec<f64> = Vec::new();
    let mut dig_done: Vec<f64> = Vec::new();
    let (mut tap_built, mut lab_built) = (false, false);
    let (mut tap_done, mut lab_done) = (None, None);
    let mut completion = None;
    let mut t = 0.0_f64;
    for _ in 0..tick_cap {
        let underground = world.resource::<Underground>().clone();
        if underground.depth() < TARGET_DEPTH && underground.excavation.is_none() {
            let result = apply_player_command(
                &mut world,
                &mut map,
                PlayerCommand::Excavate { issuer: TEAM },
            );
            match result.reject {
                None => dig_start.push(t),
                // Gather-funded: a real delivery has not landed yet.
                Some(RejectReason::InsufficientResources) => {}
                Some(other) => panic!("unexpected Excavate rejection: {other:?}"),
            }
        }
        if underground.depth() == TARGET_DEPTH {
            if !tap_built {
                tap_built = issue_build_room(
                    &mut world,
                    &mut map,
                    TARGET_DEPTH,
                    TAP_SLOT,
                    RoomKind::GeothermalTap,
                );
            }
            if !lab_built {
                lab_built =
                    issue_build_room(&mut world, &mut map, TARGET_DEPTH, LAB_SLOT, RoomKind::Lab);
            }
        }

        let depth_before = underground.depth();
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        t += f64::from(SIM_STEP_SECONDS);
        let underground = world.resource::<Underground>().clone();
        if underground.depth() > depth_before {
            dig_done.push(t);
        }
        if tap_built && tap_done.is_none() && room_complete(&world, TAP_SLOT) {
            tap_done = Some(t);
        }
        if lab_built && lab_done.is_none() && room_complete(&world, LAB_SLOT) {
            lab_done = Some(t);
        }
        if tap_done.is_some() && lab_done.is_some() {
            completion = Some(t);
            break;
        }
    }

    let completion = completion.unwrap_or_else(|| {
        panic!(
            "L3 Tap+Lab never completed within {WAVE_PREPARATION_SECONDS} s: digs started {dig_start:?} completed {dig_done:?}, rooms {tap_built}/{lab_built}"
        )
    });

    // The wave guard: wave 1 has not launched, no alien exists, and the
    // measured margin in simulated seconds is strictly positive.
    let wave = *world.resource::<WaveState>();
    assert_eq!(wave.next_wave, 1, "wave 1 must not have launched");
    let margin = wave.remaining_seconds();
    assert!(
        margin > 0.0,
        "rooms completed at {completion} s with zero margin before wave 1"
    );
    assert!(
        live_alien_ids(&world, TeamId(2)).is_empty(),
        "no alien may exist before wave 1"
    );

    // Actual completion: depth 3 with both L3 rooms finished.
    let underground = world.resource::<Underground>().clone();
    assert_eq!(underground.depth(), TARGET_DEPTH);
    let tap = underground.levels[(TARGET_DEPTH - 1) as usize].slots[TAP_SLOT as usize];
    let lab = underground.levels[(TARGET_DEPTH - 1) as usize].slots[LAB_SLOT as usize];
    assert_eq!(tap.map(|room| room.kind), Some(RoomKind::GeothermalTap));
    assert_eq!(lab.map(|room| room.kind), Some(RoomKind::Lab));
    assert!(tap.is_some_and(|room| room.complete()));
    assert!(lab.is_some_and(|room| room.complete()));

    // Funding evidence: exactly three digs and two rooms were paid, so
    // gathered = final + total spend − seeded treasury, all through real
    // deposits. Food is never touched.
    assert_eq!(dig_start.len(), 3);
    assert!(tap_built && lab_built);
    let end = stockpile(&world);
    let gathered_scrap = end.1 + total_scrap - start.1;
    let gathered_coal = end.2 + total_coal - start.2;
    assert_eq!(end.0, start.0, "no Food may be spent on the journey");
    assert!(
        gathered_scrap >= total_scrap - start.1 && gathered_coal >= total_coal - start.2,
        "gathering must cover the shortfall: +{gathered_scrap} Scrap / +{gathered_coal} Coal"
    );

    // Timing evidence: the serial catalogue dig timers and the concurrent
    // room timers hold their floor — digs 30/45/60 s strictly in series,
    // rooms no shorter than their build durations after L3 lands.
    let (l1, l2, l3) = (dig_done[0], dig_done[1], dig_done[2]);
    assert!(
        l1 + TIME_TOLERANCE >= f64::from(excavation_cost_and_seconds(1).1),
        "L1 dig floor"
    );
    assert!(
        l2 + TIME_TOLERANCE >= l1 + f64::from(excavation_cost_and_seconds(2).1),
        "serial L2 dig floor"
    );
    assert!(
        l3 + TIME_TOLERANCE >= l2 + f64::from(excavation_cost_and_seconds(3).1),
        "serial L3 dig floor"
    );
    assert!(
        l3 + TIME_TOLERANCE >= 135.0,
        "the 165 s journey floor needs its 135 s dig span"
    );
    let build_floor = l3 + f64::from(room_spec(RoomKind::GeothermalTap).build_seconds);
    let (tap_done, lab_done) = (tap_done.unwrap(), lab_done.unwrap());
    assert!(
        tap_done + TIME_TOLERANCE >= build_floor && lab_done + TIME_TOLERANCE >= build_floor,
        "room build floor"
    );
    assert!(completion < f64::from(WAVE_PREPARATION_SECONDS));

    // Heat effect: the L3 tap adds its full catalogue supply (RC 15 +
    // tap base 15 at its minimum level) with no demand of its own.
    let heat = *world.resource::<HeatState>();
    let tap_supply = room_spec(RoomKind::GeothermalTap).heat_supply_base;
    assert_eq!(
        heat.supply,
        building_spec(BuildingKind::ResearchCenter).heat_supply + tap_supply
    );
    assert_eq!(
        heat.demand,
        building_spec(BuildingKind::ResearchCenter).heat_demand
    );

    // Research effect: the completed Lab adds its whole catalogue rate on
    // top of the full-heat surface base, outside the cold multiplier.
    assert_eq!(
        research_rate(&world),
        RESEARCH_BASE_PER_SECOND + room_spec(RoomKind::Lab).research_per_second
    );

    // Timeline for the later Godot smoke (visible with `--nocapture`).
    eprintln!(
        "underground journey: digs started {dig_start:?} completed {dig_done:?}; tap done {tap_done} lab done {lab_done}; rooms complete {completion} s; margin {margin:.2} s; gathered +{gathered_scrap} Scrap/+{gathered_coal} Coal; final stockpile {end:?}"
    );
}
