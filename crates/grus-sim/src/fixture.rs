use bevy::math::Vec2;

use crate::catalog::{ResourceKind, resource_amount};
use crate::ids::{ResourceId, TeamId, UnitId};
use crate::map::{GridMap, GridPos};

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
    use std::collections::HashSet;

    use super::*;
    use crate::map::Footprint;

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
}
