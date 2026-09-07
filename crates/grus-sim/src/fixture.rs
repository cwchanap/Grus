use bevy::math::Vec2;

use crate::map::{GridMap, GridPos};

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
}

#[cfg(test)]
mod tests {
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
}
