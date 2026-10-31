use bevy::prelude::World;

use super::*;
use crate::buildings::ConstructionState;
use crate::catalog::{MIN_SURFACE_EFFICIENCY, building_spec, unit_spec};
use crate::combat::{Health, destroy_building, destroy_unit};
use crate::commands::{PlayerCommand, apply_player_command};
use crate::economy::ResourceStockpile;
use crate::heat::HeatState;
use crate::ids::{IdAllocator, UnitId};
use crate::movement::SIM_STEP_SECONDS;

const TEAM: TeamId = TeamId(1);
const ENEMY: TeamId = TeamId(2);

fn open_world() -> (World, GridMap) {
    let mut world = World::new();
    let mut economy = TeamEconomy::default();
    for team in [TEAM, ENEMY] {
        economy.insert_team(
            team,
            ResourceStockpile {
                food: 1000,
                scrap: 1000,
                coal: 1000,
                crystals: 0,
            },
        );
    }
    world.insert_resource(economy);
    world.insert_resource(IdAllocator::new(100, 100, 100));
    (world, GridMap::new(64, 64))
}

fn complete_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
    team: TeamId,
) -> Entity {
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
    entity
}

fn incomplete_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
    team: TeamId,
) -> Entity {
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
                    complete: false,
                    active_builder: None,
                },
            },
            footprint,
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(id, entity);
    entity
}

fn enqueue(
    world: &mut World,
    map: &mut GridMap,
    building: BuildingId,
    kind: UnitKind,
) -> CommandResult {
    apply_player_command(
        world,
        map,
        PlayerCommand::EnqueueUnit {
            issuer: TEAM,
            building,
            kind,
        },
    )
}

fn stockpile(world: &World, team: TeamId) -> ResourceStockpile {
    world.resource::<TeamEconomy>().0[&team].stockpile
}

/// Enqueue spending is atomic through `try_spend`: a job the team cannot
/// fully afford rejects `InsufficientResources` with every currency field
/// unchanged and no job pushed. The Rifleman's Scrap leg gates even when
/// Food alone would cover it.
#[test]
fn unaffordable_enqueue_charges_nothing_and_queues_nothing() {
    let (mut world, mut map) = open_world();
    let barracks = complete_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::Barracks,
        GridPos::new(10, 10),
        TEAM,
    );
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&TEAM)
        .unwrap()
        .stockpile = ResourceStockpile {
        food: 1000,
        scrap: 0,
        coal: 1000,
        crystals: 0,
    };

    let result = enqueue(&mut world, &mut map, BuildingId(1), UnitKind::Rifleman);

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(
        stockpile(&world, TEAM),
        ResourceStockpile {
            food: 1000,
            scrap: 0,
            coal: 1000,
            crystals: 0,
        },
        "rejected enqueue spent resources"
    );
    assert!(
        world
            .get::<ProductionQueue>(barracks)
            .is_none_or(|queue| queue.jobs.is_empty()),
        "rejected enqueue still queued a job"
    );
}

fn queue_of(world: &World, entity: Entity) -> ProductionQueue {
    world.get::<ProductionQueue>(entity).cloned().unwrap()
}

/// Regression world: two Barracks queues hold ready Guard jobs at
/// population 9 of 10, built entirely through the production APIs.
fn setup_two_ready_barracks_at_pop_9_of_10() -> (World, GridMap, Entity, Entity) {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(8, 8),
        TEAM,
    );
    for index in 1..=9i32 {
        let cell = GridPos::new(2 + (index % 3) * 2, 2 + (index / 3) * 2);
        spawn_unit(
            &mut world,
            UnitId(index as u32),
            TEAM,
            map.cell_center(cell),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }
    let first = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Barracks,
        GridPos::new(20, 20),
        TEAM,
    );
    let second = complete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Barracks,
        GridPos::new(32, 32),
        TEAM,
    );

    let first_result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Guard);
    assert_eq!(first_result.reject, None, "first enqueue rejected");
    let second_result = enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Guard);
    assert_eq!(second_result.reject, None, "second enqueue rejected");
    // Both charged once at acceptance: 2 × 60 Food.
    assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);

    // Advance to just below readiness; nothing spawns while not ready.
    for _ in 0..399 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert_eq!(population_used(&world, TEAM), 9);
    assert!((queue_of(&world, first).progress_seconds - 19.95).abs() < 1e-3);
    assert!((queue_of(&world, second).progress_seconds - 19.95).abs() < 1e-3);

    (world, map, first, second)
}

#[test]
fn worker_enqueue_charges_fifty_food_exactly_once() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );

    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, None);
    assert_eq!(stockpile(&world, TEAM).food, 950);

    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    // Spawned exactly once, charged exactly once.
    assert_eq!(stockpile(&world, TEAM).food, 950);
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
    assert_eq!(queue_of(&world, research_center).jobs.len(), 0);
    assert_eq!(population_used(&world, TEAM), 1);
    assert_eq!(population_cap(&world, TEAM), 10);
}

#[test]
fn enqueue_with_insufficient_stockpile_rejects_and_never_charges() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    // 49 Food cannot afford the 50-Food Worker.
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&TEAM)
        .unwrap()
        .stockpile
        .food = 49;

    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(stockpile(&world, TEAM).food, 49, "rejected enqueue charged");
    assert!(
        world.get::<ProductionQueue>(research_center).is_none(),
        "rejected enqueue created a queue or pushed a job"
    );
}

#[test]
fn barracks_trains_guards_and_riflemen() {
    // The Barracks is the single military producer: both combat units queue
    // on it FIFO, each charged once at acceptance.
    let (mut world, mut map) = open_world();
    // A Research Center supplies the population cap the military units consume.
    complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::ResearchCenter,
        GridPos::new(40, 30),
        TEAM,
    );
    let barracks = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::Barracks,
        GridPos::new(30, 30),
        TEAM,
    );

    for kind in [UnitKind::Guard, UnitKind::Rifleman] {
        let result = enqueue(&mut world, &mut map, BuildingId(100), kind);
        assert_eq!(result.reject, None, "{kind:?} enqueue rejected");
    }
    let queue = queue_of(&world, barracks);
    let kinds: Vec<UnitKind> = queue
        .jobs
        .iter()
        .map(|job| match job.kind {
            ProductionKind::Unit(kind) => kind,
        })
        .collect();
    assert_eq!(kinds, vec![UnitKind::Guard, UnitKind::Rifleman]);
    assert_eq!(stockpile(&world, TEAM).food, 1000 - 60 - 40);
    assert_eq!(stockpile(&world, TEAM).scrap, 1000 - 40);

    // Both jobs complete through the real queue, spawning one of each
    // (20s Guard + 25s Rifleman = 900 ticks; slack covers spawn checks).
    for _ in 0..1200 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        if queue_of(&world, barracks).jobs.is_empty() {
            break;
        }
    }
    assert!(queue_of(&world, barracks).jobs.is_empty());
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
    assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_some());
    assert_eq!(population_used(&world, TEAM), 2);
}

#[test]
fn enqueue_validates_missing_unowned_incomplete_and_producer() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Barracks,
        GridPos::new(40, 40),
        ENEMY,
    );
    incomplete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Barracks,
        GridPos::new(50, 50),
        TEAM,
    );
    complete_building(
        &mut world,
        &mut map,
        BuildingId(103),
        BuildingKind::Bunkhouse,
        GridPos::new(8, 8),
        TEAM,
    );

    // Nonexistent building.
    let result = enqueue(&mut world, &mut map, BuildingId(999), UnitKind::Worker);
    assert_eq!(result.reject, Some(RejectReason::BuildingMissing));

    // Enemy building.
    let result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Guard);
    assert_eq!(result.reject, Some(RejectReason::NotOwned));

    // Incomplete producer: its queue is locked until completion.
    let result = enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Guard);
    assert_eq!(result.reject, Some(RejectReason::Locked));

    // Wrong producer on two fronts: units belong to specific buildings.
    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Guard);
    assert_eq!(result.reject, Some(RejectReason::WrongProducer));
    let result = enqueue(&mut world, &mut map, BuildingId(103), UnitKind::Worker);
    assert_eq!(result.reject, Some(RejectReason::WrongProducer));

    // No rejection charged anything or created a queue.
    assert_eq!(
        stockpile(&world, TEAM),
        ResourceStockpile {
            food: 1000,
            scrap: 1000,
            coal: 1000,
            crystals: 0,
        }
    );
    let mut queues = world.query::<&ProductionQueue>();
    assert_eq!(queues.iter(&world).count(), 0);
}

#[test]
fn two_ready_barracks_spawn_one_then_block_population_full() {
    let (mut world, mut map, first, second) = setup_two_ready_barracks_at_pop_9_of_10();

    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    // Ascending BuildingId: Barracks 101 wins the final slot.
    assert_eq!(population_used(&world, TEAM), 10);
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
    let first_queue = queue_of(&world, first);
    assert!(first_queue.jobs.is_empty());
    assert_eq!(first_queue.blocked, None);
    let second_queue = queue_of(&world, second);
    assert_eq!(second_queue.jobs.len(), 1);
    assert_eq!(second_queue.progress_seconds, 20.0);
    assert_eq!(second_queue.blocked, Some(RejectReason::PopulationFull));
    assert_eq!(world.resource::<IdAllocator>().next_unit, 101);
}

#[test]
fn blocked_ready_job_sits_at_full_and_never_recharges() {
    let (mut world, mut map, _first, second) = setup_two_ready_barracks_at_pop_9_of_10();
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    // Waiting at 100% neither restarts training nor charges again.
    for _ in 0..100 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert_eq!(queue_of(&world, second).progress_seconds, 20.0);
    assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);

    // A freed slot spawns the retained job immediately, still uncharged.
    let filler = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.despawn(filler);
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(population_used(&world, TEAM), 10);
    assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_some());
    assert_eq!(queue_of(&world, second).jobs.len(), 0);
    assert_eq!(queue_of(&world, second).blocked, None);
    assert_eq!(stockpile(&world, TEAM).food, 1000 - 120);
}

#[test]
fn walled_producer_keeps_ready_job_with_no_spawn_space() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, None);

    // Wall in the whole immediate perimeter.
    map.set_blocked_rect(GridPos::new(29, 29), GridPos::new(34, 34));
    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    let queue = queue_of(&world, research_center);
    assert_eq!(queue.progress_seconds, 15.0);
    assert_eq!(queue.jobs.len(), 1);
    assert_eq!(queue.blocked, Some(RejectReason::NoSpawnSpace));
    assert_eq!(population_used(&world, TEAM), 0);
    assert_eq!(world.resource::<IdAllocator>().next_unit, 100);

    // Reopening one perimeter cell spawns the retained job next tick.
    map.set_blocked(GridPos::new(29, 29), false);
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(population_used(&world, TEAM), 1);
    assert_eq!(world.resource::<IdAllocator>().next_unit, 101);
    let queue = queue_of(&world, research_center);
    assert!(queue.jobs.is_empty());
    assert_eq!(queue.blocked, None);
    let spawned = world.resource::<UnitIndex>().entity(UnitId(100)).unwrap();
    let position = world.get::<SimPosition>(spawned).unwrap().current;
    assert_eq!(map.world_to_cell(position), GridPos::new(29, 29));
}

#[test]
fn occupied_perimeter_cells_block_spawn() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, None);

    // Squatters on every immediate-perimeter cell: walkable but occupied.
    // They are enemy units so the team's own population stays below cap —
    // this isolates the spawn-clearance reject.
    for (index, cell) in Footprint::new(GridPos::new(30, 30), 4, 4)
        .perimeter_cells()
        .into_iter()
        .enumerate()
    {
        spawn_unit(
            &mut world,
            UnitId(index as u32 + 1),
            ENEMY,
            map.cell_center(cell),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }
    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert_eq!(
        queue_of(&world, research_center).blocked,
        Some(RejectReason::NoSpawnSpace)
    );
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_none());
}

#[test]
fn reserved_goal_on_last_free_perimeter_cell_blocks_spawn() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, None);

    // Block every perimeter cell except (29, 29), then reserve exactly that
    // cell as another unit's MoveOrder goal — the reservation seam counts
    // it as occupied.
    for cell in Footprint::new(GridPos::new(30, 30), 4, 4).perimeter_cells() {
        if cell != GridPos::new(29, 29) {
            map.set_blocked(cell, true);
        }
    }
    let traveler = spawn_unit(
        &mut world,
        UnitId(1),
        TEAM,
        map.cell_center(GridPos::new(2, 2)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    world.entity_mut(traveler).insert(MoveOrder {
        waypoints: vec![map.cell_center(GridPos::new(29, 29))],
        next: 0,
        goal: GridPos::new(29, 29),
        map_revision: map.revision(),
        last_failed_replan: None,
    });

    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert_eq!(
        queue_of(&world, research_center).blocked,
        Some(RejectReason::NoSpawnSpace)
    );
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_none());
}

#[test]
fn two_same_tick_rallied_spawns_never_share_a_goal() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(8, 8),
        TEAM,
    );
    let _first = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Barracks,
        GridPos::new(20, 20),
        TEAM,
    );
    let _second = complete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Barracks,
        GridPos::new(40, 40),
        TEAM,
    );
    // Both producers rally to the same target cell.
    for building in [BuildingId(101), BuildingId(102)] {
        let result = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::SetRally {
                issuer: TEAM,
                building,
                target: GridPos::new(14, 14),
            },
        );
        assert_eq!(result.reject, None);
    }
    assert_eq!(
        enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Guard).reject,
        None
    );
    assert_eq!(
        enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Guard).reject,
        None
    );

    // Both jobs complete in the same step_production call.
    for _ in 0..401 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }

    let index = world.resource::<UnitIndex>();
    let first_unit = index.entity(UnitId(100)).expect("first guard spawned");
    let second_unit = index.entity(UnitId(101)).expect("second guard spawned");
    let first_goal = world
        .get::<MoveOrder>(first_unit)
        .expect("first rallied unit moves")
        .goal;
    let second_goal = world
        .get::<MoveOrder>(second_unit)
        .expect("second rallied unit moves")
        .goal;
    assert_eq!(first_goal, GridPos::new(14, 14));
    assert_ne!(
        first_goal, second_goal,
        "rallied spawns claimed one reserved goal cell"
    );
}

#[test]
fn rally_routes_spawned_units_and_failed_rally_leaves_them_idle() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    let _barracks = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Barracks,
        GridPos::new(40, 40),
        TEAM,
    );

    // Reachable rally: the spawned worker gets a normal Move.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(100),
            target: GridPos::new(10, 10),
        },
    );
    assert_eq!(result.reject, None);
    assert_eq!(
        world.get::<RallyPoint>(research_center),
        Some(&RallyPoint(GridPos::new(10, 10)))
    );
    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker);
    assert_eq!(result.reject, None);
    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    let worker = world.resource::<UnitIndex>().entity(UnitId(100)).unwrap();
    let order = world.get::<MoveOrder>(worker).expect("rallied unit moves");
    assert_eq!(order.goal, GridPos::new(10, 10));
    assert_eq!(
        order.waypoints.last().copied(),
        Some(map.cell_center(GridPos::new(10, 10)))
    );

    // Unreachable rally: the guard still spawns, but idles.
    map.set_blocked_rect(GridPos::new(0, 0), GridPos::new(8, 8));
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(101),
            target: GridPos::new(5, 5),
        },
    );
    assert_eq!(result.reject, None);
    let result = enqueue(&mut world, &mut map, BuildingId(101), UnitKind::Guard);
    assert_eq!(result.reject, None);
    for _ in 0..401 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    let guard = world.resource::<UnitIndex>().entity(UnitId(101)).unwrap();
    assert!(world.get::<MoveOrder>(guard).is_none());
    assert_eq!(world.get::<WorkerTask>(guard), Some(&WorkerTask::Idle));
    let position = world.get::<SimPosition>(guard).unwrap().current;
    assert!(
        Footprint::new(GridPos::new(40, 40), 3, 3)
            .is_immediately_adjacent(map.world_to_cell(position)),
        "spawned at the producer perimeter, got {position:?}"
    );
}

#[test]
fn set_rally_validates_missing_and_unowned_buildings() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    let enemy = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::ResearchCenter,
        GridPos::new(50, 50),
        ENEMY,
    );
    let bunkhouse = complete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Bunkhouse,
        GridPos::new(10, 50),
        TEAM,
    );

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(999),
            target: GridPos::new(10, 10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::BuildingMissing));

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(101),
            target: GridPos::new(10, 10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::NotOwned));
    assert_eq!(world.get::<RallyPoint>(enemy), None);

    // An owned non-producer rejects without storing a rally point.
    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(102),
            target: GridPos::new(10, 10),
        },
    );
    assert_eq!(result.reject, Some(RejectReason::WrongProducer));
    assert_eq!(world.get::<RallyPoint>(bunkhouse), None);
}

#[test]
fn queue_is_fifo_and_only_the_head_advances() {
    let (mut world, mut map) = open_world();
    let research_center = complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(30, 30),
        TEAM,
    );
    assert_eq!(
        enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker).reject,
        None
    );
    assert_eq!(
        enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Worker).reject,
        None
    );
    assert_eq!(stockpile(&world, TEAM).food, 900);

    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    // Exactly one spawn; the second job is now the advancing head.
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
    assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_none());
    let queue = queue_of(&world, research_center);
    assert_eq!(queue.jobs.len(), 1);
    assert!(queue.progress_seconds > 0.0);

    for _ in 0..301 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert!(world.resource::<UnitIndex>().entity(UnitId(101)).is_some());
    assert_eq!(queue_of(&world, research_center).jobs.len(), 0);
    assert_eq!(population_used(&world, TEAM), 2);
    assert_eq!(stockpile(&world, TEAM).food, 900);
}

#[test]
fn population_cap_sums_completed_tc_and_bunkhouse_and_clamps() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(8, 8),
        TEAM,
    );
    assert_eq!(population_cap(&world, TEAM), 10);

    incomplete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Bunkhouse,
        GridPos::new(20, 20),
        TEAM,
    );
    assert_eq!(
        population_cap(&world, TEAM),
        10,
        "incomplete Bunkhouse must not count"
    );

    complete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Bunkhouse,
        GridPos::new(30, 30),
        TEAM,
    );
    assert_eq!(population_cap(&world, TEAM), 20);

    for index in 0..9i32 {
        complete_building(
            &mut world,
            &mut map,
            BuildingId((103 + index) as u32),
            BuildingKind::Bunkhouse,
            GridPos::new(40 + (index % 3) * 6, 8 + (index / 3) * 6),
            TEAM,
        );
    }
    // 10 RC + 10 Bunkhouses would exceed the clamp.
    assert_eq!(population_cap(&world, TEAM), 100);

    // Population is per team: the enemy's units and buildings don't count.
    spawn_unit(
        &mut world,
        UnitId(1),
        ENEMY,
        map.cell_center(GridPos::new(2, 2)),
        UnitKind::Worker,
        unit_spec(UnitKind::Worker).speed,
    );
    complete_building(
        &mut world,
        &mut map,
        BuildingId(200),
        BuildingKind::Bunkhouse,
        GridPos::new(56, 56),
        ENEMY,
    );
    assert_eq!(population_used(&world, ENEMY), 1);
    assert_eq!(population_used(&world, TEAM), 0);
    assert_eq!(population_cap(&world, ENEMY), 10);
}

#[test]
fn unaffordable_enqueue_rejects_and_banks_nothing() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::Barracks,
        GridPos::new(8, 8),
        TEAM,
    );
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&TEAM)
        .unwrap()
        .stockpile = ResourceStockpile {
        food: 0,
        scrap: 0,
        coal: 0,
        crystals: 0,
    };

    let result = enqueue(&mut world, &mut map, BuildingId(100), UnitKind::Guard);

    assert_eq!(result.reject, Some(RejectReason::InsufficientResources));
    assert_eq!(
        stockpile(&world, TEAM),
        ResourceStockpile {
            food: 0,
            scrap: 0,
            coal: 0,
            crystals: 0,
        },
        "a rejected enqueue must not touch the stockpile"
    );
}

/// Destroying a Bunkhouse lowers the derived cap and never deletes living units
/// above it: `population_cap()` falls out of the BuildingIndex naturally.
#[test]
fn destroying_a_bunkhouse_lowers_the_cap_and_never_deletes_living_units() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(8, 8),
        TEAM,
    );
    let bunkhouse = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Bunkhouse,
        GridPos::new(20, 20),
        TEAM,
    );
    for index in 1..=12u32 {
        spawn_unit(
            &mut world,
            UnitId(index),
            TEAM,
            map.cell_center(GridPos::new(2 + index as i32, 2)),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }
    assert_eq!(population_cap(&world, TEAM), 20);
    assert_eq!(population_used(&world, TEAM), 12);

    destroy_building(&mut world, &mut map, bunkhouse);

    assert_eq!(
        population_cap(&world, TEAM),
        10,
        "the derived cap falls with the Bunkhouse"
    );
    assert_eq!(
        population_used(&world, TEAM),
        12,
        "a reduced cap never deletes living units"
    );
    for index in 1..=12u32 {
        assert!(
            world
                .resource::<UnitIndex>()
                .entity(UnitId(index))
                .is_some()
        );
    }
}

/// Reduced-cap regression: production respects the reduced derived cap — a
/// ready head blocks with PopulationFull at or above it, then completes
/// sequentially once the team falls under it.
#[test]
fn production_respects_a_reduced_cap_and_completes_sequentially() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::ResearchCenter,
        GridPos::new(8, 8),
        TEAM,
    );
    let bunkhouse = complete_building(
        &mut world,
        &mut map,
        BuildingId(101),
        BuildingKind::Bunkhouse,
        GridPos::new(20, 20),
        TEAM,
    );
    let barracks = complete_building(
        &mut world,
        &mut map,
        BuildingId(102),
        BuildingKind::Barracks,
        GridPos::new(32, 32),
        TEAM,
    );
    for index in 1..=12u32 {
        spawn_unit(
            &mut world,
            UnitId(index),
            TEAM,
            map.cell_center(GridPos::new(2 + index as i32, 2)),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }

    // The Bunkhouse falls before training: the derived cap is reduced to 10.
    destroy_building(&mut world, &mut map, bunkhouse);
    assert_eq!(population_cap(&world, TEAM), 10);
    assert_eq!(population_used(&world, TEAM), 12);

    let accepted = enqueue(&mut world, &mut map, BuildingId(102), UnitKind::Guard);
    assert_eq!(accepted.reject, None, "enqueue accepts regardless of cap");

    // Ready but 12 >= 10: the head sits at 100%, blocked, nothing spawns.
    for _ in 0..401 {
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert_eq!(queue_of(&world, barracks).progress_seconds, 20.0);
    assert_eq!(population_used(&world, TEAM), 12);
    assert_eq!(
        queue_of(&world, barracks).blocked,
        Some(RejectReason::PopulationFull)
    );
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_none());

    // Combat deaths drop the team under the reduced cap.
    for id in [UnitId(1), UnitId(2), UnitId(3)] {
        let entity = world.resource::<UnitIndex>().entity(id).unwrap();
        world.get_mut::<Health>(entity).unwrap().current = 0;
        destroy_unit(&mut world, entity);
    }
    assert_eq!(population_used(&world, TEAM), 9);

    // The retained job completes sequentially, exactly once, uncharged again.
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(population_used(&world, TEAM), 10);
    assert!(world.resource::<UnitIndex>().entity(UnitId(100)).is_some());
    assert_eq!(queue_of(&world, barracks).jobs.len(), 0);
    assert_eq!(queue_of(&world, barracks).blocked, None);
    assert_eq!(stockpile(&world, TEAM).food, 1000 - 60);
}

/// Review item: train/rally on a hidden enemy building must answer
/// `BuildingMissing` exactly like an absent id — distinct rejects would let
/// a caller probe ids to count enemy producers. Visible enemy buildings
/// stay `NotOwned`.
#[test]
fn building_commands_on_a_hidden_enemy_building_answer_building_missing() {
    use crate::visibility::{VisibilityMap, refresh_visibility};

    let (mut world, mut map) = open_world();
    world.insert_resource(VisibilityMap::default());
    let enemy = complete_building(
        &mut world,
        &mut map,
        BuildingId(150),
        BuildingKind::Barracks,
        GridPos::new(40, 40),
        ENEMY,
    );
    refresh_visibility(&mut world, &map);
    assert!(
        !crate::visibility::visible_to(&world, TEAM, GridPos::new(40, 40)),
        "premise: the enemy barracks starts hidden"
    );

    let hidden_enqueue = enqueue(&mut world, &mut map, BuildingId(150), UnitKind::Guard);
    assert_eq!(hidden_enqueue.reject, Some(RejectReason::BuildingMissing));

    let hidden_rifleman = enqueue(&mut world, &mut map, BuildingId(150), UnitKind::Rifleman);
    assert_eq!(hidden_rifleman.reject, Some(RejectReason::BuildingMissing));

    let rally = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: BuildingId(150),
            target: GridPos::new(10, 10),
        },
    );
    assert_eq!(rally.reject, Some(RejectReason::BuildingMissing));
    assert_eq!(world.get::<RallyPoint>(enemy), None);

    // Once genuinely seen, the honest answer returns.
    crate::commands::spawn_unit(
        &mut world,
        UnitId(200),
        TEAM,
        bevy::math::Vec2::new(41.5, 43.5),
        UnitKind::Worker,
        6.0,
    );
    refresh_visibility(&mut world, &map);
    assert!(crate::visibility::visible_to(
        &world,
        TEAM,
        GridPos::new(40, 40)
    ));
    let visible = enqueue(&mut world, &mut map, BuildingId(150), UnitKind::Guard);
    assert_eq!(visible.reject, Some(RejectReason::NotOwned));
}

#[test]
fn aliens_are_rejected_by_normal_enqueue_commands_without_spending() {
    let (mut world, mut map) = open_world();
    complete_building(
        &mut world,
        &mut map,
        BuildingId(100),
        BuildingKind::Barracks,
        GridPos::new(20, 20),
        TEAM,
    );
    let initial = stockpile(&world, TEAM);

    for kind in [
        UnitKind::Swarmer,
        UnitKind::Spitter,
        UnitKind::Brute,
        UnitKind::Behemoth,
    ] {
        assert_eq!(
            enqueue(&mut world, &mut map, BuildingId(100), kind).reject,
            Some(RejectReason::WrongProducer),
            "normal enqueue accepted {kind:?}"
        );
    }
    assert_eq!(stockpile(&world, TEAM), initial);
    let barracks = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(100))
        .expect("the completed Barracks");
    assert!(world.get::<ProductionQueue>(barracks).is_none());
}

/// Cold surface efficiency slows the queue head: one tick at the 25% floor
/// advances a Worker job by `0.05 × 0.25` and the job stays queued, with the
/// blocked/spawn rules untouched (nothing is ready to block here).
#[test]
fn cold_surface_efficiency_slows_the_queue_head() {
    let (mut world, mut map) = open_world();
    let rc = complete_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(10, 10),
        TEAM,
    );
    let result = enqueue(&mut world, &mut map, BuildingId(1), UnitKind::Worker);
    assert_eq!(result.reject, None);
    world.insert_resource(HeatState {
        supply: 0,
        demand: 4,
        ..HeatState::default()
    });

    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    let queue = world.get::<ProductionQueue>(rc).unwrap();
    let expected = SIM_STEP_SECONDS * MIN_SURFACE_EFFICIENCY;
    assert!(
        (queue.progress_seconds - expected).abs() < 1e-6,
        "cold queue head advanced {}, expected {expected}",
        queue.progress_seconds
    );
    assert!(!queue.jobs.is_empty());
    assert_eq!(queue.blocked, None);
}

/// Without a `HeatState` resource the head advances at the full rate and
/// nothing panics — the neutral `surface_efficiency` fallback.
#[test]
fn missing_heat_state_keeps_the_queue_head_at_full_rate() {
    let (mut world, mut map) = open_world();
    let rc = complete_building(
        &mut world,
        &mut map,
        BuildingId(1),
        BuildingKind::ResearchCenter,
        GridPos::new(10, 10),
        TEAM,
    );
    let result = enqueue(&mut world, &mut map, BuildingId(1), UnitKind::Worker);
    assert_eq!(result.reject, None);
    assert!(world.get_resource::<HeatState>().is_none());

    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    let queue = world.get::<ProductionQueue>(rc).unwrap();
    assert!(
        (queue.progress_seconds - SIM_STEP_SECONDS).abs() < 1e-6,
        "warm queue head advanced {}, expected {}",
        queue.progress_seconds,
        SIM_STEP_SECONDS
    );
}
