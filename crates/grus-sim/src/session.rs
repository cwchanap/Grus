//! Match lifecycle: the Start/Playing/Paused/Result session state, its
//! explicit transitions, and the shared gameplay gate consulted by the
//! fixed steps and the command dispatcher.

use bevy::prelude::{Resource, World};

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

/// The effective phase: a missing session means Playing.
pub fn active_phase(world: &World) -> MatchPhase {
    world
        .get_resource::<MatchSession>()
        .map(|session| session.phase)
        .unwrap_or(MatchPhase::Playing)
}

/// Whether gameplay may mutate the world. Only an explicit session outside
/// Playing freezes commands and the fixed steps.
pub fn gameplay_active(world: &World) -> bool {
    matches!(active_phase(world), MatchPhase::Playing)
}

/// Explicit Start -> Playing. Paused resumes through `set_paused(false)`; a
/// settled Result only ever leaves through a restart.
pub fn start_match(world: &mut World) {
    if let Some(mut session) = world.get_resource_mut::<MatchSession>()
        && matches!(session.phase, MatchPhase::Start)
    {
        session.phase = MatchPhase::Playing;
    }
}

/// Pause/Resume flips only between Playing and Paused; Start and a settled
/// Result are untouched. Never touches `Engine.time_scale` — that stays the
/// headless sim-speed control.
pub fn set_paused(world: &mut World, paused: bool) {
    if let Some(mut session) = world.get_resource_mut::<MatchSession>() {
        session.phase = match (session.phase, paused) {
            (MatchPhase::Playing, true) => MatchPhase::Paused,
            (MatchPhase::Paused, false) => MatchPhase::Playing,
            (phase, _) => phase,
        };
    }
}

/// Records the winner at the first Town Center destruction; later callers —
/// including same-step attackers — cannot overwrite a settled result.
/// Session-free worlds (benchmark) have nothing to resolve into.
pub fn resolve_result(world: &mut World, winner: TeamId) {
    if let Some(mut session) = world.get_resource_mut::<MatchSession>()
        && !matches!(session.phase, MatchPhase::Result(_))
    {
        session.phase = MatchPhase::Result(MatchResult(winner));
    }
}

#[cfg(test)]
mod tests;
