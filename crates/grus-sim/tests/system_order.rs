//! Canonical fixed-step order: commands → combat → movement → economy →
//! construction → production → route feedback. Movement arrival is visible to
//! economy/construction in the same fixed tick, and production runs last, so
//! the economy step of the tick in which the Age 2 job completes still uses
//! the Age 1 gather rate.

use std::cell::RefCell;

use bevy::math::Vec2;
use bevy::prelude::{Entity, World};
use grus_sim::catalog::unit_spec;
use grus_sim::{
    Age, Building, BuildingId, BuildingIndex, BuildingKind, Carry, CombatOrder, CombatTarget,
    Dropoff, Footprint, GatherProgress, GridMap, GridPos, Health, MapFixture, PlayerCommand,
    ResourceId, ResourceKind, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId, UnitId,
    UnitIndex, UnitKind, VisibilityMap, WorkerTask, apply_player_command, explored_by,
    refresh_visibility, seed_skirmish, spawn_unit, step_combat, step_construction, step_economy,
    step_movement, step_production, visible_to,
};

const TEAM: TeamId = TeamId(1);
/// Authored team-1 Town Center and berry bush.
const TOWN_CENTER: BuildingId = BuildingId(1);
const BERRIES: ResourceId = ResourceId(1);
/// The placed Barracks takes the first runtime building id after the two TCs.
const BARRACKS: BuildingId = BuildingId(3);

fn gather_progress(world: &World, worker: Entity) -> f32 {
    world
        .get::<GatherProgress>(worker)
        .map_or(0.0, |state| state.0)
}

#[test]
fn age_two_completes_after_that_ticks_economy_used_the_age_one_rate() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    // Fund the 300F + 200G Age 2 job on top of the 200F/300W/100G seed.
    if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&TEAM)
    {
        state.stockpile.food = 1000;
        state.stockpile.gold = 500;
    }

    // Gatherer A parks beside the berries at (22, 42); villager 2 stays home
    // as the builder for a Barracks site south-east of the Town Center.
    let gatherer = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world
        .entity_mut(gatherer)
        .insert(SimPosition::new(map.cell_center(GridPos::new(21, 42))));
    let builder = world.resource::<UnitIndex>().entity(UnitId(2)).unwrap();

    // Commands queued ahead of the chain; the helper drains whatever is
    // queued through the public dispatcher before each tick's systems run.
    let queue: RefCell<Vec<PlayerCommand>> = RefCell::new(vec![PlayerCommand::EnqueueAgeUp {
        issuer: TEAM,
        building: TOWN_CENTER,
    }]);
    let apply_queued_test_commands = |world: &mut World, map: &mut GridMap| {
        for command in queue.borrow_mut().drain(..) {
            let outcome = apply_player_command(world, map, command);
            assert_eq!(outcome.reject, None, "queued command rejected: {outcome:?}");
        }
    };

    // Tick arithmetic (0.3 cells per movement tick at villager speed):
    // - Age job enqueued at tick 1 completes at tick 900 (45s / 0.05).
    // - Barracks placed at tick 886: builder walks 4 cells over 14 movement
    //   ticks (887..=900) and arrives at tick 900.
    // - Gather tasked at tick 896: gatherer walks 1 cell over 4 movement
    //   ticks (897..=900) and arrives at tick 900.
    let place_barracks = PlayerCommand::PlaceBuilding {
        issuer: TEAM,
        builder: UnitId(2),
        kind: BuildingKind::Barracks,
        anchor: GridPos::new(15, 52),
    };
    let gather_berries = PlayerCommand::Gather {
        issuer: TEAM,
        workers: vec![UnitId(1)],
        source: BERRIES,
    };

    let mut progress_before_completion_tick = 0.0;
    for tick in 1..=900 {
        match tick {
            886 => queue.borrow_mut().push(place_barracks.clone()),
            896 => queue.borrow_mut().push(gather_berries.clone()),
            _ => {}
        }
        apply_queued_test_commands(&mut world, &mut map);
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        step_construction(&mut world, SIM_STEP_SECONDS);
        step_production(&mut world, &mut map, SIM_STEP_SECONDS);
        if tick == 899 {
            progress_before_completion_tick = gather_progress(&world, gatherer);
        }
    }

    // The completion tick: both arrivals are visible to their consumers in
    // this same tick.
    assert!(
        matches!(
            world.get::<WorkerTask>(gatherer),
            Some(WorkerTask::Gathering { source }) if *source == BERRIES
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

    // Production ran last: the age already flipped, but the economy step of
    // this tick ran BEFORE production and must have used the Age 1 rate.
    assert_eq!(world.resource::<TeamEconomy>().0[&TEAM].age, Age::Age2);
    let progress_after_completion_tick = gather_progress(&world, gatherer);
    let age_one_delta = progress_after_completion_tick - progress_before_completion_tick;
    assert!(
        age_one_delta > 0.0 && age_one_delta < 0.105,
        "the completion tick's economy must gather at the Age 1 rate (2.0/s), got {age_one_delta}"
    );

    // The next economy tick gathers at the Age 2 rate.
    apply_queued_test_commands(&mut world, &mut map);
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
    step_construction(&mut world, SIM_STEP_SECONDS);
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    let age_two_delta = gather_progress(&world, gatherer) - progress_after_completion_tick;
    assert!(
        age_two_delta > 0.105 && (age_two_delta - 0.11).abs() < 1e-4,
        "the next economy tick must gather at 2.2/s, got {age_two_delta}"
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

    // A second team-1 drop-off south-west of the Town Center so the
    // destroyed drop-off's worker has a same-team reroute target. Placed
    // through the public command path and completed instantly; the first
    // runtime building id after the two Town Centers is 3.
    let placed = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Storehouse,
            anchor: GridPos::new(8, 40),
        },
    );
    assert_eq!(placed.reject, None, "storehouse placement rejected");
    let storehouse_id = BuildingId(3);
    let storehouse = world
        .resource::<BuildingIndex>()
        .entity(storehouse_id)
        .unwrap();
    world
        .get_mut::<Building>(storehouse)
        .unwrap()
        .construction
        .complete = true;
    world.entity_mut(storehouse).insert(Dropoff { team: TEAM });

    // Carrying worker parked exactly at its Town Center drop-off slot.
    let worker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.entity_mut(worker).insert((
        SimPosition::new(map.cell_center(GridPos::new(11, 46))),
        Carry::Holding {
            kind: ResourceKind::Wood,
            amount: std::num::NonZeroU32::new(6).unwrap(),
        },
        GatherProgress::default(),
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: TOWN_CENTER,
            slot: GridPos::new(11, 46),
        },
    ));

    // A raider in range of the Town Center with a direct attack order; the
    // killing blow lands in this tick's combat step.
    let raider = spawn_unit(
        &mut world,
        UnitId(100),
        TeamId(2),
        Vec2::new(11.5, 49.5),
        UnitKind::Spearman,
        unit_spec(UnitKind::Spearman).speed,
    );
    world.entity_mut(raider).insert(CombatOrder::Attack {
        target: CombatTarget::Building(TOWN_CENTER),
        last_target_cell: None,
    });
    let town_center = world
        .resource::<BuildingIndex>()
        .entity(TOWN_CENTER)
        .unwrap();
    world.get_mut::<Health>(town_center).unwrap().current = 10;

    // The canonical order under test: combat -> movement -> economy.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    step_movement(&mut world, &map, SIM_STEP_SECONDS);
    step_economy(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(TOWN_CENTER)
            .is_none(),
        "combat destroyed the drop-off inside this tick"
    );
    match world.get::<WorkerTask>(worker) {
        Some(WorkerTask::ToDropoff { dropoff, .. }) => assert_eq!(
            *dropoff, storehouse_id,
            "the worker was retasked to the surviving drop-off"
        ),
        other => panic!("expected a rerouted ToDropoff, got {other:?}"),
    }
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TEAM].stockpile.wood,
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
    let site = Footprint::new(GridPos::new(30, 46), 2, 2); // Storehouse 2×2
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
            kind: BuildingKind::Storehouse,
            anchor: GridPos::new(30, 46),
        },
    );
    assert_eq!(placed.reject, None, "placement on explored ground rejected");
    let storehouse = world
        .resource::<BuildingIndex>()
        .entity(BuildingId(3))
        .unwrap();
    assert!(
        !visible_to(&world, TEAM, site),
        "a newly placed incomplete site grants no vision"
    );

    // Completion is what flips the site into a reveal origin.
    world
        .get_mut::<Building>(storehouse)
        .unwrap()
        .construction
        .complete = true;
    refresh_visibility(&mut world, &map);
    assert!(
        visible_to(&world, TEAM, site),
        "construction completion grants vision"
    );
}
