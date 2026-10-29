//! An outside gatherer must keep depositing at the Research Center
//! after a Wall ring + Gate closes around it.

use bevy::prelude::World;
use grus_sim::{
    BuildingKind, GridMap, GridPos, MapFixture, PlayerCommand, ResourceId, SIM_STEP_SECONDS,
    TeamEconomy, TeamId, UnitId, UnitIndex, WorkerTask, apply_player_command, seed_skirmish,
    step_construction, step_economy, step_movement,
};

const TEAM: TeamId = TeamId(1);
const SCRAP: ResourceId = ResourceId(3); // (56,52)

fn scrap(world: &World) -> u32 {
    world.resource::<TeamEconomy>().0[&TEAM].stockpile.scrap
}

fn drive(world: &mut World, map: &mut GridMap, ticks: usize) {
    for _ in 0..ticks {
        step_movement(world, map, SIM_STEP_SECONDS);
        step_economy(world, map, SIM_STEP_SECONDS);
        step_construction(world, map, SIM_STEP_SECONDS);
    }
}

#[test]
fn gatherer_deposits_through_gate_after_ring_closes() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let gatherer_entity = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();

    // Send worker 1 to the southern salvage before any wall exists.
    let outcome = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Gather {
            issuer: TEAM,
            workers: vec![UnitId(1)],
            source: SCRAP,
        },
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
    drive(&mut world, &mut map, 400); // ~20 sim-s: reaches the source

    // Ring geometry mirrors the smoke: RC anchor from the team-start table,
    // ring at +/-2..+5 around the 4x4 footprint.
    let rc_anchor = MapFixture::team_starts()[0].research_center_anchor;
    let ring_min = GridPos::new(rc_anchor.x - 2, rc_anchor.y - 2);
    let ring_max = GridPos::new(rc_anchor.x + 5, rc_anchor.y + 5);
    let gate = GridPos::new(rc_anchor.x + 2, ring_min.y);

    let mut ring = vec![gate];
    for x in ring_min.x..=ring_max.x {
        for cell in [GridPos::new(x, ring_min.y), GridPos::new(x, ring_max.y)] {
            if !ring.contains(&cell) {
                ring.push(cell);
            }
        }
    }
    for y in (ring_min.y + 1)..ring_max.y {
        ring.push(GridPos::new(ring_min.x, y));
        ring.push(GridPos::new(ring_max.x, y));
    }
    assert_eq!(ring.len(), 28);

    // Place the Gate first, then walls as fast as the sim accepts them,
    // driving a few ticks between placements like the real smoke.
    for (i, cell) in ring.iter().enumerate() {
        let kind = if *cell == gate {
            BuildingKind::Gate
        } else {
            BuildingKind::Wall
        };
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            PlayerCommand::PlaceBuilding {
                issuer: TEAM,
                builder: UnitId(4),
                kind,
                anchor: *cell,
            },
        );
        assert_eq!(outcome.reject, None, "placement {i} at {cell:?} rejected");
        drive(&mut world, &mut map, 40); // 2 sim-s between placements
    }

    // The ring is now fully placed (every cell blocked): deposits after this
    // point must route through the Gate. Let construction finish and give the
    // gatherer room to keep hauling (builder resumes sites automatically).
    let at_closure = scrap(&world);
    drive(&mut world, &mut map, 20000);
    let deposited_after_closure = scrap(&world) - at_closure;
    assert!(
        deposited_after_closure >= 100,
        "gatherer stalled: only {deposited_after_closure} scrap deposited after the ring closed \
         (gatherer={:?})",
        world.get::<WorkerTask>(gatherer_entity)
    );
}
