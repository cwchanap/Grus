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

#[test]
fn villagers_are_noncombatants_and_military_units_are_not() {
    assert!(unit_spec(UnitKind::Villager).combat.is_none());
    for kind in [UnitKind::Spearman, UnitKind::Archer, UnitKind::Cavalry] {
        assert!(unit_spec(kind).combat.is_some(), "{kind:?} has no combat");
    }
}

#[test]
fn military_units_carry_initial_combat_tuning() {
    let villager = unit_spec(UnitKind::Villager);
    let spearman = unit_spec(UnitKind::Spearman);
    let archer = unit_spec(UnitKind::Archer);
    let cavalry = unit_spec(UnitKind::Cavalry);

    // Health: villager 50, spearman 100, archer 70, cavalry 140.
    assert_eq!(villager.max_health, 50);
    assert_eq!(spearman.max_health, 100);
    assert_eq!(archer.max_health, 70);
    assert_eq!(cavalry.max_health, 140);

    let spear_combat = spearman.combat.expect("spearman combat");
    assert_eq!(spear_combat.damage, 10);
    assert_eq!(spear_combat.attack_range, 1.5);
    assert_eq!(spear_combat.cooldown_seconds, 1.0);
    assert!(!spear_combat.ranged);

    let archer_combat = archer.combat.expect("archer combat");
    assert_eq!(archer_combat.damage, 8);
    assert_eq!(archer_combat.attack_range, 6.0);
    assert_eq!(archer_combat.cooldown_seconds, 1.25);
    assert!(archer_combat.ranged);

    let cavalry_combat = cavalry.combat.expect("cavalry combat");
    assert_eq!(cavalry_combat.damage, 12);
    assert_eq!(cavalry_combat.attack_range, 1.5);
    assert_eq!(cavalry_combat.cooldown_seconds, 1.0);
    assert!(!cavalry_combat.ranged);
}

#[test]
fn counter_bonuses_target_exactly_one_kind_each() {
    // The counter cycle: Spearman > Cavalry, Archer > Spearman (+12 so the
    // leg is carried by counter damage, not shot timing), Cavalry > Archer.
    // The bonus applies only to the named kind; no kind counters itself.
    let spear = unit_spec(UnitKind::Spearman)
        .combat
        .expect("spearman combat");
    let archer = unit_spec(UnitKind::Archer).combat.expect("archer combat");
    let cavalry = unit_spec(UnitKind::Cavalry).combat.expect("cavalry combat");

    assert_eq!(spear.counter_target, UnitKind::Cavalry);
    assert_eq!(spear.counter_bonus, 10);
    assert_eq!(archer.counter_target, UnitKind::Spearman);
    assert_eq!(archer.counter_bonus, 12);
    assert_eq!(cavalry.counter_target, UnitKind::Archer);
    assert_eq!(cavalry.counter_bonus, 12);

    for combat in [spear, archer, cavalry] {
        assert!(combat.counter_bonus > 0);
    }
}

#[test]
fn buildings_carry_catalogue_health() {
    let expected = [
        (BuildingKind::TownCenter, 800),
        (BuildingKind::House, 250),
        (BuildingKind::Storehouse, 300),
        (BuildingKind::Farm, 200),
        (BuildingKind::Barracks, 400),
        (BuildingKind::ArcheryRange, 400),
        (BuildingKind::Stable, 400),
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
fn attack_move_radius_is_eight_world_units() {
    assert_eq!(ATTACK_MOVE_RADIUS, 8.0);
}

#[test]
fn vision_radius_covers_attack_move_radius() {
    // Attack-move must never acquire targets the owning team cannot see.
    assert!(VISION_RADIUS_CELLS as f32 >= ATTACK_MOVE_RADIUS);
}
