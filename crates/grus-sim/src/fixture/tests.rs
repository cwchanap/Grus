use super::*;
use bevy::prelude::Entity;
use std::collections::HashSet;

use crate::buildings::Building;
use crate::catalog::{Age, resource_amount};
use crate::commands::UnitIndex;
use crate::economy::{ResourceIndex, ResourceSource, TeamEconomy};
use crate::ids::IdAllocator;
use crate::map::Footprint;
use crate::movement::{SimPosition, Unit};

#[test]
fn retained_battlefield_has_multiple_routes_between_base_zones() {
    let fixture = MapFixture::battlefield();
    let map = &fixture.map;
    let start = map.world_to_cell(fixture.left_spawn);
    let goal = map.world_to_cell(fixture.right_spawn);
    let direct_manhattan = start.x.abs_diff(goal.x) + start.y.abs_diff(goal.y);
    let route = map.find_path(start, goal).expect("base zones must connect");

    assert!(route.len() as u32 > direct_manhattan + 1);
}

#[test]
fn two_hundred_unit_fixture_has_stable_unique_walkable_spawns() {
    let fixture = MapFixture::battlefield();
    let units = fixture.units_200();
    let ids = units.iter().map(|unit| unit.id).collect::<HashSet<_>>();

    assert_eq!(units.len(), 200);
    assert_eq!(ids.len(), 200);
    assert_eq!(
        units.iter().filter(|unit| unit.team == TeamId(1)).count(),
        100
    );
    assert_eq!(
        units.iter().filter(|unit| unit.team == TeamId(2)).count(),
        100
    );
    assert!(units.iter().all(|unit| {
        fixture
            .map
            .is_walkable(fixture.map.world_to_cell(unit.position))
    }));
}

#[test]
fn authored_team_and_resource_cells_are_in_bounds() {
    let map = MapFixture::battlefield().map;
    let resources = MapFixture::starting_resources()
        .into_iter()
        .chain(MapFixture::expansion_resources());

    for start in MapFixture::team_starts() {
        for cell in Footprint::new(start.town_center_anchor, 4, 4).cells() {
            assert!(
                map.in_bounds(cell),
                "town center cell {cell:?} out of bounds"
            );
        }
        for cell in start.villagers {
            assert!(map.in_bounds(cell), "villager cell {cell:?} out of bounds");
        }
    }
    for resource in resources {
        assert!(
            map.in_bounds(resource.cell),
            "resource {:?} out of bounds",
            resource.cell
        );
    }
}

#[test]
fn resources_never_overlap_town_center_footprints() {
    let town_center_cells: HashSet<GridPos> = MapFixture::team_starts()
        .iter()
        .flat_map(|start| Footprint::new(start.town_center_anchor, 4, 4).cells())
        .collect();
    let resources = MapFixture::starting_resources()
        .into_iter()
        .chain(MapFixture::expansion_resources());

    for resource in resources {
        assert!(
            !town_center_cells.contains(&resource.cell),
            "resource {:?} overlaps a town center",
            resource.cell
        );
    }
}

#[test]
fn safe_source_counts_are_mirrored_between_team_halves() {
    let safe = MapFixture::starting_resources();
    let left = safe.iter().filter(|spawn| spawn.cell.x < 64);
    let right = safe.iter().filter(|spawn| spawn.cell.x >= 64);

    for kind in [ResourceKind::Food, ResourceKind::Wood, ResourceKind::Gold] {
        assert_eq!(
            left.clone().filter(|spawn| spawn.kind == kind).count(),
            right.clone().filter(|spawn| spawn.kind == kind).count(),
            "unmirrored count for {kind:?}"
        );
    }
}

#[test]
fn villager_starts_stay_walkable_after_town_center_blocking() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map;
    for start in MapFixture::team_starts() {
        for cell in Footprint::new(start.town_center_anchor, 4, 4).cells() {
            map.set_blocked(cell, true);
        }
    }

    for start in MapFixture::team_starts() {
        for cell in start.villagers {
            assert!(
                map.is_walkable(cell),
                "villager cell {cell:?} blocked by its town center"
            );
        }
    }
}

#[test]
fn skirmish_seed_creates_real_town_centers_villagers_and_economy() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();

    seed_skirmish(&mut world, &mut map, &fixture);

    // Both teams start solvent at Age 1.
    for team in [TeamId(1), TeamId(2)] {
        let state = &world.resource::<TeamEconomy>().0[&team];
        assert_eq!(
            state.stockpile,
            ResourceStockpile {
                food: 200,
                wood: 300,
                gold: 100
            }
        );
        assert_eq!(state.age, Age::Age1);
    }

    // Two completed Town Center Buildings, each owning a 4×4 Footprint,
    // a Dropoff marker, and blocked map cells.
    let mut town_centers = world.query::<(&Building, &Footprint, &Dropoff)>();
    let town_centers: Vec<_> = town_centers.iter(&world).collect();
    assert_eq!(town_centers.len(), 2);
    for (building, footprint, dropoff) in &town_centers {
        assert_eq!(building.kind, BuildingKind::TownCenter);
        assert!(building.construction.complete);
        assert_eq!(building.construction.active_builder, None);
        assert_eq!((footprint.width, footprint.height), (4, 4));
        assert_eq!(dropoff.team, building.team);
        for cell in footprint.cells() {
            assert!(!map.is_walkable(cell), "TC cell {cell:?} unblocked");
        }
    }
    assert_eq!(
        town_centers
            .iter()
            .map(|(b, ..)| b.team)
            .collect::<HashSet<_>>()
            .len(),
        2,
        "one Town Center per team"
    );

    // Every Dropoff lives on a real Building entity — no temporary
    // Dropoff-only Town Center exists.
    let mut dropoffs = world.query::<(Entity, &Dropoff)>();
    for (entity, _) in dropoffs.iter(&world) {
        assert!(world.get::<Building>(entity).is_some());
    }

    // Four villagers per team with fresh worker components, standing on
    // walkable authored cells.
    let mut villagers =
        world.query::<(&Unit, &SimPosition, &Carry, &GatherProgress, &WorkerTask)>();
    let villagers: Vec<_> = villagers
        .iter(&world)
        .filter(|(unit, ..)| unit.kind == UnitKind::Villager)
        .collect();
    assert_eq!(villagers.len(), 8);
    for team in [TeamId(1), TeamId(2)] {
        assert_eq!(
            villagers
                .iter()
                .filter(|(unit, ..)| unit.team == team)
                .count(),
            4
        );
    }
    assert!(
        villagers
            .iter()
            .all(|(_, position, carry, progress, task)| map
                .is_walkable(map.world_to_cell(position.current))
                && **carry == Carry::Empty
                && progress.0 == 0.0
                && **task == WorkerTask::Idle)
    );

    // Eighteen authored finite sources: 1×1 blocked footprints, full
    // authored amounts, registered in the index.
    let mut sources = world.query::<(&ResourceSource, &Footprint)>();
    let sources: Vec<_> = sources.iter(&world).collect();
    assert_eq!(sources.len(), 18);
    for (source, footprint) in &sources {
        assert_eq!((footprint.width, footprint.height), (1, 1));
        assert_eq!(source.remaining, Some(resource_amount(source.kind)));
        assert_eq!(source.assigned_worker, None);
        assert!(
            !map.is_walkable(footprint.anchor),
            "source cell {:?} unblocked",
            footprint.anchor
        );
    }
    assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);

    // Indexes and allocator counters sit above the authored maxima
    // (8 units, 2 buildings, resource IDs 1..=18).
    assert_eq!(world.resource::<UnitIndex>().iter().count(), 8);
    assert_eq!(world.resource::<BuildingIndex>().iter().count(), 2);
    let allocator = world.resource::<IdAllocator>();
    assert_eq!(
        (
            allocator.next_unit,
            allocator.next_building,
            allocator.next_resource
        ),
        (9, 3, 19)
    );
}
