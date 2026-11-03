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

/// One underground room kind buildable in a completed level's slots.
/// Availability is a `min_level` floor (see `RoomSpec`), never an
/// exact-level assignment; repeated kinds are allowed in separate slots.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RoomKind {
    Shelter,
    Greenhouse,
    CoalVein,
    GeothermalTap,
    Lab,
    DeepMine,
}

impl RoomKind {
    /// Every room kind, in catalogue order.
    pub const ALL: [RoomKind; 6] = [
        RoomKind::Shelter,
        RoomKind::Greenhouse,
        RoomKind::CoalVein,
        RoomKind::GeothermalTap,
        RoomKind::Lab,
        RoomKind::DeepMine,
    ];
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
    CoalFurnace,
    FlameTower,
    CrystalLance,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 10] = [
        BuildingKind::ResearchCenter,
        BuildingKind::Bunkhouse,
        BuildingKind::Depot,
        BuildingKind::Barracks,
        BuildingKind::Wall,
        BuildingKind::Gate,
        BuildingKind::GunTurret,
        BuildingKind::CoalFurnace,
        BuildingKind::FlameTower,
        BuildingKind::CrystalLance,
    ];
}

/// One price in the four team currencies. `crystals` is stockpile-and-cost
/// surface only: `ANALYZE_COST` and the Crystal Lance are the only charges,
/// and no gatherable source produces them yet (they drop as combat kill
/// rewards).
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

/// One underground room's catalogue row. Prices are fixed by kind, never
/// scaled by depth. This table is the only room-rule/price source; Godot
/// reads it through the catalogue snapshot rather than replicating it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomSpec {
    /// First completed depth the kind may be built in.
    pub min_level: u32,
    pub cost: Cost,
    pub build_seconds: u32,
    /// Nonperiodic population grant on completion (Shelter).
    pub population_capacity: u32,
    /// Periodic stockpile output once complete, per simulated second.
    pub output: Option<(ResourceKind, f32)>,
    /// Research points per second once complete (Lab); unlike the surface
    /// source this is never scaled by cold.
    pub research_per_second: f64,
    /// Geothermal heat supply at `min_level`, plus
    /// `heat_supply_per_level` for each deeper level (0 elsewhere).
    pub heat_supply_base: u32,
    pub heat_supply_per_level: u32,
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
    /// Completed-building heat contribution. Furnace supply counts only
    /// while fueled; construction sites never count at all.
    pub heat_supply: u32,
    pub heat_demand: u32,
    /// Share of `RESEARCH_TARGET_POINTS` required before placement unlocks
    /// (0 = always available). The point threshold derives through
    /// `research::unlock_points`; there is no per-kind point table.
    pub research_unlock_percent: u32,
    /// Only a `TeamId(1)` issuer may place this kind: a Team-2 Gate would
    /// seal its own team while passing Team 1, the combat pass fires only
    /// Team-1 towers, and the one-player winter fixture has no Team-2
    /// furnace economy. `false` kinds stay placeable by any team. The flag
    /// lives in the catalogue so a new tower can never be placeable by
    /// Team 2 while remaining unfireable.
    pub team1_only: bool,
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
/// Burn rate of one completed, fueled Coal Furnace in Coal per simulated
/// second, pooled fractionally in `HeatState` and debited in whole units.
pub const FURNACE_COAL_PER_SECOND: f32 = 0.5;
/// Surface efficiency floor under a severe heat deficit.
pub const MIN_SURFACE_EFFICIENCY: f32 = 0.25;
/// Passive research rate of the seeded Research Center at full surface
/// efficiency; `research_rate` scales it by the live `surface_efficiency`,
/// so a cold surface slows research exactly like production and turret
/// cooldowns.
pub const RESEARCH_BASE_PER_SECOND: f64 = 2.0;
/// The research point total that resolves Team-1 Victory.
pub const RESEARCH_TARGET_POINTS: u32 = 9_000;
/// One Analyze conversion on the Research Center: 10 Crystals from the
/// shared Team-1 stockpile. A `Cost` const must list every field —
/// `Default` cannot initialize a const.
pub const ANALYZE_COST: Cost = Cost {
    food: 0,
    scrap: 0,
    coal: 0,
    crystals: 10,
};
/// Research points one accepted Analyze command converts.
pub const ANALYZE_RESEARCH_GAIN: f64 = 90.0;
/// Research percent at which every subsequently launched wave gets the
/// boosted threat budget; the point threshold derives from
/// `RESEARCH_TARGET_POINTS` through the same percent helper as building
/// unlocks.
pub const RESEARCH_WAVE_BOOST_PERCENT: u32 = 75;
/// Boosted wave budget = floor(base × NUMERATOR / DENOMINATOR), applied to
/// the integer budget at launch — never post-multiplied onto roster counts
/// or per-unit stats. Overflow health still scales with it: threat that
/// cannot spawn past `MAX_LIVE_ALIENS` folds into the capped spawn's HP,
/// so a boosted roster spilling past the cap buffs that spawn harder.
pub const RESEARCH_WAVE_BOOST_NUMERATOR: u64 = 3;
pub const RESEARCH_WAVE_BOOST_DENOMINATOR: u64 = 2;

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
            heat_supply: 15,
            heat_demand: 10,
            research_unlock_percent: 0,
            team1_only: true,
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
            heat_supply: 0,
            heat_demand: 1,
            research_unlock_percent: 0,
            team1_only: false,
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
            heat_supply: 0,
            heat_demand: 1,
            research_unlock_percent: 0,
            team1_only: false,
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
            heat_supply: 0,
            heat_demand: 3,
            research_unlock_percent: 0,
            team1_only: false,
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
            heat_supply: 0,
            heat_demand: 0,
            research_unlock_percent: 0,
            team1_only: false,
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
            heat_supply: 0,
            heat_demand: 0,
            research_unlock_percent: 0,
            team1_only: true,
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
            heat_supply: 0,
            heat_demand: 2,
            research_unlock_percent: 0,
            team1_only: true,
        },
        BuildingKind::CoalFurnace => BuildingSpec {
            cost: Cost {
                scrap: 80,
                ..Cost::default()
            },
            build_seconds: 20,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 300,
            combat: None,
            heat_supply: 12,
            heat_demand: 0,
            research_unlock_percent: 0,
            team1_only: true,
        },
        // Costs, ranges, counter targets and heat demands are locked
        // (HPA-188); health, build time, damage and cadence are provisional
        // first-pass values — detailed balancing is deferred to HPA-289.
        BuildingKind::FlameTower => BuildingSpec {
            cost: Cost {
                scrap: 120,
                coal: 60,
                ..Cost::default()
            },
            build_seconds: 25,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 350,
            combat: Some(CombatSpec {
                damage: 20,
                attack_range: 3.0,
                cooldown_seconds: 1.0,
                counter_target: UnitKind::Swarmer,
                counter_bonus: 10,
                ranged: true,
            }),
            heat_supply: 0,
            heat_demand: 3,
            research_unlock_percent: 25,
            team1_only: true,
        },
        BuildingKind::CrystalLance => BuildingSpec {
            cost: Cost {
                scrap: 150,
                crystals: 40,
                ..Cost::default()
            },
            build_seconds: 30,
            width: 2,
            height: 2,
            population_capacity: 0,
            max_health: 350,
            combat: Some(CombatSpec {
                damage: 30,
                attack_range: 10.0,
                cooldown_seconds: 1.5,
                counter_target: UnitKind::Brute,
                counter_bonus: 15,
                ranged: true,
            }),
            heat_supply: 0,
            heat_demand: 4,
            research_unlock_percent: 50,
            team1_only: true,
        },
    }
}

/// The underground room table. Scrap costs and build times are provisional
/// first-pass choices where the issue specified only 50–150 Scrap;
/// HPA-289 owns tuning.
pub fn room_spec(kind: RoomKind) -> RoomSpec {
    match kind {
        RoomKind::Shelter => RoomSpec {
            min_level: 1,
            cost: Cost {
                scrap: 50,
                ..Cost::default()
            },
            build_seconds: 15,
            population_capacity: 10,
            output: None,
            research_per_second: 0.0,
            heat_supply_base: 0,
            heat_supply_per_level: 0,
        },
        RoomKind::Greenhouse => RoomSpec {
            min_level: 1,
            cost: Cost {
                scrap: 75,
                ..Cost::default()
            },
            build_seconds: 20,
            population_capacity: 0,
            output: Some((ResourceKind::Food, 0.5)),
            research_per_second: 0.0,
            heat_supply_base: 0,
            heat_supply_per_level: 0,
        },
        RoomKind::CoalVein => RoomSpec {
            min_level: 2,
            cost: Cost {
                scrap: 90,
                ..Cost::default()
            },
            build_seconds: 25,
            population_capacity: 0,
            output: Some((ResourceKind::Coal, 0.4)),
            research_per_second: 0.0,
            heat_supply_base: 0,
            heat_supply_per_level: 0,
        },
        RoomKind::GeothermalTap => RoomSpec {
            min_level: 3,
            cost: Cost {
                scrap: 120,
                ..Cost::default()
            },
            build_seconds: 30,
            population_capacity: 0,
            output: None,
            research_per_second: 0.0,
            // +15 Heat at L3, +5 per deeper level.
            heat_supply_base: 15,
            heat_supply_per_level: 5,
        },
        RoomKind::Lab => RoomSpec {
            min_level: 3,
            cost: Cost {
                scrap: 150,
                ..Cost::default()
            },
            build_seconds: 30,
            population_capacity: 0,
            output: None,
            research_per_second: 1.0,
            heat_supply_base: 0,
            heat_supply_per_level: 0,
        },
        RoomKind::DeepMine => RoomSpec {
            min_level: 4,
            cost: Cost {
                scrap: 150,
                ..Cost::default()
            },
            build_seconds: 35,
            population_capacity: 0,
            output: Some((ResourceKind::Scrap, 0.4)),
            research_per_second: 0.0,
            heat_supply_base: 0,
            heat_supply_per_level: 0,
        },
    }
}

/// First-level excavation Scrap price; deeper digs grow ×1.6 per level.
pub const EXCAVATION_INITIAL_SCRAP: f64 = 100.0;
/// The ×1.6 depth multiplier per level below the first.
pub const EXCAVATION_DEPTH_MULTIPLIER: f64 = 1.6;
/// Coal charges this fraction of the already-rounded Scrap price
/// (2 → one half, rounded up).
pub const EXCAVATION_COAL_DENOMINATOR: u32 = 2;
/// Dig duration of level 1; each deeper level adds
/// `EXCAVATION_SECONDS_PER_LEVEL`.
pub const EXCAVATION_BASE_SECONDS: u32 = 30;
pub const EXCAVATION_SECONDS_PER_LEVEL: u32 = 15;
/// Optional room slots in one excavated level.
pub const ROOM_SLOTS_PER_LEVEL: usize = 4;

/// The price and dig duration of excavating target `level` (1-based):
/// round-half-up(`EXCAVATION_INITIAL_SCRAP` × 1.6^(level−1)) Scrap, half
/// that already-rounded price in Coal, and a duration growing 15 s per
/// level below the first. Plain, never `Option`: for positive values
/// `f64::round` is the required half-up rounding and `as u32` saturates
/// (even at infinity), Coal uses `div_ceil` and the duration saturating
/// arithmetic — an implausibly deep level reads as a practically
/// unaffordable saturated price instead of wrapping cheap or panicking.
/// `saturating_sub` keeps the exponent defined for the invalid level 0.
pub fn excavation_cost_and_seconds(level: u32) -> (Cost, u32) {
    let depth = level.saturating_sub(1);
    let scrap = (EXCAVATION_INITIAL_SCRAP * EXCAVATION_DEPTH_MULTIPLIER.powf(f64::from(depth)))
        .round() as u32;
    let coal = scrap.div_ceil(EXCAVATION_COAL_DENOMINATOR);
    let seconds =
        EXCAVATION_BASE_SECONDS.saturating_add(EXCAVATION_SECONDS_PER_LEVEL.saturating_mul(depth));
    (
        Cost {
            scrap,
            coal,
            ..Cost::default()
        },
        seconds,
    )
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
