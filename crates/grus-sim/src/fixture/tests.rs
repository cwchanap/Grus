use super::*;
use bevy::prelude::Entity;
use std::collections::HashSet;

use crate::buildings::Building;
use crate::catalog::resource_amount;
use crate::combat::CombatOrder;
use crate::commands::UnitIndex;
use crate::economy::{ResourceIndex, ResourceSource, TeamEconomy};
use crate::ids::IdAllocator;
use crate::map::Footprint;
use crate::movement::{SimPosition, Unit};

#[test]
fn retained_battlefield_has_multiple_routes_between_base_zones() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map;
    let start = map.world_to_cell(fixture.left_spawn);
    let goal = map.world_to_cell(fixture.right_spawn);
    let direct_manhattan = start.x.abs_diff(goal.x) + start.y.abs_diff(goal.y);
    let route = map.find_path(start, goal).expect("base zones must connect");

    assert!(route.len() as u32 > direct_manhattan + 1);

    // Blocking an interior route cell must leave an alternate route: the
    // base zones stay connected through more than one corridor.
    let interior = route[route.len() / 2];
    assert!(interior != start && interior != goal);
    assert!(map.set_blocked(interior, true));
    let alternate = map
        .find_path(start, goal)
        .expect("an alternate route must exist");
    assert!(!alternate.contains(&interior));
}

/// Team-kind census over the live `UnitIndex` — counts only.
fn units_of_kind(world: &World, team: TeamId, kind: UnitKind) -> usize {
    team_kind_ids(world, team, kind).len()
}

#[test]
fn benchmark_workload_extends_the_skirmish_to_mixed_200() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();

    seed_skirmish(&mut world, &mut map, &fixture);
    extend_with_benchmark_workload(&mut world, &mut map);

    // The mixed roster: the four authored villagers plus the workload make
    // 20/40/40 per team — 200 units total.
    for team in [TeamId(1), TeamId(2)] {
        assert_eq!(units_of_kind(&world, team, UnitKind::Villager), 20);
        assert_eq!(units_of_kind(&world, team, UnitKind::Spearman), 40);
        assert_eq!(units_of_kind(&world, team, UnitKind::Archer), 40);
        assert!(
            world.resource::<TeamEconomy>().0.contains_key(&team),
            "team {team:?} economy present"
        );
    }
    assert_eq!(world.resource::<UnitIndex>().iter().count(), 200);

    // IDs flowed only through the allocator: 8 authored + 192 allocated.
    assert_eq!(world.resource::<IdAllocator>().next_unit, 201);

    // The two starting Town Centers still stand completed, one per team.
    let town_centers = world
        .resource::<BuildingIndex>()
        .iter()
        .filter(|(_, entity)| {
            world.get::<Building>(**entity).is_some_and(|building| {
                building.kind == BuildingKind::TownCenter && building.construction.complete
            })
        })
        .count();
    assert_eq!(town_centers, 2);

    // The authored standalone resources are untouched.
    assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);

    // Every added unit stands in bounds on a walkable (non-authored-blocked)
    // cell — staging never parks a unit inside terrain, a Town Center or a
    // resource.
    for (_, entity) in world.resource::<UnitIndex>().iter() {
        let position = world.get::<SimPosition>(*entity).unwrap().current;
        let cell = map.world_to_cell(position);
        assert!(
            map.in_bounds(cell) && map.is_walkable(cell),
            "unit at {position:?} sits on invalid cell {cell:?}"
        );
    }

    // The initial Gather/AttackMove commands were applied through the
    // ordinary dispatcher and accepted: the first 12 sorted villagers per
    // side are tasked two-per-source to the authored safe sources, and both
    // armies hold one AttackMove order toward the shared midfield corridor.
    for team in [TeamId(1), TeamId(2)] {
        let tasked: HashSet<u32> = world
            .resource::<UnitIndex>()
            .iter()
            .filter_map(|(id, entity)| {
                let unit = world.get::<Unit>(*entity)?;
                (unit.team == team).then_some(())?;
                match world.get::<WorkerTask>(*entity)? {
                    WorkerTask::ToSource { .. } | WorkerTask::Gathering { .. } => Some(id.0),
                    _ => None,
                }
            })
            .collect();
        assert_eq!(
            tasked.len(),
            12,
            "team {team:?}: twelve villagers tasked by the setup gathers"
        );
        let marching = world
            .resource::<UnitIndex>()
            .iter()
            .filter(|(_, entity)| {
                world
                    .get::<Unit>(**entity)
                    .is_some_and(|unit| unit.team == team && unit.kind != UnitKind::Villager)
                    && matches!(
                        world.get::<CombatOrder>(**entity),
                        Some(CombatOrder::AttackMove { .. })
                    )
            })
            .count();
        assert_eq!(
            marching, 80,
            "team {team:?}: every combatant marches on the midfield corridor"
        );
    }
}

/// The two authored anchor accessors are the one shared base-coordinate
/// table left for session/system-order choreography: each team's slot must
/// sit in-bounds, walkable and clear of both starts and every resource, and
/// team 2 must be the exact horizontal mirror of team 1.
#[test]
fn authored_anchor_accessors_mirror_between_team_halves() {
    let fixture = MapFixture::battlefield();
    let map = &fixture.map;
    let starts = MapFixture::team_starts();
    let occupied: HashSet<GridPos> = starts
        .iter()
        .flat_map(|start| Footprint::new(start.town_center_anchor, 4, 4).cells())
        .chain(starts.iter().flat_map(|start| start.villagers))
        .chain(fixture.resources.iter().map(|spawn| spawn.cell))
        .collect();

    for team in [TeamId(1), TeamId(2)] {
        for (name, anchor, width) in [
            (
                "barracks",
                MapFixture::barracks_anchor(team),
                building_spec(BuildingKind::Barracks).width,
            ),
            (
                "storehouse",
                MapFixture::safe_storehouse_anchor(team),
                building_spec(BuildingKind::Storehouse).width,
            ),
        ] {
            for cell in Footprint::new(anchor, width, width).cells() {
                assert!(
                    map.in_bounds(cell) && map.is_walkable(cell) && !occupied.contains(&cell),
                    "team {team:?} {name} cell {cell:?} must be in-bounds, walkable and clear"
                );
            }
        }
    }

    assert_eq!(
        MapFixture::barracks_anchor(TeamId(2)),
        mirror_anchor(
            MapFixture::barracks_anchor(TeamId(1)),
            building_spec(BuildingKind::Barracks).width
        )
    );
    assert_eq!(
        MapFixture::safe_storehouse_anchor(TeamId(2)),
        mirror_anchor(
            MapFixture::safe_storehouse_anchor(TeamId(1)),
            building_spec(BuildingKind::Storehouse).width
        )
    );
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

    // Both teams start solvent.
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
    }

    // Two completed Town Center Buildings, each owning a 4×4 Footprint,
    // a Dropoff marker, full catalogue Health, and blocked map cells.
    let mut town_centers = world.query::<(&Building, &Footprint, &Dropoff, &Health)>();
    let town_centers: Vec<_> = town_centers.iter(&world).collect();
    assert_eq!(town_centers.len(), 2);
    for (building, footprint, dropoff, health) in &town_centers {
        assert_eq!(building.kind, BuildingKind::TownCenter);
        assert!(building.construction.complete);
        assert_eq!(building.construction.active_builder, None);
        assert_eq!((footprint.width, footprint.height), (4, 4));
        assert_eq!(dropoff.team, building.team);
        let max_health = building_spec(building.kind).max_health;
        assert_eq!(
            **health,
            Health {
                current: max_health,
                max: max_health
            }
        );
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
        assert_eq!(source.remaining, resource_amount(source.kind));
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
