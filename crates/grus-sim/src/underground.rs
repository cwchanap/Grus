//! The underground beneath the Research Center: excavation and room state
//! plus the one fixed step that advances it. No ECS entities, no second
//! map, no scheduler — commands (HPA-288 checkpoint 2) start paid jobs;
//! this module only advances timers and banks completed rooms' stockpile
//! output into the shared Team-1 economy.

use bevy::prelude::{Resource, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{
    BuildingKind, ROOM_SLOTS_PER_LEVEL, ResourceKind, RoomKind, excavation_cost_and_seconds,
    room_spec,
};
use crate::economy::TeamEconomy;
use crate::ids::TeamId;
use crate::session::gameplay_active;

/// Slack over the whole-unit credit threshold so accumulated float error
/// cannot strand a fraction that has actually reached the next unit — the
/// same idea as the furnace burn epsilon in `heat`.
const OUTPUT_EPSILON: f64 = 1e-9;

/// One room under construction or complete in a level slot. Completion is
/// derived — `elapsed_seconds` against the catalogue build duration —
/// never a stored flag, so a room cannot complete twice. `elapsed_seconds`
/// clamps at the build duration once complete, and `output_remainder`
/// banks the room's own fractional stockpile output between whole units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub kind: RoomKind,
    pub elapsed_seconds: f32,
    pub output_remainder: f64,
}

impl Room {
    /// A freshly placed, unpaid-progress room at zero elapsed time.
    pub fn new(kind: RoomKind) -> Self {
        Self {
            kind,
            elapsed_seconds: 0.0,
            output_remainder: 0.0,
        }
    }

    /// Whether the catalogue build duration has fully elapsed.
    pub fn complete(&self) -> bool {
        self.elapsed_seconds >= room_spec(self.kind).build_seconds as f32
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

/// Whether Team 1's Research Center lives and is complete — the same core
/// the underground commands require. Unlike the research accrual gate,
/// completion matters here: an unpowered construction site cannot run a
/// dig. In-session the core's destruction already settles Result earlier
/// in the canonical tick; this check is what stops the session-free
/// worlds (the benchmark) that have no Result to settle.
fn living_completed_core(world: &World) -> bool {
    world.get_resource::<BuildingIndex>().is_some_and(|index| {
        index.iter().any(|(_, entity)| {
            world.get::<Building>(*entity).is_some_and(|building| {
                building.team == TeamId(1)
                    && building.kind == BuildingKind::ResearchCenter
                    && building.construction.complete
            })
        })
    })
}

/// Advances the underground one fixed tick, gated by three things: the
/// shared `gameplay_active` session flag (Start, Paused and a settled
/// Result freeze progress; a session-free world — fixture tests, the
/// benchmark — plays), a finite positive delta like the wave clock's own
/// guard, and a living completed Team-1 core.
///
/// Order inside the tick: rooms already complete at tick entry bank their
/// fractional stockpile output first — a room completing this tick has
/// produced nothing yet and starts next tick, never retroactively — then
/// the single paid dig advances (completion appends exactly one fresh
/// four-slot level and clears the excavation), then each room build timer
/// advances independently and clamps at its catalogue duration. Stepping
/// never charges anything: jobs are paid up front by the commands that
/// start them.
pub fn step_underground(world: &mut World, seconds: f32) {
    if !gameplay_active(world) || !seconds.is_finite() || seconds <= 0.0 {
        return;
    }
    if !living_completed_core(world) {
        return;
    }
    let Some(mut underground) = world.get_resource_mut::<Underground>() else {
        return;
    };
    let underground = &mut *underground;

    // Bank output for rooms complete at tick entry, whole units only; the
    // per-room remainder keeps every fraction for later ticks. Per-currency
    // totals saturate like the stockpile adds below: `whole as u32` already
    // saturates one room's output, and many rooms' combined units must not
    // overflow the intermediate total.
    let mut food = 0u32;
    let mut scrap = 0u32;
    let mut coal = 0u32;
    for level in &mut underground.levels {
        for slot in &mut level.slots {
            let Some(room) = slot else { continue };
            if !room.complete() {
                continue;
            }
            let Some((kind, rate)) = room_spec(room.kind).output else {
                continue;
            };
            room.output_remainder += f64::from(rate) * f64::from(seconds);
            let whole = (room.output_remainder + OUTPUT_EPSILON).floor();
            if whole >= 1.0 {
                room.output_remainder -= whole;
                match kind {
                    ResourceKind::Food => food = food.saturating_add(whole as u32),
                    ResourceKind::Scrap => scrap = scrap.saturating_add(whole as u32),
                    ResourceKind::Coal => coal = coal.saturating_add(whole as u32),
                }
            }
        }
    }

    // The single dig. An unaffordable target (catalogue `None`) can never
    // have been started by a paid command, but if it ever were, infinite
    // duration keeps it from completing for free.
    let target = underground.excavation_target();
    let dig_complete = match underground.excavation.as_mut() {
        Some(dig) => {
            let duration = excavation_cost_and_seconds(target)
                .map_or(f32::INFINITY, |(_, dig_seconds)| dig_seconds as f32);
            dig.elapsed_seconds = (dig.elapsed_seconds + seconds).min(duration);
            dig.elapsed_seconds >= duration
        }
        None => false,
    };
    if dig_complete {
        underground.excavation = None;
        underground.levels.push(UndergroundLevel::default());
    }

    // Independent room build timers; completed rooms stay clamped at their
    // build duration.
    for level in &mut underground.levels {
        for room in level.slots.iter_mut().flatten() {
            let build = room_spec(room.kind).build_seconds as f32;
            if room.elapsed_seconds < build {
                room.elapsed_seconds = (room.elapsed_seconds + seconds).min(build);
            }
        }
    }

    // Credit whole units into the shared Team-1 stockpile — saturating, so
    // no credit can overflow a u32 field. A world without a Team-1 economy
    // row has nowhere to bank; every seeded world has one.
    if (food > 0 || scrap > 0 || coal > 0)
        && let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
        && let Some(state) = economy.0.get_mut(&TeamId(1))
    {
        state.stockpile.food = state.stockpile.food.saturating_add(food);
        state.stockpile.scrap = state.stockpile.scrap.saturating_add(scrap);
        state.stockpile.coal = state.stockpile.coal.saturating_add(coal);
    }
}

#[cfg(test)]
mod tests;
