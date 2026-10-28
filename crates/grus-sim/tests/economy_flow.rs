//! Cross-module economy flow driven only through the public simulation API:
//! gather → Research Center deposit → Depot delivery → full depletion →
//! final deposit and Idle.

use bevy::prelude::World;
use grus_sim::{
    Building, BuildingId, BuildingIndex, BuildingKind, CARRY_LIMIT, Carry, Dropoff, GridMap,
    GridPos, MapFixture, PlayerCommand, ResourceId, ResourceIndex, SIM_STEP_SECONDS, TeamEconomy,
    TeamId, UnitId, UnitIndex, WorkerTask, apply_player_command, seed_skirmish, step_construction,
    step_economy, step_movement,
};

const TEAM: TeamId = TeamId(1);
/// Authored team-1 salvage ruin (a safe Scrap source on the central city's
/// gather ring).
const SALVAGE: ResourceId = ResourceId(3);
/// Stockpile after placing the Depot and delivering the whole 400-scrap
/// salvage: 300 starting - 75 depot cost + 400 harvested.
const FINAL_SCRAP: u32 = 300 - 75 + 400;

fn scrap(world: &World) -> u32 {
    world.resource::<TeamEconomy>().0[&TEAM].stockpile.scrap
}

/// Drives the fixed movement → economy → construction chain until `done`
/// fires or the tick cap is exhausted.
fn drive(world: &mut World, map: &mut GridMap, cap: usize, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..cap {
        step_movement(world, map, SIM_STEP_SECONDS);
        step_economy(world, map, SIM_STEP_SECONDS);
        step_construction(world, map, SIM_STEP_SECONDS);
        if done(world) {
            return;
        }
    }
    panic!("simulation never reached the expected state");
}

#[test]
fn workers_gather_deposit_and_deplete_through_public_apis() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let worker_entity = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let gather_salvage = |world: &mut World, map: &mut GridMap| {
        let outcome = apply_player_command(
            world,
            map,
            PlayerCommand::Gather {
                issuer: TEAM,
                workers: vec![UnitId(1)],
                source: SALVAGE,
            },
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    };

    // 1. Gather the salvage; the first delivery lands at the starting
    //    Research Center, the only Dropoff in the world so far.
    gather_salvage(&mut world, &mut map);
    let mut first_delivery = None;
    drive(&mut world, &mut map, 4000, |world| {
        if let Some(WorkerTask::ToDropoff { dropoff, .. }) = world.get::<WorkerTask>(worker_entity)
        {
            first_delivery = Some(*dropoff);
        }
        scrap(world) >= 300 + CARRY_LIMIT
    });
    assert_eq!(scrap(&world), 300 + CARRY_LIMIT);
    assert_eq!(
        first_delivery,
        Some(BuildingId(1)),
        "the first delivery must reach the starting Research Center"
    );

    // 2. Wait until the worker is parked at the salvage, then place and
    //    complete a Depot nearer to it than the Research Center — anchored
    //    two cells south-west of the authored source so the footprint stays
    //    free of the parked gatherer.
    drive(&mut world, &mut map, 4000, |world| {
        matches!(
            world.get::<WorkerTask>(worker_entity),
            Some(WorkerTask::Gathering { .. })
        )
    });
    let salvage_cell = MapFixture::starting_resources()
        .into_iter()
        .find(|spawn| spawn.id == SALVAGE)
        .expect("the authored salvage ruin")
        .cell;
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Depot,
            anchor: GridPos::new(salvage_cell.x - 2, salvage_cell.y + 2),
        },
    );
    assert_eq!(outcome.reject, None, "depot placement rejected");
    let depot = BuildingId(2);
    let depot_entity = world.resource::<BuildingIndex>().entity(depot).unwrap();
    drive(&mut world, &mut map, 4000, |world| {
        world
            .get::<Building>(depot_entity)
            .is_some_and(|state| state.construction.complete)
    });
    assert_eq!(
        world.get::<Dropoff>(depot_entity),
        Some(&Dropoff { team: TEAM })
    );

    // 3. Retask the worker onto the salvage; a delivery must reach the nearer,
    //    reachable Depot.
    gather_salvage(&mut world, &mut map);
    let mut saw_depot_delivery = false;
    let mut scrap_when_seen = None;
    for _ in 0..8000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if let Some(WorkerTask::ToDropoff { dropoff, .. }) = world.get::<WorkerTask>(worker_entity)
            && *dropoff == depot
        {
            saw_depot_delivery = true;
            scrap_when_seen.get_or_insert(scrap(&world));
        }
        if let Some(baseline) = scrap_when_seen
            && scrap(&world) >= baseline + CARRY_LIMIT
        {
            break;
        }
    }
    assert!(
        saw_depot_delivery
            && scrap_when_seen.is_some_and(|baseline| scrap(&world) >= baseline + CARRY_LIMIT),
        "a delivery never reached the Depot"
    );

    // 4. Fully deplete the salvage; the worker delivers its final load and idles.
    drive(&mut world, &mut map, 60000, |world| {
        world.resource::<ResourceIndex>().entity(SALVAGE).is_none()
    });
    assert!(
        map.is_walkable(salvage_cell),
        "depleted salvage cell must be unblocked"
    );
    drive(&mut world, &mut map, 4000, |world| {
        world.get::<WorkerTask>(worker_entity) == Some(&WorkerTask::Idle)
    });
    assert_eq!(world.get::<Carry>(worker_entity), Some(&Carry::Empty));
    // Conservation: every harvested unit was deposited exactly once.
    assert_eq!(scrap(&world), FINAL_SCRAP);
}
