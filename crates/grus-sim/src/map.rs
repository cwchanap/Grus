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

#[derive(Debug, Resource)]
pub struct GridMap {
    width: i32,
    height: i32,
    blocked: HashSet<GridPos>,
    revision: u64,
    /// Test-only count of `find_path` invocations, used by performance
    /// regressions to prove A* is not re-run every simulation step. Absent in
    /// non-test builds.
    #[cfg(test)]
    path_calls: std::sync::atomic::AtomicU64,
}

impl Clone for GridMap {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            blocked: self.blocked.clone(),
            revision: self.revision,
            #[cfg(test)]
            path_calls: std::sync::atomic::AtomicU64::new(
                self.path_calls.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl GridMap {
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0);
        Self {
            width,
            height,
            blocked: HashSet::new(),
            revision: 0,
            #[cfg(test)]
            path_calls: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Number of times `find_path` has been invoked on this map. Test-only.
    #[cfg(test)]
    pub fn path_call_count(&self) -> u64 {
        self.path_calls.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn in_bounds(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && pos.x < self.width && pos.y < self.height
    }

    pub fn is_walkable(&self, pos: GridPos) -> bool {
        self.in_bounds(pos) && !self.blocked.contains(&pos)
    }

    pub fn set_blocked(&mut self, pos: GridPos, blocked: bool) -> bool {
        if !self.in_bounds(pos) {
            return false;
        }

        let changed = if blocked {
            self.blocked.insert(pos)
        } else {
            self.blocked.remove(&pos)
        };
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    pub fn set_blocked_rect(&mut self, min: GridPos, max_inclusive: GridPos) {
        let min_x = min.x.min(max_inclusive.x);
        let max_x = min.x.max(max_inclusive.x);
        let min_y = min.y.min(max_inclusive.y);
        let max_y = min.y.max(max_inclusive.y);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                self.set_blocked(GridPos::new(x, y), true);
            }
        }
    }

    pub fn world_to_cell(&self, world: Vec2) -> GridPos {
        GridPos::new(world.x.floor() as i32, world.y.floor() as i32)
    }

    pub fn cell_center(&self, cell: GridPos) -> Vec2 {
        Vec2::new(cell.x as f32 + 0.5, cell.y as f32 + 0.5)
    }

    pub fn find_path(&self, start: GridPos, goal: GridPos) -> Option<Vec<GridPos>> {
        #[cfg(test)]
        self.path_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
    fn occupancy_revision_changes_only_when_walkability_changes() {
        let mut map = GridMap::new(8, 8);
        let cell = GridPos::new(3, 4);

        assert_eq!(map.revision(), 0);
        assert!(map.set_blocked(cell, true));
        assert_eq!(map.revision(), 1);
        assert!(!map.is_walkable(cell));

        assert!(!map.set_blocked(cell, true));
        assert_eq!(map.revision(), 1);

        assert!(map.set_blocked(cell, false));
        assert_eq!(map.revision(), 2);
        assert!(map.is_walkable(cell));

        assert!(!map.set_blocked(cell, false));
        assert_eq!(map.revision(), 2);
    }

    #[test]
    fn world_cell_round_trip_uses_cell_centers() {
        let map = GridMap::new(8, 8);
        for cell in [GridPos::new(0, 0), GridPos::new(3, 5), GridPos::new(7, 7)] {
            assert_eq!(map.world_to_cell(map.cell_center(cell)), cell);
        }
    }
}
