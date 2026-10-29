use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::ConstructionState;
use crate::catalog::{BuildingKind, MAX_LIVE_ALIENS, building_spec, unit_spec};
use crate::combat::{CombatOrder, CombatTarget, Health, destroy_building, step_combat};
use crate::commands::{UnitIndex, spawn_unit};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SIM_STEP_SECONDS, SimPosition, Unit, step_movement};
use crate::session::{MatchPhase, MatchSession, resolve_result, set_paused, start_match};
use crate::visibility::{VisibilityMap, refresh_visibility, visible_to};

fn add_team_one_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
    complete: bool,
) -> bevy::prelude::Entity {
    let spec = building_spec(kind);
    let footprint = Footprint::new(anchor, spec.width, spec.height);
    for cell in footprint.cells() {
        if kind == BuildingKind::Gate {
            assert!(map.set_gate(cell));
        } else {
            assert!(map.set_blocked(cell, true));
        }
    }
    let entity = world
        .spawn((
            Building {
                id,
                team: TeamId(1),
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete,
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

fn add_gate_wall_ring(
    world: &mut World,
    map: &mut GridMap,
    footprint: Footprint,
    next_id: &mut u32,
) {
    let gate_cell = footprint.perimeter_cells()[0];
    for cell in footprint.perimeter_cells() {
        let kind = if cell == gate_cell {
            BuildingKind::Gate
        } else {
            BuildingKind::Wall
        };
        add_team_one_building(world, map, BuildingId(*next_id), kind, cell, true);
        *next_id += 1;
    }
}

fn add_alien(world: &mut World, id: UnitId, map: &GridMap, cell: GridPos) -> bevy::prelude::Entity {
    spawn_unit(
        world,
        id,
        TeamId(2),
        map.cell_center(cell),
        UnitKind::Swarmer,
        unit_spec(UnitKind::Swarmer).speed,
    )
}

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

#[test]
fn failed_core_route_uses_nearest_reachable_site_under_real_fog() {
    let mut world = World::new();
    let mut map = GridMap::new(48, 40);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 20));
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 18),
        true,
    );
    let unreachable_near_site = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(14, 20),
        true,
    );
    let unreachable_near_footprint = *world.get::<Footprint>(unreachable_near_site).unwrap();
    let reachable_tie_winner = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::Wall,
        GridPos::new(4, 33),
        false,
    );
    let reachable_tie_loser = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(4),
        BuildingKind::Wall,
        GridPos::new(4, 7),
        false,
    );
    for y in 0..map.height() {
        map.set_blocked(GridPos::new(13, y), true);
    }

    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    let winner_footprint = *world.get::<Footprint>(reachable_tie_winner).unwrap();
    assert!(
        !visible_to(&world, TeamId(2), winner_footprint),
        "the fallback site must begin outside real alien vision"
    );
    let near_slots = objective_slots(&map, unreachable_near_footprint);
    assert!(!near_slots.is_empty());
    assert!(near_slots.iter().all(|slot| {
        map.find_path(TeamId(2), GridPos::new(4, 20), *slot)
            .is_none()
    }));
    assert!(
        !world
            .get::<Building>(reachable_tie_winner)
            .unwrap()
            .construction
            .complete,
        "unfinished sites participate in structure selection"
    );

    retry_stranded_aliens(&mut world, &mut map);

    let order = world.get::<CombatOrder>(alien).unwrap();
    let CombatOrder::AttackMove {
        destination,
        target,
        ..
    } = order
    else {
        panic!("fallback must install an ordinary AttackMove")
    };
    assert_eq!(*target, None, "fallback never inserts a direct Attack");
    assert!(winner_footprint.is_immediately_adjacent(*destination));
    assert!(world.get::<FailedFallbackRevision>(alien).is_none());
    assert!(world.get::<MoveOrder>(alien).is_some());
    assert!(!visible_to(&world, TeamId(2), winner_footprint));
    assert_eq!(
        world.get::<Building>(unreachable_near_site).unwrap().id,
        BuildingId(2)
    );
    assert!(
        !world
            .get::<Footprint>(reachable_tie_loser)
            .unwrap()
            .is_immediately_adjacent(*destination),
        "equal-distance sites use the lower stable BuildingId"
    );

    for _ in 0..1000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        refresh_visibility(&mut world, &map);
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<CombatOrder>(alien),
            Some(CombatOrder::AttackMove {
                target: Some(CombatTarget::Building(BuildingId(3))),
                ..
            })
        ) {
            break;
        }
        assert!(matches!(
            world.get::<CombatOrder>(alien),
            Some(CombatOrder::AttackMove { target: None, .. })
        ));
    }
    assert!(visible_to(&world, TeamId(2), winner_footprint));
    assert!(matches!(
        world.get::<CombatOrder>(alien),
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Building(BuildingId(3))),
            ..
        })
    ));
}

#[test]
fn gate_in_a_full_wall_ring_is_never_an_alien_route_through() {
    let mut world = World::new();
    let mut map = GridMap::new(48, 48);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 23));
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(22, 22),
        true,
    );

    let gate_cell = GridPos::new(23, 20);
    let mut next_id = 2;
    let mut ring_walls = Vec::new();
    for y in 20..28 {
        for x in 20..28 {
            if x != 20 && x != 27 && y != 20 && y != 27 {
                continue;
            }
            let cell = GridPos::new(x, y);
            let kind = if cell == gate_cell {
                BuildingKind::Gate
            } else {
                BuildingKind::Wall
            };
            let entity =
                add_team_one_building(&mut world, &mut map, BuildingId(next_id), kind, cell, true);
            if kind == BuildingKind::Wall {
                ring_walls.push(entity);
            }
            next_id += 1;
        }
    }
    assert!(!map.is_walkable(gate_cell));
    assert!(!map.is_traversable_for(TeamId(2), gate_cell));
    assert!(map.is_traversable_for(TeamId(1), gate_cell));
    assert!(
        map.find_path(TeamId(2), GridPos::new(4, 23), GridPos::new(21, 23))
            .is_none(),
        "the wall ring, including its Gate, seals the core route"
    );
    assert!(
        map.find_path(TeamId(1), GridPos::new(4, 23), GridPos::new(21, 23))
            .is_some(),
        "the Gate remains a valid intermediate route for humans"
    );
    assert!(!ring_walls.is_empty());

    retry_stranded_aliens(&mut world, &mut map);

    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("a sealed ring still receives a normal AttackMove")
    };
    assert!(
        ring_walls.iter().any(|wall| {
            world
                .get::<Footprint>(*wall)
                .is_some_and(|footprint| footprint.is_immediately_adjacent(*destination))
        }),
        "the chosen objective is a reachable wall on the outside of the ring"
    );
    assert!(world.get::<MoveOrder>(alien).is_some());
}

#[test]
fn retry_preserves_an_accepted_fallback_order_waiting_at_a_hidden_slot() {
    let mut world = World::new();
    let mut map = GridMap::new(48, 48);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 23));
    // The sealed core sits far away so its standoff footprint cannot cover
    // the forward wall's approach slot through the `in_core_range` skip. The
    // ring is two cells thick: `plan_move_route`'s four-candidate
    // destination scan must stay inside it, or the objective dispatch
    // escapes to an outer cell and the fallback never runs.
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(38, 38),
        true,
    );
    let mut next_id = 3;
    for y in 35..45 {
        for x in 35..45 {
            if x > 36 && x < 43 && y > 36 && y < 43 {
                continue;
            }
            add_team_one_building(
                &mut world,
                &mut map,
                BuildingId(next_id),
                BuildingKind::Wall,
                GridPos::new(x, y),
                true,
            );
            next_id += 1;
        }
    }
    let forward_wall = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(16, 16),
        true,
    );
    let forward_footprint = *world.get::<Footprint>(forward_wall).unwrap();

    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    assert!(
        !visible_to(&world, TeamId(2), forward_footprint),
        "the fallback structure begins outside real alien vision"
    );

    retry_stranded_aliens(&mut world, &mut map);
    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("a sealed core still falls back to a reachable wall")
    };
    let fallback_slot = *destination;
    let objective = world.get::<FallbackObjective>(alien).unwrap().0;
    assert_eq!(objective, BuildingId(2), "the nearest reachable structure");
    assert!(forward_footprint.is_immediately_adjacent(fallback_slot));
    assert!(
        !visible_to(&world, TeamId(2), forward_footprint),
        "marching does not run the end-of-tick reveal"
    );

    // March to the approach slot without combat or visibility steps: the
    // structure stays hidden, so the order sits pre-acquisition exactly as
    // it does on the arrival tick in a live run.
    for _ in 0..2000 {
        if world.get::<MoveOrder>(alien).is_none() {
            break;
        }
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
    }
    assert!(world.get::<MoveOrder>(alien).is_none());
    assert_eq!(
        map.world_to_cell(world.get::<SimPosition>(alien).unwrap().current),
        fallback_slot,
        "the route must finish on the accepted approach slot"
    );
    assert!(!visible_to(&world, TeamId(2), forward_footprint));

    // A retry boundary inside the pre-acquisition window must not replace
    // the order or re-run the objective/fallback probes.
    let order_at_slot = world.get::<CombatOrder>(alien).unwrap().clone();
    let calls_at_slot = map.path_call_count();
    retry_stranded_aliens(&mut world, &mut map);
    retry_stranded_aliens(&mut world, &mut map);
    assert_eq!(map.path_call_count(), calls_at_slot);
    assert_eq!(
        world.get::<CombatOrder>(alien).unwrap(),
        &order_at_slot,
        "a live fallback objective keeps the targetless AttackMove"
    );
    assert_eq!(world.get::<FallbackObjective>(alien).unwrap().0, objective);
    assert!(world.get::<MoveOrder>(alien).is_none());

    // The held order still acquires on the next real fog reveal.
    refresh_visibility(&mut world, &map);
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(matches!(
        world.get::<CombatOrder>(alien),
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Building(id)),
            ..
        }) if *id == objective
    ));

    // A dead recorded structure releases the alien back to ordinary retry:
    // the sealed core stays unreachable, so fallback re-dispatches to the
    // next nearest reachable ring wall and re-records the objective.
    let objective_entity = world.resource::<BuildingIndex>().entity(objective).unwrap();
    destroy_building(&mut world, &mut map, objective_entity);
    world.entity_mut(alien).insert(CombatOrder::AttackMove {
        destination: fallback_slot,
        target: None,
        last_target_cell: None,
    });
    retry_stranded_aliens(&mut world, &mut map);
    let CombatOrder::AttackMove {
        destination: retasked,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("a dead fallback objective returns the alien to retry")
    };
    let ring_walls: Vec<Footprint> = (3..next_id)
        .filter_map(|id| {
            world
                .resource::<BuildingIndex>()
                .entity(BuildingId(id))
                .and_then(|entity| world.get::<Footprint>(entity).copied())
        })
        .collect();
    assert!(
        ring_walls
            .iter()
            .any(|footprint| footprint.is_immediately_adjacent(*retasked)),
        "the retasked objective is another reachable ring wall"
    );
    assert_ne!(
        world.get::<FallbackObjective>(alien).unwrap().0,
        objective,
        "the retask records the new structure, not the dead one"
    );
}

#[test]
fn retry_retasks_a_fallback_waiter_parked_outside_acquisition_range() {
    let mut world = World::new();
    let mut map = GridMap::new(48, 48);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 23));
    // Same sealed core as the preservation test: the objective dispatch
    // rejects Unreachable, so the fallback path runs.
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(38, 38),
        true,
    );
    let mut next_id = 3;
    for y in 35..45 {
        for x in 35..45 {
            if x > 36 && x < 43 && y > 36 && y < 43 {
                continue;
            }
            add_team_one_building(
                &mut world,
                &mut map,
                BuildingId(next_id),
                BuildingKind::Wall,
                GridPos::new(x, y),
                true,
            );
            next_id += 1;
        }
    }
    let forward_wall = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(16, 16),
        true,
    );
    let forward_footprint = *world.get::<Footprint>(forward_wall).unwrap();

    retry_stranded_aliens(&mut world, &mut map);
    assert_eq!(
        world.get::<FallbackObjective>(alien).unwrap().0,
        BuildingId(2)
    );

    // Reservation crowding can push `plan_move_route`'s candidate scan past
    // the perimeter, so the accepted goal lands outside the recorded
    // structure's acquisition reach. Stand the alien on that dead route end
    // exactly as arrival leaves it: no MoveOrder, targetless AttackMove
    // anchored to the accepted cell.
    let parked = GridPos::new(5, 5);
    assert!(map.is_walkable(parked));
    let parked_center = map.cell_center(parked);
    assert!(
        parked_center.distance(forward_footprint.closest_point(parked_center)) > ATTACK_MOVE_RADIUS,
        "the substituted destination must sit outside acquisition reach"
    );
    world.entity_mut(alien).insert(CombatOrder::AttackMove {
        destination: parked,
        target: None,
        last_target_cell: None,
    });
    world.entity_mut(alien).remove::<MoveOrder>();
    world.get_mut::<SimPosition>(alien).unwrap().current = parked_center;

    retry_stranded_aliens(&mut world, &mut map);

    // Out of the recorded structure's reach the parked alien is stranded,
    // not tasked: the retry re-dispatches toward a reachable structure
    // instead of preserving the dead route end.
    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("the retasked alien keeps an ordinary AttackMove")
    };
    assert_ne!(*destination, parked);
    assert!(
        forward_footprint.is_immediately_adjacent(*destination),
        "the nearest reachable structure is the forward wall"
    );
}

#[test]
fn launch_rejected_core_attack_moves_fall_back_to_a_reachable_wall() {
    let mut world = World::new();
    world.insert_resource(IdAllocator::new(1, 100, 1));
    let mut map = GridMap::new(128, 96);
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(62, 46),
        true,
    );
    let mut next_id = 2;
    let mut wall_footprints = Vec::new();
    for y in 44..52 {
        for x in 60..68 {
            if x != 60 && x != 67 && y != 44 && y != 51 {
                continue;
            }
            let cell = GridPos::new(x, y);
            let kind = if cell == GridPos::new(63, 44) {
                BuildingKind::Gate
            } else {
                BuildingKind::Wall
            };
            let entity =
                add_team_one_building(&mut world, &mut map, BuildingId(next_id), kind, cell, true);
            wall_footprints.push(*world.get::<Footprint>(entity).unwrap());
            next_id += 1;
        }
    }

    launch_wave(&mut world, &mut map, 1);

    let aliens = live_alien_ids(&world, TeamId(2));
    assert_eq!(aliens.len(), 12);
    let assigned = aliens
        .iter()
        .filter_map(|id| {
            let entity = world.resource::<UnitIndex>().entity(*id)?;
            let order = world.get::<CombatOrder>(entity)?;
            let CombatOrder::AttackMove {
                destination,
                target: None,
                ..
            } = order
            else {
                return None;
            };
            Some((entity, *destination))
        })
        .collect::<Vec<_>>();
    assert!(!assigned.is_empty());
    assert!(
        assigned.iter().all(|(entity, destination)| {
            world.get::<MoveOrder>(*entity).is_some()
                && wall_footprints.iter().any(|footprint| {
                    let point = map.cell_center(*destination);
                    footprint.closest_point(point).distance(point) <= ATTACK_MOVE_RADIUS
                })
        }),
        "assigned={assigned:?}, routes={:?}",
        assigned
            .iter()
            .map(|(entity, _)| world
                .get::<MoveOrder>(*entity)
                .map(|order| (order.goal, order.waypoints.len())))
            .collect::<Vec<_>>()
    );
}

#[test]
fn launch_uses_distance_ranked_fallback_when_core_has_no_approach_slots() {
    let mut world = World::new();
    world.insert_resource(IdAllocator::new(1, 100, 1));
    let mut map = GridMap::new(128, 96);
    let core = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(62, 46),
        true,
    );
    let far_low_id = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(18, 70),
        true,
    );
    let near_high_id = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(99),
        BuildingKind::Wall,
        GridPos::new(62, 9),
        true,
    );
    let core_footprint = *world.get::<Footprint>(core).unwrap();
    let mut next_id = 100;
    add_gate_wall_ring(&mut world, &mut map, core_footprint, &mut next_id);
    assert!(objective_slots(&map, core_footprint).is_empty());

    launch_wave(&mut world, &mut map, 1);

    let near_footprint = *world.get::<Footprint>(near_high_id).unwrap();
    let far_footprint = *world.get::<Footprint>(far_low_id).unwrap();
    let assignments = live_alien_ids(&world, TeamId(2))
        .into_iter()
        .filter_map(|id| {
            let entity = world.resource::<UnitIndex>().entity(id)?;
            let CombatOrder::AttackMove { destination, .. } = world.get::<CombatOrder>(entity)?
            else {
                return None;
            };
            Some(*destination)
        })
        .collect::<Vec<_>>();
    assert!(!assignments.is_empty());
    assert!(assignments.iter().all(|destination| {
        let point = map.cell_center(*destination);
        near_footprint.closest_point(point).distance_squared(point)
            < far_footprint.closest_point(point).distance_squared(point)
    }));
}

#[test]
fn retry_uses_distance_ranked_fallback_when_core_has_no_approach_slots() {
    let mut world = World::new();
    let mut map = GridMap::new(48, 40);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 20));
    let core = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 18),
        true,
    );
    let far_low_id = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(18, 5),
        true,
    );
    let near_high_id = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(99),
        BuildingKind::Wall,
        GridPos::new(10, 20),
        true,
    );
    let core_footprint = *world.get::<Footprint>(core).unwrap();
    let mut next_id = 100;
    add_gate_wall_ring(&mut world, &mut map, core_footprint, &mut next_id);
    assert!(objective_slots(&map, core_footprint).is_empty());

    retry_stranded_aliens(&mut world, &mut map);

    let near_footprint = *world.get::<Footprint>(near_high_id).unwrap();
    let far_footprint = *world.get::<Footprint>(far_low_id).unwrap();
    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("the nearest reachable structure receives a normal AttackMove")
    };
    assert!(near_footprint.is_immediately_adjacent(*destination));
    assert!(!far_footprint.is_immediately_adjacent(*destination));
}

#[test]
fn acquired_core_route_falls_back_after_a_map_change_seals_its_pursuit() {
    let mut world = World::new();
    let mut map = GridMap::new(40, 32);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(23, 14));
    let core = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 14),
        true,
    );
    let core_footprint = *world.get::<Footprint>(core).unwrap();
    let (_, core_slots) = alien_objective(&world, &map).unwrap();
    assert!(command_accepted(
        &dispatch_attack_move(&mut world, &mut map, UnitId(1), core_slots[0]),
        UnitId(1)
    ));
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(2), core_footprint));
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(matches!(
        world.get::<CombatOrder>(alien),
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Building(BuildingId(1))),
            ..
        })
    ));
    assert!(world.get::<MoveOrder>(alien).is_some());

    for y in 0..map.height() {
        if y != 14 {
            map.set_blocked(GridPos::new(25, y), true);
        }
    }
    let fallback = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::Wall,
        GridPos::new(25, 14),
        true,
    );
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(
        world.get::<MoveOrder>(alien).unwrap().last_failed_replan,
        Some(map.revision())
    );

    retry_stranded_aliens(&mut world, &mut map);

    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("an acquired but unreachable core pursuit must use fallback")
    };
    assert!(
        world
            .get::<Footprint>(fallback)
            .unwrap()
            .is_immediately_adjacent(*destination)
    );
    assert!(world.get::<MoveOrder>(alien).is_some());
    assert!(
        world
            .get::<MoveOrder>(alien)
            .unwrap()
            .last_failed_replan
            .is_none()
    );
}

#[test]
fn preserved_core_route_falls_back_after_a_wall_closes_its_only_path_and_recovers_after_breach() {
    let mut world = World::new();
    let mut map = GridMap::new(40, 32);
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(4, 16));
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 14),
        true,
    );
    let far_wall = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(14, 16),
        true,
    );
    let (_, core_slots) = alien_objective(&world, &map).unwrap();
    assert!(command_accepted(
        &dispatch_attack_move(&mut world, &mut map, UnitId(1), core_slots[0]),
        UnitId(1)
    ));
    assert!(world.get::<MoveOrder>(alien).is_some());

    // The route crosses this one-cell gap. Building a Wall there invalidates
    // the core order while movement deliberately preserves its failed route.
    for y in 0..map.height() {
        if y != 16 {
            map.set_blocked(GridPos::new(12, y), true);
        }
    }
    let breach_wall = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::Wall,
        GridPos::new(12, 16),
        true,
    );
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    assert_eq!(
        world.get::<MoveOrder>(alien).unwrap().last_failed_replan,
        Some(map.revision())
    );
    let failed_core_goal = world.get::<MoveOrder>(alien).unwrap().goal;

    world.insert_resource(WaveState {
        elapsed: 20.95,
        next_wave: 2,
    });
    step_waves(&mut world, &mut map, 0.05);
    let CombatOrder::AttackMove {
        destination,
        target: None,
        ..
    } = world.get::<CombatOrder>(alien).unwrap()
    else {
        panic!("the failed core route must be replaced with an AttackMove")
    };
    assert!(
        world
            .get::<Footprint>(breach_wall)
            .unwrap()
            .is_immediately_adjacent(*destination)
    );
    assert_ne!(*destination, failed_core_goal);
    assert_eq!(
        world.get::<MoveOrder>(alien).unwrap().last_failed_replan,
        None,
        "the accepted fallback replaces the preserved core route"
    );

    let fallback_slot = *destination;
    world.get_mut::<SimPosition>(alien).unwrap().current = map.cell_center(fallback_slot);
    world.get_mut::<Health>(breach_wall).unwrap().current = 1;
    world.get_mut::<Health>(far_wall).unwrap().current = 1;
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(3))
            .is_none()
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(matches!(
        world.get::<CombatOrder>(alien),
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Building(BuildingId(2))),
            ..
        })
    ));
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(2))
            .is_some()
    );
    assert!(
        map.is_walkable(GridPos::new(12, 16)),
        "destroying the fallback opens the route through the breach"
    );

    // Retain AttackMove's nearby wall chewing. Destroying that second site
    // ends its pursuit leg at the fallback destination, so the ordinary
    // one-second retry can assign the Research Center route again.
    for _ in 0..30 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_combat(&mut world, &mut map, 1.0);
        if world
            .resource::<BuildingIndex>()
            .entity(BuildingId(2))
            .is_none()
            && world.get::<MoveOrder>(alien).is_none()
            && !matches!(
                world.get::<CombatOrder>(alien),
                Some(CombatOrder::AttackMove {
                    target: Some(CombatTarget::Building(BuildingId(2))),
                    ..
                })
            )
        {
            break;
        }
    }
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(2))
            .is_none()
    );
    world.insert_resource(WaveState {
        elapsed: 21.95,
        next_wave: 2,
    });
    step_waves(&mut world, &mut map, 0.05);
    assert!(
        matches!(
            world.get::<CombatOrder>(alien),
            Some(CombatOrder::AttackMove { destination, .. })
                if Footprint::new(GridPos::new(30, 14), 4, 4).is_immediately_adjacent(*destination)
        ),
        "expected retry toward the Research Center, order={:?}, position={:?}, route={:?}",
        world.get::<CombatOrder>(alien),
        world.get::<SimPosition>(alien),
        world.get::<MoveOrder>(alien)
    );
    assert!(world.get::<MoveOrder>(alien).is_some());
}

#[test]
fn sealed_150_aliens_suppress_static_fallback_probes_until_the_map_changes() {
    let mut world = World::new();
    let mut map = GridMap::new(32, 32);
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(24, 14),
        true,
    );
    for y in 0..map.height() {
        map.set_blocked(GridPos::new(16, y), true);
    }
    let mut id = 1;
    for y in 0..map.height() {
        for x in 0..16 {
            let cell = GridPos::new(x, y);
            if id > MAX_LIVE_ALIENS {
                break;
            }
            add_alien(&mut world, UnitId(id), &map, cell);
            id += 1;
        }
        if id > MAX_LIVE_ALIENS {
            break;
        }
    }
    assert_eq!(
        live_alien_ids(&world, TeamId(2)).len(),
        MAX_LIVE_ALIENS as usize
    );

    retry_stranded_aliens(&mut world, &mut map);
    let calls_after_first_probe = map.path_call_count();
    println!(
        "HPA450_SEALED census={} initial_failed_probe_calls={}",
        live_alien_ids(&world, TeamId(2)).len(),
        calls_after_first_probe
    );
    assert!(calls_after_first_probe > 0);
    assert!(live_alien_ids(&world, TeamId(2)).into_iter().all(|id| {
        let entity = world.resource::<UnitIndex>().entity(id).unwrap();
        world
            .get::<FailedFallbackRevision>(entity)
            .is_some_and(|failed| failed.0 == map.revision())
    }));

    world.insert_resource(WaveState {
        elapsed: 20.0,
        next_wave: 2,
    });
    for _ in 0..3 {
        step_waves(&mut world, &mut map, 1.0);
        assert_eq!(
            map.path_call_count(),
            calls_after_first_probe,
            "unchanged failed revisions suppress core and fallback A*"
        );
    }

    map.set_blocked(GridPos::new(16, 16), false);
    let before_recovery = map.path_call_count();
    step_waves(&mut world, &mut map, 1.0);
    assert!(map.path_call_count() > before_recovery);
    assert!(live_alien_ids(&world, TeamId(2)).into_iter().any(|id| {
        world
            .resource::<UnitIndex>()
            .entity(id)
            .is_some_and(|entity| world.get::<MoveOrder>(entity).is_some())
    }));
}

#[test]
fn transiently_reserved_fallback_slots_do_not_cache_a_static_failure() {
    let mut world = World::new();
    let mut map = GridMap::new(16, 12);
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(12, 4),
        true,
    );
    let wall = add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Wall,
        GridPos::new(4, 4),
        true,
    );
    for y in 0..map.height() {
        map.set_blocked(GridPos::new(8, y), true);
    }
    let alien = add_alien(&mut world, UnitId(1), &map, GridPos::new(1, 4));
    let footprint = *world.get::<Footprint>(wall).unwrap();
    assert!(
        approach_slots(
            &map,
            footprint,
            &HashSet::new(),
            footprint.perimeter_cells().len()
        )
        .into_iter()
        .any(|slot| map.find_path(TeamId(2), GridPos::new(1, 4), slot).is_some())
    );

    let mut blocker_id = 2;
    for y in 0..map.height() {
        for x in 0..map.width() {
            let cell = GridPos::new(x, y);
            if map.is_walkable(cell) {
                add_alien(&mut world, UnitId(blocker_id), &map, cell);
                blocker_id += 1;
            }
        }
    }

    retry_stranded_aliens(&mut world, &mut map);

    assert!(world.get::<FailedFallbackRevision>(alien).is_none());
    assert!(world.get::<CombatOrder>(alien).is_none());
    assert!(world.get::<MoveOrder>(alien).is_none());
}

#[test]
fn siege_path_calls_are_measured_for_150_aliens_on_a_wall_ring_revision_bump() {
    let mut world = World::new();
    let mut map = GridMap::new(40, 40);
    add_team_one_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(18, 18),
        true,
    );
    let mut wall_cells = Vec::new();
    let mut building_id = 2;
    for y in 6..34 {
        for x in 6..34 {
            if x != 6 && x != 33 && y != 6 && y != 33 {
                continue;
            }
            let cell = GridPos::new(x, y);
            let entity = add_team_one_building(
                &mut world,
                &mut map,
                BuildingId(building_id),
                BuildingKind::Wall,
                cell,
                true,
            );
            wall_cells.push((cell, entity));
            building_id += 1;
        }
    }

    let mut start_cells = Vec::new();
    for along in 8..33 {
        for depth in 0..5 {
            start_cells.extend([
                GridPos::new(depth, along),
                GridPos::new(39 - depth, along),
                GridPos::new(along, depth),
                GridPos::new(along, 39 - depth),
            ]);
        }
    }
    for (index, cell) in start_cells
        .into_iter()
        .take(MAX_LIVE_ALIENS as usize)
        .enumerate()
    {
        add_alien(&mut world, UnitId(index as u32 + 1), &map, cell);
    }
    assert_eq!(
        live_alien_ids(&world, TeamId(2)).len(),
        MAX_LIVE_ALIENS as usize
    );

    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    retry_stranded_aliens(&mut world, &mut map);
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    let live = live_alien_ids(&world, TeamId(2));
    let attacking = live
        .iter()
        .filter(|id| {
            world
                .resource::<UnitIndex>()
                .entity(**id)
                .is_some_and(|entity| {
                    matches!(
                        world.get::<CombatOrder>(entity),
                        Some(CombatOrder::AttackMove { .. })
                    )
                })
        })
        .count();
    let engaged = live
        .iter()
        .filter(|id| {
            world
                .resource::<UnitIndex>()
                .entity(**id)
                .is_some_and(|entity| {
                    matches!(
                        world.get::<CombatOrder>(entity),
                        Some(CombatOrder::AttackMove {
                            target: Some(CombatTarget::Building(_)),
                            ..
                        })
                    )
                })
        })
        .count();
    let moving = live
        .iter()
        .filter(|id| {
            world
                .resource::<UnitIndex>()
                .entity(**id)
                .is_some_and(|entity| world.get::<MoveOrder>(entity).is_some())
        })
        .count();
    assert!(attacking > 0 && engaged > 0 && moving > 0);

    let wall = wall_cells
        .iter()
        .find(|(cell, _)| *cell == GridPos::new(6, 20))
        .map(|(_, entity)| *entity)
        .unwrap();
    let revision = map.revision();
    let before = map.path_call_count();
    destroy_building(&mut world, &mut map, wall);
    assert_eq!(map.revision(), revision + 1);
    let revision_after_breach = map.revision();
    let cycle_started = std::time::Instant::now();
    let mut retry_boundary_path_calls = 0;
    let mut retry_boundary_elapsed_us = 0;
    for tick in 0..20 {
        let wave_step_calls = map.path_call_count();
        let wave_step_started = std::time::Instant::now();
        step_waves(&mut world, &mut map, SIM_STEP_SECONDS);
        if tick == 19 {
            retry_boundary_path_calls = map.path_call_count() - wave_step_calls;
            retry_boundary_elapsed_us = wave_step_started.elapsed().as_micros();
        }
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        refresh_visibility(&mut world, &map);
    }
    let cycle_elapsed_us = cycle_started.elapsed().as_micros();
    let cycle_path_calls = map.path_call_count() - before;
    let combat_movement_path_calls = cycle_path_calls - retry_boundary_path_calls;
    let revisions_during_cycle = map.revision() - revision_after_breach;
    assert!(cycle_path_calls > 0);
    assert_eq!(world.resource::<WaveState>().elapsed.floor(), 1.0);
    println!(
        "HPA450_SIEGE census={} attack_move_orders={} engaged={} active_routes={} breach_revision_bumps=1 cycle_seconds=1.0 ticks=20 cycle_path_calls={} retry_boundary_path_calls={} combat_movement_path_calls={} cycle_elapsed_us={} retry_boundary_elapsed_us={} revisions_during_cycle={}",
        live.len(),
        attacking,
        engaged,
        moving,
        cycle_path_calls,
        retry_boundary_path_calls,
        combat_movement_path_calls,
        cycle_elapsed_us,
        retry_boundary_elapsed_us,
        revisions_during_cycle
    );
}
