use bevy::math::Vec2;
use bevy::prelude::World;

use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{Age, BuildingKind, ResourceKind, UnitKind, resource_amount, unit_spec};
use crate::commands::spawn_unit;
use crate::economy::{
    Carry, Dropoff, GatherProgress, ResourceStockpile, TeamEconomy, WorkerTask,
    spawn_resource_source,
};
use crate::ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};

#[derive(Clone, Copy, Debug)]
pub struct UnitSpawn {
    pub id: UnitId,
    pub team: TeamId,
    pub position: Vec2,
}

/// Authored starting position of one team: Town Center anchor plus the four
/// villager spawn cells. All values are cells on the 128×96 battlefield.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamStart {
    pub team: TeamId,
    pub town_center_anchor: GridPos,
    pub villagers: [GridPos; 4],
}

/// One authored finite resource node with a deterministic `ResourceId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    pub left_spawn: Vec2,
    pub right_spawn: Vec2,
    /// Authored finite resource nodes for the skirmish seed.
    pub resources: Vec<ResourceSpawn>,
}

impl MapFixture {
    /// The single authored team-start table. Anchors and villager cells come
    /// from the HPA-471 design; Town Centers are 4×4 footprints.
    pub fn team_starts() -> [TeamStart; 2] {
        [
            TeamStart {
                team: TeamId(1),
                town_center_anchor: GridPos::new(12, 46),
                villagers: [
                    GridPos::new(11, 45),
                    GridPos::new(11, 50),
                    GridPos::new(16, 45),
                    GridPos::new(16, 50),
                ],
            },
            TeamStart {
                team: TeamId(2),
                town_center_anchor: GridPos::new(112, 46),
                villagers: [
                    GridPos::new(116, 45),
                    GridPos::new(116, 50),
                    GridPos::new(111, 45),
                    GridPos::new(111, 50),
                ],
            },
        ]
    }

    /// Safe resources around each Town Center, mirrored between teams.
    /// The only resource-position table in the codebase; IDs are
    /// deterministic (team 1 safe, then team 2 safe).
    pub fn starting_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(1), ResourceKind::Food, 22, 42),
            spawn(ResourceId(2), ResourceKind::Food, 22, 54),
            spawn(ResourceId(3), ResourceKind::Wood, 20, 45),
            spawn(ResourceId(4), ResourceKind::Wood, 20, 48),
            spawn(ResourceId(5), ResourceKind::Wood, 20, 51),
            spawn(ResourceId(6), ResourceKind::Gold, 25, 48),
            spawn(ResourceId(7), ResourceKind::Food, 105, 42),
            spawn(ResourceId(8), ResourceKind::Food, 105, 54),
            spawn(ResourceId(9), ResourceKind::Wood, 107, 45),
            spawn(ResourceId(10), ResourceKind::Wood, 107, 48),
            spawn(ResourceId(11), ResourceKind::Wood, 107, 51),
            spawn(ResourceId(12), ResourceKind::Gold, 102, 48),
        ]
    }

    /// Southwest and northeast expansion resources; IDs continue after the
    /// safe sources.
    pub fn expansion_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(13), ResourceKind::Gold, 45, 16),
            spawn(ResourceId(14), ResourceKind::Wood, 43, 18),
            spawn(ResourceId(15), ResourceKind::Wood, 47, 18),
            spawn(ResourceId(16), ResourceKind::Gold, 82, 79),
            spawn(ResourceId(17), ResourceKind::Wood, 84, 77),
            spawn(ResourceId(18), ResourceKind::Wood, 80, 77),
        ]
    }

    pub fn battlefield() -> Self {
        let mut map = GridMap::new(128, 96);

        // Mirrored central blockers leave a northern and southern route.
        map.set_blocked_rect(GridPos::new(54, 20), GridPos::new(61, 75));
        map.set_blocked_rect(GridPos::new(66, 20), GridPos::new(73, 75));
        map.set_blocked_rect(GridPos::new(62, 38), GridPos::new(65, 57));

        Self {
            map,
            left_spawn: Vec2::new(14.5, 48.5),
            right_spawn: Vec2::new(113.5, 48.5),
            resources: Self::starting_resources()
                .into_iter()
                .chain(Self::expansion_resources())
                .collect(),
        }
    }

    pub fn units_200(&self) -> Vec<UnitSpawn> {
        let mut units = Vec::with_capacity(200);

        for row in 0..10 {
            for column in 0..10 {
                let offset = (row * 10 + column) as u32;
                units.push(UnitSpawn {
                    id: UnitId(offset + 1),
                    team: TeamId(1),
                    position: Vec2::new(
                        self.left_spawn.x - 4.0 + column as f32,
                        self.left_spawn.y - 4.0 + row as f32,
                    ),
                });
                units.push(UnitSpawn {
                    id: UnitId(offset + 101),
                    team: TeamId(2),
                    position: Vec2::new(
                        self.right_spawn.x + 4.0 - column as f32,
                        self.right_spawn.y - 4.0 + row as f32,
                    ),
                });
            }
        }

        units
    }
}

/// The pure simulation skirmish seed for one normal match: real completed
/// Town Center Buildings (4×4 Footprints + `Dropoff` from birth), four
/// Villagers per team with fresh worker components, both teams' economies
/// (200 Food / 300 Wood / 100 Gold, Age 1), Town Center map occupancy, the
/// authored finite resource sources, and `UnitIndex`/`BuildingIndex`
/// `IdAllocator` counters above the authored maxima. Every starting entity
/// flows through the production constructors — there is no temporary
/// Dropoff-only Town Center.
pub fn seed_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture) {
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    economy.insert_team(
        TeamId(2),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    world.insert_resource(economy);

    let mut unit_counter: u32 = 0;
    for (index, start) in MapFixture::team_starts().into_iter().enumerate() {
        let building_id = BuildingId(index as u32 + 1);
        let footprint = Footprint::new(start.town_center_anchor, 4, 4);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id: building_id,
                    team: start.team,
                    kind: BuildingKind::TownCenter,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                footprint,
                Dropoff { team: start.team },
            ))
            .id();
        let mut buildings = world.get_resource_or_insert_with(BuildingIndex::default);
        buildings.insert(building_id, entity);

        for cell in start.villagers {
            unit_counter += 1;
            let villager = spawn_unit(
                world,
                UnitId(unit_counter),
                start.team,
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
            world.entity_mut(villager).insert((
                Carry::Empty,
                GatherProgress::default(),
                WorkerTask::Idle,
            ));
        }
    }

    // Authored finite sources (deterministic IDs 1..=18) block their cells
    // and register in the resource index.
    for resource in &fixture.resources {
        spawn_resource_source(
            world,
            map,
            resource.id,
            resource.kind,
            resource.cell,
            resource.amount,
        );
    }

    world.insert_resource(IdAllocator::new(unit_counter + 1, 3, 19));
}

fn spawn(id: ResourceId, kind: ResourceKind, x: i32, y: i32) -> ResourceSpawn {
    ResourceSpawn {
        id,
        kind,
        cell: GridPos::new(x, y),
        amount: resource_amount(kind),
    }
}

#[cfg(test)]
mod tests {
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
}
