//! Team stockpiles, worker task state, and drop-off contracts. Gather and
//! deposit behavior land in a later HPA-471 task.

use std::collections::HashMap;
use std::num::NonZeroU32;

use bevy::prelude::{Component, Resource};

use crate::catalog::{Age, ResourceKind};
use crate::ids::{BuildingId, ResourceId, TeamId};
use crate::map::GridPos;

/// Carried load of a worker. Invariant: never empty while `Holding`, never
/// mixes resource kinds, never holds zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Carry {
    Empty,
    Holding {
        kind: ResourceKind,
        amount: NonZeroU32,
    },
}

/// Per-worker fractional gather accumulator; whole resources transfer when
/// progress crosses 1.0.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GatherProgress(pub f32);

/// Current activity of a worker. Arrival is positive: transitions happen when
/// the worker's cell equals the stored slot.
#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub enum WorkerTask {
    Idle,
    ToSource {
        source: ResourceId,
        slot: GridPos,
    },
    Gathering {
        source: ResourceId,
    },
    ToDropoff {
        source: ResourceId,
        dropoff: BuildingId,
        slot: GridPos,
    },
    ToConstruction {
        building: BuildingId,
        slot: GridPos,
    },
    Constructing {
        building: BuildingId,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceStockpile {
    pub food: u32,
    pub wood: u32,
    pub gold: u32,
}

#[derive(Clone, Debug)]
pub struct TeamState {
    pub stockpile: ResourceStockpile,
    pub age: Age,
    pub age_up_started: bool,
}

#[derive(Debug, Default, Resource)]
pub struct TeamEconomy(pub HashMap<TeamId, TeamState>);

impl TeamEconomy {
    pub fn insert_team(&mut self, team: TeamId, stockpile: ResourceStockpile, age: Age) {
        self.0.insert(
            team,
            TeamState {
                stockpile,
                age,
                age_up_started: false,
            },
        );
    }
}

/// Marker on buildings that accept deposits; carries only team ownership.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dropoff {
    pub team: TeamId,
}
