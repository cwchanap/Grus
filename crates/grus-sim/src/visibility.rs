//! Authoritative per-team visibility: the one fog-of-war contract shared by
//! combat, placement, gathering, rendering, and AI. `refresh_visibility` is
//! the exclusive world step; `visible_to` and `explored_by` are the only
//! public knowledge predicates, and both fall back to full information when
//! no `VisibilityMap` exists. Geometry (reveal origins, circle metric,
//! center-vs-edge semantics) is resolved here and nowhere else.

use std::collections::{HashMap, HashSet};

use bevy::prelude::{Resource, World};

use crate::buildings::Building;
use crate::catalog::VISION_RADIUS_CELLS;
use crate::ids::TeamId;
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{SimPosition, Unit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellVisibility {
    Unexplored = 0,
    Explored = 1,
    Visible = 2,
}

#[derive(Debug, Default, Resource)]
pub struct VisibilityMap {
    teams: HashMap<TeamId, TeamVision>,
    revision: u64,
}

#[derive(Debug, Default)]
struct TeamVision {
    explored: HashSet<GridPos>,
    visible: HashSet<GridPos>,
}

/// What a knowledge predicate is checked against: a unit's single current
/// cell, or a building/resource footprint. The any-cell rule for footprints
/// lives in `any_cell`, so no consumer picks its own center-vs-edge rule.
#[derive(Clone, Copy, Debug)]
pub enum VisibilitySubject {
    Cell(GridPos),
    Footprint(Footprint),
}

impl From<GridPos> for VisibilitySubject {
    fn from(cell: GridPos) -> Self {
        Self::Cell(cell)
    }
}

impl From<Footprint> for VisibilitySubject {
    fn from(footprint: Footprint) -> Self {
        Self::Footprint(footprint)
    }
}

impl VisibilitySubject {
    /// True when any cell of the subject satisfies `f`.
    fn any_cell(&self, f: impl Fn(GridPos) -> bool) -> bool {
        match self {
            Self::Cell(cell) => f(*cell),
            Self::Footprint(footprint) => footprint.cells().into_iter().any(f),
        }
    }
}

impl VisibilityMap {
    /// Monotonic change counter for cheap per-frame polling (the Godot fog
    /// overlay and minimap fetch the packed payload only when this moves).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Row-major `width`×`height` cell states (0 Unexplored / 1 Explored /
    /// 2 Visible) for `team`. The HashSet→payload conversion lives here so
    /// the underlying representation stays replaceable; the bridge is the
    /// only caller.
    pub fn packed_cell_states(&self, team: TeamId, width: i32, height: i32) -> Vec<u8> {
        let mut states = vec![0_u8; (width * height) as usize];
        if let Some(vision) = self.teams.get(&team) {
            let index = |cell: &GridPos| (cell.y * width + cell.x) as usize;
            for cell in &vision.explored {
                states[index(cell)] = 1;
            }
            for cell in &vision.visible {
                states[index(cell)] = 2;
            }
        }
        states
    }

    /// Effective cell state for `team`; a team absent from the map has
    /// explored nothing. Test-only: consumers use the two predicates below.
    #[cfg(test)]
    fn cell_state(&self, team: TeamId, cell: GridPos) -> CellVisibility {
        let Some(vision) = self.teams.get(&team) else {
            return CellVisibility::Unexplored;
        };
        if vision.visible.contains(&cell) {
            CellVisibility::Visible
        } else if vision.explored.contains(&cell) {
            CellVisibility::Explored
        } else {
            CellVisibility::Unexplored
        }
    }

    fn subject_visible(&self, team: TeamId, subject: &VisibilitySubject) -> bool {
        self.teams
            .get(&team)
            .is_some_and(|vision| subject.any_cell(|cell| vision.visible.contains(&cell)))
    }

    fn subject_explored(&self, team: TeamId, subject: &VisibilitySubject) -> bool {
        self.teams
            .get(&team)
            .is_some_and(|vision| subject.any_cell(|cell| vision.explored.contains(&cell)))
    }
}

/// The unit/building/resource cell or footprint is currently seen by `team`.
/// True when no `VisibilityMap` exists — isolated pure-sim tests and the
/// benchmark run with full information.
pub fn visible_to(world: &World, team: TeamId, subject: impl Into<VisibilitySubject>) -> bool {
    match world.get_resource::<VisibilityMap>() {
        None => true,
        Some(visibility) => visibility.subject_visible(team, &subject.into()),
    }
}

/// The unit/building/resource has ever been seen by `team` (explored state is
/// retained across refreshes). True when no `VisibilityMap` exists.
pub fn explored_by(world: &World, team: TeamId, subject: impl Into<VisibilitySubject>) -> bool {
    match world.get_resource::<VisibilityMap>() {
        None => true,
        Some(visibility) => visibility.subject_explored(team, &subject.into()),
    }
}

/// The exclusive world-level visibility step. Recomputes each team's
/// current-visible set from live entities, unions it into explored state, and
/// increments `revision` only when some team's effective visibility or
/// exploration actually changed. Reveal origins: every living unit's current
/// cell and every cell of each completed building's footprint; construction
/// sites reveal nothing. Never inserts a `VisibilityMap` — the caller opts in
/// by inserting one (normal runtime does this in `setup_fixture`, then calls
/// this once so the Start screen already has correct fog).
pub fn refresh_visibility(world: &mut World, map: &GridMap) {
    let mut origins: HashMap<TeamId, Vec<GridPos>> = HashMap::new();

    let mut units = world.query::<(&Unit, &SimPosition)>();
    for (unit, position) in units.iter(world) {
        origins
            .entry(unit.team)
            .or_default()
            .push(map.world_to_cell(position.current));
    }

    let mut buildings = world.query::<(&Building, &Footprint)>();
    for (building, footprint) in buildings.iter(world) {
        if building.construction.complete {
            origins
                .entry(building.team)
                .or_default()
                .extend(footprint.cells());
        }
    }

    let Some(mut visibility) = world.get_resource_mut::<VisibilityMap>() else {
        return;
    };

    // Recompute every team the map knows plus any team with live origins; a
    // known team with no origins currently sees nothing (explored retained).
    let teams: Vec<TeamId> = visibility
        .teams
        .keys()
        .chain(origins.keys())
        .copied()
        .collect();
    let mut changed = false;
    for team in teams {
        let visible = reveal_circle(map, origins.get(&team).map_or(&[], |cells| cells));
        let vision = visibility.teams.entry(team).or_default();
        if vision.visible != visible {
            changed = true;
            vision.visible = visible.clone();
        }
        let explored_before = vision.explored.len();
        vision.explored.extend(visible);
        changed |= vision.explored.len() != explored_before;
    }
    if changed {
        visibility.revision = visibility.revision.wrapping_add(1);
    }
}

/// Deterministic Euclidean circle reveal: every in-bounds cell within
/// `dx*dx + dy*dy <= radius*radius` of any origin. No occlusion, facing
/// cones, or alternative metrics.
fn reveal_circle(map: &GridMap, origins: &[GridPos]) -> HashSet<GridPos> {
    let radius_squared = VISION_RADIUS_CELLS * VISION_RADIUS_CELLS;
    let mut visible = HashSet::new();
    for origin in origins {
        for dy in -VISION_RADIUS_CELLS..=VISION_RADIUS_CELLS {
            for dx in -VISION_RADIUS_CELLS..=VISION_RADIUS_CELLS {
                if dx * dx + dy * dy > radius_squared {
                    continue;
                }
                let cell = GridPos::new(origin.x + dx, origin.y + dy);
                if map.in_bounds(cell) {
                    visible.insert(cell);
                }
            }
        }
    }
    visible
}

#[cfg(test)]
mod tests;
