//! Typed gameplay catalogue: the only source of costs, times, footprints,
//! population values, movement speeds, and the gathering rate.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    Food,
    Wood,
    Gold,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnitKind {
    Villager,
    Spearman,
    Archer,
}

impl UnitKind {
    pub const ALL: [UnitKind; 3] = [UnitKind::Villager, UnitKind::Spearman, UnitKind::Archer];
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    TownCenter,
    House,
    Storehouse,
    Barracks,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 4] = [
        BuildingKind::TownCenter,
        BuildingKind::House,
        BuildingKind::Storehouse,
        BuildingKind::Barracks,
    ];
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cost {
    pub food: u32,
    pub wood: u32,
    pub gold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitSpec {
    pub cost: Cost,
    pub train_seconds: u32,
    pub speed: f32,
    pub max_health: u32,
    /// `None` for noncombatants (Villager).
    pub combat: Option<CombatSpec>,
}

/// Combat tuning of one military unit. The counter bonus applies only to
/// `counter_target`; every other kind takes base damage. The deletion of the
/// PvP roster left no counter relationships: every surviving bonus is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatSpec {
    pub damage: u32,
    pub attack_range: f32,
    pub cooldown_seconds: f32,
    pub counter_target: UnitKind,
    pub counter_bonus: u32,
    pub ranged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildingSpec {
    pub cost: Cost,
    pub build_seconds: u32,
    pub width: u8,
    pub height: u8,
    pub population_capacity: u32,
    /// Construction sites use this full health from placement; progress
    /// never scales it.
    pub max_health: u32,
}

pub const CARRY_LIMIT: u32 = 10;
pub const BASE_GATHER_RATE: f32 = 2.0;
pub const MAX_POPULATION: u32 = 100;
/// Attack-move target-acquisition radius in world units (cells).
pub const ATTACK_MOVE_RADIUS: f32 = 8.0;
/// Initial per-unit vision radius in cells; a reveal covers every cell with
/// `dx*dx + dy*dy <= radius*radius` from its origin. Must never be narrower
/// than `ATTACK_MOVE_RADIUS` (locked by a catalogue regression).
pub const VISION_RADIUS_CELLS: i32 = 10;

pub fn unit_spec(kind: UnitKind) -> UnitSpec {
    match kind {
        UnitKind::Villager => UnitSpec {
            cost: Cost {
                food: 50,
                ..Cost::default()
            },
            train_seconds: 15,
            speed: 6.0,
            max_health: 50,
            combat: None,
        },
        UnitKind::Spearman => UnitSpec {
            cost: Cost {
                food: 60,
                ..Cost::default()
            },
            train_seconds: 20,
            speed: 6.0,
            max_health: 100,
            combat: Some(CombatSpec {
                damage: 10,
                attack_range: 1.5,
                cooldown_seconds: 1.0,
                counter_target: UnitKind::Archer,
                counter_bonus: 0,
                ranged: false,
            }),
        },
        UnitKind::Archer => UnitSpec {
            cost: Cost {
                food: 40,
                wood: 40,
                gold: 0,
            },
            train_seconds: 25,
            speed: 6.0,
            max_health: 70,
            combat: Some(CombatSpec {
                damage: 8,
                attack_range: 6.0,
                cooldown_seconds: 1.25,
                counter_target: UnitKind::Spearman,
                counter_bonus: 0,
                ranged: true,
            }),
        },
    }
}

pub fn building_spec(kind: BuildingKind) -> BuildingSpec {
    match kind {
        BuildingKind::TownCenter => BuildingSpec {
            cost: Cost::default(),
            build_seconds: 0,
            width: 4,
            height: 4,
            population_capacity: 10,
            max_health: 800,
        },
        BuildingKind::House => BuildingSpec {
            cost: Cost {
                wood: 50,
                ..Cost::default()
            },
            build_seconds: 15,
            width: 2,
            height: 2,
            population_capacity: 10,
            max_health: 250,
        },
        BuildingKind::Storehouse => BuildingSpec {
            cost: Cost {
                wood: 75,
                ..Cost::default()
            },
            build_seconds: 20,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 300,
        },
        BuildingKind::Barracks => BuildingSpec {
            cost: Cost {
                wood: 120,
                ..Cost::default()
            },
            build_seconds: 30,
            width: 3,
            height: 3,
            population_capacity: 0,
            max_health: 400,
        },
    }
}

/// Finite resource node amount for a standalone source: berries 600 Food,
/// tree 400 Wood, gold deposit 600 Gold.
pub const fn resource_amount(kind: ResourceKind) -> u32 {
    match kind {
        ResourceKind::Wood => 400,
        ResourceKind::Food | ResourceKind::Gold => 600,
    }
}

#[cfg(test)]
mod tests;
