use super::*;
use crate::production::{ProductionKind, produces};

#[test]
fn workers_are_noncombatants_and_military_units_are_not() {
    assert!(unit_spec(UnitKind::Worker).combat.is_none());
    for kind in [UnitKind::Guard, UnitKind::Rifleman] {
        assert!(unit_spec(kind).combat.is_some(), "{kind:?} has no combat");
    }
}

#[test]
fn human_roster_is_exactly_the_non_alien_catalogue_subset() {
    let humans: Vec<_> = UnitKind::ALL
        .into_iter()
        .filter(|kind| unit_spec(*kind).alien.is_none())
        .collect();
    assert_eq!(UnitKind::HUMAN.as_slice(), humans);
    assert_eq!(
        UnitKind::ALL
            .into_iter()
            .filter(|kind| unit_spec(*kind).alien.is_some())
            .collect::<Vec<_>>(),
        [
            UnitKind::Swarmer,
            UnitKind::Spitter,
            UnitKind::Brute,
            UnitKind::Behemoth,
        ]
    );
}

#[test]
fn military_units_carry_initial_combat_tuning() {
    let worker = unit_spec(UnitKind::Worker);
    let guard = unit_spec(UnitKind::Guard);
    let rifleman = unit_spec(UnitKind::Rifleman);

    // Health: worker 50, guard 100, rifleman 70.
    assert_eq!(worker.max_health, 50);
    assert_eq!(guard.max_health, 100);
    assert_eq!(rifleman.max_health, 70);

    let spear_combat = guard.combat.expect("guard combat");
    assert_eq!(spear_combat.damage, 10);
    assert_eq!(spear_combat.attack_range, 1.5);
    assert_eq!(spear_combat.cooldown_seconds, 1.0);
    assert!(!spear_combat.ranged);

    let rifleman_combat = rifleman.combat.expect("rifleman combat");
    assert_eq!(rifleman_combat.damage, 8);
    assert_eq!(rifleman_combat.attack_range, 6.0);
    assert_eq!(rifleman_combat.cooldown_seconds, 1.25);
    assert!(rifleman_combat.ranged);
}

#[test]
fn human_counters_target_their_alien_matchups_and_aliens_have_no_counters() {
    let guard = unit_spec(UnitKind::Guard).combat.unwrap();
    assert_eq!(guard.counter_target, UnitKind::Swarmer);
    assert_eq!(guard.counter_bonus, 5);

    let rifleman = unit_spec(UnitKind::Rifleman).combat.unwrap();
    assert_eq!(rifleman.counter_target, UnitKind::Spitter);
    assert_eq!(rifleman.counter_bonus, 4);

    for kind in [
        UnitKind::Swarmer,
        UnitKind::Spitter,
        UnitKind::Brute,
        UnitKind::Behemoth,
    ] {
        assert_eq!(
            unit_spec(kind).combat.unwrap().counter_bonus,
            0,
            "{kind:?} counter bonus"
        );
    }
}

#[test]
fn alien_catalogue_specs_pin_threat_unlock_reward_and_combat() {
    let expected = [
        (UnitKind::Swarmer, 1, 1, 1, 40, 8.0, 6, 1.0, 1.5, false),
        (UnitKind::Spitter, 3, 2, 4, 70, 6.0, 8, 1.5, 6.0, true),
        (UnitKind::Brute, 6, 5, 8, 300, 4.0, 30, 2.0, 1.5, false),
        (
            UnitKind::Behemoth,
            40,
            50,
            10,
            2_000,
            3.0,
            80,
            2.0,
            1.5,
            false,
        ),
    ];
    for (
        kind,
        threat,
        crystal_reward,
        first_wave,
        health,
        speed,
        damage,
        cooldown,
        range,
        ranged,
    ) in expected
    {
        let spec = unit_spec(kind);
        let alien = spec.alien.expect("alien specification");
        let combat = spec.combat.expect("alien combat");
        assert_eq!(alien.threat, threat);
        assert_eq!(alien.crystal_reward, crystal_reward);
        assert_eq!(alien.first_wave, first_wave);
        assert_eq!(spec.max_health, health);
        assert_eq!(spec.speed, speed);
        assert_eq!(combat.damage, damage);
        assert_eq!(combat.cooldown_seconds, cooldown);
        assert_eq!(combat.attack_range, range);
        assert_eq!(combat.ranged, ranged);
        assert_eq!(combat.counter_bonus, 0);
    }
}

#[test]
fn wave_tuning_is_catalogued_as_integer_values() {
    assert_eq!(WAVE_PREPARATION_SECONDS, 300);
    assert_eq!(WAVE_INTERVAL_SECONDS, 100);
    assert_eq!(WAVE_WARNING_SECONDS, 30);
    assert_eq!(MAX_LIVE_ALIENS, 150);
    assert_eq!(WAVE_BUDGET_BASE, 8);
    assert_eq!(WAVE_BUDGET_PER_WAVE, 4);
    assert_eq!(WAVE_BUDGET_QUADRATIC_TENTHS, 4);
    assert_eq!(WAVE_TWO_ZONE_START, 10);
    assert_eq!(WAVE_THREE_ZONE_START, 20);
}

#[test]
fn buildings_carry_catalogue_health() {
    let expected = [
        (BuildingKind::ResearchCenter, 800),
        (BuildingKind::Bunkhouse, 250),
        (BuildingKind::Depot, 300),
        (BuildingKind::Barracks, 400),
        (BuildingKind::Wall, 400),
        (BuildingKind::Gate, 600),
        (BuildingKind::GunTurret, 350),
        (BuildingKind::CoalFurnace, 300),
        (BuildingKind::FlameTower, 350),
        (BuildingKind::CrystalLance, 350),
    ];
    for (kind, max_health) in expected {
        assert_eq!(
            building_spec(kind).max_health,
            max_health,
            "{kind:?} health"
        );
    }
    assert_eq!(expected.len(), BuildingKind::ALL.len());
}

#[test]
fn winter_human_roster_stays_separate_from_the_full_catalogue() {
    let human_names: Vec<String> = UnitKind::HUMAN.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(human_names, ["Worker", "Guard", "Rifleman"]);
    let all_names: Vec<String> = UnitKind::ALL.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(
        all_names,
        [
            "Worker", "Guard", "Rifleman", "Swarmer", "Spitter", "Brute", "Behemoth"
        ]
    );
    let building_names: Vec<String> = BuildingKind::ALL.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(
        building_names,
        [
            "ResearchCenter",
            "Bunkhouse",
            "Depot",
            "Barracks",
            "Wall",
            "Gate",
            "GunTurret",
            "CoalFurnace",
            "FlameTower",
            "CrystalLance"
        ]
    );
}

#[test]
fn winter_costs_and_producers_are_pinned() {
    // Exact winter economy: Worker 50 Food at the ResearchCenter; Guard
    // 60 Food and Rifleman 40 Food + 40 Scrap at the Barracks.
    assert_eq!(
        unit_spec(UnitKind::Worker).cost,
        Cost {
            food: 50,
            scrap: 0,
            coal: 0,
            crystals: 0
        }
    );
    assert_eq!(
        unit_spec(UnitKind::Guard).cost,
        Cost {
            food: 60,
            scrap: 0,
            coal: 0,
            crystals: 0
        }
    );
    assert_eq!(
        unit_spec(UnitKind::Rifleman).cost,
        Cost {
            food: 40,
            scrap: 40,
            coal: 0,
            crystals: 0
        }
    );

    // Buildings charge Scrap only: Bunkhouse 50, Depot 75, Barracks 120.
    assert_eq!(
        building_spec(BuildingKind::ResearchCenter).cost,
        Cost::default()
    );
    assert_eq!(
        building_spec(BuildingKind::Bunkhouse).cost,
        Cost {
            scrap: 50,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::Depot).cost,
        Cost {
            scrap: 75,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::Barracks).cost,
        Cost {
            scrap: 120,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::Wall).cost,
        Cost {
            scrap: 10,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::Gate).cost,
        Cost {
            scrap: 30,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::GunTurret).cost,
        Cost {
            scrap: 100,
            coal: 20,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::CoalFurnace).cost,
        Cost {
            scrap: 80,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::FlameTower).cost,
        Cost {
            scrap: 120,
            coal: 60,
            ..Cost::default()
        }
    );
    assert_eq!(
        building_spec(BuildingKind::CrystalLance).cost,
        Cost {
            scrap: 150,
            crystals: 40,
            ..Cost::default()
        }
    );

    // Producer compatibility: ResearchCenter -> Worker; Barracks -> Guard,
    // Rifleman. Nothing else produces, and the Bunkhouse/Depot produce nothing.
    for kind in UnitKind::HUMAN {
        let producers: Vec<BuildingKind> = BuildingKind::ALL
            .into_iter()
            .filter(|building| produces(*building, ProductionKind::Unit(kind)))
            .collect();
        let expected = match kind {
            UnitKind::Worker => vec![BuildingKind::ResearchCenter],
            UnitKind::Guard | UnitKind::Rifleman => vec![BuildingKind::Barracks],
            UnitKind::Swarmer | UnitKind::Spitter | UnitKind::Brute | UnitKind::Behemoth => {
                panic!("alien kind {kind:?} entered the human roster")
            }
        };
        assert_eq!(producers, expected, "{kind:?} producer");
    }
    for kind in UnitKind::ALL
        .into_iter()
        .filter(|kind| !UnitKind::HUMAN.contains(kind))
    {
        for building in BuildingKind::ALL {
            assert!(
                !produces(building, ProductionKind::Unit(kind)),
                "aliens cannot be produced by {building:?}"
            );
        }
    }
    for building in [BuildingKind::Bunkhouse, BuildingKind::Depot] {
        for kind in UnitKind::HUMAN {
            assert!(
                !produces(building, ProductionKind::Unit(kind)),
                "{building:?} must not produce {kind:?}"
            );
        }
    }

    // Footprints and population grants stay on the winter geometry.
    let footprints = [
        (BuildingKind::ResearchCenter, 4, 4, 10),
        (BuildingKind::Bunkhouse, 2, 2, 10),
        (BuildingKind::Depot, 2, 2, 0),
        (BuildingKind::Barracks, 3, 3, 0),
        (BuildingKind::Wall, 1, 1, 0),
        (BuildingKind::Gate, 1, 1, 0),
        (BuildingKind::GunTurret, 2, 2, 0),
        (BuildingKind::CoalFurnace, 2, 2, 0),
        (BuildingKind::FlameTower, 2, 2, 0),
        (BuildingKind::CrystalLance, 2, 2, 0),
    ];
    for (kind, width, height, population) in footprints {
        let spec = building_spec(kind);
        assert_eq!(
            (spec.width, spec.height, spec.population_capacity),
            (width, height, population),
            "{kind:?} footprint/population"
        );
    }
}

#[test]
fn towers_carry_building_combat_tuning() {
    for kind in [
        BuildingKind::ResearchCenter,
        BuildingKind::Bunkhouse,
        BuildingKind::Depot,
        BuildingKind::Barracks,
        BuildingKind::Wall,
        BuildingKind::Gate,
        BuildingKind::CoalFurnace,
    ] {
        assert!(building_spec(kind).combat.is_none(), "{kind:?} combat");
    }

    assert_eq!(building_spec(BuildingKind::Wall).build_seconds, 5);
    assert_eq!(building_spec(BuildingKind::Gate).build_seconds, 8);
    let turret = building_spec(BuildingKind::GunTurret);
    assert_eq!(turret.build_seconds, 25);
    let combat = turret.combat.expect("Gun Turret combat");
    assert_eq!(combat.damage, 10);
    assert_eq!(combat.attack_range, 8.0);
    assert_eq!(combat.cooldown_seconds, 1.0);
    assert_eq!(combat.counter_target, UnitKind::Spitter);
    assert_eq!(combat.counter_bonus, 5);
    assert!(combat.ranged);

    // Provisional first-pass tower values (HPA-289 balance pass pending);
    // costs, ranges, counter matchups and heat demands are locked.
    let flame = building_spec(BuildingKind::FlameTower);
    assert_eq!(flame.build_seconds, 25);
    assert_eq!(flame.max_health, 350);
    let flame_combat = flame.combat.expect("Flame Tower combat");
    assert_eq!(flame_combat.damage, 20);
    assert_eq!(flame_combat.attack_range, 3.0);
    assert_eq!(flame_combat.cooldown_seconds, 1.0);
    assert_eq!(flame_combat.counter_target, UnitKind::Swarmer);
    assert_eq!(flame_combat.counter_bonus, 10);
    assert!(flame_combat.ranged);

    let lance = building_spec(BuildingKind::CrystalLance);
    assert_eq!(lance.build_seconds, 30);
    assert_eq!(lance.max_health, 350);
    let lance_combat = lance.combat.expect("Crystal Lance combat");
    assert_eq!(lance_combat.damage, 30);
    assert_eq!(lance_combat.attack_range, 10.0);
    assert_eq!(lance_combat.cooldown_seconds, 1.5);
    assert_eq!(lance_combat.counter_target, UnitKind::Brute);
    assert_eq!(lance_combat.counter_bonus, 15);
    assert!(lance_combat.ranged);
}

#[test]
fn winter_gather_rate_is_age_independent() {
    // Task 1 deleted the Age system; the gather rate is a flat constant
    // now and forever in this catalogue.
    assert_eq!(BASE_GATHER_RATE, 2.0);
}

#[test]
fn attack_move_radius_is_eight_world_units() {
    assert_eq!(ATTACK_MOVE_RADIUS, 8.0);
}

#[test]
fn vision_radius_covers_attack_move_radius() {
    // Attack-move must never acquire targets the owning team cannot see.
    assert!(VISION_RADIUS_CELLS as f32 >= ATTACK_MOVE_RADIUS);
}

#[test]
fn heat_supply_and_demand_are_catalogued_per_building() {
    let expected = [
        (BuildingKind::ResearchCenter, 15, 10),
        (BuildingKind::Bunkhouse, 0, 1),
        (BuildingKind::Depot, 0, 1),
        (BuildingKind::Barracks, 0, 3),
        (BuildingKind::Wall, 0, 0),
        (BuildingKind::Gate, 0, 0),
        (BuildingKind::GunTurret, 0, 2),
        (BuildingKind::CoalFurnace, 12, 0),
        (BuildingKind::FlameTower, 0, 3),
        (BuildingKind::CrystalLance, 0, 4),
    ];
    for (kind, supply, demand) in expected {
        let spec = building_spec(kind);
        assert_eq!(spec.heat_supply, supply, "{kind:?} heat supply");
        assert_eq!(spec.heat_demand, demand, "{kind:?} heat demand");
    }
    assert_eq!(expected.len(), BuildingKind::ALL.len());
}

#[test]
fn coal_furnace_matches_the_depot_envelope_at_twenty_seconds() {
    let furnace = building_spec(BuildingKind::CoalFurnace);
    assert_eq!(furnace.build_seconds, 20);
    assert_eq!(furnace.max_health, 300);
    assert_eq!((furnace.width, furnace.height), (2, 2));
}

#[test]
fn furnace_burn_rate_and_efficiency_floor_are_named_constants() {
    assert_eq!(FURNACE_COAL_PER_SECOND, 0.5);
    assert_eq!(MIN_SURFACE_EFFICIENCY, 0.25);
}

#[test]
fn research_tuning_is_catalogued_as_named_constants() {
    assert_eq!(RESEARCH_TARGET_POINTS, 9_000);
    assert_eq!(RESEARCH_BASE_PER_SECOND, 2.0);
    assert_eq!(
        ANALYZE_COST,
        Cost {
            food: 0,
            scrap: 0,
            coal: 0,
            crystals: 10,
        }
    );
    assert_eq!(ANALYZE_RESEARCH_GAIN, 90.0);
    assert_eq!(RESEARCH_WAVE_BOOST_PERCENT, 75);
    assert_eq!(RESEARCH_WAVE_BOOST_NUMERATOR, 3);
    assert_eq!(RESEARCH_WAVE_BOOST_DENOMINATOR, 2);

    // The advanced towers carry the only research gates; every other kind
    // stays always-available at 0%.
    for kind in BuildingKind::ALL {
        let expected = match kind {
            BuildingKind::FlameTower => 25,
            BuildingKind::CrystalLance => 50,
            _ => 0,
        };
        assert_eq!(
            building_spec(kind).research_unlock_percent,
            expected,
            "{kind:?} research gate"
        );
    }
}

#[test]
fn underground_room_catalogue_is_pinned() {
    // Provisional first-pass prices/timers where the issue specified only
    // 50-150 Scrap; HPA-289 owns tuning. Heat is the geothermal base at the
    // minimum level plus the per-deeper-level bonus.
    let expected = [
        (
            RoomKind::Shelter,
            RoomSpec {
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
        ),
        (
            RoomKind::Greenhouse,
            RoomSpec {
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
        ),
        (
            RoomKind::CoalVein,
            RoomSpec {
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
        ),
        (
            RoomKind::GeothermalTap,
            RoomSpec {
                min_level: 3,
                cost: Cost {
                    scrap: 120,
                    ..Cost::default()
                },
                build_seconds: 30,
                population_capacity: 0,
                output: None,
                research_per_second: 0.0,
                heat_supply_base: 15,
                heat_supply_per_level: 5,
            },
        ),
        (
            RoomKind::Lab,
            RoomSpec {
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
        ),
        (
            RoomKind::DeepMine,
            RoomSpec {
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
        ),
    ];
    for (kind, spec) in expected {
        assert_eq!(room_spec(kind), spec, "{kind:?}");
    }
    let names: Vec<String> = RoomKind::ALL.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(
        names,
        [
            "Shelter",
            "Greenhouse",
            "CoalVein",
            "GeothermalTap",
            "Lab",
            "DeepMine"
        ]
    );
    assert_eq!(expected.len(), RoomKind::ALL.len());
}

#[test]
fn room_availability_is_minimum_level_peaking_at_deep_mine() {
    // Availability is a minimum-level floor, not an exact-level
    // assignment: the deepest unlock is Deep Mine at level 4, so from L5
    // on no new kind appears and every earlier kind stays buildable.
    assert_eq!(
        RoomKind::ALL
            .iter()
            .map(|kind| room_spec(*kind).min_level)
            .max(),
        Some(4)
    );
    let buildable_at = |level: u32| {
        RoomKind::ALL
            .into_iter()
            .filter(|kind| room_spec(*kind).min_level <= level)
            .count()
    };
    assert_eq!(buildable_at(4), RoomKind::ALL.len());
    assert_eq!(buildable_at(5), RoomKind::ALL.len());
    assert_eq!(buildable_at(u32::MAX), RoomKind::ALL.len());
}

#[test]
fn excavation_tuning_is_catalogued_as_named_constants() {
    assert_eq!(EXCAVATION_INITIAL_SCRAP, 100);
    assert_eq!(EXCAVATION_DEPTH_NUMERATOR, 8);
    assert_eq!(EXCAVATION_DEPTH_DENOMINATOR, 5);
    assert_eq!(EXCAVATION_COAL_DENOMINATOR, 2);
    assert_eq!(EXCAVATION_BASE_SECONDS, 30);
    assert_eq!(EXCAVATION_SECONDS_PER_LEVEL, 15);
    assert_eq!(ROOM_SLOTS_PER_LEVEL, 4);
}

#[test]
fn excavation_prices_and_durations_match_the_design_table() {
    // Round-half-up(100 × 1.6^(n−1)) Scrap, half that rounded price in
    // Coal, 30 s + 15 s per level below the first.
    let expected = [
        (1, 100, 50, 30),
        (2, 160, 80, 45),
        (3, 256, 128, 60),
        (4, 410, 205, 75),
        (5, 655, 328, 90),
    ];
    for (level, scrap, coal, seconds) in expected {
        let (cost, dig_seconds) = excavation_cost_and_seconds(level).expect("affordable level");
        assert_eq!(
            cost,
            Cost {
                scrap,
                coal,
                ..Cost::default()
            },
            "level {level}"
        );
        assert_eq!(dig_seconds, seconds, "level {level}");
    }
}

#[test]
fn impossible_excavation_depths_are_unaffordable_never_wrapped() {
    // Level 0 is not a diggable target.
    assert_eq!(excavation_cost_and_seconds(0), None);
    // Level 38 (100 × 1.6^37 ≈ 3.57e9 Scrap) is the last price a u32
    // stockpile could ever pay; level 39's ≈ 5.71e9 does not fit and must
    // read unaffordable, never wrap into a cheap dig.
    assert!(excavation_cost_and_seconds(38).is_some());
    for level in [39, 40, 1000, u32::MAX] {
        assert_eq!(excavation_cost_and_seconds(level), None, "level {level}");
    }
    // No representable depth wraps cheaper than its predecessor.
    let mut previous_scrap = 0;
    for level in 1..=38 {
        let (cost, _) = excavation_cost_and_seconds(level).expect("affordable depth");
        assert!(cost.scrap > previous_scrap, "level {level} wrapped cheaper");
        previous_scrap = cost.scrap;
    }
}
