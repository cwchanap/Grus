use std::collections::HashSet;
use std::ops::Range;

use bevy::math::Vec2;
use bevy::prelude::{Component, Resource, World};

use crate::buildings::{Building, BuildingIndex};
use crate::catalog::{
    ATTACK_MOVE_RADIUS, BuildingKind, MAX_LIVE_ALIENS, UnitKind, WAVE_BUDGET_BASE,
    WAVE_BUDGET_PER_WAVE, WAVE_BUDGET_QUADRATIC_TENTHS, WAVE_CATCH_UP_LIMIT, WAVE_INTERVAL_SECONDS,
    WAVE_PREPARATION_SECONDS, WAVE_SWARMER_RESERVE_DIVISOR, WAVE_THREE_ZONE_START,
    WAVE_TWO_ZONE_START, WAVE_WARNING_SECONDS, unit_spec,
};
use crate::combat::{CombatOrder, Health};
use crate::commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex,
    apply_player_command, approach_slots, destination_slots, sorted_unit_ids, spawn_unit,
};
use crate::fixture::{ENTRY_ZONES, EntryZone};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition};
use crate::session::gameplay_active;
use crate::visibility::reveal_for_team;

const TIMER_BOUNDARY_EPSILON_SECONDS: f64 = 0.0001;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WaveRoster {
    pub behemoths: u64,
    pub brutes: u64,
    pub spitters: u64,
    pub swarmers: u64,
}

impl WaveRoster {
    pub fn total_threat(self) -> u64 {
        self.behemoths
            .saturating_mul(u64::from(
                unit_spec(UnitKind::Behemoth).alien.unwrap().threat,
            ))
            .saturating_add(
                self.brutes
                    .saturating_mul(u64::from(unit_spec(UnitKind::Brute).alien.unwrap().threat)),
            )
            .saturating_add(self.spitters.saturating_mul(u64::from(
                unit_spec(UnitKind::Spitter).alien.unwrap().threat,
            )))
            .saturating_add(self.swarmers.saturating_mul(u64::from(
                unit_spec(UnitKind::Swarmer).alien.unwrap().threat,
            )))
    }

    pub fn total_units(self) -> u64 {
        self.behemoths
            .saturating_add(self.brutes)
            .saturating_add(self.spitters)
            .saturating_add(self.swarmers)
    }

    /// Prefix in cap priority order, bounded by the live alien cap.
    pub fn ordered_prefix(self, limit: usize) -> Vec<UnitKind> {
        let limit = limit.min(MAX_LIVE_ALIENS as usize);
        let mut remaining = u64::try_from(limit)
            .unwrap_or(u64::MAX)
            .min(self.total_units());
        let mut result = Vec::with_capacity(usize::try_from(remaining).unwrap_or(usize::MAX));
        for (kind, count) in [
            (UnitKind::Behemoth, self.behemoths),
            (UnitKind::Brute, self.brutes),
            (UnitKind::Spitter, self.spitters),
            (UnitKind::Swarmer, self.swarmers),
        ] {
            let take = count.min(remaining);
            result.extend(std::iter::repeat_n(
                kind,
                usize::try_from(take).unwrap_or(usize::MAX),
            ));
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
        result
    }
}

/// Integer budget for a 1-based wave, with a widened intermediate for w².
pub fn wave_budget(wave: u64) -> u64 {
    let wave = u128::from(wave.max(1));
    let budget = u128::from(WAVE_BUDGET_BASE)
        .saturating_add(u128::from(WAVE_BUDGET_PER_WAVE).saturating_mul(wave))
        .saturating_add(
            u128::from(WAVE_BUDGET_QUADRATIC_TENTHS).saturating_mul(wave.saturating_mul(wave)) / 10,
        );
    u64::try_from(budget).unwrap_or(u64::MAX)
}

/// Deterministic threat-budget purchases and the resulting spawn counts.
pub fn plan_roster(wave: u64) -> WaveRoster {
    let wave = wave.max(1);
    let mut remaining = wave_budget(wave);
    let mut roster = WaveRoster::default();

    let behemoth = unit_spec(UnitKind::Behemoth).alien.unwrap();
    if wave.is_multiple_of(behemoth.first_wave) {
        roster.behemoths = 1;
        remaining = remaining.saturating_sub(u64::from(behemoth.threat));
    }

    let reserved_swarmers = remaining / WAVE_SWARMER_RESERVE_DIVISOR;
    let mut premium_budget = remaining - reserved_swarmers;
    let brute = unit_spec(UnitKind::Brute).alien.unwrap();
    let spitter = unit_spec(UnitKind::Spitter).alien.unwrap();
    match (wave >= brute.first_wave, wave >= spitter.first_wave) {
        (true, true) => {
            let pair_cost = u64::from(brute.threat).saturating_add(u64::from(spitter.threat));
            let pairs = premium_budget / pair_cost;
            roster.brutes = pairs;
            roster.spitters = pairs;
            premium_budget %= pair_cost;
            if premium_budget >= u64::from(brute.threat) {
                roster.brutes += 1;
                premium_budget -= u64::from(brute.threat);
            }
            if premium_budget >= u64::from(spitter.threat) {
                roster.spitters += 1;
                premium_budget -= u64::from(spitter.threat);
            }
        }
        (false, true) => {
            let cost = u64::from(spitter.threat);
            roster.spitters = premium_budget / cost;
            premium_budget %= cost;
        }
        (true, false) => {
            let cost = u64::from(brute.threat);
            roster.brutes = premium_budget / cost;
            premium_budget %= cost;
        }
        (false, false) => {}
    }

    roster.swarmers = reserved_swarmers + premium_budget;
    roster
}

/// Zones for one wave, starting at the rotating cardinal entry direction.
pub fn zones_for(wave: u64) -> Vec<EntryZone> {
    let wave = wave.max(1);
    let count = if wave >= WAVE_THREE_ZONE_START {
        3
    } else if wave >= WAVE_TWO_ZONE_START {
        2
    } else {
        1
    };
    let start = ((wave - 1) % ENTRY_ZONES.len() as u64) as usize;
    (0..count)
        .map(|offset| ENTRY_ZONES[(start + offset) % ENTRY_ZONES.len()].0)
        .collect()
}

/// Minimal wave clock. Warnings and due launches are derived from these two
/// values, so no announcement state can drift from the schedule.
#[derive(Clone, Copy, Debug, Resource, PartialEq)]
pub struct WaveState {
    pub elapsed: f64,
    pub next_wave: u64,
}

#[derive(Component)]
struct FailedFallbackRevision(u64);

/// The Team-1 building an accepted reachable-structure fallback sent this
/// alien toward. Gives the retry predicate objective identity for a
/// targetless `AttackMove`: an alien standing on its accepted fallback
/// destination inside the living structure's acquisition reach is still
/// tasked, so retry cannot replace the order before fog acquisition.
/// Cleared by any accepted non-fallback dispatch; a dead recorded
/// structure leaves the alien retry-eligible so survivors retask.
#[derive(Component)]
struct FallbackObjective(BuildingId);

impl Default for WaveState {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            next_wave: 1,
        }
    }
}

impl WaveState {
    /// Advances only during Playing and returns each newly due 1-based wave.
    pub fn advance(&mut self, world: &World, seconds: f32) -> Range<u64> {
        let first = self.next_wave;
        if gameplay_active(world) && seconds.is_finite() && seconds > 0.0 {
            self.elapsed += f64::from(seconds);
            while self.next_wave < u64::MAX
                && self.next_wave - first < WAVE_CATCH_UP_LIMIT
                && self.elapsed + TIMER_BOUNDARY_EPSILON_SECONDS >= wave_deadline(self.next_wave)
            {
                self.next_wave += 1;
            }
            // A pathological delta cannot owe unbounded catch-up: clamp the
            // clock to the first unlaunched deadline so the skipped indices
            // are absorbed instead of draining at the limit forever.
            self.elapsed = self.elapsed.min(wave_deadline(self.next_wave));
        }
        first..self.next_wave
    }

    pub fn remaining_seconds(self) -> f64 {
        (wave_deadline(self.next_wave) - self.elapsed).max(0.0)
    }

    pub fn incoming_zones(self) -> Option<Vec<EntryZone>> {
        let remaining = self.remaining_seconds();
        (remaining > 0.0
            && remaining <= f64::from(WAVE_WARNING_SECONDS) + TIMER_BOUNDARY_EPSILON_SECONDS)
            .then(|| zones_for(self.next_wave))
    }
}

pub(crate) fn wave_deadline(wave: u64) -> f64 {
    f64::from(WAVE_PREPARATION_SECONDS)
        + wave.saturating_sub(1) as f64 * f64::from(WAVE_INTERVAL_SECONDS)
}

/// Advances the pure-simulation wave clock, launches each newly due wave,
/// and retries stranded aliens at most once per elapsed simulation second.
pub fn step_waves(world: &mut World, map: &mut GridMap, seconds: f32) {
    let previous = world
        .get_resource::<WaveState>()
        .copied()
        .unwrap_or_default();
    let mut state = previous;
    let due_waves = state.advance(world, seconds);
    let retry_due = state.elapsed.floor() > previous.elapsed.floor();
    world.insert_resource(state);

    if retry_due {
        retry_stranded_aliens(world, map);
    }
    for wave in due_waves {
        launch_wave(world, map, wave);
    }
}

fn launch_wave(world: &mut World, map: &mut GridMap, wave: u64) {
    let roster = plan_roster(wave);
    let planned = roster.ordered_prefix(MAX_LIVE_ALIENS as usize);
    let live = live_alien_ids(world, TeamId(2)).len();
    let room = (MAX_LIVE_ALIENS as usize).saturating_sub(live);
    let spawn_order: Vec<UnitKind> = planned.into_iter().take(room).collect();
    if spawn_order.is_empty() {
        return;
    }

    let prefix_threat = spawn_order.iter().fold(0_u64, |total, kind| {
        total.saturating_add(u64::from(unit_spec(*kind).alien.unwrap().threat))
    });
    let overflow_threat = roster.total_threat().saturating_sub(prefix_threat);
    let zones = zones_for(wave);
    let mut occupied = live_unit_cells(world, map);
    let mut spawned = Vec::with_capacity(spawn_order.len());
    let mut spawn_cells = Vec::with_capacity(spawn_order.len());

    for (index, kind) in spawn_order.iter().copied().enumerate() {
        let zone = zones[index % zones.len()];
        let Some(cell) = destination_slots(map, zone.anchor(), 1, &occupied)
            .into_iter()
            .next()
        else {
            break;
        };
        occupied.insert(cell);

        let Some(id) = world
            .get_resource_mut::<IdAllocator>()
            .map(|mut allocator| allocator.allocate_unit())
        else {
            break;
        };
        let spec = unit_spec(kind);
        let entity = spawn_unit(
            world,
            id,
            TeamId(2),
            map.cell_center(cell),
            kind,
            spec.speed,
        );
        spawned.push(id);
        spawn_cells.push(cell);
        if overflow_threat > 0 {
            let max_health = scaled_health(spec.max_health, overflow_threat, prefix_threat);
            world.entity_mut(entity).insert(Health {
                current: max_health,
                max: max_health,
            });
        }
    }

    // New aliens see their surroundings immediately: the end-of-tick
    // refresh is the first refresh that would otherwise observe these
    // origins, and same-tick combat acquisition must not wait on it.
    reveal_for_team(world, map, TeamId(2), &spawn_cells);

    let objective = alien_objective(world, map);
    for (index, id) in spawned.into_iter().enumerate() {
        let target = objective
            .as_ref()
            .and_then(|(_, targets)| targets.get(index % targets.len()))
            .copied();
        match target {
            Some(target) => {
                let result = dispatch_attack_move(world, map, id, target);
                if command_accepted(&result, id) {
                    clear_failed_fallback_revision(world, id);
                    clear_fallback_objective(world, id);
                } else if command_rejected_unreachable(&result, id) {
                    // The objective dispatch and its rejection are one transaction;
                    // preserve the failed MoveOrder until a fallback is accepted.
                    try_reachable_structure_fallback(world, map, id);
                }
            }
            None => try_reachable_structure_fallback(world, map, id),
        }
    }
}

fn retry_stranded_aliens(world: &mut World, map: &mut GridMap) {
    let objective = alien_objective(world, map);
    let mut target_index = 0;
    for id in live_alien_ids(world, TeamId(2)) {
        let Some(entity) = world
            .get_resource::<UnitIndex>()
            .and_then(|index| index.entity(id))
        else {
            continue;
        };
        if let Some(failed) = world.get::<FailedFallbackRevision>(entity) {
            if failed.0 == map.revision() {
                continue;
            }
            world.entity_mut(entity).remove::<FailedFallbackRevision>();
        }
        let Some(position) = world.get::<SimPosition>(entity) else {
            continue;
        };
        let failed_route = world
            .get::<MoveOrder>(entity)
            .is_some_and(|order| order.last_failed_replan == Some(map.revision()));
        let combat_order = world.get::<CombatOrder>(entity);
        // An accepted fallback `AttackMove` stays targetless until combat
        // acquisition sees the structure: while the alien waits on the
        // recorded destination inside the living structure's acquisition
        // reach, it is tasked — not stranded — and retry must not replace
        // the order pre-acquisition. A destination substituted outside
        // that reach is a dead route end, so the parked alien stays
        // retry-eligible.
        let awaiting_fallback = matches!(
            combat_order,
            Some(CombatOrder::AttackMove {
                destination,
                target: None,
                ..
            }) if *destination == map.world_to_cell(position.current)
        ) && world.get::<FallbackObjective>(entity).is_some_and(
            |objective| fallback_objective_in_range(world, objective.0, position.current),
        );
        if matches!(combat_order, Some(CombatOrder::Attack { .. }))
            || (!failed_route
                && (world.get::<MoveOrder>(entity).is_some()
                    || matches!(
                        combat_order,
                        Some(CombatOrder::AttackMove {
                            target: Some(_),
                            ..
                        })
                    )
                    || awaiting_fallback))
        {
            continue;
        }
        let in_core_range = objective.as_ref().is_some_and(|(standoff, _)| {
            position
                .current
                .distance(standoff.closest_point(position.current))
                <= ATTACK_MOVE_RADIUS
        });
        if !failed_route && in_core_range {
            continue;
        }

        let target = objective
            .as_ref()
            .and_then(|(_, targets)| targets.get(target_index % targets.len()))
            .copied();
        if let Some(target) = target {
            let result = dispatch_attack_move(world, map, id, target);
            target_index += 1;
            if command_accepted(&result, id) {
                clear_failed_fallback_revision(world, id);
                clear_fallback_objective(world, id);
            } else if command_rejected_unreachable(&result, id) {
                try_reachable_structure_fallback(world, map, id);
            }
        } else {
            try_reachable_structure_fallback(world, map, id);
        }
    }
}

fn try_reachable_structure_fallback(world: &mut World, map: &mut GridMap, id: UnitId) {
    let Some(entity) = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(id))
    else {
        return;
    };
    let Some(position) = world.get::<SimPosition>(entity).copied() else {
        return;
    };
    if world
        .get::<FailedFallbackRevision>(entity)
        .is_some_and(|failed| failed.0 == map.revision())
    {
        return;
    }

    let mut buildings: Vec<(f32, BuildingId, Footprint)> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(building_id, building_entity)| {
                    let building = world.get::<Building>(*building_entity)?;
                    let footprint = world.get::<Footprint>(*building_entity)?;
                    (building.team == TeamId(1)).then_some((
                        footprint
                            .closest_point(position.current)
                            .distance_squared(position.current),
                        *building_id,
                        *footprint,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    buildings.sort_unstable_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });

    let start = map.world_to_cell(position.current);
    for (_, building_id, footprint) in buildings {
        let slots = approach_slots(
            map,
            footprint,
            &HashSet::new(),
            footprint.perimeter_cells().len(),
        );
        let reachable_slot = slots
            .into_iter()
            .find(|slot| map.find_path(TeamId(2), start, *slot).is_some());
        if let Some(slot) = reachable_slot {
            if command_accepted(&dispatch_attack_move(world, map, id, slot), id) {
                clear_failed_fallback_revision(world, id);
                world
                    .entity_mut(entity)
                    .insert(FallbackObjective(building_id));
            }
            // The nearest statically reachable structure is selected. A
            // command rejection here is transient crowding, so leave the
            // alien eligible for the next scheduled retry.
            return;
        }
    }

    world
        .entity_mut(entity)
        .insert(FailedFallbackRevision(map.revision()));
}

fn command_accepted(result: &CommandResult, id: UnitId) -> bool {
    result.accepted_units.contains(&id)
}

fn command_rejected_unreachable(result: &CommandResult, id: UnitId) -> bool {
    result
        .rejected_units
        .contains(&(id, RejectReason::Unreachable))
}

fn clear_failed_fallback_revision(world: &mut World, id: UnitId) {
    if let Some(entity) = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(id))
    {
        world.entity_mut(entity).remove::<FailedFallbackRevision>();
    }
}

fn clear_fallback_objective(world: &mut World, id: UnitId) {
    if let Some(entity) = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(id))
    {
        world.entity_mut(entity).remove::<FallbackObjective>();
    }
}

/// A parked fallback waiter counts as tasked only while its recorded
/// structure is alive AND inside the waiting `AttackMove`'s acquisition
/// reach — the same footprint closest-point metric `acquire_target` uses.
/// Reservation crowding can push `plan_move_route`'s candidate scan past
/// the perimeter and anchor the accepted goal beyond that reach: holding
/// such an alien out of retry would strand it forever, so the dead route
/// end stays retry-eligible. Live means indexed and not yet destroyed —
/// a `Health` of zero counts as dead even if the atomic destroy has not
/// reclaimed the entity.
fn fallback_objective_in_range(world: &World, id: BuildingId, position: Vec2) -> bool {
    world
        .get_resource::<BuildingIndex>()
        .and_then(|index| index.entity(id))
        .is_some_and(|entity| {
            world
                .get::<Health>(entity)
                .is_some_and(|health| health.current > 0)
                && world.get::<Footprint>(entity).is_some_and(|footprint| {
                    position.distance(footprint.closest_point(position)) <= ATTACK_MOVE_RADIUS
                })
        })
}

/// The director's live objective: the standoff footprint stranded aliens
/// measure "already engaged" against, plus the approach slots new orders
/// target. The lowest-id team-1 Research Center leads while it stands;
/// without it the lowest-id team-1 building — then the lowest-id live
/// team-1 unit's cell — keeps the horde tasked, so a session-free world
/// (the benchmark) retains alien intent after the core dies.
fn alien_objective(world: &World, map: &GridMap) -> Option<(Footprint, Vec<GridPos>)> {
    let mut buildings: Vec<(bool, BuildingId, Footprint)> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let building = world.get::<Building>(*entity)?;
                    let footprint = world.get::<Footprint>(*entity)?;
                    (building.team == TeamId(1)).then_some((
                        building.kind != BuildingKind::ResearchCenter,
                        *id,
                        *footprint,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    buildings.sort_unstable_by_key(|(is_other, id, _)| (*is_other, *id));
    for (is_other, _, footprint) in buildings {
        let slots = objective_slots(map, footprint);
        if !is_other {
            // A standing core with no usable perimeter slot is fallback-
            // eligible. Returning `None` sends launch and retry through the
            // distance-ranked structure probe instead of selecting the
            // lowest-ID non-core building as a substitute core objective.
            return (!slots.is_empty()).then_some((footprint, slots));
        }
        if !slots.is_empty() {
            return Some((footprint, slots));
        }
    }
    for id in sorted_unit_ids(world, TeamId(1), |_| true) {
        let Some(position) = world
            .get_resource::<UnitIndex>()
            .and_then(|index| index.entity(id))
            .and_then(|entity| world.get::<SimPosition>(entity))
        else {
            continue;
        };
        let footprint = Footprint::new(map.world_to_cell(position.current), 1, 1);
        let slots = objective_slots(map, footprint);
        if !slots.is_empty() {
            return Some((footprint, slots));
        }
    }
    None
}

fn objective_slots(map: &GridMap, footprint: Footprint) -> Vec<GridPos> {
    approach_slots(
        map,
        footprint,
        &HashSet::new(),
        footprint.perimeter_cells().len(),
    )
}

fn dispatch_attack_move(
    world: &mut World,
    map: &mut GridMap,
    id: UnitId,
    target: GridPos,
) -> CommandResult {
    let target = map.cell_center(target);
    apply_player_command(
        world,
        map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(2),
            units: vec![id],
            kind: UnitCommandKind::AttackMove { target },
        }),
    )
}

/// Live alien ids for `team` in stable-id order — the one census shared by
/// the launch cap, the stranded retry, bridge snapshots, and tests.
pub fn live_alien_ids(world: &World, team: TeamId) -> Vec<UnitId> {
    sorted_unit_ids(world, team, |unit| unit_spec(unit.kind).alien.is_some())
}

fn live_unit_cells(world: &World, map: &GridMap) -> HashSet<GridPos> {
    world
        .get_resource::<UnitIndex>()
        .into_iter()
        .flat_map(|index| index.iter())
        .filter_map(|(_, entity)| {
            world
                .get::<SimPosition>(*entity)
                .map(|position| map.world_to_cell(position.current))
        })
        .collect()
}

fn scaled_health(base: u32, overflow: u64, spawned: u64) -> u32 {
    if overflow == 0 || spawned == 0 {
        return base;
    }
    let total_points = u128::from(spawned) + u128::from(overflow);
    let denominator = u128::from(spawned);
    let scaled = (u128::from(base) * total_points + denominator / 2) / denominator;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
