use super::*;

#[test]
fn villagers_are_noncombatants_and_military_units_are_not() {
    assert!(unit_spec(UnitKind::Villager).combat.is_none());
    for kind in [UnitKind::Spearman, UnitKind::Archer] {
        assert!(unit_spec(kind).combat.is_some(), "{kind:?} has no combat");
    }
}

#[test]
fn military_units_carry_initial_combat_tuning() {
    let villager = unit_spec(UnitKind::Villager);
    let spearman = unit_spec(UnitKind::Spearman);
    let archer = unit_spec(UnitKind::Archer);

    // Health: villager 50, spearman 100, archer 70.
    assert_eq!(villager.max_health, 50);
    assert_eq!(spearman.max_health, 100);
    assert_eq!(archer.max_health, 70);

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
        (BuildingKind::TownCenter, 800),
        (BuildingKind::House, 250),
        (BuildingKind::Storehouse, 300),
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
fn attack_move_radius_is_eight_world_units() {
    assert_eq!(ATTACK_MOVE_RADIUS, 8.0);
}

#[test]
fn vision_radius_covers_attack_move_radius() {
    // Attack-move must never acquire targets the owning team cannot see.
    assert!(VISION_RADIUS_CELLS as f32 >= ATTACK_MOVE_RADIUS);
}
