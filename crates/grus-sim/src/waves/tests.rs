use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::catalog::{MAX_LIVE_ALIENS, unit_spec};
use crate::combat::{CombatOrder, CombatTarget, Health};
use crate::commands::UnitIndex;
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::session::{MatchPhase, MatchSession, resolve_result, set_paused, start_match};

fn seeded_world() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

fn unit_cells(world: &World, map: &GridMap) -> std::collections::HashSet<GridPos> {
    world
        .resource::<UnitIndex>()
        .iter()
        .filter_map(|(_, entity)| {
            world
                .get::<SimPosition>(*entity)
                .map(|position| map.world_to_cell(position.current))
        })
        .collect()
}

fn add_team_two_aliens(world: &mut World, map: &GridMap, count: usize) -> Vec<UnitId> {
    let mut occupied = unit_cells(world, map);
    let mut spawned = Vec::with_capacity(count);
    for y in 0..map.height() {
        for x in 0..map.width() {
            let cell = GridPos::new(x, y);
            if !map.is_walkable(cell) || !occupied.insert(cell) {
                continue;
            }
            let id = world.resource_mut::<IdAllocator>().allocate_unit();
            let entity = crate::commands::spawn_unit(
                world,
                id,
                TeamId(2),
                map.cell_center(cell),
                UnitKind::Swarmer,
                unit_spec(UnitKind::Swarmer).speed,
            );
            world.entity_mut(entity).insert(CombatOrder::Attack {
                target: CombatTarget::Building(BuildingId(1)),
                last_target_cell: None,
            });
            spawned.push(id);
            if spawned.len() == count {
                return spawned;
            }
        }
    }
    panic!("fixture has enough walkable cells for the live-alien cap test");
}

#[test]
fn seeded_wave_spawns_distinct_aliens_and_dispatches_attack_moves_in_the_launch_tick() {
    let (mut world, mut map) = seeded_world();
    let occupied_before = unit_cells(&world, &map);
    world.insert_resource(WaveState {
        elapsed: 299.95,
        next_wave: 1,
    });

    step_waves(&mut world, &mut map, 0.05);

    let ids = live_alien_ids(&world, TeamId(2));
    assert_eq!(world.resource::<WaveState>().next_wave, 2);
    assert_eq!(ids.len(), 12);
    let mut spawned_cells = std::collections::HashSet::new();
    for id in ids {
        let entity = world.resource::<UnitIndex>().entity(id).unwrap();
        let cell = map.world_to_cell(world.get::<SimPosition>(entity).unwrap().current);
        assert!(!occupied_before.contains(&cell));
        assert!(
            spawned_cells.insert(cell),
            "wave spawn cells must be unique"
        );
        assert!(matches!(
            world.get::<CombatOrder>(entity),
            Some(CombatOrder::AttackMove { target: None, .. })
        ));
        assert!(world.get::<MoveOrder>(entity).is_some());
    }
}

#[test]
fn wave_spawn_slides_off_a_blocked_entry_anchor_to_the_nearest_free_cell() {
    let (mut world, mut map) = seeded_world();
    // A building placed over the North anchor — wave 1's only zone — must
    // not swallow the spawn: the anchor is offered first by
    // destination_slots, so a walkability gap there would drop the alien
    // inside the footprint where no route can ever start.
    map.set_blocked_rect(GridPos::new(63, 0), GridPos::new(65, 2));
    assert!(!map.is_walkable(EntryZone::North.anchor()));
    world.insert_resource(WaveState {
        elapsed: 299.95,
        next_wave: 1,
    });

    step_waves(&mut world, &mut map, 0.05);

    let ids = live_alien_ids(&world, TeamId(2));
    assert_eq!(ids.len(), 12);
    for id in ids {
        let entity = world.resource::<UnitIndex>().entity(id).unwrap();
        let cell = map.world_to_cell(world.get::<SimPosition>(entity).unwrap().current);
        assert!(map.is_walkable(cell), "alien spawned on blocked {cell:?}");
        assert!(world.get::<MoveOrder>(entity).is_some());
        assert!(matches!(
            world.get::<CombatOrder>(entity),
            Some(CombatOrder::AttackMove { target: None, .. })
        ));
    }
}

#[test]
fn rejected_wave_routes_retry_once_per_second_after_the_path_opens() {
    let (mut world, mut map) = seeded_world();
    for x in 0..map.width() {
        map.set_blocked(GridPos::new(x, 40), true);
    }
    world.insert_resource(WaveState {
        elapsed: 299.95,
        next_wave: 1,
    });

    step_waves(&mut world, &mut map, 0.05);
    let ids = live_alien_ids(&world, TeamId(2));
    assert_eq!(ids.len(), 12);
    assert!(ids.iter().all(|id| {
        let entity = world.resource::<UnitIndex>().entity(*id).unwrap();
        world.get::<CombatOrder>(entity).is_none() && world.get::<MoveOrder>(entity).is_none()
    }));
    let rejected_route_calls = map.path_call_count();
    assert!(rejected_route_calls > 0);

    for x in 0..map.width() {
        map.set_blocked(GridPos::new(x, 40), false);
    }
    for _ in 0..19 {
        step_waves(&mut world, &mut map, 0.05);
    }
    assert_eq!(map.path_call_count(), rejected_route_calls);
    assert!(ids.iter().all(|id| {
        let entity = world.resource::<UnitIndex>().entity(*id).unwrap();
        world.get::<MoveOrder>(entity).is_none()
    }));

    step_waves(&mut world, &mut map, 0.05);
    assert!(map.path_call_count() > rejected_route_calls);
    let failed: Vec<_> = ids
        .iter()
        .filter(|id| {
            let entity = world.resource::<UnitIndex>().entity(**id).unwrap();
            world.get::<MoveOrder>(entity).is_none()
        })
        .collect();
    assert!(failed.is_empty(), "recovery rejected for {failed:?}");
}

#[test]
fn stranded_and_lost_attack_moves_recover_without_resetting_moving_or_engaged_aliens() {
    let (mut world, mut map) = seeded_world();
    world.insert_resource(WaveState {
        elapsed: 299.95,
        next_wave: 1,
    });
    step_waves(&mut world, &mut map, 0.05);
    let ids = live_alien_ids(&world, TeamId(2));

    let stranded = world.resource::<UnitIndex>().entity(ids[0]).unwrap();
    world.entity_mut(stranded).remove::<MoveOrder>();
    world.get_mut::<SimPosition>(stranded).unwrap().current = Vec2::new(20.5, 20.5);

    let lost_route = world.resource::<UnitIndex>().entity(ids[1]).unwrap();
    world.entity_mut(lost_route).remove::<MoveOrder>();
    world.entity_mut(lost_route).remove::<CombatOrder>();
    world.get_mut::<SimPosition>(lost_route).unwrap().current = Vec2::new(22.5, 20.5);

    let moving = world.resource::<UnitIndex>().entity(ids[2]).unwrap();
    world.get_mut::<SimPosition>(moving).unwrap().current = Vec2::new(24.5, 20.5);

    let engaged = world.resource::<UnitIndex>().entity(ids[3]).unwrap();
    world.entity_mut(engaged).remove::<MoveOrder>();
    world.entity_mut(engaged).insert(CombatOrder::AttackMove {
        destination: GridPos::new(20, 20),
        target: Some(CombatTarget::Building(BuildingId(1))),
        last_target_cell: None,
    });
    world.get_mut::<SimPosition>(engaged).unwrap().current = Vec2::new(26.5, 20.5);

    let in_range = world.resource::<UnitIndex>().entity(ids[4]).unwrap();
    world.entity_mut(in_range).remove::<MoveOrder>();
    world.entity_mut(in_range).insert(CombatOrder::AttackMove {
        destination: GridPos::new(20, 20),
        target: None,
        last_target_cell: None,
    });
    // In attack-move range of the authored Research Center standoff:
    // the RC anchors at (62,46), so cell (54,48) sits on its approach ring.
    let rc_anchor = MapFixture::team_starts()[0].research_center_anchor;
    world.get_mut::<SimPosition>(in_range).unwrap().current =
        map.cell_center(GridPos::new(rc_anchor.x - 8, rc_anchor.y + 2));

    world.insert_resource(WaveState {
        elapsed: 300.95,
        next_wave: 2,
    });
    step_waves(&mut world, &mut map, 0.05);

    assert!(world.get::<MoveOrder>(stranded).is_some());
    assert!(world.get::<MoveOrder>(lost_route).is_some());
    assert!(world.get::<MoveOrder>(moving).is_some());
    assert!(world.get::<MoveOrder>(engaged).is_none());
    assert!(world.get::<MoveOrder>(in_range).is_none());
}

#[test]
fn live_cap_overflow_buffs_only_fresh_wave_units_and_full_cap_launches_nothing() {
    let (mut world, mut map) = seeded_world();
    let existing = add_team_two_aliens(&mut world, &map, MAX_LIVE_ALIENS as usize - 1);
    let prior_health = world
        .get::<Health>(world.resource::<UnitIndex>().entity(existing[0]).unwrap())
        .copied()
        .unwrap();
    world.insert_resource(WaveState {
        elapsed: 1200.0,
        next_wave: 10,
    });

    step_waves(&mut world, &mut map, 0.05);

    let ids = live_alien_ids(&world, TeamId(2));
    assert_eq!(ids.len(), MAX_LIVE_ALIENS as usize);
    let fresh_id = *ids.last().unwrap();
    assert!(!existing.contains(&fresh_id));
    let fresh = world.resource::<UnitIndex>().entity(fresh_id).unwrap();
    assert_eq!(world.get::<Unit>(fresh).unwrap().kind, UnitKind::Behemoth);
    assert_eq!(world.get::<Health>(fresh).unwrap().max, 4400);
    assert_eq!(world.get::<Health>(fresh).unwrap().current, 4400);
    assert_eq!(
        world.get::<Health>(world.resource::<UnitIndex>().entity(existing[0]).unwrap()),
        Some(&prior_health)
    );

    let (mut full_world, mut full_map) = seeded_world();
    let full = add_team_two_aliens(&mut full_world, &full_map, MAX_LIVE_ALIENS as usize);
    let next_unit = full_world.resource::<IdAllocator>().next_unit;
    let full_prior_health = full_world
        .get::<Health>(full_world.resource::<UnitIndex>().entity(full[0]).unwrap())
        .copied()
        .unwrap();
    full_world.insert_resource(WaveState {
        elapsed: 1200.0,
        next_wave: 10,
    });

    step_waves(&mut full_world, &mut full_map, 0.05);

    assert_eq!(
        live_alien_ids(&full_world, TeamId(2)).len(),
        MAX_LIVE_ALIENS as usize
    );
    assert_eq!(full_world.resource::<IdAllocator>().next_unit, next_unit);
    assert_eq!(full_world.resource::<WaveState>().next_wave, 11);
    assert_eq!(
        full_world.get::<Health>(full_world.resource::<UnitIndex>().entity(full[0]).unwrap()),
        Some(&full_prior_health)
    );
}

#[test]
fn integer_wave_budgets_and_ordered_rosters_are_pinned() {
    assert_eq!(wave_budget(1), 12);
    assert_eq!(wave_budget(4), 30);
    assert_eq!(wave_budget(8), 65);
    assert_eq!(wave_budget(10), 88);

    let wave1 = plan_roster(1);
    assert_eq!(wave1.swarmers, 12);
    assert_eq!(wave1.total_threat(), 12);

    let wave4 = plan_roster(4);
    assert_eq!(
        (
            wave4.behemoths,
            wave4.brutes,
            wave4.spitters,
            wave4.swarmers
        ),
        (0, 0, 5, 15)
    );
    assert_eq!(wave4.total_threat(), 30);

    let wave8 = plan_roster(8);
    assert_eq!(
        (
            wave8.behemoths,
            wave8.brutes,
            wave8.spitters,
            wave8.swarmers
        ),
        (0, 4, 3, 32)
    );
    assert_eq!(wave8.total_threat(), 65);

    let wave10 = plan_roster(10);
    assert_eq!(
        (
            wave10.behemoths,
            wave10.brutes,
            wave10.spitters,
            wave10.swarmers
        ),
        (1, 3, 2, 24)
    );
    assert_eq!(wave10.total_threat(), 88);
    assert_eq!(
        wave10.ordered_prefix(7),
        [
            UnitKind::Behemoth,
            UnitKind::Brute,
            UnitKind::Brute,
            UnitKind::Brute,
            UnitKind::Spitter,
            UnitKind::Spitter,
            UnitKind::Swarmer,
        ]
    );

    let capped_wave = plan_roster(100).ordered_prefix(MAX_LIVE_ALIENS as usize);
    assert_eq!(capped_wave.len(), MAX_LIVE_ALIENS as usize);
    assert_eq!(capped_wave[0], UnitKind::Behemoth);
    assert!(capped_wave[1..].iter().all(|kind| *kind == UnitKind::Brute));
}

#[test]
fn roster_unlocks_and_budget_overflow_are_deterministic() {
    let wave1 = plan_roster(1);
    assert_eq!((wave1.brutes, wave1.spitters, wave1.behemoths), (0, 0, 0));

    let wave4 = plan_roster(4);
    assert_eq!((wave4.brutes, wave4.behemoths), (0, 0));
    assert!(wave4.spitters > 0);

    let wave8 = plan_roster(8);
    assert!(wave8.brutes > 0);
    assert_eq!(wave8.behemoths, 0);

    assert_eq!(wave_budget(u64::MAX), u64::MAX);
    let overflow_roster = plan_roster(u64::MAX);
    assert_eq!(overflow_roster.total_threat(), u64::MAX);
    let capped_prefix = overflow_roster.ordered_prefix(usize::MAX);
    assert_eq!(capped_prefix.len(), MAX_LIVE_ALIENS as usize);
    assert!(capped_prefix.iter().all(|kind| *kind == UnitKind::Brute));
}

#[test]
fn zones_rotate_at_the_two_and_three_zone_breakpoints() {
    assert_eq!(zones_for(9), [EntryZone::North]);
    assert_eq!(zones_for(10), [EntryZone::East, EntryZone::South]);
    assert_eq!(zones_for(19), [EntryZone::South, EntryZone::West]);
    assert_eq!(
        zones_for(20),
        [EntryZone::West, EntryZone::North, EntryZone::East]
    );
}

#[test]
fn timer_warning_and_launch_edges_are_derived_and_exactly_once() {
    let world = World::new();
    let mut state = WaveState::default();

    assert_eq!(state.advance(&world, 269.0), 1..1);
    assert_eq!(state.remaining_seconds(), 31.0);
    assert_eq!(state.incoming_zones(), None);

    assert_eq!(state.advance(&world, 1.0), 1..1);
    assert_eq!(state.remaining_seconds(), 30.0);
    assert_eq!(state.incoming_zones(), Some(vec![EntryZone::North]));

    assert_eq!(state.advance(&world, 30.0), 1..2);
    assert_eq!(state.next_wave, 2);
    assert_eq!(state.remaining_seconds(), 100.0);
    assert_eq!(state.incoming_zones(), None);
    assert_eq!(state.advance(&world, 0.05), 2..2);

    assert_eq!(state.advance(&world, 200.0), 2..4);
    assert_eq!(state.next_wave, 4);
    assert_eq!(state.advance(&world, 0.0), 4..4);
}

#[test]
fn timer_freezes_outside_playing_and_uses_the_session_free_playing_convention() {
    let mut world = World::new();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    let mut state = WaveState::default();
    assert_eq!(state.advance(&world, 100.0), 1..1);
    assert_eq!(state.elapsed, 0.0);

    start_match(&mut world);
    assert_eq!(state.advance(&world, 100.0), 1..1);
    assert_eq!(state.elapsed, 100.0);

    set_paused(&mut world, true);
    assert_eq!(state.advance(&world, 100.0), 1..1);
    assert_eq!(state.elapsed, 100.0);
    set_paused(&mut world, false);
    assert_eq!(state.advance(&world, 200.0), 1..2);

    resolve_result(&mut world, TeamId(1));
    assert_eq!(state.advance(&world, 100.0), 2..2);
    assert_eq!(state.elapsed, 300.0);

    let mut session_free = WaveState::default();
    assert_eq!(session_free.advance(&World::new(), 300.0), 1..2);
}

#[test]
fn timer_rejects_non_finite_or_negative_deltas_without_rewinding() {
    let world = World::new();
    let mut state = WaveState::default();
    state.advance(&world, 12.0);
    assert_eq!(state.advance(&world, -5.0), 1..1);
    assert_eq!(state.advance(&world, f32::NAN), 1..1);
    assert_eq!(state.elapsed, 12.0);
}

#[test]
fn a_pathological_delta_launches_at_most_the_catch_up_limit() {
    let world = World::new();
    let mut state = WaveState::default();

    // A set_sim_speed-scale spike owes ~5e8 waves; the per-tick limit bounds
    // the work and the clock clamp absorbs the skipped backlog.
    let due = state.advance(&world, 1.0e12);
    assert_eq!(due, 1..(WAVE_CATCH_UP_LIMIT + 1));
    assert_eq!(state.next_wave, WAVE_CATCH_UP_LIMIT + 1);
    assert_eq!(state.remaining_seconds(), 0.0);

    // Cadence resumes immediately and one-at-a-time from the first
    // unlaunched deadline — no perpetual maximum-rate spew.
    assert_eq!(
        state.advance(&world, 0.05),
        (WAVE_CATCH_UP_LIMIT + 1)..(WAVE_CATCH_UP_LIMIT + 2)
    );
    assert_eq!(
        state.advance(&world, 0.05),
        (WAVE_CATCH_UP_LIMIT + 2)..(WAVE_CATCH_UP_LIMIT + 2)
    );
    let nearly_interval = f64::from(WAVE_INTERVAL_SECONDS) - 0.1;
    assert_eq!(
        state.advance(&world, nearly_interval as f32),
        (WAVE_CATCH_UP_LIMIT + 2)..(WAVE_CATCH_UP_LIMIT + 3)
    );
}
