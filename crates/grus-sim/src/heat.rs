//! Heat and cold: per-tick surface supply/demand from completed Team-1
//! buildings, the pooled Coal-furnace burn accumulator, and the derived
//! surface efficiency every consumer (production, turret cooldowns, future
//! research) reads through the one `surface_efficiency` accessor.

use bevy::prelude::{Entity, Resource, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{
    BuildingKind, FURNACE_COAL_PER_SECOND, MIN_SURFACE_EFFICIENCY, building_spec,
};
use crate::economy::TeamEconomy;
use crate::ids::{BuildingId, TeamId};
use crate::session::gameplay_active;

/// Slack under the whole-unit burn threshold, so an accumulated 0.5 Coal/s
/// pool debits once it reaches a whole unit despite float error.
const BURN_EPSILON: f32 = 1e-4;

/// Live heat totals over completed Team-1 buildings. `supply`/`demand` are
/// recomputed from the world on every `step_heat` call in every session
/// phase; `burn_remainder` pools the fractional Coal burn between whole
/// units and is cleared by a fixture reset or stockpile exhaustion.
#[derive(Clone, Copy, Debug, Default, Resource, PartialEq)]
pub struct HeatState {
    pub supply: u32,
    pub demand: u32,
    pub burn_remainder: f32,
}

impl HeatState {
    /// Surface efficiency: supply/demand, zero demand = full, deficit
    /// floored at `MIN_SURFACE_EFFICIENCY`. Derived on read, never stored.
    pub fn efficiency(&self) -> f32 {
        if self.demand == 0 {
            1.0
        } else {
            (self.supply as f32 / self.demand as f32).clamp(MIN_SURFACE_EFFICIENCY, 1.0)
        }
    }
}

/// The one neutral-default heat accessor: a world without `HeatState` and
/// an inserted-but-unstepped default both read full efficiency.
pub fn surface_efficiency(world: &World) -> f32 {
    world
        .get_resource::<HeatState>()
        .map_or(1.0, HeatState::efficiency)
}

/// Team 1's Coal stockpile, or zero without an economy row.
fn team_coal(world: &World) -> u32 {
    world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&TeamId(1)))
        .map(|state| state.stockpile.coal)
        .unwrap_or(0)
}

/// Recomputes heat supply/demand from completed Team-1 buildings in every
/// session phase — Start, Paused and Result included. A furnace
/// contributes its catalogue supply only while Team 1 holds Coal at the
/// start of the tick, so a tick whose final consumption empties the
/// stockpile still supplies, and the next coal-empty tick does not. Only
/// while Playing does the burn rate accumulate into the one pooled
/// remainder and debit whole units from the shared stockpile, capped by
/// what is left; the remainder clears the moment the stockpile empties so
/// no fractional debt survives to a refuel.
pub fn step_heat(world: &mut World, seconds: f32) {
    let mut supply: u32 = 0;
    let mut demand: u32 = 0;
    let mut furnaces: u32 = 0;
    if let Some(index) = world.get_resource::<BuildingIndex>() {
        // Stable iteration: BuildingIndex entries sorted by building id.
        let mut entries: Vec<(BuildingId, Entity)> =
            index.iter().map(|(id, entity)| (*id, *entity)).collect();
        entries.sort_unstable_by_key(|&(id, _)| id);
        for (_, entity) in entries {
            let Some(building) = world.get::<Building>(entity) else {
                continue;
            };
            if building.team != TeamId(1) || !building.construction.complete {
                continue;
            }
            let spec = building_spec(building.kind);
            if building.kind == BuildingKind::CoalFurnace {
                furnaces += 1;
            } else {
                supply += spec.heat_supply;
            }
            demand += spec.heat_demand;
        }
    }
    let fueled = team_coal(world) > 0;
    if fueled {
        supply += furnaces * building_spec(BuildingKind::CoalFurnace).heat_supply;
    }

    // A purchase (not a burn) that empties the stockpile between ticks skips
    // the debit branch below, so drop the stale fraction right here — no
    // phantom debt may survive to a refuel.
    if gameplay_active(world) && !fueled {
        world
            .get_resource_or_insert_with(HeatState::default)
            .burn_remainder = 0.0;
    }

    if gameplay_active(world) && fueled {
        let pooled = world
            .get_resource::<HeatState>()
            .map(|heat| heat.burn_remainder)
            .unwrap_or(0.0)
            + furnaces as f32 * FURNACE_COAL_PER_SECOND * seconds;
        let mut debited = 0;
        let whole = (pooled + BURN_EPSILON).floor() as u32;
        if whole > 0
            && let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
            && let Some(state) = economy.0.get_mut(&TeamId(1))
        {
            debited = whole.min(state.stockpile.coal);
            state.stockpile.coal -= debited;
        }
        let remainder = if team_coal(world) == 0 {
            0.0
        } else {
            pooled - debited as f32
        };
        world
            .get_resource_or_insert_with(HeatState::default)
            .burn_remainder = remainder;
    }

    // The snapshot publish runs in every phase, even where nothing burned.
    let mut heat = world.get_resource_or_insert_with(HeatState::default);
    heat.supply = supply;
    heat.demand = demand;
}

#[cfg(test)]
mod tests;
