//! Cross-module economy flow driven only through the public simulation API:
//! gather → Town Center deposit → Storehouse delivery → full depletion →
//! final deposit and Idle.

use bevy::prelude::World;
use grus_sim::{
    Building, BuildingId, BuildingIndex, BuildingKind, CARRY_LIMIT, Carry, Dropoff, GridMap,
    GridPos, MapFixture, PlayerCommand, ResourceId, ResourceIndex, SIM_STEP_SECONDS, TeamEconomy,
    TeamId, UnitId, UnitIndex, WorkerTask, apply_player_command, seed_skirmish, step_construction,
    step_economy, step_movement,
};

const TEAM: TeamId = TeamId(1);
/// Authored team-1 tree at (20, 45).
const TREE: ResourceId = ResourceId(3);
/// Stockpile after placing the Storehouse and delivering the whole 400-wood
/// tree: 300 starting - 75 storehouse cost + 400 harvested.
const FINAL_WOOD: u32 = 300 - 75 + 400;

fn wood(world: &World) -> u32 {
    world.resource::<TeamEconomy>().0[&TEAM].stockpile.wood
}

/// Drives the fixed movement → economy → construction chain until `done`
/// fires or the tick cap is exhausted.
fn drive(world: &mut World, map: &mut GridMap, cap: usize, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..cap {
        step_movement(world, map, SIM_STEP_SECONDS);
        step_economy(world, map, SIM_STEP_SECONDS);
        step_construction(world, SIM_STEP_SECONDS);
        if done(world) {
            return;
        }
    }
    panic!("simulation never reached the expected state");
}

#[test]
fn villagers_gather_deposit_and_deplete_through_public_apis() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let worker_entity = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    let gather_tree = |world: &mut World, map: &mut GridMap| {
        let outcome = apply_player_command(
            world,
            map,
            PlayerCommand::Gather {
                issuer: TEAM,
                workers: vec![UnitId(1)],
                source: TREE,
            },
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    };

    // 1. Gather the tree; the first delivery lands at the starting Town
    //    Center, the only Dropoff in the world so far.
    gather_tree(&mut world, &mut map);
    let mut first_delivery = None;
    drive(&mut world, &mut map, 4000, |world| {
        if let Some(WorkerTask::ToDropoff { dropoff, .. }) = world.get::<WorkerTask>(worker_entity)
        {
            first_delivery = Some(*dropoff);
        }
        wood(world) >= 300 + CARRY_LIMIT
    });
    assert_eq!(wood(&world), 300 + CARRY_LIMIT);
    assert_eq!(
        first_delivery,
        Some(BuildingId(1)),
        "the first delivery must reach the starting Town Center"
    );

    // 2. Wait until the villager is parked at the tree, then place and
    //    complete a Storehouse near it (footprint guaranteed free of units).
    drive(&mut world, &mut map, 4000, |world| {
        matches!(
            world.get::<WorkerTask>(worker_entity),
            Some(WorkerTask::Gathering { .. })
        )
    });
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::PlaceBuilding {
            issuer: TEAM,
            builder: UnitId(2),
            kind: BuildingKind::Storehouse,
            anchor: GridPos::new(17, 43),
        },
    );
    assert_eq!(outcome.reject, None, "storehouse placement rejected");
    let storehouse = BuildingId(3);
    let storehouse_entity = world
        .resource::<BuildingIndex>()
        .entity(storehouse)
        .unwrap();
    drive(&mut world, &mut map, 4000, |world| {
        world
            .get::<Building>(storehouse_entity)
            .is_some_and(|state| state.construction.complete)
    });
    assert_eq!(
        world.get::<Dropoff>(storehouse_entity),
        Some(&Dropoff { team: TEAM })
    );

    // 3. Retask the villager onto the tree; a delivery must reach the nearer,
    //    reachable Storehouse.
    gather_tree(&mut world, &mut map);
    let mut saw_storehouse_delivery = false;
    let mut wood_when_seen = None;
    for _ in 0..8000 {
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
        if let Some(WorkerTask::ToDropoff { dropoff, .. }) = world.get::<WorkerTask>(worker_entity)
            && *dropoff == storehouse
        {
            saw_storehouse_delivery = true;
            wood_when_seen.get_or_insert(wood(&world));
        }
        if let Some(baseline) = wood_when_seen
            && wood(&world) >= baseline + CARRY_LIMIT
        {
            break;
        }
    }
    assert!(
        saw_storehouse_delivery
            && wood_when_seen.is_some_and(|baseline| wood(&world) >= baseline + CARRY_LIMIT),
        "a delivery never reached the Storehouse"
    );

    // 4. Fully deplete the tree; the worker delivers its final load and idles.
    drive(&mut world, &mut map, 60000, |world| {
        world.resource::<ResourceIndex>().entity(TREE).is_none()
    });
    assert!(
        map.is_walkable(GridPos::new(20, 45)),
        "depleted tree cell must be unblocked"
    );
    drive(&mut world, &mut map, 4000, |world| {
        world.get::<WorkerTask>(worker_entity) == Some(&WorkerTask::Idle)
    });
    assert_eq!(world.get::<Carry>(worker_entity), Some(&Carry::Empty));
    // Conservation: every harvested unit was deposited exactly once.
    assert_eq!(wood(&world), FINAL_WOOD);
}
