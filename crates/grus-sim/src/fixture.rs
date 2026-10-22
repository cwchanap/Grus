use bevy::math::Vec2;
use bevy::prelude::World;

use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    Age, BuildingKind, ResourceKind, UnitKind, building_spec, resource_amount, unit_spec,
};
use crate::combat::Health;
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, apply_player_command, spawn_unit, team_kind_ids,
};
use crate::economy::{
    Carry, Dropoff, GatherProgress, ResourceStockpile, TeamEconomy, WorkerTask,
    spawn_resource_source,
};
use crate::ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};

/// Authored starting position of one team: Town Center anchor plus the four
/// villager spawn cells. All values are cells on the 128×96 battlefield.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamStart {
    pub team: TeamId,
    pub town_center_anchor: GridPos,
    pub villagers: [GridPos; 4],
}

/// One authored finite resource node with a deterministic `ResourceId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    pub left_spawn: Vec2,
    pub right_spawn: Vec2,
    /// Authored finite resource nodes for the skirmish seed.
    pub resources: Vec<ResourceSpawn>,
}

impl MapFixture {
    /// The single authored team-start table. Anchors and villager cells come
    /// from the HPA-471 design; Town Centers are 4×4 footprints.
    pub fn team_starts() -> [TeamStart; 2] {
        [
            TeamStart {
                team: TeamId(1),
                town_center_anchor: GridPos::new(12, 46),
                villagers: [
                    GridPos::new(11, 45),
                    GridPos::new(11, 50),
                    GridPos::new(16, 45),
                    GridPos::new(16, 50),
                ],
            },
            TeamStart {
                team: TeamId(2),
                town_center_anchor: GridPos::new(112, 46),
                villagers: [
                    GridPos::new(116, 45),
                    GridPos::new(116, 50),
                    GridPos::new(111, 45),
                    GridPos::new(111, 50),
                ],
            },
        ]
    }

    /// Safe resources around each Town Center, mirrored between teams.
    /// The only resource-position table in the codebase; IDs are
    /// deterministic (team 1 safe, then team 2 safe).
    pub fn starting_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(1), ResourceKind::Food, 22, 42),
            spawn(ResourceId(2), ResourceKind::Food, 22, 54),
            spawn(ResourceId(3), ResourceKind::Wood, 20, 45),
            spawn(ResourceId(4), ResourceKind::Wood, 20, 48),
            spawn(ResourceId(5), ResourceKind::Wood, 20, 51),
            spawn(ResourceId(6), ResourceKind::Gold, 25, 48),
            spawn(ResourceId(7), ResourceKind::Food, 105, 42),
            spawn(ResourceId(8), ResourceKind::Food, 105, 54),
            spawn(ResourceId(9), ResourceKind::Wood, 107, 45),
            spawn(ResourceId(10), ResourceKind::Wood, 107, 48),
            spawn(ResourceId(11), ResourceKind::Wood, 107, 51),
            spawn(ResourceId(12), ResourceKind::Gold, 102, 48),
        ]
    }

    /// Southwest and northeast expansion resources; IDs continue after the
    /// safe sources.
    pub fn expansion_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(13), ResourceKind::Gold, 45, 16),
            spawn(ResourceId(14), ResourceKind::Wood, 43, 18),
            spawn(ResourceId(15), ResourceKind::Wood, 47, 18),
            spawn(ResourceId(16), ResourceKind::Gold, 82, 79),
            spawn(ResourceId(17), ResourceKind::Wood, 84, 77),
            spawn(ResourceId(18), ResourceKind::Wood, 80, 77),
        ]
    }

    pub fn battlefield() -> Self {
        let mut map = GridMap::new(128, 96);

        // Mirrored central blockers leave a northern and southern route.
        map.set_blocked_rect(GridPos::new(54, 20), GridPos::new(61, 75));
        map.set_blocked_rect(GridPos::new(66, 20), GridPos::new(73, 75));
        map.set_blocked_rect(GridPos::new(62, 38), GridPos::new(65, 57));

        Self {
            map,
            left_spawn: Vec2::new(14.5, 48.5),
            right_spawn: Vec2::new(113.5, 48.5),
            resources: Self::starting_resources()
                .into_iter()
                .chain(Self::expansion_resources())
                .collect(),
        }
    }

    /// The authored Barracks anchor of one team's base layout (3×3
    /// footprint): clear of every authored start cell and resource node on
    /// both starts. Team 2 mirrors the team-1 anchor on the
    /// vertical center line (footprint-width aware).
    pub fn barracks_anchor(team: TeamId) -> GridPos {
        let anchor = GridPos::new(17, 51);
        if team == TeamId(2) {
            mirror_anchor(anchor, building_spec(BuildingKind::Barracks).width)
        } else {
            anchor
        }
    }

    /// The authored safe Storehouse anchor of one team's base layout (2×2
    /// footprint), beside the Town Center's resource line. Team 2 mirrors
    /// the team-1 anchor on the vertical center line (footprint-width
    /// aware).
    pub fn safe_storehouse_anchor(team: TeamId) -> GridPos {
        let anchor = GridPos::new(21, 46);
        if team == TeamId(2) {
            mirror_anchor(anchor, building_spec(BuildingKind::Storehouse).width)
        } else {
            anchor
        }
    }
}

/// The default skirmish seed: Team 1 at the authored left start, Team 2 at
/// the authored right start. The pure simulation skirmish seed for one
/// normal match: real completed Town Center Buildings (4×4 Footprints +
/// `Dropoff` from birth), four Villagers per team with fresh worker
/// components, both teams' economies (200 Food / 300 Wood / 100 Gold, Age
/// 1), Town Center map occupancy, the authored finite resource sources, and
/// `UnitIndex`/`BuildingIndex` `IdAllocator` counters above the authored
/// maxima. Every starting entity flows through the production constructors —
/// there is no temporary Dropoff-only Town Center.
pub fn seed_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture) {
    let starts = MapFixture::team_starts();
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    economy.insert_team(
        TeamId(2),
        ResourceStockpile {
            food: 200,
            wood: 300,
            gold: 100,
        },
        Age::Age1,
    );
    world.insert_resource(economy);

    let mut unit_counter: u32 = 0;
    let town_center_health = building_spec(BuildingKind::TownCenter).max_health;
    for (index, start) in starts.into_iter().enumerate() {
        let building_id = BuildingId(index as u32 + 1);
        let footprint = Footprint::new(start.town_center_anchor, 4, 4);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id: building_id,
                    team: start.team,
                    kind: BuildingKind::TownCenter,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                footprint,
                Dropoff { team: start.team },
                Health {
                    current: town_center_health,
                    max: town_center_health,
                },
            ))
            .id();
        let mut buildings = world.get_resource_or_insert_with(BuildingIndex::default);
        buildings.insert(building_id, entity);

        for cell in start.villagers {
            unit_counter += 1;
            let villager = spawn_unit(
                world,
                UnitId(unit_counter),
                start.team,
                map.cell_center(cell),
                UnitKind::Villager,
                unit_spec(UnitKind::Villager).speed,
            );
            world.entity_mut(villager).insert((
                Carry::Empty,
                GatherProgress::default(),
                WorkerTask::Idle,
            ));
        }
    }

    // Authored finite sources (deterministic IDs 1..=18) block their cells
    // and register in the resource index.
    for resource in &fixture.resources {
        spawn_resource_source(
            world,
            map,
            resource.id,
            resource.kind,
            resource.cell,
            resource.amount,
        );
    }

    world.insert_resource(IdAllocator::new(unit_counter + 1, 3, 19));
}

/// Horizontal mirror of a `width`-wide footprint anchor on the 128-wide
/// battlefield: cells `x..x+w` map to `127-(x+w-1)..127-x`.
fn mirror_anchor(anchor: GridPos, width: u8) -> GridPos {
    GridPos::new(127 - anchor.x - i32::from(width) + 1, anchor.y)
}

/// Point reflection through the map center — the axis pair the battlefield
/// blockers and the expansion resources are symmetric under.
fn point_reflect(cell: GridPos) -> GridPos {
    GridPos::new(127 - cell.x, 95 - cell.y)
}

fn spawn(id: ResourceId, kind: ResourceKind, x: i32, y: i32) -> ResourceSpawn {
    ResourceSpawn {
        id,
        kind,
        cell: GridPos::new(x, y),
        amount: resource_amount(kind),
    }
}

/// Per-team workload roster on top of the four authored villagers.
const BENCHMARK_ROSTER: [(UnitKind, usize); 4] = [
    (UnitKind::Villager, 16),
    (UnitKind::Spearman, 30),
    (UnitKind::Archer, 30),
    (UnitKind::Cavalry, 20),
];

/// The shared AttackMove target: the authored northern midfield corridor
/// waypoint (team-1 scout route leg) — walkable, center-map, and never a
/// Town Center, so the benchmark load fights in the field instead of ending
/// the match.
const BENCHMARK_MIDFIELD: GridPos = GridPos::new(64, 12);

/// Extends an already-seeded **non-mirrored** skirmish with the benchmark
/// workload: 96 more units per team (the roster above) staged on
/// deterministic mirrored walkable cells — team 2 point-reflects team 1's
/// cells through the map center — IDs taken only from the `IdAllocator`, the
/// production worker bundle on every spawn, then the initial commands
/// applied through the ordinary dispatcher: the first 12 sorted villagers
/// per side go two-per-source onto the authored safe sources, and every
/// combatant AttackMoves to `BENCHMARK_MIDFIELD`. Panics on any rejected
/// setup command: a benchmark that boots with rejected orders would
/// silently measure an idle crowd.
///
/// Requires the non-mirrored seed: staging assigns team 1 the left region
/// and `safe_source_ids` assigns team 1 the left-half sources, so a
/// mirrored seed would stage team 1 inside team 2's base and gather it to
/// the wrong side. That precondition is asserted, not assumed.
pub fn extend_with_benchmark_workload(world: &mut World, map: &mut GridMap) {
    assert_non_mirrored_seed(world);
    for team in [TeamId(1), TeamId(2)] {
        let mut cells = benchmark_staging_cells(map, team).into_iter();
        for (kind, count) in BENCHMARK_ROSTER {
            for _ in 0..count {
                let cell = cells.next().expect("staging capacity");
                let unit_id = world.resource_mut::<IdAllocator>().allocate_unit();
                let spawned = spawn_unit(
                    world,
                    unit_id,
                    team,
                    map.cell_center(cell),
                    kind,
                    unit_spec(kind).speed,
                );
                world.entity_mut(spawned).insert((
                    Carry::Empty,
                    GatherProgress::default(),
                    WorkerTask::Idle,
                ));
            }
        }
    }

    // Membership resolves from the live world, not spawn-order bookkeeping.
    let target = map.cell_center(BENCHMARK_MIDFIELD);
    for team in [TeamId(1), TeamId(2)] {
        let workers: Vec<UnitId> = team_kind_ids(world, team, UnitKind::Villager)
            .into_iter()
            .take(12)
            .collect();
        for (pair, source) in workers.chunks(2).zip(safe_source_ids(team)) {
            accept(
                world,
                map,
                PlayerCommand::Gather {
                    issuer: team,
                    workers: pair.to_vec(),
                    source,
                },
            );
        }
        let army: Vec<UnitId> = [UnitKind::Spearman, UnitKind::Archer, UnitKind::Cavalry]
            .into_iter()
            .flat_map(|kind| team_kind_ids(world, team, kind))
            .collect();
        accept(
            world,
            map,
            PlayerCommand::Units(UnitCommand {
                issuer: team,
                units: army,
                kind: UnitCommandKind::AttackMove { target },
            }),
        );
    }
}

/// The benchmark workload's non-mirrored precondition: each team's seeded
/// Town Center stands on its own authored start, so the left-region
/// staging and left-half safe sources really belong to team 1.
fn assert_non_mirrored_seed(world: &World) {
    for start in MapFixture::team_starts() {
        let anchored = world.resource::<BuildingIndex>().iter().any(|(_, entity)| {
            world.get::<Building>(*entity).is_some_and(|building| {
                building.team == start.team
                    && building.kind == BuildingKind::TownCenter
                    && world
                        .get::<Footprint>(*entity)
                        .is_some_and(|footprint| footprint.anchor == start.town_center_anchor)
            })
        });
        assert!(
            anchored,
            "benchmark workload requires the non-mirrored authored seed: \
             team {:?} has no Town Center on its own start",
            start.team
        );
    }
}

/// Applies one deterministic setup command and panics unless it was fully
/// accepted (no top-level reject, no rejected units).
fn accept(world: &mut World, map: &mut GridMap, command: PlayerCommand) {
    let result = apply_player_command(world, map, command);
    assert!(
        result.reject.is_none() && result.rejected_units.is_empty(),
        "benchmark setup command rejected: {result:?}"
    );
}

/// The first `BENCHMARK_ROSTER` units per team stage on the first 96
/// row-major walkable cells of x 26..=53, y 20..=75 — a region holding no
/// authored blocker, Town Center or source — and team 2 mirrors them by
/// point reflection through the map center (the axis the authored map is
/// symmetric under).
fn benchmark_staging_cells(map: &GridMap, team: TeamId) -> Vec<GridPos> {
    if team == TeamId(2) {
        let cells: Vec<GridPos> = benchmark_staging_cells(map, TeamId(1))
            .into_iter()
            .map(point_reflect)
            .collect();
        // The reflected cells are walkable only while the map keeps the
        // authored point-reflection symmetry — check instead of assuming
        // so a fixture retune fails loudly here instead of spawning team 2
        // into terrain.
        assert!(
            cells.iter().all(|cell| map.is_walkable(*cell)),
            "team-2 staging reflected a walkable team-1 cell onto blocked ground"
        );
        return cells;
    }
    let capacity: usize = BENCHMARK_ROSTER.iter().map(|(_, count)| count).sum();
    let mut cells = Vec::with_capacity(capacity);
    'scan: for y in 20..=75 {
        for x in 26..=53 {
            let cell = GridPos::new(x, y);
            if map.is_walkable(cell) {
                cells.push(cell);
                if cells.len() == capacity {
                    break 'scan;
                }
            }
        }
    }
    assert_eq!(
        cells.len(),
        capacity,
        "the staging region lost walkable cells"
    );
    cells
}

/// The authored safe (starting) sources of one team's half, in ID order.
fn safe_source_ids(team: TeamId) -> Vec<ResourceId> {
    MapFixture::starting_resources()
        .into_iter()
        .filter(|spawn| {
            let left_half = spawn.cell.x < 64;
            left_half == (team == TeamId(1))
        })
        .map(|spawn| spawn.id)
        .collect()
}

#[cfg(test)]
mod tests;
