use super::*;
use bevy::math::Vec2;
use bevy::prelude::Entity;
use std::collections::HashSet;

use crate::buildings::Building;
use crate::catalog::resource_amount;
use crate::combat::CombatOrder;
use crate::commands::{UnitIndex, approach_slots};
use crate::economy::{ResourceIndex, ResourceSource, TeamEconomy, WorkerTask};
use crate::ids::IdAllocator;
use crate::map::Footprint;
use crate::movement::{SimPosition, Unit};

/// Team-1's Research Center anchors the authored central city: its 4×4
/// footprint center must sit on the map midpoint (64, 48), within half a
/// footprint of slack so a one-cell anchor shift still fails loudly.
#[test]
fn central_research_center_centers_on_the_map_midpoint() {
    let start = MapFixture::team_starts()
        .into_iter()
        .find(|start| start.team == TeamId(1))
        .expect("an authored team-1 start");
    let center = Footprint::new(start.research_center_anchor, 4, 4).center();

    assert!(
        center.distance(Vec2::new(64.0, 48.0)) <= 2.0,
        "team-1 Research Center center {center:?} is not the map midpoint (64,48)"
    );
}

/// The `RUINS` table is the ONLY authored blocker source: a fresh
/// battlefield must block exactly the union of its rectangles — nothing
/// more, nothing less, and every rect min/max in bounds.
#[test]
fn ruins_are_the_only_authored_blocked_cells() {
    let map = MapFixture::battlefield().map;

    let mut ruined: HashSet<GridPos> = HashSet::new();
    for (min, max) in RUINS {
        assert!(
            map.in_bounds(*min) && map.in_bounds(*max),
            "ruin rect {min:?}..={max:?} leaves the battlefield"
        );
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                ruined.insert(GridPos::new(x, y));
            }
        }
    }

    for y in 0..map.height() {
        for x in 0..map.width() {
            let cell = GridPos::new(x, y);
            assert_eq!(
                map.is_walkable(cell),
                !ruined.contains(&cell),
                "cell {cell:?}: walkability and the RUINS table disagree"
            );
        }
    }
}

/// Ruins decorate the outskirts of the central city: they may not sit on
/// any authored start, worker, resource, or benchmark staging region, and
/// the central construction/gather ring around the city stays clear.
#[test]
fn ruins_avoid_every_authored_occupant_and_staging_region() {
    let fixture = MapFixture::battlefield();
    let starts = MapFixture::team_starts();
    let mut off_limits: HashSet<GridPos> = starts
        .iter()
        .flat_map(|start| Footprint::new(start.research_center_anchor, 4, 4).cells())
        .chain(starts.iter().flat_map(|start| start.workers))
        .chain(fixture.resources.iter().map(|spawn| spawn.cell))
        .collect();
    // The row-major west staging scan (x 26..=53, y 20..=75).
    for y in 20..=75 {
        for x in 26..=53 {
            off_limits.insert(GridPos::new(x, y));
        }
    }
    // The city's own construction/gather ring: a generous rectangle around
    // the midpoint that holds the base, its safe sources, and build sites.
    for y in 38..=58 {
        for x in 52..=75 {
            off_limits.insert(GridPos::new(x, y));
        }
    }

    for (min, max) in RUINS {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let cell = GridPos::new(x, y);
                assert!(
                    !off_limits.contains(&cell),
                    "ruin cell {cell:?} overlaps authored geometry or staging"
                );
            }
        }
    }
}

/// Every map border reaches the city, and blocking one route cell leaves an
/// alternate route.
#[test]
fn battlefield_connects_every_entry_lane_to_the_city() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map;
    let starts = MapFixture::team_starts();
    // Block the seeded Research Centers the way the runtime seed does, so
    // routes must skirt the actual building footprints.
    for start in &starts {
        for cell in Footprint::new(start.research_center_anchor, 4, 4).cells() {
            map.set_blocked(cell, true);
        }
    }
    for resource in &fixture.resources {
        map.set_blocked(resource.cell, true);
    }

    let city = starts
        .iter()
        .find(|start| start.team == TeamId(1))
        .expect("the central city start");
    // A free west-face cell immediately beside the central Research Center.
    let city_gate = GridPos::new(city.research_center_anchor.x - 1, 47);
    assert!(map.is_walkable(city_gate));

    for (name, border) in [
        ("west", GridPos::new(0, 48)),
        ("east", GridPos::new(127, 48)),
        ("north", GridPos::new(64, 0)),
        ("south", GridPos::new(64, 95)),
    ] {
        assert!(map.is_walkable(border), "{name} border probe is blocked");
        assert!(
            map.find_path(border, city_gate).is_some(),
            "no path from the {name} border to the central city"
        );
    }

    // One blocked cell on the east entry route must leave an alternate
    // route through the open field.
    let route = map
        .find_path(city.workers[0], GridPos::new(127, 48))
        .expect("the east entry route exists");
    let interior = route[route.len() / 2];
    assert!(interior != city.workers[0] && interior != GridPos::new(127, 48));
    assert!(map.set_blocked(interior, true));
    let alternate = map
        .find_path(city.workers[0], GridPos::new(127, 48))
        .expect("an alternate route must exist");
    assert!(!alternate.contains(&interior));
}

#[test]
fn authored_alien_entry_zones_reach_the_central_research_center() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map;
    for start in MapFixture::team_starts() {
        for cell in Footprint::new(start.research_center_anchor, 4, 4).cells() {
            map.set_blocked(cell, true);
        }
    }
    for resource in &fixture.resources {
        map.set_blocked(resource.cell, true);
    }

    let core = MapFixture::team_starts()
        .into_iter()
        .find(|start| start.team == TeamId(1))
        .expect("the central team-1 start");
    let core_slots = approach_slots(
        &map,
        Footprint::new(core.research_center_anchor, 4, 4),
        &HashSet::new(),
        usize::MAX,
    );
    assert!(!core_slots.is_empty(), "the core has no walkable perimeter");

    for (zone, anchor) in ENTRY_ZONES {
        assert!(
            map.is_walkable(anchor),
            "{zone:?} anchor {anchor:?} is blocked"
        );
        assert!(
            core_slots
                .iter()
                .any(|slot| map.find_path(anchor, *slot).is_some()),
            "{zone:?} anchor {anchor:?} cannot reach the core perimeter"
        );
    }
}

/// Team-kind census over the live `UnitIndex` — counts only.
fn units_of_kind(world: &World, team: TeamId, kind: UnitKind) -> usize {
    sorted_unit_ids(world, team, |unit| unit.kind == kind).len()
}

#[test]
fn benchmark_workload_uses_the_shared_director_for_150_aliens_and_50_defenders() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();

    seed_skirmish(&mut world, &mut map, &fixture);
    extend_with_benchmark_workload(&mut world, &mut map);

    assert_eq!(units_of_kind(&world, TeamId(1), UnitKind::Worker), 4);
    assert_eq!(units_of_kind(&world, TeamId(1), UnitKind::Guard), 23);
    assert_eq!(units_of_kind(&world, TeamId(1), UnitKind::Rifleman), 23);
    let alien_count = [
        UnitKind::Swarmer,
        UnitKind::Spitter,
        UnitKind::Brute,
        UnitKind::Behemoth,
    ]
    .into_iter()
    .map(|kind| units_of_kind(&world, TeamId(2), kind))
    .sum::<usize>();
    assert_eq!(alien_count, 150);
    assert_eq!(world.resource::<TeamEconomy>().0.len(), 1);
    assert!(world.resource::<TeamEconomy>().0.contains_key(&TeamId(1)));
    assert_eq!(world.resource::<UnitIndex>().iter().count(), 200);

    // IDs flowed only through the allocator: 4 authored + 46 defenders + 150 aliens.
    assert_eq!(world.resource::<IdAllocator>().next_unit, 201);

    let mut worker_sources: Vec<(UnitId, ResourceId)> = world
        .resource::<UnitIndex>()
        .iter()
        .filter_map(|(id, entity)| match world.get::<WorkerTask>(*entity)? {
            WorkerTask::ToSource { source, .. } | WorkerTask::Gathering { source } => {
                Some((*id, *source))
            }
            _ => None,
        })
        .collect();
    worker_sources.sort_unstable_by_key(|(id, _)| *id);
    assert_eq!(
        worker_sources,
        vec![
            (UnitId(1), ResourceId(1)),
            (UnitId(2), ResourceId(2)),
            (UnitId(3), ResourceId(3)),
            (UnitId(4), ResourceId(4)),
        ],
        "benchmark workers must receive safe sources in stable unit-ID order"
    );

    // Only the human Research Center remains; all 18 sources survive.
    let research_centers = world
        .resource::<BuildingIndex>()
        .iter()
        .filter(|(_, entity)| {
            world.get::<Building>(**entity).is_some_and(|building| {
                building.kind == BuildingKind::ResearchCenter && building.construction.complete
            })
        })
        .count();
    assert_eq!(research_centers, 1);

    // The authored standalone resources are untouched.
    assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);

    // Every added unit stands in bounds on a walkable (non-authored-blocked)
    // cell — staging never parks a unit inside terrain, a Research Center or a
    // resource.
    for (_, entity) in world.resource::<UnitIndex>().iter() {
        let position = world.get::<SimPosition>(*entity).unwrap().current;
        let cell = map.world_to_cell(position);
        assert!(
            map.in_bounds(cell) && map.is_walkable(cell),
            "unit at {position:?} sits on invalid cell {cell:?}"
        );
    }

    let attack_movers = world
        .resource::<UnitIndex>()
        .iter()
        .filter(|(_, entity)| {
            matches!(
                world.get::<CombatOrder>(**entity),
                Some(CombatOrder::AttackMove { .. })
            )
        })
        .count();
    assert_eq!(
        attack_movers, 196,
        "all 46 defenders and 150 aliens have accepted AttackMove orders"
    );
    assert_eq!(
        world
            .resource::<UnitIndex>()
            .iter()
            .filter(|(_, entity)| matches!(
                world.get::<WorkerTask>(**entity),
                Some(WorkerTask::ToSource { .. } | WorkerTask::Gathering { .. })
            ))
            .count(),
        4,
        "the four human Workers have accepted real Gather orders"
    );
    assert_eq!(world.resource::<WaveState>().next_wave, 101);
}

#[test]
fn benchmark_workload_keeps_real_activity_for_the_sixty_second_sample() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(crate::visibility::VisibilityMap::default());
    crate::visibility::refresh_visibility(&mut world, &map);
    extend_with_benchmark_workload(&mut world, &mut map);

    for second in 1..=60 {
        let positions: std::collections::HashMap<_, _> = world
            .resource::<UnitIndex>()
            .iter()
            .map(|(id, entity)| (*id, world.get::<SimPosition>(*entity).unwrap().current))
            .collect();
        let strikes = world
            .get_resource::<crate::combat::CombatEventTotal>()
            .map_or(0, |total| total.0);
        for _ in 0..20 {
            step_waves(&mut world, &mut map, SIM_STEP_SECONDS);
            crate::combat::step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
            crate::movement::step_movement(&mut world, &map, SIM_STEP_SECONDS);
            crate::economy::step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
            crate::buildings::step_construction(&mut world, SIM_STEP_SECONDS);
            crate::production::step_production(&mut world, &mut map, SIM_STEP_SECONDS);
            crate::visibility::refresh_visibility(&mut world, &map);
        }
        let moving = world.resource::<UnitIndex>().iter().any(|(id, entity)| {
            positions.get(id).is_some_and(|before| {
                before.distance(world.get::<SimPosition>(*entity).unwrap().current) > 0.05
            })
        });
        let new_strikes = world
            .get_resource::<crate::combat::CombatEventTotal>()
            .map_or(0, |total| total.0);
        let humans = world
            .resource::<UnitIndex>()
            .iter()
            .filter(|(_, entity)| world.get::<Unit>(**entity).unwrap().team == TeamId(1))
            .count();
        let aliens = world
            .resource::<UnitIndex>()
            .iter()
            .filter(|(_, entity)| world.get::<Unit>(**entity).unwrap().team == TeamId(2))
            .count();
        // A contested second must show movement or combat; once either side
        // is extinct, quiet is the real end state rather than a frozen sim.
        assert!(
            moving || new_strikes > strikes || humans == 0 || aliens == 0,
            "benchmark went idle at second {second}: humans={humans}, live={}, buildings={}",
            world.resource::<UnitIndex>().iter().count(),
            world.resource::<BuildingIndex>().iter().count(),
        );
    }
}

/// The human reinforcements stage on unique cells clear of authored workers.
#[test]
fn benchmark_staging_cells_are_free_unique_and_clear_of_authored_occupancy() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let authored_units: HashSet<GridPos> = MapFixture::team_starts()
        .iter()
        .flat_map(|start| start.workers)
        .collect();

    let cells = benchmark_staging_cells(&map);
    assert_eq!(cells.len(), BENCHMARK_REINFORCEMENTS);
    let unique: HashSet<GridPos> = cells.iter().copied().collect();
    assert_eq!(unique.len(), cells.len(), "staging cells must be unique");
    for cell in &cells {
        assert!(map.in_bounds(*cell) && map.is_walkable(*cell));
        assert!(
            !authored_units.contains(cell),
            "staging cell {cell:?} coincides with an authored worker"
        );
    }
}

/// The authored human build anchors live on the `TeamStart` table itself:
/// Barracks/Depot slots equal the table fields and stay clear of authored
/// occupants and resources.
#[test]
fn authored_anchor_accessors_read_the_team_start_table() {
    let fixture = MapFixture::battlefield();
    let map = &fixture.map;
    let starts = MapFixture::team_starts();
    let occupied: HashSet<GridPos> = starts
        .iter()
        .flat_map(|start| Footprint::new(start.research_center_anchor, 4, 4).cells())
        .chain(starts.iter().flat_map(|start| start.workers))
        .chain(fixture.resources.iter().map(|spawn| spawn.cell))
        .collect();

    for start in &starts {
        assert_eq!(
            MapFixture::barracks_anchor(start.team),
            start.barracks_anchor,
            "barracks_anchor must read the TeamStart table"
        );
        assert_eq!(
            MapFixture::depot_anchor(start.team),
            start.depot_anchor,
            "depot_anchor must read the TeamStart table"
        );
        for (name, anchor, spec) in [
            (
                "barracks",
                start.barracks_anchor,
                building_spec(BuildingKind::Barracks),
            ),
            (
                "depot",
                start.depot_anchor,
                building_spec(BuildingKind::Depot),
            ),
        ] {
            for cell in Footprint::new(anchor, spec.width, spec.height).cells() {
                assert!(
                    map.in_bounds(cell) && map.is_walkable(cell) && !occupied.contains(&cell),
                    "team {:?} {name} cell {cell:?} must be in-bounds, walkable and clear",
                    start.team
                );
            }
        }
    }
}

#[test]
fn authored_team_and_resource_cells_are_in_bounds() {
    let map = MapFixture::battlefield().map;
    let resources = MapFixture::starting_resources()
        .into_iter()
        .chain(MapFixture::expansion_resources());

    for start in MapFixture::team_starts() {
        for cell in Footprint::new(start.research_center_anchor, 4, 4).cells() {
            assert!(
                map.in_bounds(cell),
                "research center cell {cell:?} out of bounds"
            );
        }
        for cell in start.workers {
            assert!(map.in_bounds(cell), "worker cell {cell:?} out of bounds");
        }
    }
    for resource in resources {
        assert!(
            map.in_bounds(resource.cell),
            "resource {:?} out of bounds",
            resource.cell
        );
    }
}

/// Every authored resource cell is unique and avoids the seeded
/// buildings, the worker starts, and the ruins table.
#[test]
fn resources_never_overlap_buildings_workers_or_ruins() {
    let starts = MapFixture::team_starts();
    let research_center_cells: HashSet<GridPos> = starts
        .iter()
        .flat_map(|start| Footprint::new(start.research_center_anchor, 4, 4).cells())
        .collect();
    let worker_cells: HashSet<GridPos> = starts.iter().flat_map(|start| start.workers).collect();
    let ruin_cells: HashSet<GridPos> = RUINS
        .iter()
        .flat_map(|(min, max)| {
            (min.y..=max.y).flat_map(|y| (min.x..=max.x).map(move |x| GridPos::new(x, y)))
        })
        .collect();
    let resources = MapFixture::starting_resources()
        .into_iter()
        .chain(MapFixture::expansion_resources());

    let mut seen: HashSet<GridPos> = HashSet::new();
    for resource in resources {
        assert!(seen.insert(resource.cell), "duplicate resource cell");
        assert!(
            !research_center_cells.contains(&resource.cell),
            "resource {:?} overlaps a research center",
            resource.cell
        );
        assert!(
            !worker_cells.contains(&resource.cell),
            "resource {:?} overlaps a worker start",
            resource.cell
        );
        assert!(
            !ruin_cells.contains(&resource.cell),
            "resource {:?} overlaps a ruin",
            resource.cell
        );
    }
}

/// Safe-source membership is authored on the `TeamStart` table — never an
/// `x < 64` side inference. Each team's six ids must resolve to real
/// source records, cover all three gatherable kinds, and sit near the
/// team's own base.
#[test]
fn safe_source_membership_is_explicit_and_complete() {
    let resources: std::collections::HashMap<ResourceId, ResourceSpawn> =
        MapFixture::starting_resources()
            .into_iter()
            .chain(MapFixture::expansion_resources())
            .map(|spawn| (spawn.id, spawn))
            .collect();

    for start in MapFixture::team_starts() {
        let resolved: Vec<ResourceSpawn> = MapFixture::safe_source_ids(start.team)
            .into_iter()
            .map(|id| {
                *resources
                    .get(&id)
                    .unwrap_or_else(|| panic!("{id:?} not authored"))
            })
            .collect();
        assert_eq!(
            resolved.len(),
            6,
            "team {:?} safe membership must hold six sources",
            start.team
        );
        for kind in [ResourceKind::Food, ResourceKind::Scrap, ResourceKind::Coal] {
            assert!(
                resolved.iter().any(|spawn| spawn.kind == kind),
                "team {:?} safe membership lacks {kind:?}",
                start.team
            );
        }
        // Membership is a set: no id may repeat, and the ids must equal the
        // TeamStart table's own array.
        let unique: HashSet<ResourceId> = resolved.iter().map(|spawn| spawn.id).collect();
        assert_eq!(unique.len(), 6, "safe-source ids repeat");
        assert_eq!(
            unique,
            start.safe_sources.into_iter().collect::<HashSet<_>>(),
            "safe_source_ids must read the TeamStart table"
        );
        // Near the team's own base: every safe source inside vision range
        // of the Research Center's edge.
        let core = Footprint::new(start.research_center_anchor, 4, 4);
        for spawn in resolved {
            let near = core.edge_cells().iter().any(|edge| {
                let dx = edge.x - spawn.cell.x;
                let dy = edge.y - spawn.cell.y;
                dx * dx + dy * dy <= crate::catalog::VISION_RADIUS_CELLS.pow(2)
            });
            assert!(
                near,
                "safe source {:?} is not near team {:?}'s base",
                spawn.cell, start.team
            );
        }
    }
}

#[test]
fn worker_starts_stay_walkable_and_adjacent_to_their_research_center() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map;
    for start in MapFixture::team_starts() {
        for cell in Footprint::new(start.research_center_anchor, 4, 4).cells() {
            map.set_blocked(cell, true);
        }
    }

    for start in MapFixture::team_starts() {
        let footprint = Footprint::new(start.research_center_anchor, 4, 4);
        for cell in start.workers {
            assert!(
                map.is_walkable(cell),
                "worker cell {cell:?} blocked by its research center or the ruins"
            );
            assert!(
                footprint.is_immediately_adjacent(cell),
                "worker cell {cell:?} is not adjacent to its Research Center"
            );
        }
    }
}

/// Legal approaches and drop-offs on the fully seeded map: every authored
/// source keeps a reachable, walkable perimeter cell from its owning
/// team's workers; every drop-off/build anchor keeps a full ring of
/// approach cells.
#[test]
fn every_authored_source_and_build_site_keeps_reachable_approaches() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    let starts = MapFixture::team_starts();
    let reachable_slots = |from: GridPos, footprint: Footprint| {
        footprint
            .perimeter_cells()
            .into_iter()
            .filter(|cell| map.is_walkable(*cell))
            .filter(|cell| map.find_path(from, *cell).is_some())
            .count()
    };

    // Safe sources: reachable from the owning team's first worker.
    for start in &starts {
        for id in MapFixture::safe_source_ids(start.team) {
            let spawn = fixture
                .resources
                .iter()
                .find(|spawn| spawn.id == id)
                .expect("safe membership resolves");
            let footprint = Footprint::new(spawn.cell, 1, 1);
            let slots = reachable_slots(start.workers[0], footprint);
            assert!(
                slots >= 3,
                "safe source {:?} keeps only {slots} reachable approach cells",
                spawn.cell
            );
        }
    }

    // Expansions: reachable from the central city's workers.
    let city_worker = starts[0].workers[0];
    for spawn in MapFixture::expansion_resources() {
        let footprint = Footprint::new(spawn.cell, 1, 1);
        let slots = reachable_slots(city_worker, footprint);
        assert!(
            slots >= 3,
            "expansion {:?} keeps only {slots} reachable approach cells",
            spawn.cell
        );
    }

    // Drop-offs: each Research Center and each authored Depot anchor keeps
    // a healthy ring of reachable slots.
    for start in &starts {
        let core = Footprint::new(start.research_center_anchor, 4, 4);
        let slots = reachable_slots(start.workers[0], core);
        assert!(
            slots >= 6,
            "team {:?} Research Center keeps only {slots} reachable drop-off slots",
            start.team
        );
        for (name, anchor, spec) in [
            (
                "barracks",
                start.barracks_anchor,
                building_spec(BuildingKind::Barracks),
            ),
            (
                "depot",
                start.depot_anchor,
                building_spec(BuildingKind::Depot),
            ),
        ] {
            let footprint = Footprint::new(anchor, spec.width, spec.height);
            let slots = reachable_slots(start.workers[0], footprint);
            assert!(
                slots >= 4,
                "team {:?} {name} site keeps only {slots} reachable approach cells",
                start.team
            );
        }
    }
}

#[test]
fn survival_seed_creates_one_real_human_core_four_workers_and_economy() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();

    seed_skirmish(&mut world, &mut map, &fixture);

    let economies = &world.resource::<TeamEconomy>().0;
    assert_eq!(economies.len(), 1, "only the human economy is seeded");
    assert_eq!(
        economies[&TeamId(1)].stockpile,
        ResourceStockpile {
            food: 200,
            scrap: 300,
            coal: 150,
            crystals: 0,
        }
    );

    // One completed human Research Center owns a 4×4 Footprint, Dropoff,
    // full catalogue Health, and blocked map cells.
    let mut research_centers = world.query::<(&Building, &Footprint, &Dropoff, &Health)>();
    let research_centers: Vec<_> = research_centers.iter(&world).collect();
    assert_eq!(research_centers.len(), 1);
    for (building, footprint, dropoff, health) in &research_centers {
        assert_eq!(building.kind, BuildingKind::ResearchCenter);
        assert!(building.construction.complete);
        assert_eq!(building.construction.active_builder, None);
        assert_eq!((footprint.width, footprint.height), (4, 4));
        assert_eq!(dropoff.team, building.team);
        let max_health = building_spec(building.kind).max_health;
        assert_eq!(
            **health,
            Health {
                current: max_health,
                max: max_health
            }
        );
        for cell in footprint.cells() {
            assert!(
                !map.is_walkable(cell),
                "Research Center cell {cell:?} unblocked"
            );
        }
    }
    assert_eq!(
        research_centers
            .iter()
            .map(|(b, ..)| b.team)
            .collect::<HashSet<_>>()
            .len(),
        1,
        "the survival fixture seeds one human Research Center"
    );

    // Every Dropoff lives on a real Building entity — no temporary
    // Dropoff-only Research Center exists.
    let mut dropoffs = world.query::<(Entity, &Dropoff)>();
    for (entity, _) in dropoffs.iter(&world) {
        assert!(world.get::<Building>(entity).is_some());
    }

    // Four human workers with fresh worker components, standing on
    // walkable authored cells.
    let mut workers = world.query::<(&Unit, &SimPosition, &Carry, &GatherProgress, &WorkerTask)>();
    let workers: Vec<_> = workers
        .iter(&world)
        .filter(|(unit, ..)| unit.kind == UnitKind::Worker)
        .collect();
    assert_eq!(workers.len(), 4);
    assert!(workers.iter().all(|(unit, ..)| unit.team == TeamId(1)));
    assert!(workers.iter().all(|(_, position, carry, progress, task)| {
        map.is_walkable(map.world_to_cell(position.current))
            && **carry == Carry::Empty
            && progress.0 == 0.0
            && **task == WorkerTask::Idle
    }));

    // Eighteen authored finite sources: 1×1 blocked footprints, full
    // authored amounts, registered in the index.
    let mut sources = world.query::<(&ResourceSource, &Footprint)>();
    let sources: Vec<_> = sources.iter(&world).collect();
    assert_eq!(sources.len(), 18);
    for (source, footprint) in &sources {
        assert_eq!((footprint.width, footprint.height), (1, 1));
        assert_eq!(source.remaining, resource_amount(source.kind));
        assert!(
            !map.is_walkable(footprint.anchor),
            "source cell {:?} unblocked",
            footprint.anchor
        );
    }
    assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);

    // Indexes and allocator counters sit above the authored maxima
    // (4 units, 1 building, resource IDs 1..=18).
    assert_eq!(world.resource::<UnitIndex>().iter().count(), 4);
    assert_eq!(world.resource::<BuildingIndex>().iter().count(), 1);
    assert_eq!(*world.resource::<WaveState>(), WaveState::default());
    let allocator = world.resource::<IdAllocator>();
    assert_eq!(
        (
            allocator.next_unit,
            allocator.next_building,
            allocator.next_resource
        ),
        (5, 2, 19)
    );
}
