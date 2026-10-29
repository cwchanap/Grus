use bevy::math::Vec2;
use bevy::prelude::{Entity, World};

use super::*;
use crate::buildings::ConstructionState;
use crate::catalog::{BuildingKind, unit_spec};
use crate::commands::spawn_unit;
use crate::fixture::{ENTRY_ZONES, MapFixture, ResourceSpawn, seed_skirmish};
use crate::ids::{ResourceId, UnitId};

/// The authored skirmish seed with no visibility inserted: pure-sim tests run
/// with full information until they opt in.
fn seeded() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

fn lone_unit_world(team: TeamId, cell: GridPos) -> (World, GridMap) {
    let mut world = World::new();
    spawn_unit(
        &mut world,
        UnitId(1),
        team,
        Vec2::new(cell.x as f32 + 0.5, cell.y as f32 + 0.5),
        crate::catalog::UnitKind::Worker,
        unit_spec(crate::catalog::UnitKind::Worker).speed,
    );
    (world, GridMap::new(128, 96))
}

fn single_unit_entity(world: &mut World) -> Entity {
    let mut query = world.query::<(Entity, &crate::movement::Unit)>();
    query.iter(world).next().unwrap().0
}

#[test]
fn initial_reveal_shows_the_human_core_and_hides_entry_zones() {
    let (mut world, map) = seeded();
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);

    let visibility = world.resource::<VisibilityMap>();
    let own_start = MapFixture::team_starts()[0];
    assert_eq!(
        visibility.cell_state(TeamId(1), own_start.research_center_anchor),
        CellVisibility::Visible,
        "own Research Center footprint is visible after initial reveal"
    );
    assert_eq!(
        visibility.cell_state(TeamId(1), ENTRY_ZONES[0].1),
        CellVisibility::Unexplored,
        "alien entry anchors stay unexplored before aliens arrive"
    );
    assert_eq!(
        visibility.cell_state(TeamId(2), own_start.research_center_anchor),
        CellVisibility::Unexplored,
        "team 2 without live entities sees nothing"
    );
    assert_eq!(visibility.revision, 1, "initial reveal changes state");
}

#[test]
fn authored_start_resources_are_explored_and_expansions_are_not() {
    let (mut world, map) = seeded();
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);

    // Safe-source ownership is the authored TeamStart membership — never an
    // x<64 side inference: the central human city's safe ring may straddle
    // the midline. The human sees its six safe sources; eastern deposits and
    // every expansion stay Unexplored.
    let resources: std::collections::HashMap<ResourceId, ResourceSpawn> =
        MapFixture::starting_resources()
            .into_iter()
            .chain(MapFixture::expansion_resources())
            .map(|spawn| (spawn.id, spawn))
            .collect();
    for start in MapFixture::team_starts() {
        let other = if start.team == TeamId(1) {
            TeamId(2)
        } else {
            TeamId(1)
        };
        for id in MapFixture::safe_source_ids(start.team) {
            let spawn = resources[&id];
            assert!(
                visible_to(&world, start.team, spawn.cell),
                "safe source at {:?} must be visible to its owner {team:?}",
                spawn.cell,
                team = start.team
            );
            assert!(explored_by(&world, start.team, spawn.cell));
            assert!(
                !explored_by(&world, other, spawn.cell),
                "safe source at {:?} must stay unknown to {other:?}",
                spawn.cell
            );
        }
    }
    for spawn in MapFixture::expansion_resources() {
        assert!(
            !explored_by(&world, TeamId(1), spawn.cell),
            "expansion resource at {:?} must be unexplored for team 1",
            spawn.cell
        );
        assert!(!explored_by(&world, TeamId(2), spawn.cell));
    }
}

#[test]
fn exploration_is_retained_but_current_vision_is_lost_after_moving_away() {
    let (mut world, map) = lone_unit_world(TeamId(1), GridPos::new(20, 20));
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);
    assert_eq!(world.resource::<VisibilityMap>().revision, 1);
    assert!(visible_to(&world, TeamId(1), GridPos::new(20, 20)));

    // Teleport the lone unit far away without stepping movement.
    let entity = single_unit_entity(&mut world);
    world.get_mut::<SimPosition>(entity).unwrap().current = Vec2::new(80.5, 80.5);

    refresh_visibility(&mut world, &map);

    let visibility = world.resource::<VisibilityMap>();
    assert_eq!(
        visibility.revision, 2,
        "losing and gaining cells is a change"
    );
    assert_eq!(
        visibility.cell_state(TeamId(1), GridPos::new(20, 20)),
        CellVisibility::Explored,
        "explored state is retained after moving away"
    );
    assert_eq!(
        visibility.cell_state(TeamId(1), GridPos::new(80, 80)),
        CellVisibility::Visible,
    );
    assert!(explored_by(&world, TeamId(1), GridPos::new(20, 20)));
    assert!(!visible_to(&world, TeamId(1), GridPos::new(20, 20)));
}

#[test]
fn revision_is_stable_when_nothing_changes() {
    let (mut world, map) = seeded();
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);
    let revision = world.resource::<VisibilityMap>().revision;
    assert_eq!(revision, 1);

    refresh_visibility(&mut world, &map);
    assert_eq!(
        world.resource::<VisibilityMap>().revision,
        revision,
        "a refresh over an unchanged world must not bump the revision"
    );
}

#[test]
fn reveal_clips_to_map_bounds() {
    let (mut world, map) = lone_unit_world(TeamId(1), GridPos::new(0, 0));
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);

    let visibility = world.resource::<VisibilityMap>();
    let vision = &visibility.teams[&TeamId(1)];
    assert!(
        vision.visible.iter().all(|cell| map.in_bounds(*cell)),
        "no out-of-bounds cell may enter the visible set"
    );
    assert!(vision.visible.contains(&GridPos::new(0, 0)));
    assert!(
        vision
            .visible
            .contains(&GridPos::new(VISION_RADIUS_CELLS, 0))
    );
    assert!(
        !vision
            .visible
            .contains(&GridPos::new(VISION_RADIUS_CELLS + 1, 0))
    );
    assert!(
        !vision
            .visible
            .contains(&GridPos::new(0, VISION_RADIUS_CELLS + 1))
    );
    assert!(!vision.visible.contains(&GridPos::new(-1, -1)));
}

#[test]
fn construction_site_reveals_nothing_until_complete() {
    let mut world = World::new();
    let map = GridMap::new(128, 96);
    let footprint = Footprint::new(GridPos::new(60, 60), 4, 4);
    let entity = world
        .spawn((
            Building {
                id: crate::ids::BuildingId(1),
                team: TeamId(1),
                kind: BuildingKind::ResearchCenter,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: None,
                },
            },
            footprint,
        ))
        .id();
    world.insert_resource(VisibilityMap::default());

    refresh_visibility(&mut world, &map);

    assert!(!explored_by(&world, TeamId(1), footprint));
    assert!(!visible_to(&world, TeamId(2), footprint));
    assert_eq!(
        world.resource::<VisibilityMap>().revision,
        0,
        "an incomplete site reveals nothing, so nothing changed"
    );

    world
        .get_mut::<Building>(entity)
        .unwrap()
        .construction
        .complete = true;
    refresh_visibility(&mut world, &map);

    assert!(visible_to(&world, TeamId(1), footprint));
    assert_eq!(
        world
            .resource::<VisibilityMap>()
            .cell_state(TeamId(1), GridPos::new(73, 63)),
        CellVisibility::Visible,
        "every footprint cell is a reveal origin: the far corner reaches cells \
         the footprint center alone would not",
    );
}

#[test]
fn footprint_predicates_qualify_when_any_cell_qualifies() {
    let (mut world, map) = lone_unit_world(TeamId(1), GridPos::new(50, 50));
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);

    // Straddles the vision edge: only the near column of cells qualifies.
    let straddling = Footprint::new(GridPos::new(58, 50), 4, 4);
    assert!(visible_to(&world, TeamId(1), straddling));
    assert!(explored_by(&world, TeamId(1), straddling));

    // Nearest cell is 12 away from the unit cell — fully beyond reach.
    let beyond = Footprint::new(GridPos::new(62, 50), 4, 4);
    assert!(!visible_to(&world, TeamId(1), beyond));
    assert!(!explored_by(&world, TeamId(1), beyond));
}

#[test]
fn a_team_without_live_entities_sees_nothing_but_keeps_exploration() {
    let (mut world, map) = lone_unit_world(TeamId(1), GridPos::new(20, 20));
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), GridPos::new(20, 20)));

    let entity = single_unit_entity(&mut world);
    world.despawn(entity);
    refresh_visibility(&mut world, &map);

    assert!(!visible_to(&world, TeamId(1), GridPos::new(20, 20)));
    assert!(explored_by(&world, TeamId(1), GridPos::new(20, 20)));
}

#[test]
fn predicates_report_full_information_when_visibility_map_is_absent() {
    let (world, _map) = seeded();
    assert!(visible_to(&world, TeamId(1), GridPos::new(112, 46)));
    assert!(explored_by(
        &world,
        TeamId(1),
        Footprint::new(GridPos::new(102, 48), 4, 4)
    ));
}

#[test]
fn packed_cell_states_report_the_row_major_payload_and_revision() {
    let (mut world, map) = lone_unit_world(TeamId(1), GridPos::new(20, 20));
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    let visibility = world.resource::<VisibilityMap>();

    let states = visibility.packed_cell_states(TeamId(1), map.width(), map.height());
    assert_eq!(states.len(), (128 * 96) as usize);
    // Row-major indexing: (x, y) reads as y * width + x. The lone unit's
    // own cell is Visible; the enemy start cell is Unexplored.
    assert_eq!(states[(20 * 128 + 20) as usize], 2);
    assert_eq!(states[(46 * 128 + 112) as usize], 0);
    assert!(visibility.revision() >= 1);

    // A team absent from the map reads as fully Unexplored.
    let enemy_states = visibility.packed_cell_states(TeamId(2), map.width(), map.height());
    assert!(enemy_states.iter().all(|&state| state == 0));
}

/// Review item: revealing from a building's edge cells only must cover
/// exactly the same union as revealing from every footprint cell — the
/// interior origins' circles are subsumed by the edge cells' circles. This
/// locks the equivalence the refresh optimization relies on.
#[test]
fn edge_cell_origins_reveal_the_same_union_as_every_cell() {
    let map = GridMap::new(64, 64);
    for (anchor, width, height) in [
        (GridPos::new(10, 10), 1_u8, 1_u8),
        (GridPos::new(20, 10), 2, 2),
        (GridPos::new(30, 10), 3, 3),
        (GridPos::new(40, 10), 4, 4),
        (GridPos::new(50, 10), 6, 4),
    ] {
        let footprint = Footprint::new(anchor, width, height);
        let from_all: HashSet<GridPos> = reveal_circle(&map, &footprint.cells());
        let from_edges: HashSet<GridPos> = reveal_circle(&map, &footprint.edge_cells());
        assert_eq!(
            from_all, from_edges,
            "edge origins must cover the same cells as all origins for {footprint:?}"
        );
    }
}

#[test]
fn a_destroyed_buildings_reveal_drops_on_the_next_refresh() {
    // The cached static union must rebuild when the completed-building set
    // shrinks — a lingering signature would keep a dead wall's circle
    // visible forever.
    let mut world = World::new();
    let map = GridMap::new(64, 64);
    let anchor = GridPos::new(30, 30);
    let footprint = Footprint::new(anchor, 1, 1);
    let entity = world
        .spawn((
            Building {
                id: crate::ids::BuildingId(1),
                team: TeamId(1),
                kind: BuildingKind::Wall,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            footprint,
        ))
        .id();
    world.insert_resource(VisibilityMap::default());
    let near_wall = GridPos::new(30 + VISION_RADIUS_CELLS - 1, 30);

    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), near_wall));
    // Unchanged refresh: the cache returns the same static union.
    refresh_visibility(&mut world, &map);
    assert!(visible_to(&world, TeamId(1), near_wall));

    world.despawn(entity);
    refresh_visibility(&mut world, &map);
    assert!(
        !visible_to(&world, TeamId(1), near_wall),
        "a despawned building must not keep revealing"
    );
    assert!(
        explored_by(&world, TeamId(1), near_wall),
        "explored state still retains the dead wall's reveal"
    );
}
