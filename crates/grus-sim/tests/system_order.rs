//! Canonical fixed-step order: `step_tick` runs waves → heat → combat →
//! movement → economy → construction → production → visibility, and these
//! tests call that one function. Aliens enter combat in their spawn tick,
//! heat computed in the tick scales its combat/production consumers, movement
//! arrival is visible to economy/construction in the same tick, construction
//! completion feeds production, and rallied production moves next.

use std::cell::RefCell;
use std::collections::VecDeque;

use bevy::math::Vec2;
use bevy::prelude::{Entity, World};
use grus_sim::catalog::{building_spec, unit_spec};
use grus_sim::{
    AttackCooldown, Building, BuildingId, BuildingIndex, BuildingKind, Carry, CombatOrder,
    CombatTarget, ConstructionState, Dropoff, EntryZone, Footprint, GatherProgress, GridMap,
    GridPos, Health, HeatState, MIN_SURFACE_EFFICIENCY, MapFixture, MatchPhase, MatchSession,
    MoveOrder, PlayerCommand, ProductionJob, ProductionKind, ProductionQueue, ResourceId,
    ResourceKind, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId, UnitId, UnitIndex, UnitKind,
    VisibilityMap, WaveState, WorkerTask, apply_player_command, explored_by, population_used,
    refresh_visibility, seed_skirmish, spawn_unit, step_combat, step_construction, step_production,
    step_tick, step_waves, visible_to,
};

const TEAM: TeamId = TeamId(1);
/// Authored team-1 Research Center and ration cache.
const RESEARCH_CENTER: BuildingId = BuildingId(1);
const RATIONS: ResourceId = ResourceId(1);
/// The placed Barracks takes the first runtime building id after the seeded
/// human Research Center.
const BARRACKS: BuildingId = BuildingId(2);

/// Production runs after movement, economy and construction: in the tick a
/// queued unit completes, a worker's gather arrival transitions to
/// `Gathering`, a builder's arrival transitions to `Constructing` and the
/// spawn exists — but the rallied spawn has not moved yet, because movement
/// already ran. It starts marching on the next tick.
#[test]
fn production_completion_spawns_after_movement_so_the_rallied_unit_marches_next_tick() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    // Gatherer A parks beside the authored safe ration cache; worker 2 stays
    // home as the builder for the authored Barracks slot.
    let rations_cell = MapFixture::starting_resources()
        .into_iter()
        .find(|spawn| spawn.id == RATIONS)
        .expect("the authored ration cache")
        .cell;
    let gatherer = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(gatherer).insert(SimPosition::new(
        map.cell_center(GridPos::new(rations_cell.x - 1, rations_cell.y)),
    ));
    let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();

    // A Research Center rally point on open ground south-east of the
    // footprint: the spawn must march to it, but only after its completion
    // tick.
    let rally = GridPos::new(70, 58);

    // Commands queued ahead of the chain; the helper drains whatever is
    // queued through the public dispatcher before each tick's systems run.
    let queue: RefCell<Vec<PlayerCommand>> = RefCell::new(vec![
        PlayerCommand::SetRally {
            issuer: TEAM,
            building: RESEARCH_CENTER,
            target: rally,
        },
        PlayerCommand::EnqueueUnit {
            issuer: TEAM,
            building: RESEARCH_CENTER,
            kind: UnitKind::Worker,
        },
    ]);
    let apply_queued_test_commands = |world: &mut World, map: &mut GridMap| {
        for command in queue.borrow_mut().drain(..) {
            let outcome = apply_player_command(world, map, command);
            assert_eq!(outcome.reject, None, "queued command rejected: {outcome:?}");
        }
    };

    // Tick arithmetic (0.3 cells per movement tick at worker speed):
    // - The Worker job enqueued at tick 1 completes at tick 300 (15s /
    //   0.05).
    // - Barracks placed at tick 270: builder walks 6 cells over 24 movement
    //   ticks (271..=294) and arrives at tick 294.
    // - Gather tasked at tick 296: gatherer walks 1 cell over 4 movement
    //   ticks (297..=300) and arrives at tick 300.
    let place_barracks = PlayerCommand::PlaceBuilding {
        issuer: TEAM,
        builder: UnitId(2),
        kind: BuildingKind::Barracks,
        anchor: MapFixture::barracks_anchor(TEAM),
    };
    let gather_rations = PlayerCommand::Gather {
        issuer: TEAM,
        workers: vec![UnitId(1)],
        source: RATIONS,
    };

    for tick in 1..=300 {
        match tick {
            270 => queue.borrow_mut().push(place_barracks.clone()),
            296 => queue.borrow_mut().push(gather_rations.clone()),
            _ => {}
        }
        apply_queued_test_commands(&mut world, &mut map);
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    // The completion tick: both arrivals are visible to their consumers in
    // this same tick.
    assert!(
        matches!(
            world.get::<WorkerTask>(gatherer),
            Some(WorkerTask::Gathering { source }) if *source == RATIONS
        ),
        "the gather arrival must transition inside this tick's economy step"
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(builder),
            Some(WorkerTask::Constructing { building }) if *building == BARRACKS
        ),
        "the builder arrival must transition inside this tick's construction step"
    );
    assert!(
        world
            .get::<Building>(world.resource::<BuildingIndex>().entity(BARRACKS).unwrap())
            .unwrap()
            .construction
            .progress_seconds
            > 0.0,
        "construction advanced in the arrival tick"
    );

    // Production ran last: the rallied spawn exists and holds its MoveOrder,
    // but it still stands exactly on its spawn cell — movement already ran
    // this tick, so it cannot have marched.
    let spawned = world
        .resource::<UnitIndex>()
        .entity(UnitId(5))
        .expect("the trained worker spawned in this tick");
    let spawn_position = world.get::<SimPosition>(spawned).unwrap().current;
    let spawn_cell = map.world_to_cell(spawn_position);
    assert_eq!(
        spawn_position,
        map.cell_center(spawn_cell),
        "the rallied spawn must not move inside its completion tick"
    );
    assert!(
        world.get::<MoveOrder>(spawned).is_some(),
        "the rally order is installed inside the completion tick"
    );

    // One more tick through the canonical `step_tick`: the same unit has
    // now left its spawn cell, proving the movement step consumed the rally
    // order only from the next tick on.
    apply_queued_test_commands(&mut world, &mut map);
    step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_ne!(
        world.get::<SimPosition>(spawned).unwrap().current,
        spawn_position,
        "the rallied spawn must start marching on the next tick"
    );
}

/// Combat-before-economy is a correctness contract: in one tick a carrying
/// worker stands at its drop-off slot while combat destroys that drop-off.
/// The deposit at the dead slot must never land — the destruction retasks the
/// worker before the economy step reads its arrival. Moving combat after
/// economy makes this test fail: the stale deposit banks first.
#[test]
fn combat_destroys_the_dropoff_before_economy_deposits_at_it() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    // A second team-1 drop-off beside the Research Center's resource line so the
    // destroyed drop-off's worker has a same-team reroute target. Placed
    // through the public command path on the authored Depot slot and
    // completed instantly; BuildingId 2 follows the single seeded core.
    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Depot,
            anchor: MapFixture::depot_anchor(TEAM),
        },
    );
    assert_eq!(placed.reject, None, "depot placement rejected");
    let depot_id = BuildingId(2);
    let depot = world.resource::<BuildingIndex>().entity(depot_id).unwrap();
    world
        .get_mut::<Building>(depot)
        .unwrap()
        .construction
        .complete = true;
    world.entity_mut(depot).insert(Dropoff { team: TEAM });

    // Carrying worker parked exactly at its Research Center drop-off slot —
    // a free cell on the west face of the central city's footprint.
    let dropoff_slot = GridPos::new(
        MapFixture::team_starts()[0].research_center_anchor.x - 1,
        47,
    );
    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(worker).insert((
        SimPosition::new(map.cell_center(dropoff_slot)),
        Carry::Holding {
            kind: ResourceKind::Scrap,
            amount: std::num::NonZeroU32::new(6).unwrap(),
        },
        GatherProgress::default(),
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: RESEARCH_CENTER,
            slot: dropoff_slot,
        },
    ));

    // A raider in range of the Research Center with a direct attack order; the
    // killing blow lands in this tick's combat step.
    let raider = spawn_unit(
        &mut world,
        UnitId(100),
        TeamId(2),
        map.cell_center(GridPos::new(dropoff_slot.x, 49)),
        UnitKind::Guard,
        unit_spec(UnitKind::Guard).speed,
    );
    world.entity_mut(raider).insert(CombatOrder::Attack {
        target: CombatTarget::Building(RESEARCH_CENTER),
        last_target_cell: None,
    });
    let research_center = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .unwrap();
    world.get_mut::<Health>(research_center).unwrap().current = 10;

    // The canonical tick under test: inside `step_tick`, combat runs
    // before movement and movement before economy, so the stale deposit at
    // the just-destroyed drop-off can never land.
    step_tick(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(RESEARCH_CENTER)
            .is_none(),
        "combat destroyed the drop-off inside this tick"
    );
    match world.get::<WorkerTask>(worker) {
        Some(WorkerTask::ToDropoff { dropoff, .. }) => assert_eq!(
            *dropoff, depot_id,
            "the worker was retasked to the surviving drop-off"
        ),
        other => panic!("expected a rerouted ToDropoff, got {other:?}"),
    }
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TEAM].stockpile.scrap,
        300 - 75,
        "the stale deposit at the just-destroyed drop-off must never land"
    );
}

#[test]
fn wave_spawn_is_acquired_and_hits_the_core_in_the_same_tick() {
    wave_spawn_hits_core_in_launch_tick(false);
}

/// The same ordering guarantee on the shipped fog-enabled path: the
/// director's spawn-time vision grant must put the core inside team-2
/// sight before `step_combat` runs, otherwise `acquire_target` rejects
/// every candidate until the end-of-tick visibility refresh.
#[test]
fn wave_spawn_is_acquired_and_hits_the_core_in_the_same_tick_under_fog() {
    wave_spawn_hits_core_in_launch_tick(true);
}

fn wave_spawn_hits_core_in_launch_tick(fog: bool) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    if fog {
        world.insert_resource(VisibilityMap::default());
    }

    // Move the core beside the authored north entry so wave 1 is already
    // inside attack range when it launches. This makes the chain order
    // observable: combat must run after the director in the spawn tick.
    let core_entity = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .unwrap();
    let old = *world.get::<Footprint>(core_entity).unwrap();
    for cell in old.cells() {
        map.set_blocked(cell, false);
    }
    let north = EntryZone::North.anchor();
    let core = Footprint::new(GridPos::new(north.x - 2, north.y + 1), 4, 4);
    for cell in core.cells() {
        assert!(map.is_walkable(cell));
        map.set_blocked(cell, true);
    }
    world.entity_mut(core_entity).insert(core);
    world.get_mut::<Health>(core_entity).unwrap().current = 1;
    world.insert_resource(WaveState {
        elapsed: 299.95,
        next_wave: 1,
    });
    if fog {
        refresh_visibility(&mut world, &map);
    }

    step_waves(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(world.resource::<WaveState>().next_wave, 2);
    assert_eq!(world.resource::<UnitIndex>().iter().count(), 16);

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(
        world.resource::<MatchSession>().phase,
        MatchPhase::Result(grus_sim::MatchResult(TeamId(2)))
    );
    assert!(
        world
            .resource::<grus_sim::CombatEvents>()
            .0
            .iter()
            .any(|event| {
                event.target == CombatTarget::Building(RESEARCH_CENTER) && event.killed
            })
    );
}

/// Visibility contract of the construction boundary: a newly placed site
/// grants no vision of itself while incomplete, and completion turns the
/// footprint into a reveal origin. With every unit far from the site, the
/// granted vision can only come from the completed building.
#[test]
fn incomplete_site_grants_no_vision_and_completion_grants_it() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    // Fog opt-in for this contract test; normal Godot setup gains the map
    // in a later task.
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);

    // Scout the future site, then walk home: the ground stays explored but
    // falls out of current vision.
    let site = Footprint::new(GridPos::new(30, 46), 2, 2); // Depot 2×2
    let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();
    world
        .entity_mut(builder)
        .insert(SimPosition::new(map.cell_center(GridPos::new(30, 46))));
    refresh_visibility(&mut world, &map);
    world
        .entity_mut(builder)
        .insert(SimPosition::new(map.cell_center(GridPos::new(17, 52))));
    refresh_visibility(&mut world, &map);
    assert!(explored_by(&world, TEAM, site));
    assert!(
        !visible_to(&world, TEAM, site),
        "scouted ground is explored but no longer currently visible"
    );

    // Placement on explored ground is accepted; the incomplete site grants
    // no vision of itself.
    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Depot,
            anchor: GridPos::new(30, 46),
        },
    );
    assert_eq!(placed.reject, None, "placement on explored ground rejected");
    let depot = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(2))
        .unwrap();
    // Live-state check: recompute fog so this assert sees the freshly
    // placed site, not the pre-placement map the last refresh produced.
    refresh_visibility(&mut world, &map);
    assert!(
        !visible_to(&world, TEAM, site),
        "a newly placed incomplete site grants no vision"
    );

    // Completion is what flips the site into a reveal origin — driven
    // through the real construction step (teleport the tasked builder onto
    // its stored slot, then step until the 20 s Depot job finishes;
    // the bound covers the full build plus float-error slack)
    // instead of poking `construction.complete` by hand.
    let slot = match world.get::<WorkerTask>(builder).unwrap() {
        WorkerTask::ToConstruction { slot, .. } => *slot,
        other => panic!("placed site must task its builder, got {other:?}"),
    };
    world
        .entity_mut(builder)
        .insert(SimPosition::new(map.cell_center(slot)));
    for _ in 0..450 {
        step_construction(&mut world, &map, SIM_STEP_SECONDS);
        if world.get::<Building>(depot).unwrap().construction.complete {
            break;
        }
    }
    assert!(
        world.get::<Building>(depot).unwrap().construction.complete,
        "the depot never completed through the real construction step"
    );
    refresh_visibility(&mut world, &map);
    assert!(
        visible_to(&world, TEAM, site),
        "construction completion grants vision"
    );
}

// ---- Canonical step_tick ordering ---------------------------------------

/// Directly seeds a completed Team-1 building, blocking its footprint and
/// registering the index — the same constructor the heat and combat unit
/// tests use. Gun turrets come with a mid-recovery `AttackCooldown(1.0)`.
fn seed_completed_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
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
                team: TEAM,
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
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
    if kind == BuildingKind::GunTurret {
        world.entity_mut(entity).insert(AttackCooldown(1.0));
    }
    entity
}

/// The position one movement leg away from `slot`'s centre, just inside a
/// walkable neighbour cell (past the shared cell border): `floor` maps to
/// that neighbour, but one 0.3-cell worker step toward the centre crosses
/// the border into the slot cell.
fn border_position(map: &GridMap, slot: GridPos) -> Vec2 {
    let center = map.cell_center(slot);
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        if map.is_walkable(GridPos::new(slot.x + dx, slot.y + dy)) {
            return Vec2::new(center.x + dx as f32 * 0.6, center.y + dy as f32 * 0.6);
        }
    }
    panic!("no walkable neighbour beside slot {slot:?}");
}

/// The cold surface of one tick: heat computed at the top of `step_tick`
/// must scale the SAME tick's turret cooldown drain and queue-head progress.
/// The seeded Research Center is poked incomplete so supply drops to 0 while
/// the seeded Barracks (3) and Gun Turret (2) hold demand at 5 — the 25%
/// floor. If heat ran after either consumer, they would read the seeded
/// default (full efficiency) and both asserts fail.
#[test]
fn heat_computed_in_the_tick_scales_combat_and_production_in_the_same_tick() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    let research_center = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .unwrap();
    world
        .get_mut::<Building>(research_center)
        .unwrap()
        .construction
        .complete = false;
    let barracks = seed_completed_building(
        &mut world,
        &mut map,
        BARRACKS,
        BuildingKind::Barracks,
        MapFixture::barracks_anchor(TEAM),
    );
    let turret = seed_completed_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::GunTurret,
        MapFixture::depot_anchor(TEAM),
    );
    world.entity_mut(barracks).insert(ProductionQueue {
        jobs: VecDeque::from([ProductionJob {
            kind: ProductionKind::Unit(UnitKind::Worker),
        }]),
        ..ProductionQueue::default()
    });

    step_tick(&mut world, &mut map, SIM_STEP_SECONDS);

    // step_heat recomputed the deficit inside this tick…
    let heat = world.resource::<HeatState>();
    assert_eq!((heat.supply, heat.demand), (0, 5));
    assert_eq!(heat.efficiency(), MIN_SURFACE_EFFICIENCY);
    // …and the same tick's combat and production consumed it.
    let expected_cooldown = (1.0 - SIM_STEP_SECONDS * MIN_SURFACE_EFFICIENCY).max(0.0);
    assert_eq!(
        world.get::<AttackCooldown>(turret),
        Some(&AttackCooldown(expected_cooldown)),
        "the turret cooldown must drain at the recomputed floor efficiency"
    );
    let queue = world.get::<ProductionQueue>(barracks).unwrap();
    assert!(
        (queue.progress_seconds - SIM_STEP_SECONDS * MIN_SURFACE_EFFICIENCY).abs() < 1e-6,
        "the queue head must advance at the recomputed floor efficiency, got {}",
        queue.progress_seconds
    );
}

/// Movement arrival is visible to economy and construction inside the same
/// `step_tick`: both a gatherer and a builder start a fraction of a cell
/// outside their slots, so only the movement step can complete the arrival
/// the later steps must consume.
#[test]
fn movement_arrival_is_visible_to_construction_and_economy_in_the_same_tick() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    // Builder placed through the real command path, then parked one movement
    // leg outside the authored slot the placement routed it to. Placement
    // happens BEFORE the gatherer's order is stamped below: blocking the
    // footprint bumps the map revision, and a stale order would replan the
    // gatherer backwards in its one movement step.
    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Barracks,
            anchor: MapFixture::barracks_anchor(TEAM),
        },
    );
    assert_eq!(placed.reject, None, "barracks placement rejected");

    // Gatherer one movement leg outside an immediate-perimeter slot of the
    // authored ration cache: the arrival must land ON the perimeter, or the
    // same tick's adjacency gate reroutes instead of gathering.
    let ration_cell = MapFixture::starting_resources()
        .into_iter()
        .find(|spawn| spawn.id == RATIONS)
        .expect("the authored ration cache")
        .cell;
    let gather_slot = GridPos::new(ration_cell.x - 1, ration_cell.y);
    let gatherer = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(gatherer).insert((
        SimPosition::new(border_position(&map, gather_slot)),
        GatherProgress::default(),
        WorkerTask::ToSource {
            source: RATIONS,
            slot: gather_slot,
        },
        MoveOrder {
            waypoints: vec![map.cell_center(gather_slot)],
            next: 0,
            goal: gather_slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));

    let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();
    let (barracks_id, build_slot) = match world.get::<WorkerTask>(builder).unwrap() {
        WorkerTask::ToConstruction { building, slot } => (*building, *slot),
        other => panic!("placement must task its builder, got {other:?}"),
    };
    world.entity_mut(builder).remove::<MoveOrder>();
    world.entity_mut(builder).insert((
        SimPosition::new(border_position(&map, build_slot)),
        MoveOrder {
            waypoints: vec![map.cell_center(build_slot)],
            next: 0,
            goal: build_slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));

    step_tick(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        matches!(
            world.get::<WorkerTask>(gatherer),
            Some(WorkerTask::Gathering { source }) if *source == RATIONS
        ),
        "the gather arrival must transition inside this tick's economy step"
    );
    assert!(
        world.get::<GatherProgress>(gatherer).unwrap().0 > 0.0,
        "the same tick's economy step must gather after the arrival"
    );
    assert!(
        matches!(
            world.get::<WorkerTask>(builder),
            Some(WorkerTask::Constructing { building }) if *building == barracks_id
        ),
        "the build arrival must transition inside this tick's construction step"
    );
    assert!(
        world
            .get::<Building>(
                world
                    .resource::<BuildingIndex>()
                    .entity(barracks_id)
                    .unwrap()
            )
            .unwrap()
            .construction
            .progress_seconds
            > 0.0,
        "the same tick's construction step must advance after the arrival"
    );
}

/// Construction completion feeds production in the same tick: the seeded
/// Research Center's population cap of 10 is exactly full, and a Bunkhouse
/// one tick from completion must raise the cap BEFORE the same tick's
/// production spawns the ready queue head. With production before
/// construction the spawn hits `PopulationFull` and no unit appears.
#[test]
fn construction_completion_feeds_production_in_the_same_tick() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    // Fill the cap: four seed workers plus six extras.
    for id in 10..=15 {
        spawn_unit(
            &mut world,
            UnitId(id),
            TEAM,
            Vec2::new(90.5, 20.5),
            UnitKind::Worker,
            unit_spec(UnitKind::Worker).speed,
        );
    }
    assert_eq!(population_used(&world, TEAM), 10);

    // A Worker one tick from completing on the Research Center queue.
    let enqueued = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::EnqueueUnit {
            issuer: TEAM,
            building: RESEARCH_CENTER,
            kind: UnitKind::Worker,
        },
    );
    assert_eq!(enqueued.reject, None, "enqueue rejected");
    let research_center = world
        .resource::<BuildingIndex>()
        .entity(RESEARCH_CENTER)
        .unwrap();
    world
        .get_mut::<ProductionQueue>(research_center)
        .unwrap()
        .progress_seconds = 15.0; // Worker train_seconds.

    // A Bunkhouse one tick from completing, its builder already Constructing
    // on the slot. Placement runs from the builder's start cell (no distance
    // gate — the builder keeps its authored start), and the Bunkhouse takes
    // the authored 2×2 depot slot: proven free ground on a fresh battlefield.
    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Bunkhouse,
            anchor: MapFixture::depot_anchor(TEAM),
        },
    );
    assert_eq!(placed.reject, None, "bunkhouse placement rejected");
    let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();
    let bunkhouse_id = BuildingId(2);
    let bunkhouse = world
        .resource::<BuildingIndex>()
        .entity(bunkhouse_id)
        .unwrap();
    let slot = match world.get::<WorkerTask>(builder).unwrap() {
        WorkerTask::ToConstruction { slot, .. } => *slot,
        other => panic!("placed site must task its builder, got {other:?}"),
    };
    world.entity_mut(builder).remove::<MoveOrder>();
    world.entity_mut(builder).insert((
        SimPosition::new(map.cell_center(slot)),
        WorkerTask::Constructing {
            building: bunkhouse_id,
        },
    ));
    world
        .get_mut::<Building>(bunkhouse)
        .unwrap()
        .construction
        .progress_seconds = 15.0; // Bunkhouse build_seconds.

    step_tick(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world
            .get::<Building>(bunkhouse)
            .unwrap()
            .construction
            .complete,
        "the Bunkhouse must complete inside this tick's construction step"
    );
    assert!(
        world.resource::<UnitIndex>().entity(UnitId(5)).is_some(),
        "the queue head must spawn in the same tick the cap rose"
    );
    let queue = world.get::<ProductionQueue>(research_center).unwrap();
    assert!(queue.jobs.is_empty(), "the ready job must complete");
    assert_eq!(queue.progress_seconds, 0.0);
}

/// A `World` without `HeatState` must not panic and must run both cold-
/// sensitive consumers at the full rate — the neutral `surface_efficiency`
/// fallback. Called directly (no `step_tick`): inside the canonical tick
/// `step_heat` always re-inserts the state, so absence is only observable at
/// the consumer level.
#[test]
fn missing_heat_state_gives_production_and_combat_the_full_rate() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.remove_resource::<HeatState>();
    let barracks = seed_completed_building(
        &mut world,
        &mut map,
        BARRACKS,
        BuildingKind::Barracks,
        MapFixture::barracks_anchor(TEAM),
    );
    let turret = seed_completed_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::GunTurret,
        MapFixture::depot_anchor(TEAM),
    );
    world.entity_mut(barracks).insert(ProductionQueue {
        jobs: VecDeque::from([ProductionJob {
            kind: ProductionKind::Unit(UnitKind::Worker),
        }]),
        ..ProductionQueue::default()
    });

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world.get_resource::<HeatState>().is_none(),
        "the consumer-level scenario must keep HeatState absent"
    );
    let expected_cooldown = (1.0 - SIM_STEP_SECONDS).max(0.0);
    assert_eq!(
        world.get::<AttackCooldown>(turret),
        Some(&AttackCooldown(expected_cooldown)),
        "absent HeatState must leave the turret cooldown at full rate"
    );
    let queue = world.get::<ProductionQueue>(barracks).unwrap();
    assert!(
        (queue.progress_seconds - SIM_STEP_SECONDS).abs() < 1e-6,
        "absent HeatState must leave the queue head at full rate, got {}",
        queue.progress_seconds
    );
}
