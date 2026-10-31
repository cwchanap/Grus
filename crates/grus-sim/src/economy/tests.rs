use std::collections::HashSet;

use bevy::math::Vec2;

use super::*;
use crate::catalog::unit_spec;
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::heat::HeatState;
use crate::ids::IdAllocator;
use crate::movement::{SIM_STEP_SECONDS, step_movement};
use crate::visibility::{VisibilityMap, explored_by, refresh_visibility, visible_to};

fn test_economy(world: &mut World) {
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            scrap: 300,
            coal: 100,
            crystals: 0,
        },
    );
    world.insert_resource(economy);
}

fn spawn_worker(world: &mut World, id: UnitId, position: Vec2) -> Entity {
    let entity = spawn_unit(
        world,
        id,
        TeamId(1),
        position,
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world
        .entity_mut(entity)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    entity
}

fn gather_command(issuer: TeamId, workers: Vec<UnitId>, source: ResourceId) -> PlayerCommand {
    PlayerCommand::Gather {
        issuer,
        workers,
        source,
    }
}

fn skirmish_world() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

/// Winter spending contract: `can_afford` is an immutable all-four-field
/// check; `try_spend` compares every field first and deducts all-or-nothing.
/// A failure on ANY currency leaves every field — not just the gated one —
/// untouched, so rejection can never partially charge.
#[test]
fn try_spend_is_atomic_across_all_four_fields() {
    let full = ResourceStockpile {
        food: 100,
        scrap: 100,
        coal: 100,
        crystals: 0,
    };
    for cost in [
        Cost {
            food: 200,
            scrap: 1,
            coal: 1,
            crystals: 0,
        },
        Cost {
            food: 1,
            scrap: 200,
            coal: 1,
            crystals: 0,
        },
        Cost {
            food: 1,
            scrap: 1,
            coal: 200,
            crystals: 0,
        },
        Cost {
            food: 1,
            scrap: 1,
            coal: 1,
            crystals: 1,
        },
    ] {
        let mut stock = full;
        assert!(!stock.can_afford(cost), "{cost:?} must be unaffordable");
        assert!(!stock.try_spend(cost), "{cost:?} must fail");
        assert_eq!(stock, full, "failed spend must not touch any field");
    }

    let mut stock = ResourceStockpile {
        food: 100,
        scrap: 100,
        coal: 100,
        crystals: 10,
    };
    let cost = Cost {
        food: 10,
        scrap: 20,
        coal: 30,
        crystals: 4,
    };
    // can_afford is read-only: it answers true and mutates nothing.
    assert!(stock.can_afford(cost));
    assert_eq!(
        stock,
        ResourceStockpile {
            food: 100,
            scrap: 100,
            coal: 100,
            crystals: 10,
        },
        "can_afford mutated the stockpile"
    );
    assert!(stock.try_spend(cost));
    assert_eq!(
        stock,
        ResourceStockpile {
            food: 90,
            scrap: 80,
            coal: 70,
            crystals: 6,
        }
    );
}

/// Opts the skirmish world into runtime fog and stamps the initial reveal.
fn reveal(world: &mut World, map: &GridMap) {
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(world, map);
}

#[test]
fn gather_assigns_four_workers_unique_immediate_slots() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(5, 1, 2));
    for id in 1..=4 {
        let offset = id - 1;
        spawn_worker(
            &mut world,
            UnitId(id),
            Vec2::new(
                10.5 + (offset % 2) as f32 * 4.0,
                10.5 + (offset / 2) as f32 * 4.0,
            ),
        );
    }
    let source = spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Scrap,
        GridPos::new(12, 12),
        400,
    );
    let footprint = world.get::<Footprint>(source).copied().unwrap();

    // A normal Move onto the blocked source center stays Unreachable.
    let source_center = map.cell_center(GridPos::new(12, 12));
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)],
            kind: UnitCommandKind::Move {
                target: source_center,
            },
        }),
    );
    assert_eq!(outcome.accepted_units, Vec::<UnitId>::new());
    assert_eq!(outcome.rejected_units.len(), 4);
    assert!(
        outcome
            .rejected_units
            .iter()
            .all(|(_, reason)| *reason == RejectReason::Unreachable)
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(
            TeamId(1),
            vec![UnitId(4), UnitId(3), UnitId(2), UnitId(1)],
            ResourceId(1),
        ),
    );
    assert_eq!(
        outcome.accepted_units,
        vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)]
    );
    assert!(outcome.rejected_units.is_empty());

    let index = world.resource::<UnitIndex>();
    let mut slots = HashSet::new();
    for id in 1..=4 {
        let entity = index.entity(UnitId(id)).unwrap();
        let WorkerTask::ToSource { source, slot } = world.get::<WorkerTask>(entity).unwrap() else {
            panic!("expected ToSource for {id:?}");
        };
        assert_eq!(*source, ResourceId(1));
        assert!(map.is_walkable(*slot), "slot {slot:?} is not walkable");
        assert!(footprint.is_immediately_adjacent(*slot));
        slots.insert(*slot);
    }
    assert_eq!(slots.len(), 4, "all four ToSource slots must be unique");
}

#[test]
fn gather_beyond_perimeter_capacity_rejects_as_crowded() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(10, 1, 2));
    let footprint = Footprint::new(GridPos::new(12, 12), 1, 1);
    for (index, cell) in footprint.perimeter_cells().into_iter().enumerate() {
        spawn_worker(&mut world, UnitId(index as u32 + 1), map.cell_center(cell));
    }
    let outsider = spawn_worker(&mut world, UnitId(9), Vec2::new(2.5, 2.5));
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Scrap,
        GridPos::new(12, 12),
        400,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), (1..=9).map(UnitId).collect(), ResourceId(1)),
    );

    assert_eq!(outcome.accepted_units.len(), 8);
    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(9), RejectReason::Crowded)]
    );
    let slots: HashSet<GridPos> = (1..=8)
        .map(|id| {
            let entity = world.resource::<UnitIndex>().entity(UnitId(id)).unwrap();
            match world.get::<WorkerTask>(entity).unwrap() {
                WorkerTask::ToSource { slot, .. } => *slot,
                other => panic!("expected ToSource, got {other:?}"),
            }
        })
        .collect();
    assert_eq!(slots.len(), 8, "accepted workers keep unique slots");
    assert_eq!(world.get::<WorkerTask>(outsider), Some(&WorkerTask::Idle));
}

#[test]
fn gather_while_holding_deposits_first_then_gathers_the_requested_source() {
    let (mut world, mut map) = skirmish_world();

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // Park the worker away from both the ration cache and the Research
    // Center so the deposit leg is a real walk.
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(20.5, 42.5)));
    world.entity_mut(worker).insert(Carry::Holding {
        kind: ResourceKind::Scrap,
        amount: NonZeroU32::new(6).unwrap(),
    });

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    match world.get::<WorkerTask>(worker).unwrap() {
        WorkerTask::ToDropoff {
            source, dropoff, ..
        } => {
            assert_eq!(*source, ResourceId(1));
            assert_eq!(
                *dropoff,
                BuildingId(1),
                "carrying workers route to the starting Research Center first"
            );
        }
        other => panic!("expected ToDropoff, got {other:?}"),
    }

    // Nothing is deposited while the worker is still walking.
    for _ in 0..10 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.scrap, 300);

    for _ in 0..600 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<TeamEconomy>().0[&TeamId(1)]
            .stockpile
            .scrap
            == 306
        {
            break;
        }
    }
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.scrap, 306);
    assert_eq!(state.stockpile.food, 200, "no Food before gathering begins");
    assert_eq!(world.get::<Carry>(worker), Some(&Carry::Empty));
    match world.get::<WorkerTask>(worker).unwrap() {
        WorkerTask::ToSource { source, slot } => {
            assert_eq!(*source, ResourceId(1));
            let source_cell = MapFixture::starting_resources()
                .into_iter()
                .find(|spawn| spawn.id == ResourceId(1))
                .expect("the authored ration cache")
                .cell;
            assert!(
                Footprint::new(source_cell, 1, 1).is_immediately_adjacent(*slot),
                "worker routes to the ration-cache slot after depositing"
            );
        }
        other => panic!("expected ToSource after deposit, got {other:?}"),
    }
}

#[test]
fn ten_gather_ticks_transfer_one_whole_food() {
    let (mut world, mut map) = skirmish_world();

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // Park the worker next to the authored 600-Food ration cache.
    let food_cell = MapFixture::starting_resources()
        .into_iter()
        .find(|spawn| spawn.id == ResourceId(1))
        .expect("the authored ration cache")
        .cell;
    world.entity_mut(worker).insert(SimPosition::new(
        map.cell_center(GridPos::new(food_cell.x - 1, food_cell.y)),
    ));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        if world.get::<MoveOrder>(worker).is_none() {
            break;
        }
    }
    assert!(world.get::<MoveOrder>(worker).is_none(), "never arrived");

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Gathering { .. })
        ),
        "arrival must transition into Gathering"
    );

    for _ in 0..10 {
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(1).unwrap(),
        })
    );
    let source_entity = world
        .resource::<ResourceIndex>()
        .entity(ResourceId(1))
        .unwrap();
    assert_eq!(
        world
            .get::<ResourceSource>(source_entity)
            .unwrap()
            .remaining,
        599
    );
}

#[test]
fn rejected_gather_preserves_the_previous_task_and_route() {
    let (mut world, mut map) = skirmish_world();

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(3)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    let old_task = world.get::<WorkerTask>(worker).unwrap().clone();
    let old_route = world.get::<MoveOrder>(worker).unwrap().waypoints.clone();

    // Unknown source: the whole command rejects before any cancellation.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(99)),
    );
    assert_eq!(outcome.reject, Some(RejectReason::SourceMissing));
    assert_eq!(world.get::<WorkerTask>(worker), Some(&old_task));
    assert_eq!(world.get::<MoveOrder>(worker).unwrap().waypoints, old_route);
}

#[test]
fn depleted_source_despawns_unblocks_and_the_worker_deposits_then_idles() {
    let (mut world, mut map) = skirmish_world();

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // A tiny finite scrap source next to the worker's parked spot.
    let source = spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(50),
        ResourceKind::Scrap,
        GridPos::new(13, 43),
        2,
    );
    world
        .entity_mut(worker)
        .insert(SimPosition::new(map.cell_center(GridPos::new(13, 42))));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(50)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        if world.get::<MoveOrder>(worker).is_none() {
            break;
        }
    }
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    for _ in 0..30 {
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::ToDropoff { .. })
        ) {
            break;
        }
    }
    // The last unit drained the source: despawned, unblocked, and the
    // worker delivers the final carry.
    assert_eq!(
        world.resource::<ResourceIndex>().entity(ResourceId(50)),
        None,
        "depleted source must leave the index"
    );
    assert!(
        world.get::<ResourceSource>(source).is_none(),
        "entity despawned"
    );
    assert!(map.is_walkable(GridPos::new(13, 43)), "footprint unblocked");
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Scrap,
            amount: NonZeroU32::new(2).unwrap(),
        })
    );

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.get::<WorkerTask>(worker) == Some(&WorkerTask::Idle) {
            break;
        }
    }
    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
    assert_eq!(world.get::<Carry>(worker), Some(&Carry::Empty));
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(
        state.stockpile,
        ResourceStockpile {
            food: 200,
            scrap: 302,
            coal: 150,
            crystals: 0,
        }
    );
}

#[test]
fn walled_in_gatherer_idles_through_cleanup_preserving_carry() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);

    // A standalone finite Food source, plus a Research Center drop-off.
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Food,
        GridPos::new(4, 16),
        400,
    );
    let research_center = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::ResearchCenter,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(16, 16), 4, 4),
            Dropoff { team: TeamId(1) },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(2), research_center);

    // The gatherer sits walled in with a full carry, so its drop-off route
    // is impossible.
    let worker = spawn_worker(&mut world, UnitId(1), map.cell_center(GridPos::new(4, 4)));
    for cell in [
        GridPos::new(3, 3),
        GridPos::new(4, 3),
        GridPos::new(5, 3),
        GridPos::new(3, 4),
        GridPos::new(5, 4),
        GridPos::new(3, 5),
        GridPos::new(4, 5),
        GridPos::new(5, 5),
    ] {
        map.set_blocked(cell, true);
    }
    world.entity_mut(worker).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
    ));

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
    assert!(world.get::<MoveOrder>(worker).is_none());
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        }),
        "cleanup never discards Carry"
    );
    assert_eq!(
        world.resource::<LastRouteReject>().0.get(&TeamId(1)),
        Some(&RejectReason::Unreachable),
        "typed code recorded for bridge feedback"
    );
}

/// The feedback slot is team-aware: a Team-2 worker's route failure records
/// under Team 2 and never overwrites a pending Team-1 reject (the bridge
/// drains Team 1 only).
#[test]
fn route_reject_feedback_is_team_aware() {
    let mut world = World::new();
    let map = GridMap::new(16, 16);
    test_economy(&mut world);

    let team_one = spawn_worker(&mut world, UnitId(1), map.cell_center(GridPos::new(2, 2)));
    let team_two = spawn_worker(&mut world, UnitId(2), map.cell_center(GridPos::new(4, 2)));
    world.entity_mut(team_two).insert(Unit {
        id: UnitId(2),
        team: TeamId(2),
        kind: UnitKind::Worker,
        speed: 6.0,
    });

    idle_worker_on_route_failure(&mut world, team_one, RejectReason::Unreachable);
    idle_worker_on_route_failure(&mut world, team_two, RejectReason::Crowded);

    let rejects = &world.resource::<LastRouteReject>().0;
    assert_eq!(rejects.get(&TeamId(1)), Some(&RejectReason::Unreachable));
    assert_eq!(rejects.get(&TeamId(2)), Some(&RejectReason::Crowded));

    // The latest failure per team wins.
    idle_worker_on_route_failure(&mut world, team_one, RejectReason::Unreachable);
    assert_eq!(
        world.resource::<LastRouteReject>().0.get(&TeamId(1)),
        Some(&RejectReason::Unreachable)
    );
}

/// Ordering must measure each drop-off's geometric center, not its anchor
/// cell. Worker at (10.5, 44.5): the 4×4 Research Center's anchor cell center
/// (12.5, 46.5) is 8 units² away while the 2×2 Depot's anchor cell
/// center (13.5, 44.5) is 9 — anchor-based sorting picks the Research Center.
/// Geometric centers are (14, 48) at 24.5 vs (14, 45) at 12.5: the
/// Depot is truly nearer and must win.
#[test]
fn nearest_dropoff_orders_by_geometric_center_not_anchor() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 64);
    test_economy(&mut world);

    let research_center = world
        .spawn((
            Building {
                id: BuildingId(1),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::ResearchCenter,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(12, 46), 4, 4),
            Dropoff { team: TeamId(1) },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(1), research_center);
    let depot = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::Depot,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(13, 44), 2, 2),
            Dropoff { team: TeamId(1) },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(2), depot);
    for cell in Footprint::new(GridPos::new(12, 46), 4, 4)
        .cells()
        .into_iter()
        .chain(Footprint::new(GridPos::new(13, 44), 2, 2).cells())
    {
        map.set_blocked(cell, true);
    }

    let worker = spawn_worker(&mut world, UnitId(1), Vec2::new(10.5, 44.5));
    let used = seed_used_excluding(&world, &map, worker);

    let (dropoff, slot, _) = nearest_reachable_dropoff(&world, &map, worker, &used)
        .expect("a reachable drop-off exists");

    assert_eq!(
        dropoff,
        BuildingId(2),
        "the truly nearer Depot must win over the anchor-nearer Research Center"
    );
    assert!(
        Footprint::new(GridPos::new(13, 44), 2, 2).is_immediately_adjacent(slot),
        "assigned slot {slot:?} is on the Depot perimeter"
    );
}

/// Exact distance ties break on `BuildingId`, never on `BuildingIndex`'s
/// arbitrary `HashMap` iteration order. Worker at (10.5, 10.5): the left
/// Depot's geometric center (9, 10) and the right one's (12, 10) are
/// both exactly 2.5 units² away, so the lower id must win every run. The
/// higher id is inserted first so insertion order alone cannot explain
/// the outcome.
#[test]
fn nearest_dropoff_breaks_distance_ties_on_building_id() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 64);
    test_economy(&mut world);

    for (id, anchor) in [
        (BuildingId(2), GridPos::new(8, 9)),
        (BuildingId(1), GridPos::new(11, 9)),
    ] {
        let depot = world
            .spawn((
                Building {
                    id,
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::Depot,
                    construction: crate::buildings::ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(anchor, 2, 2),
                Dropoff { team: TeamId(1) },
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(id, depot);
        for cell in Footprint::new(anchor, 2, 2).cells() {
            map.set_blocked(cell, true);
        }
    }

    let worker = spawn_worker(&mut world, UnitId(1), Vec2::new(10.5, 10.5));
    let used = seed_used_excluding(&world, &map, worker);

    let (dropoff, slot, _) = nearest_reachable_dropoff(&world, &map, worker, &used)
        .expect("a reachable drop-off exists");

    assert_eq!(
        dropoff,
        BuildingId(1),
        "the lower BuildingId must win an exact distance tie"
    );
    assert!(
        Footprint::new(GridPos::new(11, 9), 2, 2).is_immediately_adjacent(slot),
        "assigned slot {slot:?} is on the winning Depot perimeter"
    );
}

#[test]
fn gather_rejects_foreign_and_non_worker_workers_without_touching_them() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(10, 1, 2));
    let worker = spawn_worker(&mut world, UnitId(1), Vec2::new(8.5, 8.5));
    let guard = spawn_unit(
        &mut world,
        UnitId(8),
        TeamId(1),
        Vec2::new(10.5, 8.5),
        UnitKind::Guard,
        6.0,
    );
    let foreign = spawn_unit(
        &mut world,
        UnitId(9),
        TeamId(2),
        Vec2::new(2.5, 2.5),
        UnitKind::Worker,
        6.0,
    );
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Scrap,
        GridPos::new(12, 12),
        400,
    );
    // Observable state on the rejected workers: "without touching them"
    // means both task and move order must survive the command verbatim.
    world.entity_mut(guard).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(18, 6))],
            next: 0,
            goal: GridPos::new(18, 6),
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));
    world.entity_mut(foreign).insert((
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(1),
            slot: GridPos::new(3, 3),
        },
        MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(3, 3))],
            next: 0,
            goal: GridPos::new(3, 3),
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(
            TeamId(1),
            vec![UnitId(9), UnitId(8), UnitId(1)],
            ResourceId(1),
        ),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        outcome.rejected_units,
        vec![
            (UnitId(8), RejectReason::NotWorker),
            (UnitId(9), RejectReason::NotOwned),
        ]
    );
    assert_eq!(
        world.get::<WorkerTask>(guard),
        Some(&WorkerTask::Gathering {
            source: ResourceId(1)
        }),
        "the rejected non-worker keeps its old task"
    );
    assert_eq!(
        world.get::<MoveOrder>(guard).map(|order| order.goal),
        Some(GridPos::new(18, 6)),
        "the rejected non-worker keeps its old move order"
    );
    assert_eq!(
        world.get::<WorkerTask>(foreign),
        Some(&WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(1),
            slot: GridPos::new(3, 3),
        }),
        "the rejected foreign worker keeps its old task"
    );
    assert_eq!(
        world.get::<MoveOrder>(foreign).map(|order| order.goal),
        Some(GridPos::new(3, 3)),
        "the rejected foreign worker keeps its old move order"
    );
    match world.get::<WorkerTask>(worker).unwrap() {
        WorkerTask::ToSource { source, .. } => assert_eq!(*source, ResourceId(1)),
        other => panic!("expected ToSource, got {other:?}"),
    }
}

/// Fog knowledge gate: a guessed ResourceId on unexplored ground answers
/// exactly like an absent id — distinct rejects would let a caller probe
/// ids to discover unexplored sources.
#[test]
fn gather_rejects_an_unexplored_standalone_source() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);
    assert!(
        !explored_by(&world, TeamId(1), GridPos::new(45, 16)),
        "the northern expansion coal starts unexplored"
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(13)),
    );

    assert_eq!(outcome.reject, Some(RejectReason::SourceMissing));
    assert!(outcome.accepted_units.is_empty());
}

/// Fog knowledge gate: standalone sources are static map contents — once
/// explored they stay gatherable even after current vision is lost.
#[test]
fn explored_source_stays_gatherable_after_vision_is_lost() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);

    // Scout the northern expansion coal, then return home: the source stays
    // known but falls out of current vision.
    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(worker)
        .insert(SimPosition::new(map.cell_center(GridPos::new(45, 19))));
    refresh_visibility(&mut world, &map);
    world
        .entity_mut(worker)
        .insert(SimPosition::new(map.cell_center(GridPos::new(20, 19))));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), GridPos::new(45, 16)));
    assert!(!visible_to(&world, TeamId(1), GridPos::new(45, 16)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(13)),
    );

    assert_eq!(
        outcome.reject, None,
        "explored knowledge survives lost vision: {outcome:?}"
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    assert!(matches!(
        world.get::<WorkerTask>(worker),
        Some(WorkerTask::ToSource {
            source: ResourceId(13),
            ..
        })
    ));
}

#[test]
fn gather_replaces_an_active_move_order_and_frees_its_goal() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(10, 1, 2));
    let worker = spawn_worker(&mut world, UnitId(1), Vec2::new(8.5, 8.5));
    let old_goal = GridPos::new(18, 6);
    world.entity_mut(worker).insert(MoveOrder {
        waypoints: vec![map.cell_center(old_goal)],
        next: 0,
        goal: old_goal,
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Scrap,
        GridPos::new(12, 12),
        400,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    match world.get::<WorkerTask>(worker).unwrap() {
        WorkerTask::ToSource { source, slot } => {
            assert_eq!(*source, ResourceId(1));
            assert_ne!(
                *slot, old_goal,
                "the stale move goal is not reused as the gather slot"
            );
        }
        other => panic!("expected ToSource, got {other:?}"),
    }
    let order = world.get::<MoveOrder>(worker).expect("fresh order");
    assert_ne!(
        order.goal, old_goal,
        "the gather route replaces, not keeps, the old one"
    );

    // The released goal is claimable again by the next move command.
    let late = spawn_worker(&mut world, UnitId(2), Vec2::new(8.5, 10.5));
    let old_goal_center = map.cell_center(old_goal);
    let late_outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(2)],
            kind: UnitCommandKind::Move {
                target: old_goal_center,
            },
        }),
    );
    assert_eq!(late_outcome.accepted_units, vec![UnitId(2)]);
    assert_eq!(
        world.get::<MoveOrder>(late).unwrap().goal,
        old_goal,
        "the released goal is claimable again"
    );
}

#[test]
fn deposits_bank_into_the_matching_stockpile_arm() {
    let (mut world, mut map) = skirmish_world();

    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(worker)
        .insert(SimPosition::new(Vec2::new(20.5, 42.5)));
    world.entity_mut(worker).insert(Carry::Holding {
        kind: ResourceKind::Coal,
        amount: NonZeroU32::new(7).unwrap(),
    });

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    // Deposit order: the carried coal banks on arrival at the research-center
    // drop-off, then one food gather cycle banks food. The loop polls
    // movement + economy until both have landed in their stockpile arms.
    for _ in 0..4000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        if state.stockpile.coal == 157 && state.stockpile.food == 210 {
            break;
        }
    }
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.coal, 157, "carried coal banks in full");
    assert_eq!(state.stockpile.food, 210, "a gather cycle banks ten food");
    assert_eq!(state.stockpile.scrap, 300, "scrap stays untouched");
    assert_eq!(world.get::<Carry>(worker), Some(&Carry::Empty));
}

#[test]
fn deposited_workers_route_back_to_their_finite_source() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);

    // A standalone finite Food source, plus a Research Center drop-off. Finite
    // nodes keep no exclusive holder: every worker returning from a deposit
    // routes back to the node while it has remaining yield.
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Food,
        GridPos::new(4, 16),
        400,
    );
    let research_center = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::ResearchCenter,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(16, 16), 4, 4),
            Dropoff { team: TeamId(1) },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(2), research_center);

    // The first worker deposits at the Research Center and routes back to the
    // source it was working.
    let first = spawn_worker(&mut world, UnitId(1), map.cell_center(GridPos::new(6, 17)));
    world.entity_mut(first).insert((
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(2),
            slot: GridPos::new(6, 17),
        },
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
    ));

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

    match world.get::<WorkerTask>(first).unwrap() {
        WorkerTask::ToSource { source, .. } => {
            assert_eq!(
                *source,
                ResourceId(1),
                "the worker routes back to its source"
            )
        }
        other => panic!("expected ToSource, got {other:?}"),
    }
    assert_eq!(world.get::<Carry>(first), Some(&Carry::Empty));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.food,
        210,
        "the deposit banks the carried food"
    );

    // A second worker delivering from the same node gets the same
    // return leg — finite sources share, they do not reserve.
    let second = spawn_worker(&mut world, UnitId(2), map.cell_center(GridPos::new(6, 17)));
    world.entity_mut(second).insert((
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(2),
            slot: GridPos::new(6, 17),
        },
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
    ));

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

    match world.get::<WorkerTask>(second).unwrap() {
        WorkerTask::ToSource { source, .. } => {
            assert_eq!(*source, ResourceId(1), "the second worker returns too")
        }
        other => panic!("expected ToSource, got {other:?}"),
    }
    assert_eq!(world.get::<Carry>(second), Some(&Carry::Empty));
}

#[test]
fn vanished_source_idles_empty_workers_and_routes_full_ones_to_the_dropoff() {
    let (mut world, mut map) = skirmish_world();

    let source_entity = world
        .resource::<ResourceIndex>()
        .entity(ResourceId(1))
        .expect("the fixture's ration cache");
    let empty = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let full = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();
    world
        .entity_mut(empty)
        .insert(SimPosition::new(map.cell_center(GridPos::new(21, 42))));
    world.entity_mut(empty).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        Carry::Empty,
        GatherProgress::default(),
    ));
    world
        .entity_mut(full)
        .insert(SimPosition::new(map.cell_center(GridPos::new(21, 41))));
    world.entity_mut(full).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
    ));

    // The source depletes away under both gatherers.
    world.resource_mut::<ResourceIndex>().remove(ResourceId(1));
    for cell in Footprint::new(GridPos::new(22, 42), 1, 1).cells() {
        map.set_blocked(cell, false);
    }
    world.despawn(source_entity);

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(
        world.get::<WorkerTask>(empty),
        Some(&WorkerTask::Idle),
        "an empty-handed worker whose source vanished simply idles"
    );
    match world.get::<WorkerTask>(full).unwrap() {
        WorkerTask::ToDropoff { source, .. } => {
            assert_eq!(
                *source,
                ResourceId(1),
                "the full worker delivers what he holds"
            )
        }
        other => panic!("expected ToDropoff, got {other:?}"),
    }
    assert_eq!(
        world.get::<Carry>(full),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        }),
        "partial progress is not banked but the carry is kept"
    );
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.food, 200, "nothing deposits before arrival");
}

#[test]
fn cancel_unit_activity_drops_combat_orders_and_preserves_carry() {
    let mut world = World::new();
    let map = GridMap::new(24, 24);
    let worker = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 4.5),
        UnitKind::Worker,
        6.0,
    );
    world.entity_mut(worker).insert((
        Carry::Holding {
            kind: ResourceKind::Scrap,
            amount: NonZeroU32::new(5).unwrap(),
        },
        GatherProgress(0.5),
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        CombatOrder::AttackMove {
            destination: GridPos::new(9, 9),
            target: None,
            last_target_cell: None,
        },
        MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(5, 4))],
            next: 0,
            goal: GridPos::new(5, 4),
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));

    cancel_unit_activity(&mut world, worker);

    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
    assert_eq!(
        world.get::<GatherProgress>(worker),
        Some(&GatherProgress(0.0))
    );
    assert!(world.get::<MoveOrder>(worker).is_none());
    assert!(world.get::<CombatOrder>(worker).is_none());
    // Carry is never touched.
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Scrap,
            amount: NonZeroU32::new(5).unwrap(),
        })
    );
}

/// Review item: a gatherer pushed away from its source (e.g. teleported by
/// a later building placement) must walk back instead of gathering from up
/// to seven cells away; nothing transfers while it is away.
#[test]
fn displaced_gatherer_walks_back_instead_of_gathering_from_afar() {
    let (mut world, mut map) = skirmish_world();
    let worker = world
        .resource::<UnitIndex>()
        .entity(UnitId(1))
        .expect("seeded worker");

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    for _ in 0..600 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Gathering { .. })
        ) {
            break;
        }
    }
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::Gathering {
                source: ResourceId(1)
            })
        ),
        "premise: the worker reached Gathering"
    );

    // Teleport it six cells off the source's perimeter.
    let away = map.cell_center(GridPos::new(30, 48));
    world.entity_mut(worker).insert(SimPosition::new(away));
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        matches!(
            world.get::<WorkerTask>(worker),
            Some(WorkerTask::ToSource {
                source: ResourceId(1),
                ..
            })
        ),
        "a non-adjacent gatherer must be walking back, not gathering: {:?}",
        world.get::<WorkerTask>(worker)
    );
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Empty),
        "nothing transfers while away from the source"
    );
    assert!(
        world.get::<MoveOrder>(worker).is_some(),
        "the walk back installs a real route"
    );

    // The loop still completes: it walks back and gathers again.
    for _ in 0..600 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if world
            .get::<Carry>(worker)
            .map_or(0, |carry| carry.amount_or_zero())
            > 0
        {
            break;
        }
    }
    assert!(
        matches!(world.get::<Carry>(worker), Some(Carry::Holding { .. })),
        "the gatherer resumes gathering after walking back"
    );
}

/// Fog privacy at the gather reservation seam: a hidden enemy standing on a
/// source's only free approach cell must not surface as a gather reject —
/// hidden units reserve nothing, the worker is assigned that slot and the
/// collision resolves physically on arrival. Making the enemy visible flips
/// the outcome, proving the filter (not the map) decided it.
#[test]
fn hidden_units_do_not_reserve_gather_approach_cells() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);

    // The rightmost standalone source: far from Team 1's home vision.
    let (source, entity) = world
        .resource::<ResourceIndex>()
        .iter()
        .filter(|(_, entity)| world.get::<Building>(**entity).is_none())
        .max_by_key(|(_, entity)| {
            let footprint = world.get::<Footprint>(**entity).unwrap();
            footprint.center().x as i32
        })
        .map(|(id, entity)| (*id, *entity))
        .unwrap();
    let footprint = *world.get::<Footprint>(entity).unwrap();

    // Wall the whole perimeter except its leftmost cell: exactly one
    // approach slot survives, and it is the one the hidden enemy takes.
    let free = footprint
        .perimeter_cells()
        .into_iter()
        .filter(|cell| map.is_walkable(*cell))
        .min_by_key(|cell| cell.x)
        .unwrap();
    for cell in footprint.perimeter_cells() {
        if cell != free {
            map.set_blocked(cell, true);
        }
    }

    // Explore the source with a scout, then send it home: the ground stays
    // explored but the approach cell leaves current vision.
    let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(scout).insert(SimPosition::new(
        map.cell_center(GridPos::new(free.x - 5, free.y)),
    ));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), footprint));
    world
        .entity_mut(scout)
        .insert(SimPosition::new(map.cell_center(GridPos::new(20, 44))));
    refresh_visibility(&mut world, &map);
    assert!(!visible_to(&world, TeamId(1), free));

    // A hidden enemy worker parks on the only approach cell.
    let enemy = spawn_worker(&mut world, UnitId(80), map.cell_center(free));
    world.entity_mut(enemy).insert(Unit {
        id: UnitId(80),
        team: TeamId(2),
        kind: UnitKind::Worker,
        speed: 6.0,
    });
    refresh_visibility(&mut world, &map);
    assert!(!visible_to(&world, TeamId(1), free));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], source),
    );
    assert_eq!(
        outcome.accepted_units,
        vec![UnitId(1)],
        "a hidden occupant never reserves the gather approach cell"
    );
    assert!(matches!(
        world.get::<WorkerTask>(scout),
        Some(WorkerTask::ToSource { slot, .. }) if *slot == free
    ));

    // Control: once the approach cell is on camera the enemy reserves it,
    // so a second worker's gather rejects.
    world.entity_mut(scout).insert(SimPosition::new(
        map.cell_center(GridPos::new(free.x - 5, free.y)),
    ));
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), free));
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(2)], source),
    );
    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(2), RejectReason::Crowded)],
        "a visible occupant still reserves the approach cell"
    );
}

/// Cold surface efficiency never touches the gather loop: the proven
/// ten-ticks-per-food contract holds unchanged with a freezing HeatState.
#[test]
fn cold_surface_efficiency_does_not_slow_gathering() {
    let (mut world, mut map) = skirmish_world();

    // Park the worker beside the authored ration cache and task it directly.
    let food_cell = MapFixture::starting_resources()
        .into_iter()
        .find(|spawn| spawn.id == ResourceId(1))
        .expect("the authored ration cache")
        .cell;
    let slot = GridPos::new(food_cell.x - 1, food_cell.y);
    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(worker).insert((
        SimPosition::new(map.cell_center(slot)),
        WorkerTask::ToSource {
            source: ResourceId(1),
            slot,
        },
    ));
    world.insert_resource(HeatState {
        supply: 0,
        demand: 4,
        ..HeatState::default()
    });

    // Arrival tick plus the ten gather ticks that bank one whole food warm.
    for _ in 0..11 {
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(1).unwrap()
        }),
        "cold must leave the gather cadence at real time"
    );
}
