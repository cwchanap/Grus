use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    BuildingKind, RESEARCH_BASE_PER_SECOND, ROOM_SLOTS_PER_LEVEL, RoomKind, room_spec,
};
use crate::commands::{CommandResult, PlayerCommand, RejectReason, apply_player_command};
use crate::economy::{ResourceStockpile, TeamEconomy};
use crate::fixture::{MapFixture, clear_match_world, seed_skirmish};
use crate::heat::HeatState;
use crate::ids::{BuildingId, TeamId};
use crate::map::GridMap;
use crate::movement::SIM_STEP_SECONDS;
use crate::research::research_rate;
use crate::session::{MatchPhase, MatchResult, MatchSession, set_paused};
use crate::tick::step_tick;

fn battlefield() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

/// Team 1's (food, scrap, coal) — the three currencies rooms credit.
fn stockpile(world: &World) -> (u32, u32, u32) {
    let state = world
        .resource::<TeamEconomy>()
        .0
        .get(&TeamId(1))
        .expect("team 1 economy row");
    (
        state.stockpile.food,
        state.stockpile.scrap,
        state.stockpile.coal,
    )
}

/**
 * A room placed with its full build time already elapsed, i.e. complete.
 */
fn completed_room(kind: RoomKind) -> Option<Room> {
    let mut room = Room::new(kind);
    room.progress_seconds = room_spec(kind).build_seconds as f32;
    Some(room)
}

fn one_level(rooms: [Option<Room>; ROOM_SLOTS_PER_LEVEL]) -> Vec<UndergroundLevel> {
    vec![UndergroundLevel { slots: rooms }]
}

/// A fresh underground is empty: no levels, no dig, and the first dig's
/// target derives from the depth rather than being stored.
#[test]
fn default_underground_is_empty_and_derives_its_target_from_depth() {
    let underground = Underground::default();
    assert_eq!(underground.depth(), 0);
    assert!(underground.levels.is_empty());
    assert!(underground.excavation.is_none());
    assert_eq!(underground.excavation_target(), 1);
}

/// Completion is derived from build progress against the catalogue build
/// duration — never a stored flag, so nothing can complete twice.
#[test]
fn room_completion_derives_from_build_progress() {
    for kind in RoomKind::ALL {
        let mut room = Room::new(kind);
        assert!(!room.complete(), "{kind:?} starts incomplete");
        room.progress_seconds = room_spec(kind).build_seconds as f32;
        assert!(room.complete(), "{kind:?} completes at the catalogue time");
    }
}

/// Stepping a world with no underground state is a harmless no-op: the
/// seeded default is removed first so the step truly faces an absent
/// resource, and it neither inserts one nor touches the stockpile.
#[test]
fn a_world_without_underground_state_steps_as_a_no_op() {
    let (mut world, _map) = battlefield();
    world.remove_resource::<Underground>();
    let before = stockpile(&world);
    step_underground(&mut world, SIM_STEP_SECONDS);
    assert!(world.get_resource::<Underground>().is_none());
    assert_eq!(stockpile(&world), before);
}

/// The dig accumulates across ticks, completes exactly once, appends
/// exactly one fresh four-slot level, and stepping itself never charges
/// the stockpile — the dig was paid up front when it started.
#[test]
fn paid_dig_advances_in_ticks_and_completes_exactly_once() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: Vec::new(),
        excavation: Some(Excavation {
            elapsed_seconds: 0.0,
        }),
    });
    let before = stockpile(&world);

    step_underground(&mut world, 15.0);
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 0, "halfway through the 30 s dig");
    assert_eq!(
        underground.excavation.as_ref().unwrap().elapsed_seconds,
        15.0
    );

    // 30 s total: the level-1 dig duration.
    step_underground(&mut world, 15.0);
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 1);
    assert!(underground.excavation.is_none());
    assert_eq!(underground.levels[0].slots.len(), ROOM_SLOTS_PER_LEVEL);
    assert!(
        underground.levels[0]
            .slots
            .iter()
            .all(|slot| slot.is_none())
    );
    assert_eq!(stockpile(&world), before, "stepping never charges");

    // Far past the completion tick: still exactly one level, and the next
    // dig targets the new depth.
    step_underground(&mut world, 30.0);
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 1, "completion cannot repeat");
    assert_eq!(underground.excavation_target(), 2);
}

/// The dig and room build timers are independent: both advance in the same
/// ticks, a room may finish before the dig, and a dig completion leaves
/// existing rooms untouched.
#[test]
fn dig_and_room_timers_advance_concurrently() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([Some(Room::new(RoomKind::Greenhouse)), None, None, None]),
        excavation: Some(Excavation {
            elapsed_seconds: 0.0,
        }),
    });

    // Greenhouse builds in 20 s; the dig toward level 2 needs 45 s.
    step_underground(&mut world, 20.0);
    let underground = world.resource::<Underground>();
    assert!(
        underground.levels[0].slots[0].as_ref().unwrap().complete(),
        "room finishes before the dig"
    );
    assert_eq!(underground.depth(), 1, "the seeded level is unchanged");
    assert_eq!(
        underground.excavation.as_ref().unwrap().elapsed_seconds,
        20.0
    );

    // 45 s total: the dig completes and leaves the room untouched.
    step_underground(&mut world, 25.0);
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 2);
    assert!(underground.levels[0].slots[0].as_ref().unwrap().complete());
    assert!(
        underground.levels[1]
            .slots
            .iter()
            .all(|slot| slot.is_none())
    );
}

/// Both timers snap to required using production's `PROGRESS_EPSILON`
/// rule: at the exact 0.05 s fixed step a 45 s dig is 900 ticks and a
/// 15 s room build is 300 — f32 accumulation drifts a hair under the
/// required time, the tolerance snaps it to exactly the catalogue
/// value on the boundary tick, and never a tick early.
#[test]
fn dig_and_room_timers_snap_to_required_at_the_fixed_tick() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([Some(Room::new(RoomKind::Shelter)), None, None, None]),
        excavation: Some(Excavation::default()),
    });

    for tick in 1..=299u32 {
        step_underground(&mut world, SIM_STEP_SECONDS);
        let underground = world.resource::<Underground>();
        assert!(
            !underground.levels[0].slots[0].as_ref().unwrap().complete(),
            "Shelter complete before tick {tick}"
        );
    }
    step_underground(&mut world, SIM_STEP_SECONDS); // tick 300: 15 s exactly
    assert_eq!(
        world.resource::<Underground>().levels[0].slots[0]
            .as_ref()
            .unwrap()
            .progress_seconds,
        room_spec(RoomKind::Shelter).build_seconds as f32,
        "the snap rule lands exactly on the required time"
    );

    for tick in 301..=899u32 {
        step_underground(&mut world, SIM_STEP_SECONDS);
        assert_eq!(
            world.resource::<Underground>().depth(),
            1,
            "the 45 s dig completed before tick {tick}"
        );
    }
    step_underground(&mut world, SIM_STEP_SECONDS); // tick 900: 45 s exactly
    assert_eq!(world.resource::<Underground>().depth(), 2);
    assert!(world.resource::<Underground>().excavation.is_none());
}

/// An unfinished room yields nothing, the tick that completes it yields
/// nothing (it was not complete at tick entry), and the next tick starts
/// banking — never retroactive production.
#[test]
fn rooms_yield_nothing_until_complete_and_begin_the_next_tick() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([Some(Room::new(RoomKind::Greenhouse)), None, None, None]),
        excavation: None,
    });

    // 19.5 of the 20 build seconds: still constructing, no output.
    step_underground(&mut world, 19.5);
    assert!(
        !world.resource::<Underground>().levels[0].slots[0]
            .as_ref()
            .unwrap()
            .complete()
    );
    assert_eq!(stockpile(&world).0, 200);

    // The completing tick itself: no output yet.
    step_underground(&mut world, 0.5);
    assert!(
        world.resource::<Underground>().levels[0].slots[0]
            .as_ref()
            .unwrap()
            .complete()
    );
    assert_eq!(stockpile(&world).0, 200, "no output on the completing tick");

    // 0.5 Food/s × 2 s = exactly one whole Food, zero remainder left.
    step_underground(&mut world, 2.0);
    assert_eq!(stockpile(&world).0, 201);
}

/// At the 0.05 s fixed step a completed Greenhouse banks 0.025 Food per
/// tick: 39 ticks stay fractional, the 40th credits exactly one unit —
/// no unit is lost or credited early.
#[test]
fn fractional_output_banks_whole_units_at_the_fixed_tick_size() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([completed_room(RoomKind::Greenhouse), None, None, None]),
        excavation: None,
    });

    for tick in 1..=40u32 {
        step_underground(&mut world, SIM_STEP_SECONDS);
        let expected = if tick < 40 { 200 } else { 201 };
        assert_eq!(stockpile(&world).0, expected, "food after tick {tick}");
    }
}

/// Larger test deltas credit the same per-room rates: 0.4 Coal/s and
/// 0.4 Scrap/s give 2 units per 5 s and 1 per 2.5 s, each into its own
/// currency.
#[test]
fn larger_test_deltas_credit_the_same_room_rates() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([
            completed_room(RoomKind::CoalVein),
            completed_room(RoomKind::DeepMine),
            None,
            None,
        ]),
        excavation: None,
    });

    step_underground(&mut world, 5.0);
    assert_eq!(stockpile(&world), (200, 302, 152));
    step_underground(&mut world, 2.5);
    assert_eq!(stockpile(&world), (200, 303, 153));
}

/// Every room banks its own fractional remainder: two completed
/// Greenhouses at 0.5 Food/s hold one second's 0.5 + 0.5 fractional, and
/// only the next second credits one whole unit per room — a pooled
/// accumulator would have paid early.
#[test]
fn every_room_banks_its_own_fractional_remainder() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([
            completed_room(RoomKind::Greenhouse),
            completed_room(RoomKind::Greenhouse),
            None,
            None,
        ]),
        excavation: None,
    });

    step_underground(&mut world, 1.0);
    assert_eq!(stockpile(&world).0, 200, "each 0.5 stays fractional");
    step_underground(&mut world, 1.0);
    assert_eq!(stockpile(&world).0, 202, "both rooms credit independently");
}

/// Stockpile credit must saturate, not overflow: with the accepted finite
/// delta 2^32 s, two completed Greenhouses each bank exactly
/// 0.5 × 2^32 = 2_147_483_648 Food — creditable by each room's own
/// `as u32` floor — and the shared stockpile's `saturating_add` clamps
/// their sum (one past `u32::MAX`) rather than wrapping to a small number.
#[test]
fn stockpile_credit_saturates_instead_of_overflowing() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([
            completed_room(RoomKind::Greenhouse),
            completed_room(RoomKind::Greenhouse),
            None,
            None,
        ]),
        excavation: None,
    });

    step_underground(&mut world, 4_294_967_296.0_f32);
    assert_eq!(stockpile(&world).0, u32::MAX);
    for slot in 0..2 {
        assert_eq!(
            world.resource::<Underground>().levels[0].slots[slot]
                .as_ref()
                .unwrap()
                .production_remainder,
            0.0,
            "each room banked its exact whole units"
        );
    }
}

/// Start, Paused and a settled Result freeze every timer and all output;
/// resuming from Paused continues the very same dig and starts banking.
#[test]
fn start_paused_and_result_freeze_progress_and_pause_resumes() {
    for phase in [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(TeamId(2))),
    ] {
        let (mut world, _map) = battlefield();
        world.insert_resource(MatchSession { phase });
        world.insert_resource(Underground {
            levels: one_level([completed_room(RoomKind::Greenhouse), None, None, None]),
            excavation: Some(Excavation {
                elapsed_seconds: 29.5,
            }),
        });

        step_underground(&mut world, 2.0);
        let underground = world.resource::<Underground>();
        assert_eq!(underground.depth(), 1, "{phase:?} froze the dig");
        assert_eq!(
            underground.excavation.as_ref().unwrap().elapsed_seconds,
            29.5
        );
        assert_eq!(
            underground.levels[0].slots[0]
                .as_ref()
                .unwrap()
                .production_remainder,
            0.0,
            "{phase:?} froze the output accumulator"
        );
        assert_eq!(stockpile(&world).0, 200, "{phase:?} froze the credits");
    }

    // Resume: the frozen dig continues and completes, output resumes.
    let (mut world, _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Paused,
    });
    world.insert_resource(Underground {
        levels: one_level([completed_room(RoomKind::Greenhouse), None, None, None]),
        excavation: Some(Excavation {
            elapsed_seconds: 29.5,
        }),
    });
    step_underground(&mut world, 2.0);
    assert_eq!(world.resource::<Underground>().depth(), 1);

    // Resume: the frozen dig continues (29.5 + 15.5 reaches the 45 s
    // level-2 duration) and the Greenhouse banks 0.5 × 15.5 = 7.75 → 7.
    set_paused(&mut world, false);
    step_underground(&mut world, 15.5);
    assert_eq!(world.resource::<Underground>().depth(), 2);
    assert_eq!(stockpile(&world).0, 207);
}

/// Zero, negative, NaN and infinite deltas advance nothing — the same
/// validity guard as the wave clock and research step.
#[test]
fn nonpositive_or_nonfinite_seconds_advance_nothing() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([completed_room(RoomKind::Greenhouse), None, None, None]),
        excavation: Some(Excavation::default()),
    });

    for seconds in [0.0, -0.05, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        step_underground(&mut world, seconds);
        let underground = world.resource::<Underground>();
        assert_eq!(
            underground.excavation.unwrap().elapsed_seconds,
            0.0,
            "dig frozen for seconds {seconds}"
        );
        assert_eq!(
            underground.levels[0].slots[0]
                .as_ref()
                .unwrap()
                .production_remainder,
            0.0,
            "output frozen for seconds {seconds}"
        );
        assert_eq!(stockpile(&world).0, 200, "no credits for seconds {seconds}");
    }
}

/// Session-free worlds read as Playing (the benchmark convention), but
/// progress still requires a living Team-1 core — the same
/// completion-agnostic fact research accrual checks: with no core index
/// at all every timer stalls; the identical world with a core — even one
/// still under construction — plays.
#[test]
fn session_free_worlds_stall_without_a_living_core() {
    #[derive(Copy, Clone, Debug)]
    enum Core {
        None,
        Incomplete,
        Complete,
    }

    fn minimal_world(core: Core) -> World {
        let mut world = World::new();
        let mut economy = TeamEconomy::default();
        economy.insert_team(TeamId(1), ResourceStockpile::default());
        world.insert_resource(economy);
        if !matches!(core, Core::None) {
            let mut index = BuildingIndex::default();
            let entity = world
                .spawn(Building {
                    id: BuildingId(1),
                    team: TeamId(1),
                    kind: BuildingKind::ResearchCenter,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: matches!(core, Core::Complete),
                        active_builder: None,
                    },
                })
                .id();
            index.insert(BuildingId(1), entity);
            world.insert_resource(index);
        }
        world
    }

    fn stalled_underground() -> Underground {
        Underground {
            levels: one_level([completed_room(RoomKind::Greenhouse), None, None, None]),
            excavation: Some(Excavation {
                elapsed_seconds: 29.5,
            }),
        }
    }

    let mut world = minimal_world(Core::None);
    world.insert_resource(stalled_underground());
    step_underground(&mut world, 30.0);
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 1, "a missing core stalls the dig");
    assert_eq!(
        underground.excavation.as_ref().unwrap().elapsed_seconds,
        29.5
    );
    assert_eq!(stockpile(&world).0, 0, "a missing core stalls the output");

    // The same minimal world with any living core plays — completion is
    // not part of the gate — so the dig completes (29.5 + 16 ≥ 45 s for
    // level 2) and the Greenhouse banks 0.5 × 16 = 8 Food.
    for core in [Core::Incomplete, Core::Complete] {
        let mut world = minimal_world(core);
        world.insert_resource(stalled_underground());
        step_underground(&mut world, 16.0);
        assert_eq!(world.resource::<Underground>().depth(), 2, "{core:?} plays");
        assert_eq!(stockpile(&world).0, 8, "{core:?} plays");
    }
}

const TEAM: TeamId = TeamId(1);
const ENEMY: TeamId = TeamId(2);
/// `seed_skirmish` allocates `BuildingId(1)` to Team 1's Research Center.
const RESEARCH_CENTER: BuildingId = BuildingId(1);

fn excavate(world: &mut World, map: &mut GridMap, issuer: TeamId) -> CommandResult {
    apply_player_command(world, map, PlayerCommand::Excavate { issuer })
}

fn build_room(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    level: u32,
    slot: u8,
    kind: RoomKind,
) -> CommandResult {
    apply_player_command(
        world,
        map,
        PlayerCommand::BuildRoom {
            issuer,
            level,
            slot,
            kind,
        },
    )
}

fn set_scrap_and_coal(world: &mut World, scrap: u32, coal: u32) {
    let mut economy = world.resource_mut::<TeamEconomy>();
    let state = economy.0.get_mut(&TEAM).unwrap();
    state.stockpile.scrap = scrap;
    state.stockpile.coal = coal;
}

/// Despawns Team 1's seeded Research Center — both its entity and index
/// entry — the way combat removal does, leaving the world without a
/// living core.
fn despawn_research_center(world: &mut World) {
    let entity = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .expect("seeded research center");
    world.despawn(entity);
    world
        .get_resource_mut::<BuildingIndex>()
        .unwrap()
        .remove(RESEARCH_CENTER);
}

/// An accepted Excavate pays the L1 price (100 Scrap, 50 Coal) exactly
/// once through the one atomic `try_spend` and starts the dig timer —
/// without exposing a level: depth stays zero until `step_underground`
/// completes the dig.
#[test]
fn excavate_pays_once_and_starts_the_timer_without_exposing_a_level() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);

    let result = excavate(&mut world, &mut map, TEAM);

    assert_eq!(result.reject, None);
    assert_eq!(stockpile(&world), (200, 200, 100));
    let underground = world.resource::<Underground>();
    assert_eq!(underground.depth(), 0, "no level before the dig completes");
    assert_eq!(
        underground.excavation,
        Some(Excavation {
            elapsed_seconds: 0.0
        })
    );
}

/// Two digs in the same tick: the first pays and starts, the second hits
/// the single-dig rule (`ExcavationInProgress`) before any spend — the
/// stockpile moves once and the timer stays at zero.
#[test]
fn a_second_excavate_in_the_same_tick_rejects_without_double_spending() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);

    let first = excavate(&mut world, &mut map, TEAM);
    let second = excavate(&mut world, &mut map, TEAM);

    assert_eq!(first.reject, None);
    assert_eq!(second.reject, Some(RejectReason::ExcavationInProgress));
    assert_eq!(stockpile(&world), (200, 200, 100), "the second dig spent");
    assert_eq!(
        world.resource::<Underground>().excavation,
        Some(Excavation {
            elapsed_seconds: 0.0
        })
    );
}

/// A foreign issuer and a world without a living Team-1 core both answer
/// `Locked` — the global command gates on the issuer and the shared
/// living-core fact, with no building ID in the command — and neither
/// spends nor starts a dig.
#[test]
fn excavate_rejects_foreign_issuers_and_a_missing_core_without_state_change() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    let before = stockpile(&world);

    let foreign = excavate(&mut world, &mut map, ENEMY);
    assert_eq!(foreign.reject, Some(RejectReason::Locked));

    despawn_research_center(&mut world);
    let no_core = excavate(&mut world, &mut map, TEAM);
    assert_eq!(no_core.reject, Some(RejectReason::Locked));

    assert_eq!(stockpile(&world), before, "a rejected Excavate spent");
    assert_eq!(
        world.resource::<Underground>(),
        &Underground::default(),
        "a rejected Excavate changed underground state"
    );
}

/// The whole price pays or nothing does: short on Coal with plenty of
/// Scrap rejects `InsufficientResources` and deducts neither currency.
#[test]
fn excavate_rejects_when_the_stockpile_cannot_pay_the_whole_price() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 49); // L1 needs 100 Scrap + 50 Coal

    let result = excavate(&mut world, &mut map, TEAM);

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world), (200, 300, 49), "a partial price leaked");
    assert_eq!(world.resource::<Underground>(), &Underground::default());
}

/// A saturated deep dig charges its full saturated price (u32::MAX Scrap,
/// u32::MAX over 2 rounded up in Coal) or nothing: one Coal short of the
/// price rejects atomically, and the funded dig charges exactly the
/// saturated price once — never a wrapped cheap or free level.
#[test]
fn excavate_at_a_saturated_depth_charges_it_all_or_nothing() {
    let (mut world, mut map) = battlefield();
    world.insert_resource(Underground {
        levels: (0..38).map(|_| UndergroundLevel::default()).collect(),
        excavation: None,
    });

    // Target 39's price saturates: u32::MAX Scrap and u32::MAX.div_ceil(2)
    // Coal. One Coal short: all-or-nothing refuses the whole price.
    set_scrap_and_coal(&mut world, u32::MAX, u32::MAX.div_ceil(2) - 1);
    let rejected = excavate(&mut world, &mut map, TEAM);
    assert_eq!(rejected.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world), (200, u32::MAX, u32::MAX.div_ceil(2) - 1));
    assert_eq!(world.resource::<Underground>().excavation, None);

    // Funded: the dig starts and charges exactly the saturated price.
    set_scrap_and_coal(&mut world, u32::MAX, u32::MAX);
    let accepted = excavate(&mut world, &mut map, TEAM);
    assert_eq!(accepted.reject, None);
    assert_eq!(
        stockpile(&world),
        (200, 0, u32::MAX - u32::MAX.div_ceil(2)),
        "the saturated price charged exactly once"
    );
    assert!(world.resource::<Underground>().excavation.is_some());
}

/// Start, Paused and a settled Result lock both underground commands like
/// every other command kind — the shared `apply_player_command` gate
/// rejects before validation or spend.
#[test]
fn underground_commands_share_the_session_gate() {
    for phase in [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(ENEMY)),
    ] {
        let (mut world, mut map) = battlefield();
        set_scrap_and_coal(&mut world, 300, 150);
        world.insert_resource(Underground {
            levels: one_level([None, None, None, None]),
            excavation: None,
        });
        let before = stockpile(&world);
        world.insert_resource(MatchSession { phase });

        let dig = excavate(&mut world, &mut map, TEAM);
        let room = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);

        assert_eq!(
            dig.reject,
            Some(RejectReason::SessionLocked),
            "{phase:?} must lock Excavate"
        );
        assert_eq!(
            room.reject,
            Some(RejectReason::SessionLocked),
            "{phase:?} must lock BuildRoom"
        );
        assert_eq!(stockpile(&world), before, "{phase:?} spent");
        assert!(world.resource::<Underground>().excavation.is_none());
        assert_eq!(world.resource::<Underground>().levels[0].slots[0], None);
    }
}

/// An accepted BuildRoom pays the kind's fixed Scrap price once and writes
/// a fresh zero-progress room into the slot.
#[test]
fn build_room_pays_once_and_occupies_its_slot() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: None,
    });

    let result = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);

    assert_eq!(result.reject, None);
    assert_eq!(stockpile(&world), (200, 250, 150));
    assert_eq!(
        world.resource::<Underground>().levels[0].slots[0],
        Some(Room {
            kind: RoomKind::Shelter,
            progress_seconds: 0.0,
            production_remainder: 0.0
        })
    );
}

/// Short Scrap rejects `InsufficientResources` and writes no room.
#[test]
fn build_room_rejects_insufficient_scrap_without_spend() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 49, 150); // Shelter costs 50 Scrap
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: None,
    });

    let result = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world), (200, 49, 150));
    assert_eq!(world.resource::<Underground>().levels[0].slots[0], None);
}

/// Two rooms in one slot in the same tick: the first pays and occupies,
/// the second hits `Occupied` before any spend — even a different
/// kind cannot double-buy the slot.
#[test]
fn a_second_room_in_the_same_slot_rejects_without_double_spending() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: None,
    });

    let first = build_room(&mut world, &mut map, TEAM, 1, 2, RoomKind::Shelter);
    let second = build_room(&mut world, &mut map, TEAM, 1, 2, RoomKind::Greenhouse);

    assert_eq!(first.reject, None);
    assert_eq!(second.reject, Some(RejectReason::Occupied));
    assert_eq!(stockpile(&world), (200, 250, 150), "the second room spent");
    assert_eq!(
        world.resource::<Underground>().levels[0].slots[2]
            .unwrap()
            .kind,
        RoomKind::Shelter
    );
}

/// Slots 0–3 of a level all accept a room; slot 4 is `OutOfBounds` and
/// never spends.
#[test]
fn slots_zero_through_three_accept_and_slot_four_is_out_of_bounds() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: None,
    });

    for slot in 0u8..4 {
        let result = build_room(&mut world, &mut map, TEAM, 1, slot, RoomKind::Shelter);
        assert_eq!(result.reject, None, "slot {slot}");
    }

    let fifth = build_room(&mut world, &mut map, TEAM, 1, 4, RoomKind::Shelter);
    assert_eq!(fifth.reject, Some(RejectReason::OutOfBounds));
    assert_eq!(stockpile(&world), (200, 100, 150), "slot 4 spent");
}

/// A level with all four slots filled cannot accept a fifth room: every
/// slot answers `Occupied` without spending.
#[test]
fn a_full_level_cannot_accept_a_fifth_room() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: one_level([
            completed_room(RoomKind::Shelter),
            completed_room(RoomKind::Greenhouse),
            completed_room(RoomKind::CoalVein),
            completed_room(RoomKind::DeepMine),
        ]),
        excavation: None,
    });
    let before = stockpile(&world);

    for slot in 0u8..4 {
        let result = build_room(&mut world, &mut map, TEAM, 1, slot, RoomKind::Lab);
        assert_eq!(result.reject, Some(RejectReason::Occupied), "slot {slot}");
    }
    assert_eq!(stockpile(&world), before);
}

/// Level 0, a level past the completed depth and the level an active dig
/// is still opening are all `OutOfBounds` — including the
/// seeded empty underground, whose depth is zero.
#[test]
fn build_room_rejects_missing_and_still_undug_levels() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);

    let no_state = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);
    assert_eq!(no_state.reject, Some(RejectReason::OutOfBounds));
    assert_eq!(
        world.resource::<Underground>(),
        &Underground::default(),
        "a rejected BuildRoom changed underground state"
    );

    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: Some(Excavation {
            elapsed_seconds: 0.0,
        }),
    });
    let before = stockpile(&world);
    let zero = build_room(&mut world, &mut map, TEAM, 0, 0, RoomKind::Shelter);
    let undug = build_room(&mut world, &mut map, TEAM, 2, 0, RoomKind::CoalVein);

    assert_eq!(zero.reject, Some(RejectReason::OutOfBounds));
    assert_eq!(undug.reject, Some(RejectReason::OutOfBounds));
    assert_eq!(stockpile(&world), before);
    assert_eq!(world.resource::<Underground>().levels[0].slots[0], None);
}

/// L3 kinds at L2 answer `Locked` (minimum level, not exact level): Lab
/// and Geothermal Tap stay locked on the second level and open on the
/// third — the accepted control proves the gate is the kind's
/// `min_level`, not the level count.
#[test]
fn l3_room_kinds_stay_locked_below_level_three() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 500, 150);
    world.insert_resource(Underground {
        levels: vec![UndergroundLevel::default(), UndergroundLevel::default()],
        excavation: None,
    });
    let before = stockpile(&world);

    let lab = build_room(&mut world, &mut map, TEAM, 2, 0, RoomKind::Lab);
    let tap = build_room(&mut world, &mut map, TEAM, 2, 1, RoomKind::GeothermalTap);

    assert_eq!(lab.reject, Some(RejectReason::Locked));
    assert_eq!(tap.reject, Some(RejectReason::Locked));
    assert_eq!(stockpile(&world), before, "a locked kind spent");

    world.resource_mut::<Underground>().levels = vec![UndergroundLevel::default(); 3];
    let opened = build_room(&mut world, &mut map, TEAM, 3, 0, RoomKind::Lab);
    assert_eq!(opened.reject, None);
    assert_eq!(stockpile(&world).1, 350, "the L3 Lab paid 150 Scrap");
}

/// At L5 every previously unlocked kind stays placeable on the deepest
/// completed level through the real command path: Shelter (min L1) and
/// Deep Mine (min L4) both validate, spend exactly their fixed Scrap
/// prices and land at the addressed slots — `min_level` is a floor, not
/// an exact-level assignment.
#[test]
fn l5_accepts_previously_unlocked_kinds_through_the_command() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: vec![UndergroundLevel::default(); 5],
        excavation: None,
    });

    let shelter = build_room(&mut world, &mut map, TEAM, 5, 0, RoomKind::Shelter);
    let mine = build_room(&mut world, &mut map, TEAM, 5, 3, RoomKind::DeepMine);

    assert_eq!(shelter.reject, None);
    assert_eq!(mine.reject, None);
    assert_eq!(
        stockpile(&world),
        (200, 100, 150),
        "the Shelter paid 50 and the Deep Mine 150 Scrap"
    );
    let level = &world.resource::<Underground>().levels[4];
    assert_eq!(level.slots[0].unwrap().kind, RoomKind::Shelter);
    assert_eq!(level.slots[3].unwrap().kind, RoomKind::DeepMine);
}

/// BuildRoom needs no selection and no free dig: separate slots
/// build in parallel in one tick, and rooms build in already dug levels
/// while a deeper dig runs.
#[test]
fn rooms_build_in_parallel_and_during_a_deeper_dig() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 500, 150);
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: Some(Excavation {
            elapsed_seconds: 0.0,
        }),
    });

    let first = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);
    let second = build_room(&mut world, &mut map, TEAM, 1, 1, RoomKind::Greenhouse);

    assert_eq!(first.reject, None);
    assert_eq!(second.reject, None);
    let underground = world.resource::<Underground>();
    assert!(underground.levels[0].slots[0].is_some());
    assert!(underground.levels[0].slots[1].is_some());
    assert_eq!(
        underground.excavation,
        Some(Excavation {
            elapsed_seconds: 0.0
        }),
        "building a room never disturbs the dig"
    );
}

/// BuildRoom gates on the shared living-core fact (the same one research
/// accrual uses — alive, not necessarily completed) and the Team-1
/// issuer: a despawned core answers `Locked`, a foreign issuer `Locked`,
/// and neither spends nor writes a room; an incomplete-but-living core
/// still accepts.
#[test]
fn build_room_needs_a_living_core_and_a_team1_issuer() {
    let (mut world, mut map) = battlefield();
    set_scrap_and_coal(&mut world, 300, 150);
    world.insert_resource(Underground {
        levels: one_level([None, None, None, None]),
        excavation: None,
    });
    let before = stockpile(&world);

    despawn_research_center(&mut world);
    let no_core = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);
    assert_eq!(no_core.reject, Some(RejectReason::Locked));
    assert_eq!(stockpile(&world), before);
    assert_eq!(world.resource::<Underground>().levels[0].slots[0], None);

    // The core back again, still under construction: alive is the only
    // fact the gate checks, so the room is accepted and paid once.
    let entity = world
        .spawn(Building {
            id: RESEARCH_CENTER,
            team: TEAM,
            kind: BuildingKind::ResearchCenter,
            construction: ConstructionState {
                progress_seconds: 0.0,
                complete: false,
                active_builder: None,
            },
        })
        .id();
    world
        .get_resource_mut::<BuildingIndex>()
        .unwrap()
        .insert(RESEARCH_CENTER, entity);
    let foreign = build_room(&mut world, &mut map, ENEMY, 1, 0, RoomKind::Shelter);
    let incomplete = build_room(&mut world, &mut map, TEAM, 1, 0, RoomKind::Shelter);
    assert_eq!(foreign.reject, Some(RejectReason::Locked));
    assert_eq!(incomplete.reject, None);
    assert_eq!(
        stockpile(&world),
        (200, 250, 150),
        "the Shelter paid 50 Scrap"
    );
    assert!(world.resource::<Underground>().levels[0].slots[0].is_some());
}

/// Every reset path (restart, fixture reset, benchmark) reseeds through
/// `seed_skirmish` after the one shared `clear_match_world` — the same
/// function the bridge calls, not a copied list. The clear removes the
/// underground before the reseed inserts a fresh default: a populated
/// world — completed levels plus an active dig — reads depth zero right
/// after the clear, and the reseed keeps it at the seeded default.
#[test]
fn match_clear_then_reseed_returns_a_populated_underground_to_default() {
    let (mut world, _map) = battlefield();
    world.insert_resource(Underground {
        levels: one_level([completed_room(RoomKind::Shelter), None, None, None]),
        excavation: Some(Excavation {
            elapsed_seconds: 12.0,
        }),
    });

    clear_match_world(&mut world);
    assert!(
        world.get_resource::<Underground>().is_none(),
        "the clear itself removes the underground"
    );

    let fixture = MapFixture::battlefield();
    let mut fresh_map = fixture.map.clone();
    seed_skirmish(&mut world, &mut fresh_map, &fixture);
    assert_eq!(world.resource::<Underground>(), &Underground::default());
}

/// The checkpoint-3 proof journey: three paid digs reach L3 through the
/// canonical tick, a paid Geothermal Tap and Lab complete there, and the
/// authoritative heat snapshot and research rate carry their combined
/// effects (+15 supply, +1 point/s); the shared match clear + reseed then
/// returns everything to the seeded default.
#[test]
fn paid_journey_to_l3_tap_and_lab_through_the_canonical_tick() {
    let (mut world, mut map) = battlefield();
    // Fund the three digs (100/50 + 160/80 + 256/128 Scrap/Coal) and the
    // L3 rooms (Tap 120, Lab 150 Scrap); Food is never charged.
    set_scrap_and_coal(&mut world, 900, 500);

    // Level 1: 100 Scrap / 50 Coal, a 30 s dig through the canonical tick.
    assert_eq!(excavate(&mut world, &mut map, TEAM).reject, None);
    assert_eq!(stockpile(&world), (200, 800, 450));
    for _ in 0..700 {
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<Underground>().depth() == 1 {
            break;
        }
    }
    assert_eq!(world.resource::<Underground>().depth(), 1);

    // Level 2: 160 / 80, a 45 s dig.
    assert_eq!(excavate(&mut world, &mut map, TEAM).reject, None);
    assert_eq!(stockpile(&world), (200, 640, 370));
    for _ in 0..1000 {
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<Underground>().depth() == 2 {
            break;
        }
    }
    assert_eq!(world.resource::<Underground>().depth(), 2);

    // Level 3: 256 / 128, a 60 s dig.
    assert_eq!(excavate(&mut world, &mut map, TEAM).reject, None);
    assert_eq!(stockpile(&world), (200, 384, 242));
    for _ in 0..1300 {
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<Underground>().depth() == 3 {
            break;
        }
    }
    assert_eq!(world.resource::<Underground>().depth(), 3);

    // Paid rooms on L3, building concurrently through their 30 s: the Tap
    // (120 Scrap) and the Lab (150 Scrap).
    assert_eq!(
        build_room(&mut world, &mut map, TEAM, 3, 0, RoomKind::GeothermalTap).reject,
        None
    );
    assert_eq!(
        build_room(&mut world, &mut map, TEAM, 3, 1, RoomKind::Lab).reject,
        None
    );
    assert_eq!(stockpile(&world), (200, 114, 242));
    for _ in 0..700 {
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<Underground>().levels[2].slots[1]
            .as_ref()
            .is_some_and(Room::complete)
        {
            break;
        }
    }
    assert!(
        world.resource::<Underground>().levels[2]
            .slots
            .iter()
            .flatten()
            .all(Room::complete),
        "both L3 rooms completed"
    );

    // Authoritative snapshots carry the completed rooms' effects: the
    // seeded core's 15 supply plus the L3 Tap's +15 against the core's 10
    // demand, and the base surface rate plus the Lab's additive +1 point/s.
    let heat = world.resource::<HeatState>();
    assert_eq!((heat.supply, heat.demand), (30, 10));
    assert_eq!(heat.efficiency(), 1.0);
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND + 1.0);

    // The shared restart path: the one match clear, then the shared seed
    // — the underground returns to its default and heat/research with it.
    clear_match_world(&mut world);
    let fixture = MapFixture::battlefield();
    let mut fresh_map = fixture.map.clone();
    seed_skirmish(&mut world, &mut fresh_map, &fixture);
    assert_eq!(world.resource::<Underground>(), &Underground::default());
    assert_eq!(*world.resource::<HeatState>(), HeatState::default());
    assert_eq!(research_rate(&world), RESEARCH_BASE_PER_SECOND);
}
