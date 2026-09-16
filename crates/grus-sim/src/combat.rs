//! Authoritative combat contracts: health, combat orders with their cached
//! pursuit cell, attack cooldowns, and the current-tick combat event buffer.
//! Damage, pursuit, destruction, and the combat fixed step land with the
//! combat system task.

use bevy::math::Vec2;
use bevy::prelude::{Component, Resource};

use crate::ids::{BuildingId, UnitId};
use crate::map::GridPos;

/// Live and maximum hit points. Spawned units and seeded/placed buildings
/// start at full health from their catalogue spec.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

/// Combat identity of an attackable target, stable across pursuit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatTarget {
    Unit(UnitId),
    Building(BuildingId),
}

/// Current combat intent of a unit. The last known target cell is cached
/// here — there is no separate pursuit component.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum CombatOrder {
    Attack {
        target: CombatTarget,
        last_target_cell: Option<GridPos>,
    },
    AttackMove {
        destination: GridPos,
        target: Option<CombatTarget>,
        last_target_cell: Option<GridPos>,
    },
}

/// Seconds until the next attack is ready; zero means ready.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackCooldown(pub f32);

/// One hit recorded during a single combat tick: attacker/target identity,
/// damage, hit position, ranged/melee, and whether the target died.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatEvent {
    pub attacker: UnitId,
    pub target: CombatTarget,
    pub damage: u32,
    pub position: Vec2,
    pub ranged: bool,
    pub killed: bool,
}

/// Sim-owned drain buffer of current-tick combat events, cleared at the
/// start of every combat step so headless runs cannot accumulate stale
/// events. The bridge drains it during presentation.
#[derive(Debug, Default, Resource)]
pub struct CombatEvents(pub Vec<CombatEvent>);
