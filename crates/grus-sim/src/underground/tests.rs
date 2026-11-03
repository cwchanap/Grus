use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{BuildingKind, ROOM_SLOTS_PER_LEVEL, RoomKind, room_spec};
use crate::economy::{ResourceStockpile, TeamEconomy};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, TeamId};
use crate::map::GridMap;
use crate::movement::SIM_STEP_SECONDS;
use crate::session::{MatchPhase, MatchResult, MatchSession, set_paused};

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

/// A room placed with its full build time already elapsed, i.e. complete.
fn completed_room(kind: RoomKind) -> Option<Room> {
    let mut room = Room::new(kind);
    room.elapsed_seconds = room_spec(kind).build_seconds as f32;
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

/// Completion is derived from elapsed time against the catalogue build
/// duration — never a stored flag, so nothing can complete twice.
#[test]
fn room_completion_derives_from_elapsed_build_time() {
    for kind in RoomKind::ALL {
        let mut room = Room::new(kind);
        assert!(!room.complete(), "{kind:?} starts incomplete");
        room.elapsed_seconds = room_spec(kind).build_seconds as f32;
        assert!(room.complete(), "{kind:?} completes at the catalogue time");
    }
}

/// Stepping a world with no underground state is a harmless no-op.
#[test]
fn a_world_without_underground_state_steps_as_a_no_op() {
    let (mut world, _map) = battlefield();
    let before = stockpile(&world);
    step_underground(&mut world, SIM_STEP_SECONDS);
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
                .output_remainder,
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
                .output_remainder,
            0.0,
            "output frozen for seconds {seconds}"
        );
        assert_eq!(stockpile(&world).0, 200, "no credits for seconds {seconds}");
    }
}

/// Session-free worlds read as Playing (the benchmark convention), but
/// progress still requires a living completed Team-1 core: with no core
/// index at all or an incomplete core every timer stalls; the identical
/// world with a completed core plays.
#[test]
fn session_free_worlds_stall_without_a_living_completed_core() {
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

    for core in [Core::None, Core::Incomplete] {
        let mut world = minimal_world(core);
        world.insert_resource(stalled_underground());
        step_underground(&mut world, 30.0);
        let underground = world.resource::<Underground>();
        assert_eq!(underground.depth(), 1, "{core:?} core stalls the dig");
        assert_eq!(
            underground.excavation.as_ref().unwrap().elapsed_seconds,
            29.5
        );
        assert_eq!(stockpile(&world).0, 0, "{core:?} core stalls the output");
    }

    // The same minimal world with a living completed core plays: the dig
    // completes (29.5 + 16 ≥ 45 s for level 2) and the Greenhouse banks
    // 0.5 × 16 = 8 Food.
    let mut world = minimal_world(Core::Complete);
    world.insert_resource(stalled_underground());
    step_underground(&mut world, 16.0);
    assert_eq!(world.resource::<Underground>().depth(), 2);
    assert_eq!(stockpile(&world).0, 8);
}
