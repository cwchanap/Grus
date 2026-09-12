//! Typed gameplay catalogue: the only source of costs, times, footprints,
//! unlocks, population values, movement speeds, and gathering rates.

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
    Cavalry,
}

impl UnitKind {
    pub const ALL: [UnitKind; 4] = [
        UnitKind::Villager,
        UnitKind::Spearman,
        UnitKind::Archer,
        UnitKind::Cavalry,
    ];
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    TownCenter,
    House,
    Storehouse,
    Farm,
    Barracks,
    ArcheryRange,
    Stable,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 7] = [
        BuildingKind::TownCenter,
        BuildingKind::House,
        BuildingKind::Storehouse,
        BuildingKind::Farm,
        BuildingKind::Barracks,
        BuildingKind::ArcheryRange,
        BuildingKind::Stable,
    ];
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Age {
    Age1,
    Age2,
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
    pub required_age: Age,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuildingSpec {
    pub cost: Cost,
    pub build_seconds: u32,
    pub width: u8,
    pub height: u8,
    pub population_capacity: u32,
    pub required_age: Age,
}

pub const AGE_TWO_COST: Cost = Cost {
    food: 300,
    wood: 0,
    gold: 200,
};
pub const AGE_TWO_SECONDS: u32 = 45;
pub const CARRY_LIMIT: u32 = 10;
pub const BASE_GATHER_RATE: f32 = 2.0;
pub const AGE_TWO_GATHER_RATE: f32 = 2.2;
pub const MAX_POPULATION: u32 = 100;

pub fn unit_spec(kind: UnitKind) -> UnitSpec {
    match kind {
        UnitKind::Villager => UnitSpec {
            cost: Cost {
                food: 50,
                ..Cost::default()
            },
            train_seconds: 15,
            speed: 6.0,
            required_age: Age::Age1,
        },
        UnitKind::Spearman => UnitSpec {
            cost: Cost {
                food: 60,
                ..Cost::default()
            },
            train_seconds: 20,
            speed: 6.0,
            required_age: Age::Age1,
        },
        UnitKind::Archer => UnitSpec {
            cost: Cost {
                food: 40,
                wood: 40,
                gold: 0,
            },
            train_seconds: 25,
            speed: 6.0,
            required_age: Age::Age1,
        },
        UnitKind::Cavalry => UnitSpec {
            cost: Cost {
                food: 80,
                wood: 0,
                gold: 60,
            },
            train_seconds: 30,
            speed: 8.0,
            required_age: Age::Age2,
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
            required_age: Age::Age1,
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
            required_age: Age::Age1,
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
            required_age: Age::Age1,
        },
        BuildingKind::Farm => BuildingSpec {
            cost: Cost {
                wood: 60,
                ..Cost::default()
            },
            build_seconds: 15,
            width: 2,
            height: 2,
            population_capacity: 0,
            required_age: Age::Age1,
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
            required_age: Age::Age1,
        },
        BuildingKind::ArcheryRange => BuildingSpec {
            cost: Cost {
                wood: 120,
                ..Cost::default()
            },
            build_seconds: 30,
            width: 3,
            height: 3,
            population_capacity: 0,
            required_age: Age::Age1,
        },
        BuildingKind::Stable => BuildingSpec {
            cost: Cost {
                wood: 150,
                ..Cost::default()
            },
            build_seconds: 40,
            width: 3,
            height: 3,
            population_capacity: 0,
            required_age: Age::Age2,
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
mod tests {
    use super::*;

    #[test]
    fn only_stable_and_cavalry_require_age_two() {
        let gated_buildings = BuildingKind::ALL
            .into_iter()
            .filter(|kind| building_spec(*kind).required_age == Age::Age2)
            .collect::<Vec<_>>();
        let gated_units = UnitKind::ALL
            .into_iter()
            .filter(|kind| unit_spec(*kind).required_age == Age::Age2)
            .collect::<Vec<_>>();

        assert_eq!(gated_buildings, vec![BuildingKind::Stable]);
        assert_eq!(gated_units, vec![UnitKind::Cavalry]);
    }
}
