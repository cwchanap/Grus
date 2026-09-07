use bevy::math::Vec2;
use bevy::prelude::Component;

use crate::ids::{TeamId, UnitId};

#[derive(Clone, Copy, Component, Debug)]
pub struct Unit {
    pub id: UnitId,
    pub team: TeamId,
    pub speed: f32,
}

#[derive(Clone, Copy, Component, Debug)]
pub struct SimPosition {
    pub previous: Vec2,
    pub current: Vec2,
}

impl SimPosition {
    pub const fn new(position: Vec2) -> Self {
        Self {
            previous: position,
            current: position,
        }
    }
}

#[derive(Clone, Component, Debug)]
pub struct MoveOrder {
    pub waypoints: Vec<Vec2>,
    pub next: usize,
}
