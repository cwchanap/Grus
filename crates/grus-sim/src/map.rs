use std::collections::HashSet;

use bevy::math::Vec2;
use bevy::prelude::Resource;
use pathfinding::prelude::astar;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct GridMap {
    width: i32,
    height: i32,
    blocked: HashSet<GridPos>,
}

impl GridMap {
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0);
        Self {
            width,
            height,
            blocked: HashSet::new(),
        }
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub fn in_bounds(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    pub fn is_walkable(&self, pos: GridPos) -> bool {
        self.in_bounds(pos) && !self.blocked.contains(&pos)
    }

    pub fn set_blocked_rect(&mut self, _min: GridPos, _max_inclusive: GridPos) {
        // RED phase: intentionally left empty so the behavior-first test proves it can fail.
    }

    pub fn world_to_cell(&self, world: Vec2) -> GridPos {
        GridPos::new(world.x.floor() as i32, world.y.floor() as i32)
    }

    pub fn cell_center(&self, cell: GridPos) -> Vec2 {
        Vec2::new(cell.x as f32 + 0.5, cell.y as f32 + 0.5)
    }

    pub fn find_path(&self, start: GridPos, goal: GridPos) -> Option<Vec<GridPos>> {
        if !self.is_walkable(start) || !self.is_walkable(goal) {
            return None;
        }

        astar(
            &start,
            |pos| {
                [
                    GridPos::new(pos.x + 1, pos.y),
                    GridPos::new(pos.x - 1, pos.y),
                    GridPos::new(pos.x, pos.y + 1),
                    GridPos::new(pos.x, pos.y - 1),
                ]
                .into_iter()
                .filter(|candidate| self.is_walkable(*candidate))
                .map(|candidate| (candidate, 1_u32))
                .collect::<Vec<_>>()
            },
            |pos| pos.x.abs_diff(goal.x) + pos.y.abs_diff(goal.y),
            |pos| *pos == goal,
        )
        .map(|(path, _)| path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_rect_prevents_walkability() {
        let mut map = GridMap::new(8, 8);
        map.set_blocked_rect(GridPos::new(2, 2), GridPos::new(4, 4));

        assert!(map.is_walkable(GridPos::new(1, 1)));
        assert!(!map.is_walkable(GridPos::new(2, 2)));
        assert!(!map.is_walkable(GridPos::new(4, 4)));
        assert!(map.is_walkable(GridPos::new(5, 5)));
    }

    #[test]
    fn world_cell_round_trip_uses_cell_centers() {
        let map = GridMap::new(8, 8);
        for cell in [GridPos::new(0, 0), GridPos::new(3, 5), GridPos::new(7, 7)] {
            assert_eq!(map.world_to_cell(map.cell_center(cell)), cell);
        }
    }
}
