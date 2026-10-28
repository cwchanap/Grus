//! Typed gameplay catalogue: the only source of costs, times, footprints,
//! population values, movement speeds, wave tuning, and the gathering rate.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    Food,
    Scrap,
    Coal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnitKind {
    Worker,
    Guard,
    Rifleman,
    Swarmer,
    Spitter,
    Brute,
    Behemoth,
}

impl UnitKind {
    /// Every human and alien unit kind, including kinds that cannot be trained.
    pub const ALL: [UnitKind; 7] = [
        UnitKind::Worker,
        UnitKind::Guard,
        UnitKind::Rifleman,
        UnitKind::Swarmer,
        UnitKind::Spitter,
        UnitKind::Brute,
        UnitKind::Behemoth,
    ];
    /// Human kinds accepted by normal production and the player catalogue.
    pub const HUMAN: [UnitKind; 3] = [UnitKind::Worker, UnitKind::Guard, UnitKind::Rifleman];
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    ResearchCenter,
    Bunkhouse,
    Depot,
    Barracks,
    Wall,
    Gate,
    GunTurret,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 7] = [
        BuildingKind::ResearchCenter,
        BuildingKind::Bunkhouse,
        BuildingKind::Depot,
        BuildingKind::Barracks,
        BuildingKind::Wall,
        BuildingKind::Gate,
        BuildingKind::GunTurret,
    ];
}

/// One price in the four team currencies. `crystals` is stockpile-and-cost
/// surface only: nothing in the winter baseline charges it, and no
/// gatherable source produces it yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cost {
    pub food: u32,
    pub scrap: u32,
    pub coal: u32,
    pub crystals: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitSpec {
    pub cost: Cost,
    pub train_seconds: u32,
    pub speed: f32,
    pub max_health: u32,
    /// `None` for noncombatants (Worker).
    pub combat: Option<CombatSpec>,
    /// Present only on the four alien kinds; normal production stays human-only.
    pub alien: Option<AlienSpec>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlienSpec {
    pub threat: u32,
    pub crystal_reward: u32,
    /// First wave the kind may appear; for Behemoth it doubles as the
    /// every-Nth-wave cadence (10 → waves 10, 20, 30, ...).
    pub first_wave: u64,
}

/// Combat tuning for one attacker. The counter bonus applies only to
/// `counter_target`; every other kind takes base damage.
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
    /// Present on buildings that attack without unit orders.
    pub combat: Option<CombatSpec>,
}

pub const CARRY_LIMIT: u32 = 10;
pub const BASE_GATHER_RATE: f32 = 2.0;
pub const MAX_POPULATION: u32 = 100;
pub const WAVE_PREPARATION_SECONDS: u32 = 300;
pub const WAVE_INTERVAL_SECONDS: u32 = 100;
pub const WAVE_WARNING_SECONDS: u32 = 30;
pub const MAX_LIVE_ALIENS: u32 = 150;
pub const WAVE_BUDGET_BASE: u64 = 8;
pub const WAVE_BUDGET_PER_WAVE: u64 = 4;
pub const WAVE_BUDGET_QUADRATIC_TENTHS: u64 = 4;
pub const WAVE_TWO_ZONE_START: u64 = 10;
pub const WAVE_THREE_ZONE_START: u64 = 20;
/// Fraction of the post-Behemoth wave pool reserved for Swarmers before
/// premium purchases (`remaining / WAVE_SWARMER_RESERVE_DIVISOR`).
pub const WAVE_SWARMER_RESERVE_DIVISOR: u64 = 2;
/// Most waves one fixed step may launch, bounding catch-up work after a
/// huge time-scale delta. The clock clamps to the first unlaunched
/// deadline, so skipped wave indices are absorbed rather than owed
/// forever.
pub const WAVE_CATCH_UP_LIMIT: u64 = 8;
/// Attack-move target-acquisition radius in world units (cells).
pub const ATTACK_MOVE_RADIUS: f32 = 8.0;
/// Initial per-unit vision radius in cells; a reveal covers every cell with
/// `dx*dx + dy*dy <= radius*radius` from its origin. Must never be narrower
/// than `ATTACK_MOVE_RADIUS` (locked by a catalogue regression).
pub const VISION_RADIUS_CELLS: i32 = 10;

pub fn unit_spec(kind: UnitKind) -> UnitSpec {
    match kind {
        UnitKind::Worker => UnitSpec {
            cost: Cost {
                food: 50,
                ..Cost::default()
            },
            train_seconds: 15,
            speed: 6.0,
            max_health: 50,
            combat: None,
            alien: None,
        },
        UnitKind::Guard => UnitSpec {
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
                counter_target: UnitKind::Swarmer,
                counter_bonus: 5,
                ranged: false,
            }),
            alien: None,
        },
        UnitKind::Rifleman => UnitSpec {
            cost: Cost {
                food: 40,
                scrap: 40,
                coal: 0,
                crystals: 0,
            },
            train_seconds: 25,
            speed: 6.0,
            max_health: 70,
            combat: Some(CombatSpec {
                damage: 8,
                attack_range: 6.0,
                cooldown_seconds: 1.25,
                counter_target: UnitKind::Spitter,
                counter_bonus: 4,
                ranged: true,
            }),
            alien: None,
        },
        UnitKind::Swarmer => UnitSpec {
            cost: Cost::default(),
            train_seconds: 0,
            speed: 8.0,
            max_health: 40,
            combat: Some(CombatSpec {
                damage: 6,
                attack_range: 1.5,
                cooldown_seconds: 1.0,
                counter_target: UnitKind::Worker,
                counter_bonus: 0,
                ranged: false,
            }),
            alien: Some(AlienSpec {
                threat: 1,
                crystal_reward: 1,
                first_wave: 1,
            }),
        },
        UnitKind::Spitter => UnitSpec {
            cost: Cost::default(),
            train_seconds: 0,
            speed: 6.0,
            max_health: 70,
            combat: Some(CombatSpec {
                damage: 8,
                attack_range: 6.0,
                cooldown_seconds: 1.5,
                counter_target: UnitKind::Worker,
                counter_bonus: 0,
                ranged: true,
            }),
            alien: Some(AlienSpec {
                threat: 3,
                crystal_reward: 2,
                first_wave: 4,
            }),
        },
        UnitKind::Brute => UnitSpec {
            cost: Cost::default(),
            train_seconds: 0,
            speed: 4.0,
            max_health: 300,
            combat: Some(CombatSpec {
                damage: 30,
                attack_range: 1.5,
                cooldown_seconds: 2.0,
                counter_target: UnitKind::Worker,
                counter_bonus: 0,
                ranged: false,
            }),
            alien: Some(AlienSpec {
                threat: 6,
                crystal_reward: 5,
                first_wave: 8,
            }),
        },
        UnitKind::Behemoth => UnitSpec {
            cost: Cost::default(),
            train_seconds: 0,
            speed: 3.0,
            max_health: 2_000,
            combat: Some(CombatSpec {
                damage: 80,
                attack_range: 1.5,
                cooldown_seconds: 2.0,
                counter_target: UnitKind::Worker,
                counter_bonus: 0,
                ranged: false,
            }),
            alien: Some(AlienSpec {
                threat: 40,
                crystal_reward: 50,
                first_wave: 10,
            }),
        },
    }
}

pub fn building_spec(kind: BuildingKind) -> BuildingSpec {
    match kind {
        BuildingKind::ResearchCenter => BuildingSpec {
            cost: Cost::default(),
            build_seconds: 0,
            width: 4,
            height: 4,
            population_capacity: 10,
            max_health: 800,
            combat: None,
        },
        BuildingKind::Bunkhouse => BuildingSpec {
            cost: Cost {
                scrap: 50,
                ..Cost::default()
            },
            build_seconds: 15,
            width: 2,
            height: 2,
            population_capacity: 10,
            max_health: 250,
            combat: None,
        },
        BuildingKind::Depot => BuildingSpec {
            cost: Cost {
                scrap: 75,
                ..Cost::default()
            },
            build_seconds: 20,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 300,
            combat: None,
        },
        BuildingKind::Barracks => BuildingSpec {
            cost: Cost {
                scrap: 120,
                ..Cost::default()
            },
            build_seconds: 30,
            width: 3,
            height: 3,
            population_capacity: 0,
            max_health: 400,
            combat: None,
        },
        BuildingKind::Wall => BuildingSpec {
            cost: Cost {
                scrap: 10,
                ..Cost::default()
            },
            build_seconds: 5,
            width: 1,
            height: 1,
            population_capacity: 0,
            max_health: 400,
            combat: None,
        },
        BuildingKind::Gate => BuildingSpec {
            cost: Cost {
                scrap: 30,
                ..Cost::default()
            },
            build_seconds: 8,
            width: 1,
            height: 1,
            population_capacity: 0,
            max_health: 600,
            combat: None,
        },
        BuildingKind::GunTurret => BuildingSpec {
            cost: Cost {
                scrap: 100,
                coal: 20,
                ..Cost::default()
            },
            build_seconds: 25,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 350,
            combat: Some(CombatSpec {
                damage: 10,
                attack_range: 8.0,
                cooldown_seconds: 1.0,
                counter_target: UnitKind::Spitter,
                counter_bonus: 5,
                ranged: true,
            }),
        },
    }
}

/// Finite resource node amount for a standalone source: ration cache 600
/// Food, salvage ruin 400 Scrap, coal seam 600 Coal.
pub const fn resource_amount(kind: ResourceKind) -> u32 {
    match kind {
        ResourceKind::Scrap => 400,
        ResourceKind::Food | ResourceKind::Coal => 600,
    }
}

#[cfg(test)]
mod tests;
