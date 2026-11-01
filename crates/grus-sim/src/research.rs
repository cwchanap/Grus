//! Research: Team 1's monotonic point total toward Victory, the
//! heat-scaled passive rate, the catalogue-driven building-unlock check,
//! and the research fixed step. One `f64` accumulator keeps the long-horizon
//! accumulation exact at the 20 Hz fixed step; milestones derive from the
//! live total rather than stored flags.

use bevy::prelude::{Resource, World};

use crate::buildings::{Building, BuildingIndex, commandable_completed_building};
use crate::catalog::{
    ANALYZE_COST, ANALYZE_RESEARCH_GAIN, BuildingKind, RESEARCH_BASE_PER_SECOND,
    RESEARCH_TARGET_POINTS, building_spec,
};
use crate::commands::{CommandResult, RejectReason};
use crate::economy::try_spend;
use crate::heat::surface_efficiency;
use crate::ids::{BuildingId, TeamId};
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
/// Only a finite, positive grant moves the total — a NaN delta would
/// poison every later threshold read forever (the tower unlocks fail
/// closed while the wave boost fails open, and Victory can never fire),
/// and a negative one would drag the monotonic total backward. Called
/// only from Playing-gated paths — `step_research` checks
/// `gameplay_active`, accepted Analyze commands pass the session gate —
/// and `resolve_result` itself ignores a settled session, so a later call
/// can never overwrite the winner.
pub fn add_research(world: &mut World, points: f64) {
    if !points.is_finite() || points <= 0.0 {
        return;
    }
    let total = (research_points(world) + points).clamp(0.0, f64::from(RESEARCH_TARGET_POINTS));
    world
        .get_resource_or_insert_with(ResearchState::default)
        .points = total;
    if total >= f64::from(RESEARCH_TARGET_POINTS) {
        resolve_result(world, TeamId(1));
    }
}

/// Advances passive research one fixed tick, gated by three things: the
/// shared `gameplay_active` session flag (Start, Paused and a settled
/// Result freeze progress, and a session-free world — fixture tests, the
/// benchmark — plays), a finite positive delta like the wave clock's own
/// `advance` guard, and a living Team-1 core. In-session the core's
/// destruction already settles Result inside combat earlier in this same
/// canonical tick; the living-core check is what stops the session-free
/// worlds that have no Result to settle — the benchmark's Research
/// Center falls mid-run and must not keep banking points into the
/// wave-boost threshold. Runs after production and before visibility: it
/// consumes the heat totals `step_heat` computed at the top of this
/// tick, and a lethal same-tick hit on the core wins the tie for Team 2.
pub fn step_research(world: &mut World, seconds: f32) {
    if !gameplay_active(world) || !seconds.is_finite() || seconds <= 0.0 {
        return;
    }
    if !living_research_core(world) {
        return;
    }
    add_research(world, research_rate(world) * f64::from(seconds));
}

/// Whether Team 1's Research Center still lives — its `BuildingIndex`
/// entry is the alive fact (combat removal drops it before the despawn).
/// Completion is deliberately not part of the gate: canonical-tick tests
/// run an incomplete core to zero the heat supply, and accrual has no
/// reason to stop for it.
fn living_research_core(world: &World) -> bool {
    world.get_resource::<BuildingIndex>().is_some_and(|index| {
        index.iter().any(|(_, entity)| {
            world.get::<Building>(*entity).is_some_and(|building| {
                building.team == TeamId(1) && building.kind == BuildingKind::ResearchCenter
            })
        })
    })
}

/// Applies an accepted `Analyze` on the issuer's Research Center: the
/// Team-1 issuer gate, then the commandable-building validation chain
/// (exists → owned → completed → kind → under the cap), then one atomic
/// `try_spend` of `ANALYZE_COST` before the shared `add_research` banks
/// `ANALYZE_RESEARCH_GAIN`. A rejection anywhere — validation or spend —
/// deducts nothing and grants nothing; the command is immediate, never a
/// production-queue job.
pub(crate) fn apply_analyze(
    world: &mut World,
    issuer: TeamId,
    building: BuildingId,
) -> CommandResult {
    let mut result = CommandResult::default();
    if let Err(reason) = validate_analyze(world, issuer, building) {
        result.reject = Some(reason);
        return result;
    }

    // Atomic spend before the grant: a failed `try_spend` leaves every
    // currency field unchanged and no points are banked.
    if !try_spend(world, issuer, ANALYZE_COST) {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    add_research(world, ANALYZE_RESEARCH_GAIN);
    result
}

/// Analyze validation: a Team-1-only command like the Team-1-only
/// placement kinds, so a foreign issuer answers `Locked` before any
/// building resolution — a Team-2 core can never spend Team-2 Crystals
/// into Team-1's point total and hand it a Victory. Then the
/// commandable-building resolution (a hidden enemy is indistinguishable
/// from absent — fog privacy) → owned → completed → the one
/// research-capable kind → under the cap: a banked target leaves nothing
/// to buy, and session-free worlds have no Result gate to lock the
/// command first. The Research Center is the only Analyze target.
fn validate_analyze(
    world: &World,
    issuer: TeamId,
    building: BuildingId,
) -> Result<(), RejectReason> {
    if issuer != TeamId(1) {
        return Err(RejectReason::Locked);
    }
    let building_entity = commandable_completed_building(world, issuer, building)?;
    let state = world
        .get::<Building>(building_entity)
        .ok_or(RejectReason::BuildingMissing)?;
    if state.kind != BuildingKind::ResearchCenter {
        return Err(RejectReason::WrongProducer);
    }
    if research_points(world) >= f64::from(RESEARCH_TARGET_POINTS) {
        return Err(RejectReason::Locked);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
