//! The underground beneath the Research Center: excavation and room state
//! plus the one fixed step that advances it. No ECS entities, no second
//! map, no scheduler — the paid jobs start here through the two player
//! commands (`apply_excavate`, `apply_build_room`); this module only
//! advances timers and banks completed rooms' stockpile output into the
//! shared Team-1 economy.

use bevy::prelude::{Resource, World};

use crate::catalog::{
    ROOM_SLOTS_PER_LEVEL, ResourceKind, RoomKind, excavation_cost_and_seconds, room_spec,
};
use crate::commands::{CommandResult, RejectReason};
use crate::economy::{TeamEconomy, try_spend};
use crate::ids::TeamId;
use crate::production::PROGRESS_EPSILON;
use crate::research::living_research_core;
use crate::session::gameplay_active;

/// Slack over the whole-unit credit threshold so accumulated float error
/// cannot strand a fraction that has actually reached the next unit — the
/// same idea as the furnace burn epsilon in `heat`.
const OUTPUT_EPSILON: f64 = 1e-9;

/// One room under construction or complete in a level slot. Completion is
/// derived — `progress_seconds` against the catalogue build duration —
/// never a stored flag, so a room cannot complete twice. `progress_seconds`
/// snaps to the build duration through production's `PROGRESS_EPSILON`
/// rule, and `production_remainder` banks the room's own fractional
/// stockpile output between whole units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub kind: RoomKind,
    pub progress_seconds: f32,
    pub production_remainder: f64,
}

impl Room {
    /// A freshly placed, unpaid-progress room at zero progress.
    pub fn new(kind: RoomKind) -> Self {
        Self {
            kind,
            progress_seconds: 0.0,
            production_remainder: 0.0,
        }
    }

    /// Whether the catalogue build duration has fully elapsed.
    pub fn complete(&self) -> bool {
        self.progress_seconds >= room_spec(self.kind).build_seconds as f32
    }
}

/// One excavated level: exactly `ROOM_SLOTS_PER_LEVEL` optional room slots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UndergroundLevel {
    pub slots: [Option<Room>; ROOM_SLOTS_PER_LEVEL],
}

/// The single paid dig. Its target is always `levels.len() + 1` — nothing
/// redundant is stored. Only one dig may be active, and once cleared the
/// appended level remains permanently available.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Excavation {
    pub elapsed_seconds: f32,
}

/// The whole underground state. Depth is `levels.len()`, derived on read.
#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub struct Underground {
    pub levels: Vec<UndergroundLevel>,
    pub excavation: Option<Excavation>,
}

impl Underground {
    /// Completed depth in levels; an empty underground is depth 0.
    pub fn depth(&self) -> u32 {
        self.levels.len() as u32
    }

    /// The level an active dig is opening — always one below the current
    /// completed depth.
    pub fn excavation_target(&self) -> u32 {
        self.depth() + 1
    }
}

/// Applies an accepted `Excavate` — the global panel action with no
/// building ID: the Team-1 issuer gate and the shared living-core fact
/// (`Locked` for either), then the single-dig rule. Pricing and the start
/// happen only after every check passes: one atomic `try_spend` of the
/// target level's plain saturated price, then the timer starts without
/// exposing a level — `step_underground` appends the level when the dig
/// completes. A rejection anywhere deducts nothing and starts nothing.
pub(crate) fn apply_excavate(world: &mut World, issuer: TeamId) -> CommandResult {
    let mut result = CommandResult::default();
    if let Err(reason) = validate_excavate(world, issuer) {
        result.reject = Some(reason);
        return result;
    }

    let target = world
        .get_resource::<Underground>()
        .map_or(1, Underground::excavation_target);
    let (cost, _) = excavation_cost_and_seconds(target);
    if !try_spend(world, issuer, cost) {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    world
        .get_resource_or_insert_with(Underground::default)
        .excavation = Some(Excavation::default());
    result
}

/// Excavate validation: a Team-1-only command like Analyze, so a foreign
/// issuer answers `Locked` before anything else; the same shared
/// living-core fact research accrual checks answers `Locked` too (in
/// session a destroyed core already settled Result, so this is mostly the
/// session-free worlds' guard); then the single-dig rule. Pricing is
/// deliberately not here: it is derived state, checked in `apply_excavate`
/// after these facts hold.
fn validate_excavate(world: &World, issuer: TeamId) -> Result<(), RejectReason> {
    if issuer != TeamId(1) {
        return Err(RejectReason::Locked);
    }
    if !living_research_core(world) {
        return Err(RejectReason::Locked);
    }
    if world
        .get_resource::<Underground>()
        .is_some_and(|underground| underground.excavation.is_some())
    {
        return Err(RejectReason::ExcavationInProgress);
    }
    Ok(())
}

/// Applies an accepted `BuildRoom`: the Team-1 issuer gate and the same
/// shared living-core lookup (no building selection), then every
/// prerequisite — a completed level, a valid slot, an empty slot, the
/// kind's `min_level` — before one atomic `try_spend` of the kind's fixed
/// Scrap price writes a fresh zero-progress room. A rejection anywhere
/// deducts nothing and writes no room.
pub(crate) fn apply_build_room(
    world: &mut World,
    issuer: TeamId,
    level: u32,
    slot: u8,
    kind: RoomKind,
) -> CommandResult {
    let mut result = CommandResult::default();
    if let Err(reason) = validate_build_room(world, issuer, level, slot, kind) {
        result.reject = Some(reason);
        return result;
    }

    if !try_spend(world, issuer, room_spec(kind).cost) {
        result.reject = Some(RejectReason::InsufficientResources);
        return result;
    }

    let mut underground = world.get_resource_or_insert_with(Underground::default);
    // Validation guaranteed `level` is an existing 1-based level index.
    underground.levels[(level - 1) as usize].slots[slot as usize] = Some(Room::new(kind));
    result
}

/// BuildRoom validation, in reject order: Team 1 only (`Locked`), a
/// living Team-1 core through the one shared fact (`Locked` — the level
/// under a dead core belongs to nobody), a completed dug-out level
/// (`OutOfBounds` — level 0 and the level an active dig is still opening
/// are both missing), a slot inside the four-slot level (`OutOfBounds`),
/// an empty slot (`Occupied` — a room under construction already owns its
/// slot) and the kind's `min_level` (`Locked`).
fn validate_build_room(
    world: &World,
    issuer: TeamId,
    level: u32,
    slot: u8,
    kind: RoomKind,
) -> Result<(), RejectReason> {
    if issuer != TeamId(1) {
        return Err(RejectReason::Locked);
    }
    if !living_research_core(world) {
        return Err(RejectReason::Locked);
    }
    let Some(underground) = world.get_resource::<Underground>() else {
        return Err(RejectReason::OutOfBounds);
    };
    if level == 0 || level > underground.depth() {
        return Err(RejectReason::OutOfBounds);
    }
    if slot as usize >= ROOM_SLOTS_PER_LEVEL {
        return Err(RejectReason::OutOfBounds);
    }
    if underground.levels[(level - 1) as usize].slots[slot as usize].is_some() {
        return Err(RejectReason::Occupied);
    }
    if level < room_spec(kind).min_level {
        return Err(RejectReason::Locked);
    }
    Ok(())
}

/// Advances the underground one fixed tick, gated by three things: the
/// shared `gameplay_active` session flag (Start, Paused and a settled
/// Result freeze progress; a session-free world — fixture tests, the
/// benchmark — plays), a finite positive delta like the wave clock's own
/// guard, and the shared living-core fact (a session-free world with a
/// destroyed core cannot continue underground work).
///
/// Order inside the tick: rooms already complete at tick entry bank their
/// fractional stockpile output first — a room completing this tick has
/// produced nothing yet and starts next tick, never retroactively — then
/// the single paid dig advances (completion appends exactly one fresh
/// four-slot level and clears the excavation), then each room build timer
/// advances independently. Both timers use production's snap-to-required
/// `PROGRESS_EPSILON` tolerance, so float drift cannot delay a completion
/// past its boundary tick. Stepping never charges anything: jobs are paid
/// up front by the commands that start them.
pub fn step_underground(world: &mut World, seconds: f32) {
    if !gameplay_active(world) || !seconds.is_finite() || seconds <= 0.0 {
        return;
    }
    if !living_research_core(world) {
        return;
    }
    let Some(mut underground) = world.get_resource_mut::<Underground>() else {
        return;
    };
    let underground = &mut *underground;

    // Whole units banked below through the one shared saturating
    // `ResourceStockpile::add` — one ResourceKind→currency match. Collected
    // here because the stockpile borrow cannot overlap the underground's.
    let mut credits: Vec<(ResourceKind, u32)> = Vec::new();
    for level in &mut underground.levels {
        for slot in &mut level.slots {
            let Some(room) = slot else { continue };
            if !room.complete() {
                continue;
            }
            let Some((kind, rate)) = room_spec(room.kind).output else {
                continue;
            };
            room.production_remainder += f64::from(rate) * f64::from(seconds);
            let whole = (room.production_remainder + OUTPUT_EPSILON).floor();
            if whole >= 1.0 {
                room.production_remainder -= whole;
                // `as u32` saturates, so one room's own output cannot wrap.
                credits.push((kind, whole as u32));
            }
        }
    }

    // The single dig, snapping to the target level's duration.
    let target = underground.excavation_target();
    let dig_complete = match underground.excavation.as_mut() {
        Some(dig) => {
            let required = excavation_cost_and_seconds(target).1 as f32;
            let next = dig.elapsed_seconds + seconds;
            dig.elapsed_seconds = if next + PROGRESS_EPSILON >= required {
                required
            } else {
                next
            };
            dig.elapsed_seconds >= required
        }
        None => false,
    };
    if dig_complete {
        underground.excavation = None;
        underground.levels.push(UndergroundLevel::default());
    }

    // Independent room build timers, snapping the same way; completed
    // rooms stay clamped at their build duration.
    for level in &mut underground.levels {
        for room in level.slots.iter_mut().flatten() {
            let required = room_spec(room.kind).build_seconds as f32;
            if room.progress_seconds < required {
                let next = room.progress_seconds + seconds;
                room.progress_seconds = if next + PROGRESS_EPSILON >= required {
                    required
                } else {
                    next
                };
            }
        }
    }

    // Credit whole units into the shared Team-1 stockpile — saturating, so
    // no credit can overflow a u32 field. A world without a Team-1 economy
    // row has nowhere to bank; every seeded world has one.
    if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&TeamId(1))
    {
        for (kind, amount) in credits {
            state.stockpile.add(kind, amount);
        }
    }
}

#[cfg(test)]
mod tests;
