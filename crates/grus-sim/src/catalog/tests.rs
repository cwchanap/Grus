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
fn surviving_units_carry_zero_counter_bonuses() {
    // The PvP counter cycle was deleted with the obsolete roster: every
    // surviving spec still names a concrete counter target but applies no
    // bonus damage.
    for kind in UnitKind::ALL {
        let Some(combat) = unit_spec(kind).combat else {
            continue;
        };
        assert_eq!(combat.counter_bonus, 0, "{kind:?} counter bonus");
    }
}

#[test]
fn buildings_carry_catalogue_health() {
    let expected = [
        (BuildingKind::ResearchCenter, 800),
        (BuildingKind::Bunkhouse, 250),
        (BuildingKind::Depot, 300),
        (BuildingKind::Barracks, 400),
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
fn winter_roster_has_exactly_three_units_and_four_buildings() {
    let unit_names: Vec<String> = UnitKind::ALL.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(unit_names, ["Worker", "Guard", "Rifleman"]);
    let building_names: Vec<String> = BuildingKind::ALL.iter().map(|k| format!("{k:?}")).collect();
    assert_eq!(
        building_names,
        ["ResearchCenter", "Bunkhouse", "Depot", "Barracks"]
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

    // Producer compatibility: ResearchCenter -> Worker; Barracks -> Guard,
    // Rifleman. Nothing else produces, and the Bunkhouse/Depot produce nothing.
    for kind in UnitKind::ALL {
        let producers: Vec<BuildingKind> = BuildingKind::ALL
            .into_iter()
            .filter(|building| produces(*building, ProductionKind::Unit(kind)))
            .collect();
        let expected = match kind {
            UnitKind::Worker => vec![BuildingKind::ResearchCenter],
            UnitKind::Guard | UnitKind::Rifleman => vec![BuildingKind::Barracks],
        };
        assert_eq!(producers, expected, "{kind:?} producer");
    }
    for building in [BuildingKind::Bunkhouse, BuildingKind::Depot] {
        for kind in UnitKind::ALL {
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
