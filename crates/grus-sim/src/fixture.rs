use bevy::math::Vec2;

use crate::ids::{TeamId, UnitId};
use crate::map::{GridMap, GridPos};

#[derive(Clone, Copy, Debug)]
pub struct UnitSpawn {
    pub id: UnitId,
    pub team: TeamId,
    pub position: Vec2,
}

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    pub left_spawn: Vec2,
    pub right_spawn: Vec2,
    pub starting_resources: Vec<Vec2>,
    pub expansion_resources: Vec<Vec2>,
}

impl MapFixture {
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
            starting_resources: vec![
                Vec2::new(22.5, 42.5),
                Vec2::new(22.5, 54.5),
                Vec2::new(105.5, 42.5),
                Vec2::new(105.5, 54.5),
            ],
            expansion_resources: vec![Vec2::new(45.5, 16.5), Vec2::new(82.5, 79.5)],
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

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
        assert_eq!(units.iter().filter(|unit| unit.team == TeamId(1)).count(), 100);
        assert_eq!(units.iter().filter(|unit| unit.team == TeamId(2)).count(), 100);
        assert!(units.iter().all(|unit| fixture
            .map
            .is_walkable(fixture.map.world_to_cell(unit.position))));
    }
}
