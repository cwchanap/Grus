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
            "GunTurret"
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
fn only_gun_turret_has_building_combat_tuning() {
    for kind in [
        BuildingKind::ResearchCenter,
        BuildingKind::Bunkhouse,
        BuildingKind::Depot,
        BuildingKind::Barracks,
        BuildingKind::Wall,
        BuildingKind::Gate,
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
