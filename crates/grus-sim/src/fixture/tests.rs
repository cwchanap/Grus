use super::*;
use bevy::prelude::Entity;
use std::collections::HashSet;

use crate::buildings::Building;
use crate::catalog::{Age, resource_amount};
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
    // 20/30/30/20 per team — 200 units total.
    for team in [TeamId(1), TeamId(2)] {
        assert_eq!(units_of_kind(&world, team, UnitKind::Villager), 20);
        assert_eq!(units_of_kind(&world, team, UnitKind::Spearman), 30);
        assert_eq!(units_of_kind(&world, team, UnitKind::Archer), 30);
        assert_eq!(units_of_kind(&world, team, UnitKind::Cavalry), 20);
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

#[test]
fn mirrored_seeding_swaps_the_authored_team_starts() {
    let fixture = MapFixture::battlefield();
    let [left, right] = MapFixture::team_starts();

    for (mirrored, team_one_start, team_two_start) in [
        // Default: Team 1 left, Team 2 right.
        (false, &left, &right),
        // Mirrored: Team 1 right, Team 2 left.
        (true, &right, &left),
    ] {
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish_mirrored(&mut world, &mut map, &fixture, mirrored);

        for (team, start) in [(TeamId(1), team_one_start), (TeamId(2), team_two_start)] {
            // The team's Town Center stands on the expected authored anchor.
            let tc_entity = world
                .resource::<BuildingIndex>()
                .iter()
                .find_map(|(_, entity)| {
                    world
                        .get::<Building>(*entity)
                        .is_some_and(|building| {
                            building.team == team && building.kind == BuildingKind::TownCenter
                        })
                        .then_some(*entity)
                })
                .expect("a seeded Town Center per team");
            assert_eq!(
                world.get::<Footprint>(tc_entity).unwrap().anchor,
                start.town_center_anchor,
                "mirrored={mirrored}: team {team:?} town center on the wrong start"
            );

            // And its four villagers stand exactly on that start's cells.
            let villager_cells: HashSet<GridPos> = world
                .resource::<UnitIndex>()
                .iter()
                .filter_map(|(_, entity)| {
                    let unit = world.get::<Unit>(*entity)?;
                    (unit.team == team && unit.kind == UnitKind::Villager).then(|| {
                        map.world_to_cell(world.get::<SimPosition>(*entity).unwrap().current)
                    })
                })
                .collect();
            let expected: HashSet<GridPos> = start.villagers.into_iter().collect();
            assert_eq!(
                villager_cells, expected,
                "mirrored={mirrored}: team {team:?} villagers on the wrong start"
            );
        }
    }
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
fn ai_map_plan_slots_are_buildable_mirrored_and_route_walkable() {
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
        let plan = MapFixture::team_plan(team);
        // The plan's Town Center slot is not a build to place — it restates
        // the authored start's already-seeded anchor.
        let start = starts
            .iter()
            .find(|start| start.team == team)
            .expect("an authored start for every planned team");
        assert_eq!(
            plan.town_center_anchor, start.town_center_anchor,
            "team {team:?}: the plan's Town Center slot restates the authored start"
        );
        let check_anchor = |name: &str, anchor: GridPos, size: u8| {
            for cell in Footprint::new(anchor, size, size).cells() {
                assert!(
                    map.in_bounds(cell) && map.is_walkable(cell) && !occupied.contains(&cell),
                    "team {team:?} {name} cell {cell:?} must be in-bounds, walkable and clear"
                );
            }
        };
        for slot in &plan.house_slots {
            check_anchor("house", *slot, 2);
        }
        for slot in &plan.farm_slots {
            check_anchor("farm", *slot, 2);
        }
        for slot in plan
            .safe_storehouse_slots
            .iter()
            .chain(&plan.expansion_storehouse_slots)
        {
            check_anchor("storehouse", *slot, 2);
        }
        check_anchor("barracks", plan.barracks_anchor, 3);
        check_anchor("archery range", plan.archery_range_anchor, 3);
        check_anchor("stable", plan.stable_anchor, 3);

        // Authored slots never overlap each other.
        let mut all_slot_cells = Vec::new();
        for (anchor, size) in plan
            .house_slots
            .iter()
            .chain(&plan.farm_slots)
            .chain(&plan.safe_storehouse_slots)
            .chain(&plan.expansion_storehouse_slots)
            .map(|slot| (*slot, 2_u8))
            .chain([
                (plan.barracks_anchor, 3_u8),
                (plan.archery_range_anchor, 3),
                (plan.stable_anchor, 3),
            ])
        {
            all_slot_cells.extend(Footprint::new(anchor, size, size).cells());
        }
        let unique: HashSet<GridPos> = all_slot_cells.iter().copied().collect();
        assert_eq!(
            unique.len(),
            all_slot_cells.len(),
            "team {team:?} authored slots overlap"
        );

        // The scout route is non-empty and walkable end to end.
        assert!(!plan.scout_route.is_empty());
        for cell in &plan.scout_route {
            assert!(
                map.is_walkable(*cell),
                "team {team:?} route cell {cell:?} blocked"
            );
        }
    }

    // Team 2 is the exact mirror of team 1.
    let one = MapFixture::team_plan(TeamId(1));
    let two = MapFixture::team_plan(TeamId(2));
    assert_eq!(
        two.town_center_anchor,
        mirror_anchor(one.town_center_anchor, 4)
    );
    assert_eq!(two.house_slots, mirrored(&one.house_slots, 2));
    assert_eq!(two.farm_slots, mirrored(&one.farm_slots, 2));
    assert_eq!(
        two.safe_storehouse_slots,
        mirrored(&one.safe_storehouse_slots, 2)
    );
    assert_eq!(
        two.expansion_storehouse_slots,
        one.expansion_storehouse_slots
            .iter()
            .map(|slot| point_reflect_anchor(*slot, BuildingKind::Storehouse))
            .collect::<Vec<_>>()
    );
    // Footprint-aware reflection keeps the mirrored Storehouse off the
    // team-2 expansion trees' gathering cells: the raw point reflection of
    // the anchor (82,75) covered tree 17's (83,76) approach cell.
    let battlefield = MapFixture::battlefield();
    let southeast_trees: HashSet<GridPos> = battlefield
        .resources
        .iter()
        .filter(|spawn| spawn.id.0 >= 16)
        .flat_map(|spawn| Footprint::new(spawn.cell, 1, 1).perimeter_cells())
        .collect();
    for slot in &two.expansion_storehouse_slots {
        for cell in Footprint::new(*slot, 2, 2).cells() {
            assert!(
                !southeast_trees.contains(&cell),
                "mirrored expansion Storehouse covers a team-2 gathering cell {cell:?}"
            );
        }
    }
    assert_eq!(two.barracks_anchor, mirror_anchor(one.barracks_anchor, 3));
    assert_eq!(
        two.archery_range_anchor,
        mirror_anchor(one.archery_range_anchor, 3)
    );
    assert_eq!(two.stable_anchor, mirror_anchor(one.stable_anchor, 3));
    assert_eq!(
        two.scout_route,
        one.scout_route
            .iter()
            .map(|cell| point_reflect(*cell))
            .collect::<Vec<_>>()
    );
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
