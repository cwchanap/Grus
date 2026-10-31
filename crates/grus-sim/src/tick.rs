//! The canonical fixed-step tick: one function both the runtime bridge and
//! the integration tests call, so the executed order and the tested order
//! cannot drift. `crates/grus-sim/tests/system_order.rs` proves the sequence
//! itself by calling `step_tick`.

use bevy::prelude::World;

use crate::map::GridMap;
use crate::{
    refresh_visibility, step_combat, step_construction, step_economy, step_heat, step_movement,
    step_production, step_waves,
};

/// Advances the simulation one fixed tick in the authoritative order:
/// **waves → heat → combat → movement → economy → construction →
/// production → visibility**. Heat totals computed here scale this same
/// tick's turret cooldowns and production progress; this tick's movement
/// arrivals are visible to economy and construction; a construction
/// completed here (population capacity, producer queues) is visible to
/// production; the closing visibility refresh is what presentation and the
/// next tick's command validation consume.
pub fn step_tick(world: &mut World, map: &mut GridMap, seconds: f32) {
    step_waves(world, map, seconds);
    step_heat(world, seconds);
    step_combat(world, map, seconds);
    step_movement(world, map, seconds);
    step_economy(world, map, seconds);
    step_construction(world, map, seconds);
    step_production(world, map, seconds);
    refresh_visibility(world, map);
}
