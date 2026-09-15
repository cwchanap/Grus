use std::collections::HashSet;

use bevy::math::Vec2;
use bevy::prelude::{Component, Resource};
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

/// One spatial rectangle contract shared by buildings and 1×1 resource
/// sources. `anchor` is the inclusive top-left cell.
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct Footprint {
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}

impl Footprint {
    pub const fn new(anchor: GridPos, width: u8, height: u8) -> Self {
        Self {
            anchor,
            width,
            height,
        }
    }

    /// All covered cells, row-major from the anchor.
    pub fn cells(&self) -> Vec<GridPos> {
        let mut cells = Vec::with_capacity(self.width as usize * self.height as usize);
        for dy in 0..i32::from(self.height) {
            for dx in 0..i32::from(self.width) {
                cells.push(GridPos::new(self.anchor.x + dx, self.anchor.y + dy));
            }
        }
        cells
    }

    /// Geometric center in world coordinates: `anchor + half the footprint`
    /// in each axis. For 1×1 footprints this equals the anchor's cell center.
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            self.anchor.x as f32 + f32::from(self.width) / 2.0,
            self.anchor.y as f32 + f32::from(self.height) / 2.0,
        )
    }

    /// The ring of cells immediately surrounding the footprint, row-major.
    /// Never contains a footprint cell and never scans a wider ring.
    pub fn perimeter_cells(&self) -> Vec<GridPos> {
        let min_x = self.anchor.x - 1;
        let max_x = self.anchor.x + i32::from(self.width);
        let min_y = self.anchor.y - 1;
        let max_y = self.anchor.y + i32::from(self.height);
        let mut cells = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if x > min_x && x < max_x && y > min_y && y < max_y {
                    continue;
                }
                cells.push(GridPos::new(x, y));
            }
        }
        cells
    }

    /// True when `cell` is on the immediate perimeter of the footprint.
    pub fn is_immediately_adjacent(&self, cell: GridPos) -> bool {
        !self.contains(cell)
            && cell.x >= self.anchor.x - 1
            && cell.x <= self.anchor.x + i32::from(self.width)
            && cell.y >= self.anchor.y - 1
            && cell.y <= self.anchor.y + i32::from(self.height)
    }

    fn contains(&self, cell: GridPos) -> bool {
        cell.x >= self.anchor.x
            && cell.x < self.anchor.x + i32::from(self.width)
            && cell.y >= self.anchor.y
            && cell.y < self.anchor.y + i32::from(self.height)
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
mod tests;
