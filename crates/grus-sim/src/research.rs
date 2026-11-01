//! Research: Team 1's monotonic point total toward Victory, the
//! heat-scaled passive rate, the catalogue-driven building-unlock check,
//! and the research fixed step. One `f64` accumulator keeps the long-horizon
//! accumulation exact at the 20 Hz fixed step; milestones derive from the
//! live total rather than stored flags.

use bevy::prelude::{Resource, World};

use crate::catalog::{
    BuildingKind, RESEARCH_BASE_PER_SECOND, RESEARCH_TARGET_POINTS, building_spec,
};
use crate::heat::surface_efficiency;
use crate::ids::TeamId;
use crate::session::{gameplay_active, resolve_result};

/// Team 1's accumulated research points, capped at
/// `RESEARCH_TARGET_POINTS`; reaching the target resolves Victory. Seeded
/// at zero with the skirmish fixture and dropped with the match — a
/// missing resource reads as zero through `research_points`.
#[derive(Clone, Copy, Debug, Default, Resource, PartialEq)]
pub struct ResearchState {
    pub points: f64,
}

/// The one neutral-default research accessor: a world without
/// `ResearchState` reads zero points.
pub fn research_points(world: &World) -> f64 {
    world
        .get_resource::<ResearchState>()
        .map(|state| state.points)
        .unwrap_or(0.0)
}

/// The point threshold behind a catalogue `research_unlock_percent` (or
/// `RESEARCH_WAVE_BOOST_PERCENT`): `RESEARCH_TARGET_POINTS × percent /
/// 100` in integer points. The single derivation shared by
/// `building_unlocked`, wave-launch escalation and the bridge's
/// `unlock_points` snapshot — no copied point constants anywhere else.
pub fn unlock_points(percent: u32) -> u32 {
    RESEARCH_TARGET_POINTS * percent / 100
}

/// Whether `kind`'s catalogue research gate is open at the live point
/// total. Kinds with `research_unlock_percent: 0` are always unlocked;
/// placement validation and preview share this one authoritative rule.
pub fn building_unlocked(world: &World, kind: BuildingKind) -> bool {
    research_points(world) >= f64::from(unlock_points(building_spec(kind).research_unlock_percent))
}

/// Live research rate in points per simulated second: the surface source
/// is `RESEARCH_BASE_PER_SECOND` scaled by the same `surface_efficiency`
/// every heat consumer reads, so a cold surface slows research like
/// production and turret cooldowns. The underground source is zero until
/// HPA-288 adds it as an additive term here. Advancement and any display
/// ETA share this one source function so they can never diverge.
pub fn research_rate(world: &World) -> f64 {
    RESEARCH_BASE_PER_SECOND * f64::from(surface_efficiency(world))
}

/// Adds `points` to the one accumulator, clamped to the catalogue target;
/// reaching it resolves Team-1 Victory through the shared session helper.
/// Called only from Playing-gated paths — `step_research` checks
/// `gameplay_active`, accepted Analyze commands pass the session gate —
/// and `resolve_result` itself ignores a settled session, so a later call
/// can never overwrite the winner.
pub fn add_research(world: &mut World, points: f64) {
    let total = (research_points(world) + points).clamp(0.0, f64::from(RESEARCH_TARGET_POINTS));
    world
        .get_resource_or_insert_with(ResearchState::default)
        .points = total;
    if total >= f64::from(RESEARCH_TARGET_POINTS) {
        resolve_result(world, TeamId(1));
    }
}

/// Advances passive research one fixed tick, gated only by the shared
/// `gameplay_active` session flag — Start, Paused and a settled Result
/// freeze progress, and a session-free world (fixture tests, the
/// benchmark) plays. The seeded Research Center is complete and
/// unplaceable, and its destruction already settles Result inside combat
/// earlier in this same canonical tick, so no second living-core check
/// exists here. Runs after production and before visibility: it consumes
/// the heat totals `step_heat` computed at the top of this tick, and a
/// lethal same-tick hit on the core wins the tie for Team 2.
pub fn step_research(world: &mut World, seconds: f32) {
    if !gameplay_active(world) {
        return;
    }
    add_research(world, research_rate(world) * f64::from(seconds));
}

#[cfg(test)]
mod tests;
