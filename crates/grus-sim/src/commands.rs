use bevy::math::Vec2;

use crate::ids::{TeamId, UnitId};

#[derive(Clone, Debug)]
pub enum UnitCommandKind {
    Move { target: Vec2 },
    Stop,
}

#[derive(Clone, Debug)]
pub struct UnitCommand {
    pub issuer: TeamId,
    pub units: Vec<UnitId>,
    pub kind: UnitCommandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRejectReason {
    UnknownUnit,
    NotOwned,
    Unreachable,
}

#[derive(Clone, Debug, Default)]
pub struct CommandOutcome {
    pub accepted: Vec<UnitId>,
    pub rejected: Vec<(UnitId, CommandRejectReason)>,
}
