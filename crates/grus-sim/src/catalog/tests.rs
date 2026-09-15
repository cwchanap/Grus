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
