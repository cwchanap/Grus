//! Typed gameplay catalogue: the only source of costs, times, footprints,
//! population values, movement speeds, and the gathering rate.

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
}

impl UnitKind {
    pub const ALL: [UnitKind; 3] = [UnitKind::Worker, UnitKind::Guard, UnitKind::Rifleman];
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    ResearchCenter,
    Bunkhouse,
    Depot,
    Barracks,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 4] = [
        BuildingKind::ResearchCenter,
        BuildingKind::Bunkhouse,
        BuildingKind::Depot,
        BuildingKind::Barracks,
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
        UnitKind::Worker => UnitSpec {
            cost: Cost {
                food: 50,
                ..Cost::default()
            },
            train_seconds: 15,
            speed: 6.0,
            max_health: 50,
            combat: None,
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
                counter_target: UnitKind::Rifleman,
                counter_bonus: 0,
                ranged: false,
            }),
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
                counter_target: UnitKind::Guard,
                counter_bonus: 0,
                ranged: true,
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
