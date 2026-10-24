//! Canonical fixed-step order: commands → combat → movement → economy →
//! construction → production → visibility → route feedback. Movement
//! arrival is visible to economy/construction in the same fixed tick, and
//! production runs last, so a unit completing in tick N does not move until
//! tick N+1 even when its producer owns a rally point.

use std::cell::RefCell;

use bevy::prelude::World;
use grus_sim::catalog::unit_spec;
use grus_sim::{
    Building, BuildingId, BuildingIndex, BuildingKind, Carry, CombatOrder, CombatTarget, Dropoff,
    Footprint, GatherProgress, GridMap, GridPos, Health, MapFixture, MoveOrder, PlayerCommand,
    ResourceId, ResourceKind, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId, UnitId,
    UnitIndex, UnitKind, VisibilityMap, WorkerTask, apply_player_command, explored_by,
    refresh_visibility, seed_skirmish, spawn_unit, step_combat, step_construction, step_economy,
    step_movement, step_production, visible_to,
};

const TEAM: TeamId = TeamId(1);
/// Authored team-1 Research Center and ration cache.
const RESEARCH_CENTER: BuildingId = BuildingId(1);
const RATIONS: ResourceId = ResourceId(1);
/// The placed Barracks takes the first runtime building id after the two
/// seeded Research Centers.
const BARRACKS: BuildingId = BuildingId(3);

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
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        step_construction(&mut world, SIM_STEP_SECONDS);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
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
        .entity(UnitId(9))
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

    // One more tick: the same unit has now left its spawn cell, proving the
    // movement step consumed the rally order only from the next tick on.
    apply_queued_test_commands(&mut world, &mut map);
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    step_construction(&mut world, SIM_STEP_SECONDS);
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);
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
    // completed instantly; the first runtime building id after the two
    // seeded Research Centers is 3.
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
    let depot_id = BuildingId(3);
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

    // The canonical order under test: combat -> movement -> economy.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

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
        .entity(BuildingId(3))
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
        step_construction(&mut world, SIM_STEP_SECONDS);
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
