//! Canonical fixed-step order: commands → movement → economy → construction →
//! production. Movement arrival is visible to economy/construction in the same
//! fixed tick, and production runs last, so the economy step of the tick in
//! which the Age 2 job completes still uses the Age 1 gather rate.

use std::cell::RefCell;

use bevy::prelude::{Entity, World};
use grus_sim::{
    Age, Building, BuildingId, BuildingIndex, BuildingKind, GatherProgress, GridMap, GridPos,
    MapFixture, PlayerCommand, ResourceId, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId,
    UnitId, UnitIndex, WorkerTask, apply_player_command, seed_skirmish, step_combat,
    step_construction, step_economy, step_movement, step_production,
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
        step_combat(&mut world, &map, SIM_STEP_SECONDS);
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
    step_combat(&mut world, &map, SIM_STEP_SECONDS);
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
