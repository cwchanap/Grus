use super::*;

#[test]
fn blocked_rect_prevents_walkability() {
    let mut map = GridMap::new(8, 8);
    map.set_blocked_rect(GridPos::new(2, 2), GridPos::new(4, 4));

    assert!(map.is_walkable(GridPos::new(1, 1)));
    assert!(!map.is_walkable(GridPos::new(2, 2)));
    assert!(!map.is_walkable(GridPos::new(4, 4)));
    assert!(map.is_walkable(GridPos::new(5, 5)));
}

#[test]
fn occupancy_revision_changes_only_when_walkability_changes() {
    let mut map = GridMap::new(8, 8);
    let cell = GridPos::new(3, 4);

    assert_eq!(map.revision(), 0);
    assert!(map.set_blocked(cell, true));
    assert_eq!(map.revision(), 1);
    assert!(!map.is_walkable(cell));

    assert!(!map.set_blocked(cell, true));
    assert_eq!(map.revision(), 1);

    assert!(map.set_blocked(cell, false));
    assert_eq!(map.revision(), 2);
    assert!(map.is_walkable(cell));

    assert!(!map.set_blocked(cell, false));
    assert_eq!(map.revision(), 2);
}

#[test]
fn world_cell_round_trip_uses_cell_centers() {
    let map = GridMap::new(8, 8);
    for cell in [GridPos::new(0, 0), GridPos::new(3, 5), GridPos::new(7, 7)] {
        assert_eq!(map.world_to_cell(map.cell_center(cell)), cell);
    }
}

#[test]
fn footprint_cells_perimeter_and_adjacency_stay_disjoint_and_deterministic() {
    let footprint = Footprint::new(GridPos::new(3, 3), 2, 2);

    let cells = footprint.cells();
    assert_eq!(
        cells,
        vec![
            GridPos::new(3, 3),
            GridPos::new(4, 3),
            GridPos::new(3, 4),
            GridPos::new(4, 4)
        ]
    );

    let perimeter = footprint.perimeter_cells();
    assert_eq!(perimeter.len(), 12);
    assert!(!perimeter.contains(&GridPos::new(3, 3)));
    assert!(!perimeter.contains(&GridPos::new(4, 4)));
    assert!(perimeter.contains(&GridPos::new(2, 2)));
    assert!(perimeter.contains(&GridPos::new(5, 5)));
    assert_eq!(perimeter.first().copied(), Some(GridPos::new(2, 2)));
    assert!(
        perimeter
            .iter()
            .all(|cell| footprint.is_immediately_adjacent(*cell))
    );
    assert!(perimeter.iter().all(|cell| !cells.contains(cell)));

    assert!(footprint.is_immediately_adjacent(GridPos::new(2, 3)));
    assert!(!footprint.is_immediately_adjacent(GridPos::new(1, 1)));
    assert!(!footprint.is_immediately_adjacent(GridPos::new(3, 3)));
}

#[test]
fn set_blocked_out_of_bounds_returns_false_and_keeps_the_revision() {
    let mut map = GridMap::new(8, 8);
    let revision = map.revision();

    assert!(!map.set_blocked(GridPos::new(-1, 4), true));
    assert!(!map.set_blocked(GridPos::new(8, 4), false));
    assert_eq!(
        map.revision(),
        revision,
        "a rejected write must not bump the revision"
    );
}

#[test]
fn closest_point_gives_town_center_melee_range() {
    // Authored 4×4 Town Center footprint. Range against buildings is measured
    // to the nearest point of the footprint rectangle — `Footprint::center()`
    // is never the melee metric.
    let town_center = Footprint::new(GridPos::new(12, 46), 4, 4);
    let attacker = Vec2::new(11.5, 48.5);

    let closest = town_center.closest_point(attacker);
    assert_eq!(closest, Vec2::new(12.0, 48.5));
    assert!(
        attacker.distance(closest) <= 1.5,
        "a unit on the perimeter cell is in melee range of the footprint"
    );
    assert!(
        attacker.distance(town_center.center()) > 1.5,
        "the geometric center would wrongly report out of melee range"
    );

    // A point inside the rectangle is its own closest point.
    assert_eq!(
        town_center.closest_point(Vec2::new(13.2, 47.4)),
        Vec2::new(13.2, 47.4)
    );
    // Distant points clamp onto the rectangle edge.
    assert_eq!(
        town_center.closest_point(Vec2::new(20.5, 40.5)),
        Vec2::new(16.0, 46.0)
    );
}

#[test]
fn find_path_rejects_unwalkable_endpoints() {
    let mut map = GridMap::new(8, 8);
    map.set_blocked(GridPos::new(2, 2), true);

    assert_eq!(
        map.find_path(GridPos::new(2, 2), GridPos::new(6, 6)),
        None,
        "a blocked start cell has no path"
    );
    assert_eq!(
        map.find_path(GridPos::new(0, 0), GridPos::new(2, 2)),
        None,
        "a blocked goal cell has no path"
    );
    assert!(
        map.find_path(GridPos::new(0, 0), GridPos::new(6, 6))
            .is_some()
    );
}
