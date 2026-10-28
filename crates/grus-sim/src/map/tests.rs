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
fn gate_occupancy_is_atomic_team_aware_and_cleared_by_unblocking() {
    let mut map = GridMap::new(8, 8);
    let gate = GridPos::new(3, 4);
    let human = TeamId(1);
    let alien = TeamId(2);

    assert_eq!(map.revision(), 0);
    assert!(map.set_gate(gate));
    assert_eq!(map.revision(), 1, "gate placement is one occupancy change");
    assert!(!map.is_walkable(gate), "gates reserve free ground");
    assert!(map.is_traversable_for(human, gate));
    assert!(!map.is_traversable_for(alien, gate));

    assert!(!map.set_gate(gate), "a gate cannot be claimed twice");
    assert!(
        !map.set_gate(GridPos::new(8, 4)),
        "out-of-bounds gates fail"
    );
    assert_eq!(map.revision(), 1);

    let wall = GridPos::new(5, 5);
    assert!(map.set_blocked(wall, true));
    assert!(
        !map.set_gate(wall),
        "a Wall cannot be converted into a Gate"
    );
    assert!(!map.is_traversable_for(human, wall));
    assert_eq!(map.revision(), 2);

    let cloned = map.clone();
    assert!(cloned.is_traversable_for(human, gate));
    assert!(!cloned.is_traversable_for(alien, gate));

    assert!(map.set_blocked(gate, false));
    assert_eq!(map.revision(), 3, "gate removal is one occupancy change");
    assert!(map.is_walkable(gate));
    assert!(
        map.is_traversable_for(alien, gate),
        "removal clears gate access"
    );
    assert!(map.is_traversable_for(human, gate));
    assert!(!map.set_blocked(gate, false));
    assert_eq!(map.revision(), 3);
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
fn closest_point_gives_research_center_melee_range() {
    // Authored 4×4 Research Center footprint. Range against buildings is measured
    // to the nearest point of the footprint rectangle — `Footprint::center()`
    // is never the melee metric.
    let research_center = Footprint::new(GridPos::new(12, 46), 4, 4);
    let attacker = Vec2::new(11.5, 48.5);

    let closest = research_center.closest_point(attacker);
    assert_eq!(closest, Vec2::new(12.0, 48.5));
    assert!(
        attacker.distance(closest) <= 1.5,
        "a unit on the perimeter cell is in melee range of the footprint"
    );
    assert!(
        attacker.distance(research_center.center()) > 1.5,
        "the geometric center would wrongly report out of melee range"
    );

    // A point inside the rectangle is its own closest point.
    assert_eq!(
        research_center.closest_point(Vec2::new(13.2, 47.4)),
        Vec2::new(13.2, 47.4)
    );
    // Distant points clamp onto the rectangle edge.
    assert_eq!(
        research_center.closest_point(Vec2::new(20.5, 40.5)),
        Vec2::new(16.0, 46.0)
    );
}

#[test]
fn find_path_rejects_unwalkable_endpoints() {
    let mut map = GridMap::new(8, 8);
    let team = TeamId(1);
    map.set_blocked(GridPos::new(2, 2), true);

    assert_eq!(
        map.find_path(team, GridPos::new(2, 2), GridPos::new(6, 6)),
        None,
        "a blocked start cell has no path"
    );
    assert_eq!(
        map.find_path(team, GridPos::new(0, 0), GridPos::new(2, 2)),
        None,
        "a blocked goal cell has no path"
    );
    assert!(
        map.find_path(team, GridPos::new(0, 0), GridPos::new(6, 6))
            .is_some()
    );
}

#[test]
fn only_humans_can_start_on_or_cross_a_gate_and_no_team_can_target_it() {
    let mut map = GridMap::new(5, 3);
    let gate = GridPos::new(2, 1);
    let start = GridPos::new(2, 0);
    let goal = GridPos::new(2, 2);
    let human = TeamId(1);
    let alien = TeamId(2);
    map.set_gate(gate);

    assert!(
        map.find_path(human, gate, goal).is_some(),
        "human gate start"
    );
    assert_eq!(map.find_path(alien, gate, goal), None, "alien gate start");
    assert_eq!(map.find_path(human, start, gate), None, "human gate goal");
    assert_eq!(map.find_path(alien, start, gate), None, "alien gate goal");
}

#[test]
fn humans_cross_gate_intermediate_cells_but_both_teams_stop_at_walls() {
    let mut map = GridMap::new(5, 3);
    let gate = GridPos::new(2, 1);
    let start = GridPos::new(2, 0);
    let goal = GridPos::new(2, 2);
    for x in 0..5 {
        if x != gate.x {
            map.set_blocked(GridPos::new(x, 1), true);
        }
    }
    map.set_gate(gate);

    let human_path = map
        .find_path(TeamId(1), start, goal)
        .expect("the human route crosses the gate");
    assert!(human_path.contains(&gate));
    assert_eq!(
        map.find_path(TeamId(2), start, goal),
        None,
        "aliens cannot cross the gate"
    );

    let mut wall_map = GridMap::new(5, 3);
    for x in 0..5 {
        wall_map.set_blocked(GridPos::new(x, 1), true);
    }
    for team in [TeamId(1), TeamId(2)] {
        assert_eq!(
            wall_map.find_path(team, start, goal),
            None,
            "walls block {team:?}"
        );
    }
}
