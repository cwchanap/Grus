//! Match lifecycle contracts: the Start/Playing/Paused/Result session state.
//! Gating, transitions, and result resolution land with the session task.

use bevy::prelude::Resource;

use crate::ids::TeamId;

/// The authoritative result fact: which team won. Godot derives its
/// Victory/Defeat overlay by comparing the winner to the local team; there
/// is no draw state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchResult(pub TeamId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchPhase {
    Start,
    Playing,
    Paused,
    Result(MatchResult),
}

/// Optional per-world session state. A missing `MatchSession` resource means
/// Playing, so pure-sim fixtures and the benchmark need no lifecycle
/// boilerplate; normal Godot setup inserts `Start`.
#[derive(Debug, Resource)]
pub struct MatchSession {
    pub phase: MatchPhase,
}
