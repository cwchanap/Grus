use std::ops::Range;

use bevy::prelude::{Resource, World};

use crate::catalog::{
    MAX_LIVE_ALIENS, UnitKind, WAVE_BUDGET_BASE, WAVE_BUDGET_PER_WAVE,
    WAVE_BUDGET_QUADRATIC_TENTHS, WAVE_INTERVAL_SECONDS, WAVE_PREPARATION_SECONDS,
    WAVE_THREE_ZONE_START, WAVE_TWO_ZONE_START, WAVE_WARNING_SECONDS, unit_spec,
};
use crate::fixture::{ENTRY_ZONES, EntryZone};
use crate::session::gameplay_active;

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

    if wave.is_multiple_of(10) {
        roster.behemoths = 1;
        remaining = remaining.saturating_sub(u64::from(
            unit_spec(UnitKind::Behemoth).alien.unwrap().threat,
        ));
    }

    let reserved_swarmers = remaining / 2;
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
                && self.elapsed + TIMER_BOUNDARY_EPSILON_SECONDS >= wave_deadline(self.next_wave)
            {
                self.next_wave += 1;
            }
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

fn wave_deadline(wave: u64) -> f64 {
    f64::from(WAVE_PREPARATION_SECONDS)
        + wave.saturating_sub(1) as f64 * f64::from(WAVE_INTERVAL_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::World;

    use crate::ids::TeamId;
    use crate::session::{MatchPhase, MatchSession, resolve_result, set_paused, start_match};

    #[test]
    fn integer_wave_budgets_and_ordered_rosters_are_pinned() {
        assert_eq!(wave_budget(1), 12);
        assert_eq!(wave_budget(4), 30);
        assert_eq!(wave_budget(8), 65);
        assert_eq!(wave_budget(10), 88);

        let wave1 = plan_roster(1);
        assert_eq!(wave1.swarmers, 12);
        assert_eq!(wave1.total_threat(), 12);

        let wave4 = plan_roster(4);
        assert_eq!(
            (
                wave4.behemoths,
                wave4.brutes,
                wave4.spitters,
                wave4.swarmers
            ),
            (0, 0, 5, 15)
        );
        assert_eq!(wave4.total_threat(), 30);

        let wave8 = plan_roster(8);
        assert_eq!(
            (
                wave8.behemoths,
                wave8.brutes,
                wave8.spitters,
                wave8.swarmers
            ),
            (0, 4, 3, 32)
        );
        assert_eq!(wave8.total_threat(), 65);

        let wave10 = plan_roster(10);
        assert_eq!(
            (
                wave10.behemoths,
                wave10.brutes,
                wave10.spitters,
                wave10.swarmers
            ),
            (1, 3, 2, 24)
        );
        assert_eq!(wave10.total_threat(), 88);
        assert_eq!(
            wave10.ordered_prefix(7),
            [
                UnitKind::Behemoth,
                UnitKind::Brute,
                UnitKind::Brute,
                UnitKind::Brute,
                UnitKind::Spitter,
                UnitKind::Spitter,
                UnitKind::Swarmer,
            ]
        );

        let capped_wave = plan_roster(100).ordered_prefix(MAX_LIVE_ALIENS as usize);
        assert_eq!(capped_wave.len(), MAX_LIVE_ALIENS as usize);
        assert_eq!(capped_wave[0], UnitKind::Behemoth);
        assert!(capped_wave[1..].iter().all(|kind| *kind == UnitKind::Brute));
    }

    #[test]
    fn roster_unlocks_and_budget_overflow_are_deterministic() {
        let wave1 = plan_roster(1);
        assert_eq!((wave1.brutes, wave1.spitters, wave1.behemoths), (0, 0, 0));

        let wave4 = plan_roster(4);
        assert_eq!((wave4.brutes, wave4.behemoths), (0, 0));
        assert!(wave4.spitters > 0);

        let wave8 = plan_roster(8);
        assert!(wave8.brutes > 0);
        assert_eq!(wave8.behemoths, 0);

        assert_eq!(wave_budget(u64::MAX), u64::MAX);
        let overflow_roster = plan_roster(u64::MAX);
        assert_eq!(overflow_roster.total_threat(), u64::MAX);
        let capped_prefix = overflow_roster.ordered_prefix(usize::MAX);
        assert_eq!(capped_prefix.len(), MAX_LIVE_ALIENS as usize);
        assert!(capped_prefix.iter().all(|kind| *kind == UnitKind::Brute));
    }

    #[test]
    fn zones_rotate_at_the_two_and_three_zone_breakpoints() {
        assert_eq!(zones_for(9), [EntryZone::North]);
        assert_eq!(zones_for(10), [EntryZone::East, EntryZone::South]);
        assert_eq!(zones_for(19), [EntryZone::South, EntryZone::West]);
        assert_eq!(
            zones_for(20),
            [EntryZone::West, EntryZone::North, EntryZone::East]
        );
    }

    #[test]
    fn timer_warning_and_launch_edges_are_derived_and_exactly_once() {
        let world = World::new();
        let mut state = WaveState::default();

        assert_eq!(state.advance(&world, 269.0), 1..1);
        assert_eq!(state.remaining_seconds(), 31.0);
        assert_eq!(state.incoming_zones(), None);

        assert_eq!(state.advance(&world, 1.0), 1..1);
        assert_eq!(state.remaining_seconds(), 30.0);
        assert_eq!(state.incoming_zones(), Some(vec![EntryZone::North]));

        assert_eq!(state.advance(&world, 30.0), 1..2);
        assert_eq!(state.next_wave, 2);
        assert_eq!(state.remaining_seconds(), 100.0);
        assert_eq!(state.incoming_zones(), None);
        assert_eq!(state.advance(&world, 0.05), 2..2);

        assert_eq!(state.advance(&world, 200.0), 2..4);
        assert_eq!(state.next_wave, 4);
        assert_eq!(state.advance(&world, 0.0), 4..4);
    }

    #[test]
    fn timer_freezes_outside_playing_and_uses_the_session_free_playing_convention() {
        let mut world = World::new();
        world.insert_resource(MatchSession {
            phase: MatchPhase::Start,
        });
        let mut state = WaveState::default();
        assert_eq!(state.advance(&world, 100.0), 1..1);
        assert_eq!(state.elapsed, 0.0);

        start_match(&mut world);
        assert_eq!(state.advance(&world, 100.0), 1..1);
        assert_eq!(state.elapsed, 100.0);

        set_paused(&mut world, true);
        assert_eq!(state.advance(&world, 100.0), 1..1);
        assert_eq!(state.elapsed, 100.0);
        set_paused(&mut world, false);
        assert_eq!(state.advance(&world, 200.0), 1..2);

        resolve_result(&mut world, TeamId(1));
        assert_eq!(state.advance(&world, 100.0), 2..2);
        assert_eq!(state.elapsed, 300.0);

        let mut session_free = WaveState::default();
        assert_eq!(session_free.advance(&World::new(), 300.0), 1..2);
    }

    #[test]
    fn timer_rejects_non_finite_or_negative_deltas_without_rewinding() {
        let world = World::new();
        let mut state = WaveState::default();
        state.advance(&world, 12.0);
        assert_eq!(state.advance(&world, -5.0), 1..1);
        assert_eq!(state.advance(&world, f32::NAN), 1..1);
        assert_eq!(state.elapsed, 12.0);
    }
}
