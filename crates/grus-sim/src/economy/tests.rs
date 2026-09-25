use std::collections::HashSet;

use bevy::math::Vec2;

use super::*;
use crate::buildings::{ConstructionState, step_construction};
use crate::catalog::{BuildingKind, building_spec, unit_spec};
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit,
};
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::IdAllocator;
use crate::movement::{SIM_STEP_SECONDS, step_movement};
use crate::visibility::{VisibilityMap, explored_by, refresh_visibility, visible_to};

fn test_economy(world: &mut World) {
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    world.insert_resource(economy);
}

fn spawn_villager(world: &mut World, id: UnitId, position: Vec2) -> Entity {
    let entity = spawn_unit(
        world,
        id,
        TeamId(1),
        position,
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
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

/// Opts the skirmish world into runtime fog and stamps the initial reveal.
fn reveal(world: &mut World, map: &GridMap) {
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(world, map);
}

/// Spawns a completed Farm with a renewable Food source at `anchor`, fully
/// registered (footprint blocked, indexes updated) — the manual shape of the
/// step_construction completion grants, for fog knowledge tests.
fn spawn_completed_farm(
    world: &mut World,
    map: &mut GridMap,
    team: TeamId,
    anchor: GridPos,
) -> (Entity, ResourceId) {
    let spec = building_spec(BuildingKind::Farm);
    let footprint = Footprint::new(anchor, spec.width, spec.height);
    for cell in footprint.cells() {
        map.set_blocked(cell, true);
    }
    let id = BuildingId(50);
    let entity = world
        .spawn((
            Building {
                id,
                team,
                kind: BuildingKind::Farm,
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
    let source_id = ResourceId(99);
    world.entity_mut(entity).insert(ResourceSource {
        id: source_id,
        kind: ResourceKind::Food,
        remaining: None,
        assigned_worker: None,
    });
    world
        .get_resource_or_insert_with(ResourceIndex::default)
        .insert(source_id, entity);
    (entity, source_id)
}

#[test]
fn gather_assigns_four_villagers_unique_immediate_slots() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(5, 1, 2));
    for id in 1..=4 {
        let offset = id - 1;
        spawn_villager(
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
        ResourceKind::Wood,
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
        spawn_villager(&mut world, UnitId(index as u32 + 1), map.cell_center(cell));
    }
    let outsider = spawn_villager(&mut world, UnitId(9), Vec2::new(2.5, 2.5));
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Wood,
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
    assert_eq!(slots.len(), 8, "accepted villagers keep unique slots");
    assert_eq!(world.get::<WorkerTask>(outsider), Some(&WorkerTask::Idle));
}

#[test]
fn gather_while_holding_deposits_first_then_gathers_the_requested_source() {
    let (mut world, mut map) = skirmish_world();

    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // Park the villager away from both the berries and the Town Center so
    // the deposit leg is a real walk.
    world
        .entity_mut(villager)
        .insert(SimPosition::new(Vec2::new(20.5, 42.5)));
    world.entity_mut(villager).insert(Carry::Holding {
        kind: ResourceKind::Wood,
        amount: NonZeroU32::new(6).unwrap(),
    });

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    match world.get::<WorkerTask>(villager).unwrap() {
        WorkerTask::ToDropoff {
            source, dropoff, ..
        } => {
            assert_eq!(*source, ResourceId(1));
            assert_eq!(
                *dropoff,
                BuildingId(1),
                "carrying workers route to the starting Town Center first"
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
    assert_eq!(state.stockpile.wood, 300);

    for _ in 0..600 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.wood == 306 {
            break;
        }
    }
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.wood, 306);
    assert_eq!(state.stockpile.food, 200, "no Food before gathering begins");
    assert_eq!(world.get::<Carry>(villager), Some(&Carry::Empty));
    match world.get::<WorkerTask>(villager).unwrap() {
        WorkerTask::ToSource { source, slot } => {
            assert_eq!(*source, ResourceId(1));
            assert!(
                Footprint::new(GridPos::new(22, 42), 1, 1).is_immediately_adjacent(*slot),
                "worker routes to the berry slot after depositing"
            );
        }
        other => panic!("expected ToSource after deposit, got {other:?}"),
    }
}

#[test]
fn ten_gather_ticks_transfer_one_whole_food() {
    let (mut world, mut map) = skirmish_world();

    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // Park the villager next to the 600-Food berry bush at (22, 42).
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(21, 42))));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        if world.get::<MoveOrder>(villager).is_none() {
            break;
        }
    }
    assert!(world.get::<MoveOrder>(villager).is_none(), "never arrived");

    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        matches!(
            world.get::<WorkerTask>(villager),
            Some(WorkerTask::Gathering { .. })
        ),
        "arrival must transition into Gathering"
    );

    for _ in 0..10 {
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    assert_eq!(
        world.get::<Carry>(villager),
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
        Some(599)
    );
}

#[test]
fn rejected_gather_preserves_the_previous_task_and_route() {
    let (mut world, mut map) = skirmish_world();

    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(3)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    let old_task = world.get::<WorkerTask>(villager).unwrap().clone();
    let old_route = world.get::<MoveOrder>(villager).unwrap().waypoints.clone();

    // Unknown source: the whole command rejects before any cancellation.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(99)),
    );
    assert_eq!(outcome.reject, Some(RejectReason::SourceMissing));
    assert_eq!(world.get::<WorkerTask>(villager), Some(&old_task));
    assert_eq!(
        world.get::<MoveOrder>(villager).unwrap().waypoints,
        old_route
    );
}

#[test]
fn depleted_source_despawns_unblocks_and_the_worker_deposits_then_idles() {
    let (mut world, mut map) = skirmish_world();

    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    // A tiny finite wood source next to the villager's parked spot.
    let source = spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(50),
        ResourceKind::Wood,
        GridPos::new(13, 43),
        2,
    );
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(13, 42))));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(50)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        if world.get::<MoveOrder>(villager).is_none() {
            break;
        }
    }
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    for _ in 0..30 {
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if matches!(
            world.get::<WorkerTask>(villager),
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
        world.get::<Carry>(villager),
        Some(&Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(2).unwrap(),
        })
    );

    for _ in 0..400 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if world.get::<WorkerTask>(villager) == Some(&WorkerTask::Idle) {
            break;
        }
    }
    assert_eq!(world.get::<WorkerTask>(villager), Some(&WorkerTask::Idle));
    assert_eq!(world.get::<Carry>(villager), Some(&Carry::Empty));
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(
        state.stockpile,
        ResourceStockpile {
            food: 200,
            wood: 302,
            gold: 100,
        }
    );
}

#[test]
fn farm_allows_one_worker_until_retasked_or_stopped() {
    let (mut world, mut map) = skirmish_world();

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(2),
            kind: crate::catalog::BuildingKind::Farm,
            anchor: GridPos::new(11, 41),
        },
    );
    assert_eq!(outcome.reject, None, "farm placement rejected: {outcome:?}");
    let farm = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(3))
        .unwrap();
    for _ in 0..2000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_construction(&mut world, SIM_STEP_SECONDS);
        if world.get::<Building>(farm).unwrap().construction.complete {
            break;
        }
    }
    assert!(world.get::<Building>(farm).unwrap().construction.complete);
    let farm_source = world.get::<ResourceSource>(farm).unwrap();
    let farm_source_id = farm_source.id;
    assert_eq!(farm_source.kind, ResourceKind::Food);
    assert_eq!(farm_source.remaining, None, "farms are renewable");
    assert_eq!(farm_source.assigned_worker, None);

    // First worker reserves; the second rejects FarmOccupied.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1), UnitId(3)], farm_source_id),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        outcome.rejected_units,
        vec![(UnitId(3), RejectReason::FarmOccupied)]
    );
    assert_eq!(
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        Some(UnitId(1))
    );

    // Stop releases the assignment.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::Stop,
        }),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        None
    );

    // The freed farm accepts the next worker; an accepted retask releases.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(3)], farm_source_id),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(3)]);
    assert_eq!(
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        Some(UnitId(3))
    );
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(3)],
            kind: UnitCommandKind::Move {
                target: Vec2::new(2.5, 2.5),
            },
        }),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(3)]);
    assert_eq!(
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        None
    );
}

#[test]
fn walled_in_gatherer_idles_through_cleanup_and_releases_the_farm() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);

    // A completed Farm serving villager 1, plus a Town Center drop-off.
    let farm = world
        .spawn((
            Building {
                id: BuildingId(1),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::Farm,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(4, 16), 2, 2),
            ResourceSource {
                id: ResourceId(1),
                kind: ResourceKind::Food,
                remaining: None,
                assigned_worker: Some(UnitId(1)),
            },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(1), farm);
    world
        .get_resource_or_insert_with(ResourceIndex::default)
        .insert(ResourceId(1), farm);
    let town_center = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::TownCenter,
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
        .insert(BuildingId(2), town_center);

    // The assigned gatherer sits walled in with a full carry, so its
    // drop-off route is impossible.
    let worker = spawn_villager(&mut world, UnitId(1), map.cell_center(GridPos::new(4, 4)));
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
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        None,
        "route failure must release the Farm assignment"
    );
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

    let team_one = spawn_villager(&mut world, UnitId(1), map.cell_center(GridPos::new(2, 2)));
    let team_two = spawn_villager(&mut world, UnitId(2), map.cell_center(GridPos::new(4, 2)));
    world.entity_mut(team_two).insert(Unit {
        id: UnitId(2),
        team: TeamId(2),
        kind: UnitKind::Villager,
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
/// cell. Worker at (10.5, 44.5): the 4×4 Town Center's anchor cell center
/// (12.5, 46.5) is 8 units² away while the 2×2 Storehouse's anchor cell
/// center (13.5, 44.5) is 9 — anchor-based sorting picks the Town Center.
/// Geometric centers are (14, 48) at 24.5 vs (14, 45) at 12.5: the
/// Storehouse is truly nearer and must win.
#[test]
fn nearest_dropoff_orders_by_geometric_center_not_anchor() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 64);
    test_economy(&mut world);

    let town_center = world
        .spawn((
            Building {
                id: BuildingId(1),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::TownCenter,
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
        .insert(BuildingId(1), town_center);
    let storehouse = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::Storehouse,
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
        .insert(BuildingId(2), storehouse);
    for cell in Footprint::new(GridPos::new(12, 46), 4, 4)
        .cells()
        .into_iter()
        .chain(Footprint::new(GridPos::new(13, 44), 2, 2).cells())
    {
        map.set_blocked(cell, true);
    }

    let worker = spawn_villager(&mut world, UnitId(1), Vec2::new(10.5, 44.5));
    let used = seed_used_excluding(&world, &map, worker);

    let (dropoff, slot, _) = nearest_reachable_dropoff(&world, &map, worker, &used)
        .expect("a reachable drop-off exists");

    assert_eq!(
        dropoff,
        BuildingId(2),
        "the truly nearer Storehouse must win over the anchor-nearer Town Center"
    );
    assert!(
        Footprint::new(GridPos::new(13, 44), 2, 2).is_immediately_adjacent(slot),
        "assigned slot {slot:?} is on the Storehouse perimeter"
    );
}

/// Exact distance ties break on `BuildingId`, never on `BuildingIndex`'s
/// arbitrary `HashMap` iteration order. Worker at (10.5, 10.5): the left
/// Storehouse's geometric center (9, 10) and the right one's (12, 10) are
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
        let storehouse = world
            .spawn((
                Building {
                    id,
                    team: TeamId(1),
                    kind: crate::catalog::BuildingKind::Storehouse,
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
            .insert(id, storehouse);
        for cell in Footprint::new(anchor, 2, 2).cells() {
            map.set_blocked(cell, true);
        }
    }

    let worker = spawn_villager(&mut world, UnitId(1), Vec2::new(10.5, 10.5));
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
        "assigned slot {slot:?} is on the winning Storehouse perimeter"
    );
}

#[test]
fn gather_rejects_foreign_and_non_villager_workers_without_touching_them() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(10, 1, 2));
    let villager = spawn_villager(&mut world, UnitId(1), Vec2::new(8.5, 8.5));
    let spearman = spawn_unit(
        &mut world,
        UnitId(8),
        TeamId(1),
        Vec2::new(10.5, 8.5),
        UnitKind::Spearman,
        6.0,
    );
    let foreign = spawn_unit(
        &mut world,
        UnitId(9),
        TeamId(2),
        Vec2::new(2.5, 2.5),
        UnitKind::Villager,
        6.0,
    );
    spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Wood,
        GridPos::new(12, 12),
        400,
    );
    // Observable state on the rejected workers: "without touching them"
    // means both task and move order must survive the command verbatim.
    world.entity_mut(spearman).insert((
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
            (UnitId(8), RejectReason::NotVillager),
            (UnitId(9), RejectReason::NotOwned),
        ]
    );
    assert_eq!(
        world.get::<WorkerTask>(spearman),
        Some(&WorkerTask::Gathering {
            source: ResourceId(1)
        }),
        "the rejected non-villager keeps its old task"
    );
    assert_eq!(
        world.get::<MoveOrder>(spearman).map(|order| order.goal),
        Some(GridPos::new(18, 6)),
        "the rejected non-villager keeps its old move order"
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
    match world.get::<WorkerTask>(villager).unwrap() {
        WorkerTask::ToSource { source, .. } => assert_eq!(*source, ResourceId(1)),
        other => panic!("expected ToSource, got {other:?}"),
    }
}

/// Fog knowledge gate: a guessed ResourceId on unexplored ground rejects —
/// standalone sources are gatherable only once their cell was explored.
#[test]
fn gather_rejects_an_unexplored_standalone_source() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);
    assert!(
        !explored_by(&world, TeamId(1), GridPos::new(45, 16)),
        "the northern expansion gold starts unexplored"
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(13)),
    );

    assert_eq!(outcome.reject, Some(RejectReason::Unexplored));
    assert!(outcome.accepted_units.is_empty());
}

/// Fog knowledge gate: standalone sources are static map contents — once
/// explored they stay gatherable even after current vision is lost.
#[test]
fn explored_source_stays_gatherable_after_vision_is_lost() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);

    // Scout the northern expansion gold, then return home: the source stays
    // known but falls out of current vision.
    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(45, 19))));
    refresh_visibility(&mut world, &map);
    world
        .entity_mut(villager)
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
        world.get::<WorkerTask>(villager),
        Some(WorkerTask::ToSource {
            source: ResourceId(13),
            ..
        })
    ));
}

/// Fog knowledge gate: own completed Farms are always valid gather
/// knowledge — the own-farm branch never consults exploration state.
#[test]
fn own_completed_farm_is_always_valid_gather_knowledge() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);
    // Spawned after the reveal and never refreshed: the farm is in the
    // ResourceIndex but its ground was never explored.
    let (_, farm_source) =
        spawn_completed_farm(&mut world, &mut map, TeamId(1), GridPos::new(45, 40));
    assert!(!explored_by(&world, TeamId(1), GridPos::new(45, 40)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], farm_source),
    );

    assert_eq!(
        outcome.reject, None,
        "own completed farm needs no exploration: {outcome:?}"
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
}

/// Fog knowledge gate: an enemy Farm is an enemy building — it needs
/// current visibility and is never admitted as a last-seen ghost merely
/// because it sits in the ResourceIndex.
#[test]
fn enemy_farm_is_never_an_own_gather_source_through_fog() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);
    let (_, farm_source) =
        spawn_completed_farm(&mut world, &mut map, TeamId(2), GridPos::new(45, 40));

    // Scout the enemy farm, then fall back home: its ground stays explored
    // but the building leaves current vision.
    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(45, 43))));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), GridPos::new(45, 40)));
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(20, 19))));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TeamId(1), GridPos::new(45, 40)));
    assert!(!visible_to(&world, TeamId(1), GridPos::new(45, 40)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], farm_source),
    );

    assert_eq!(
        outcome.reject,
        Some(RejectReason::Unexplored),
        "an enemy farm is not admitted as a last-seen ghost"
    );
}

/// Ownership: current vision makes an enemy Farm *known*, but it is still
/// never an own economic source — the command authority rejects it where
/// the Godot picker never offers it. The same unconditional rule covers
/// full-information worlds (no `VisibilityMap`).
#[test]
fn visible_enemy_farm_is_rejected_not_gathered() {
    let (mut world, mut map) = skirmish_world();
    reveal(&mut world, &map);
    let (_, farm_source) =
        spawn_completed_farm(&mut world, &mut map, TeamId(2), GridPos::new(45, 40));

    // Stand next to the farm: its footprint is currently visible.
    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(villager)
        .insert(SimPosition::new(map.cell_center(GridPos::new(45, 43))));
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), GridPos::new(45, 40)));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], farm_source),
    );

    assert_eq!(
        outcome.reject,
        Some(RejectReason::NotOwned),
        "a visible enemy farm is known but never gatherable: {outcome:?}"
    );
    assert!(outcome.accepted_units.is_empty());
}

/// The same ownership rule without fog: a full-information world refuses
/// the foreign Farm too — ownership is not a visibility artifact.
#[test]
fn full_information_enemy_farm_is_still_not_gatherable() {
    let (mut world, mut map) = skirmish_world();
    let (_, farm_source) =
        spawn_completed_farm(&mut world, &mut map, TeamId(2), GridPos::new(45, 40));

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], farm_source),
    );

    assert_eq!(outcome.reject, Some(RejectReason::NotOwned));
    assert!(outcome.accepted_units.is_empty());
}

#[test]
fn gather_replaces_an_active_move_order_and_frees_its_goal() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);
    world.insert_resource(IdAllocator::new(10, 1, 2));
    let villager = spawn_villager(&mut world, UnitId(1), Vec2::new(8.5, 8.5));
    let old_goal = GridPos::new(18, 6);
    world.entity_mut(villager).insert(MoveOrder {
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
        ResourceKind::Wood,
        GridPos::new(12, 12),
        400,
    );

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );

    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    match world.get::<WorkerTask>(villager).unwrap() {
        WorkerTask::ToSource { source, slot } => {
            assert_eq!(*source, ResourceId(1));
            assert_ne!(
                *slot, old_goal,
                "the stale move goal is not reused as the gather slot"
            );
        }
        other => panic!("expected ToSource, got {other:?}"),
    }
    let order = world.get::<MoveOrder>(villager).expect("fresh order");
    assert_ne!(
        order.goal, old_goal,
        "the gather route replaces, not keeps, the old one"
    );

    // The released goal is claimable again by the next move command.
    let late = spawn_villager(&mut world, UnitId(2), Vec2::new(8.5, 10.5));
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

    let villager = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(villager)
        .insert(SimPosition::new(Vec2::new(20.5, 42.5)));
    world.entity_mut(villager).insert(Carry::Holding {
        kind: ResourceKind::Gold,
        amount: NonZeroU32::new(7).unwrap(),
    });

    let outcome = apply_player_command(
        &mut world,
        &mut map,
        gather_command(TeamId(1), vec![UnitId(1)], ResourceId(1)),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    // Deposit order: the carried gold banks on arrival at the town-center
    // drop-off, then one food gather cycle banks food. The loop polls
    // movement + economy until both have landed in their stockpile arms.
    for _ in 0..4000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
        if state.stockpile.gold == 107 && state.stockpile.food == 210 {
            break;
        }
    }
    let state = &world.resource::<TeamEconomy>().0[&TeamId(1)];
    assert_eq!(state.stockpile.gold, 107, "carried gold banks in full");
    assert_eq!(state.stockpile.food, 210, "a gather cycle banks ten food");
    assert_eq!(state.stockpile.wood, 300, "wood stays untouched");
    assert_eq!(world.get::<Carry>(villager), Some(&Carry::Empty));
}

#[test]
fn farm_cycle_reclaims_the_farm_for_its_holder_and_idles_everyone_else() {
    let mut world = World::new();
    let mut map = GridMap::new(24, 24);
    test_economy(&mut world);

    // A completed Farm assigned to villager 1, plus a Town Center drop-off.
    let farm = world
        .spawn((
            Building {
                id: BuildingId(1),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::Farm,
                construction: crate::buildings::ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(4, 16), 2, 2),
            ResourceSource {
                id: ResourceId(1),
                kind: ResourceKind::Food,
                remaining: None,
                assigned_worker: Some(UnitId(1)),
            },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(1), farm);
    world
        .get_resource_or_insert_with(ResourceIndex::default)
        .insert(ResourceId(1), farm);
    let town_center = world
        .spawn((
            Building {
                id: BuildingId(2),
                team: TeamId(1),
                kind: crate::catalog::BuildingKind::TownCenter,
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
        .insert(BuildingId(2), town_center);

    // The holder deposits at the Town Center and routes back to his farm.
    let holder = spawn_villager(&mut world, UnitId(1), map.cell_center(GridPos::new(6, 17)));
    world.entity_mut(holder).insert((
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

    match world.get::<WorkerTask>(holder).unwrap() {
        WorkerTask::ToSource { source, .. } => {
            assert_eq!(*source, ResourceId(1), "the holder routes back to his farm")
        }
        other => panic!("expected ToSource, got {other:?}"),
    }
    assert_eq!(world.get::<Carry>(holder), Some(&Carry::Empty));
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.food,
        210,
        "the deposit banks the carried food"
    );

    // A second worker delivering to the same farm cannot reclaim it.
    let late = spawn_villager(&mut world, UnitId(2), map.cell_center(GridPos::new(6, 17)));
    world.entity_mut(late).insert((
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

    assert_eq!(world.get::<WorkerTask>(late), Some(&WorkerTask::Idle));
    assert!(world.get::<MoveOrder>(late).is_none());
    assert_eq!(
        world.get::<ResourceSource>(farm).unwrap().assigned_worker,
        Some(UnitId(1)),
        "the farm keeps serving only its assigned holder"
    );
}

#[test]
fn vanished_source_idles_empty_workers_and_routes_full_ones_to_the_dropoff() {
    let (mut world, mut map) = skirmish_world();

    let source_entity = world
        .resource::<ResourceIndex>()
        .entity(ResourceId(1))
        .expect("the fixture's berry bush");
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
        UnitKind::Villager,
        6.0,
    );
    world.entity_mut(worker).insert((
        Carry::Holding {
            kind: ResourceKind::Wood,
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
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(5).unwrap(),
        })
    );
}
