use bevy::prelude::World;

use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    BuildingKind, ResourceKind, UnitKind, building_spec, resource_amount, unit_spec,
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

/// Authored starting layout of one team: Research Center anchor, the four
/// worker spawn cells, the team's authored Barracks/Depot build anchors,
/// and the six safe starting resource ids. All values are cells on the
/// 128×96 battlefield; every per-team coordinate lives on this one table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamStart {
    pub team: TeamId,
    pub research_center_anchor: GridPos,
    pub workers: [GridPos; 4],
    pub barracks_anchor: GridPos,
    pub depot_anchor: GridPos,
    pub safe_sources: [ResourceId; 6],
}

/// One authored finite resource node with a deterministic `ResourceId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}

/// The single authored blocker table for the battlefield — sparse ruins
/// around the central city, each pair a rectangle's minimum and INCLUSIVE
/// maximum cell. `MapFixture::battlefield` loops exactly this data through
/// `GridMap::set_blocked_rect`, and the Godot bridge hands the same
/// rectangles to the presentation layer: nothing may copy the coordinates.
/// Every rect stays clear of the central construction/gather ring, the
/// N/E/S/W access lanes, both bases' authored occupants, and the two
/// benchmark staging regions (x 26..=53 y 20..=75 and its point
/// reflection) — the fixture tests prove each of those invariants.
pub const RUINS: &[(GridPos, GridPos)] = &[
    (GridPos::new(48, 12), GridPos::new(51, 14)),
    (GridPos::new(64, 24), GridPos::new(66, 26)),
    (GridPos::new(78, 10), GridPos::new(81, 12)),
    (GridPos::new(16, 30), GridPos::new(19, 33)),
    (GridPos::new(56, 30), GridPos::new(58, 33)),
    (GridPos::new(34, 82), GridPos::new(37, 85)),
    (GridPos::new(84, 80), GridPos::new(88, 83)),
];

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    /// Authored finite resource nodes for the skirmish seed.
    pub resources: Vec<ResourceSpawn>,
}

impl MapFixture {
    /// The single authored team-start table. Team 1's Research Center
    /// anchors the central city: the (62,46) 4×4 footprint centers exactly
    /// on the map midpoint (64,48), its four workers occupy the free
    /// diagonal-corner cells, and its Barracks/Depot build anchors hug the
    /// safe ring. Team 2 keeps the passive eastern base; every per-team
    /// coordinate lives here — nothing mirrors the human site.
    pub fn team_starts() -> [TeamStart; 2] {
        [
            TeamStart {
                team: TeamId(1),
                research_center_anchor: GridPos::new(62, 46),
                workers: [
                    GridPos::new(61, 45),
                    GridPos::new(61, 50),
                    GridPos::new(66, 45),
                    GridPos::new(66, 50),
                ],
                barracks_anchor: GridPos::new(58, 46),
                depot_anchor: GridPos::new(63, 51),
                safe_sources: [
                    ResourceId(1),
                    ResourceId(2),
                    ResourceId(3),
                    ResourceId(4),
                    ResourceId(5),
                    ResourceId(6),
                ],
            },
            TeamStart {
                team: TeamId(2),
                research_center_anchor: GridPos::new(112, 46),
                workers: [
                    GridPos::new(116, 45),
                    GridPos::new(116, 50),
                    GridPos::new(111, 45),
                    GridPos::new(111, 50),
                ],
                barracks_anchor: GridPos::new(108, 51),
                depot_anchor: GridPos::new(117, 51),
                safe_sources: [
                    ResourceId(7),
                    ResourceId(8),
                    ResourceId(9),
                    ResourceId(10),
                    ResourceId(11),
                    ResourceId(12),
                ],
            },
        ]
    }

    /// The safe sources ringing each Research Center — ration caches,
    /// salvage ruins, and a coal seam within each base's vision. The only
    /// resource-position table in the codebase; IDs are deterministic
    /// (team 1 safe 1-6 near the central city, then team 2 safe 7-12 east).
    pub fn starting_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(1), ResourceKind::Food, 58, 42),
            spawn(ResourceId(2), ResourceKind::Food, 69, 42),
            spawn(ResourceId(3), ResourceKind::Scrap, 56, 52),
            spawn(ResourceId(4), ResourceKind::Scrap, 71, 52),
            spawn(ResourceId(5), ResourceKind::Scrap, 64, 55),
            spawn(ResourceId(6), ResourceKind::Coal, 64, 41),
            spawn(ResourceId(7), ResourceKind::Food, 108, 42),
            spawn(ResourceId(8), ResourceKind::Food, 117, 42),
            spawn(ResourceId(9), ResourceKind::Scrap, 107, 52),
            spawn(ResourceId(10), ResourceKind::Scrap, 120, 52),
            spawn(ResourceId(11), ResourceKind::Scrap, 113, 55),
            spawn(ResourceId(12), ResourceKind::Coal, 113, 41),
        ]
    }

    /// Northwest and southeast expansion resources; IDs continue after the
    /// safe sources.
    pub fn expansion_resources() -> Vec<ResourceSpawn> {
        vec![
            spawn(ResourceId(13), ResourceKind::Coal, 45, 16),
            spawn(ResourceId(14), ResourceKind::Scrap, 43, 18),
            spawn(ResourceId(15), ResourceKind::Scrap, 47, 18),
            spawn(ResourceId(16), ResourceKind::Coal, 82, 79),
            spawn(ResourceId(17), ResourceKind::Scrap, 84, 77),
            spawn(ResourceId(18), ResourceKind::Scrap, 80, 77),
        ]
    }

    pub fn battlefield() -> Self {
        let mut map = GridMap::new(128, 96);

        // The authored ruins are the only static blockers.
        for &(min, max) in RUINS {
            map.set_blocked_rect(min, max);
        }

        Self {
            map,
            resources: Self::starting_resources()
                .into_iter()
                .chain(Self::expansion_resources())
                .collect(),
        }
    }

    /// The `TeamStart` row for a team. Panics for ids outside the authored
    /// table — an unknown team is a fixture bug, never a silent borrow of
    /// the human start.
    fn team_start(team: TeamId) -> TeamStart {
        Self::team_starts()
            .into_iter()
            .find(|start| start.team == team)
            .unwrap_or_else(|| panic!("no authored TeamStart row for {team:?}"))
    }

    /// The authored Barracks anchor of one team's base layout (3×3
    /// footprint), read from the `TeamStart` table.
    pub fn barracks_anchor(team: TeamId) -> GridPos {
        Self::team_start(team).barracks_anchor
    }

    /// The authored safe Depot anchor of one team's base layout (2×2
    /// footprint), read from the `TeamStart` table.
    pub fn depot_anchor(team: TeamId) -> GridPos {
        Self::team_start(team).depot_anchor
    }

    /// The authored safe (starting) source ids of one team, read from the
    /// `TeamStart` table — explicit membership, never a side inference.
    pub fn safe_source_ids(team: TeamId) -> [ResourceId; 6] {
        Self::team_start(team).safe_sources
    }
}

/// The default skirmish seed: Team 1 at the authored central start, Team 2
/// at the authored eastern start. The pure simulation skirmish seed for one
/// normal match: real completed Research Center Buildings (4×4 Footprints +
/// `Dropoff` from birth), four Workers per team with fresh worker
/// components, both teams' economies (200 Food / 300 Scrap / 150 Coal /
/// 0 Crystals), Research Center map occupancy, the authored finite resource
/// sources, and `UnitIndex`/`BuildingIndex` `IdAllocator` counters above the
/// authored maxima. Every starting entity flows through the production
/// constructors — there is no temporary Dropoff-only Research Center.
pub fn seed_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture) {
    let starts = MapFixture::team_starts();
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 200,
            scrap: 300,
            coal: 150,
            crystals: 0,
        },
    );
    economy.insert_team(
        TeamId(2),
        ResourceStockpile {
            food: 200,
            scrap: 300,
            coal: 150,
            crystals: 0,
        },
    );
    world.insert_resource(economy);

    let mut unit_counter: u32 = 0;
    let research_center_health = building_spec(BuildingKind::ResearchCenter).max_health;
    for (index, start) in starts.into_iter().enumerate() {
        let building_id = BuildingId(index as u32 + 1);
        let footprint = Footprint::new(start.research_center_anchor, 4, 4);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }
        let entity = world
            .spawn((
                Building {
                    id: building_id,
                    team: start.team,
                    kind: BuildingKind::ResearchCenter,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                footprint,
                Dropoff { team: start.team },
                Health {
                    current: research_center_health,
                    max: research_center_health,
                },
            ))
            .id();
        let mut buildings = world.get_resource_or_insert_with(BuildingIndex::default);
        buildings.insert(building_id, entity);

        for cell in start.workers {
            unit_counter += 1;
            let worker = spawn_unit(
                world,
                UnitId(unit_counter),
                start.team,
                map.cell_center(cell),
                UnitKind::Worker,
                unit_spec(UnitKind::Worker).speed,
            );
            world.entity_mut(worker).insert((
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

/// Point reflection through the map center — used only to stage the east
/// benchmark army's cells from the west scan (local staging reflection,
/// not a map-symmetry contract).
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

/// Per-team workload roster on top of the four authored Workers.
const BENCHMARK_ROSTER: [(UnitKind, usize); 3] = [
    (UnitKind::Worker, 16),
    (UnitKind::Guard, 40),
    (UnitKind::Rifleman, 40),
];

/// The shared AttackMove target: the authored northern midfield corridor
/// waypoint — walkable, center-map, and never a Research Center. With the
/// city now mid-map, team 2's march from the reflected east staging cells
/// passes inside the 8-cell acquisition radius of the central base, so the
/// measured load is a raid on the city rather than a pure field fight —
/// and numbers recorded before the relocation are not comparable.
const BENCHMARK_MIDFIELD: GridPos = GridPos::new(64, 12);

/// Extends an already-seeded skirmish with the benchmark workload: 96 more
/// units per team (the roster above) staged on deterministic walkable cells —
/// team 2 point-reflects team 1's scanned cells through the map center
/// after checking every reflected cell stays walkable — IDs taken only
/// from the `IdAllocator`, the production worker bundle on every spawn,
/// then the initial commands applied through the ordinary dispatcher: the
/// first 12 sorted workers per side go two-per-source onto that team's
/// authored `safe_sources`, and every combatant AttackMoves to
/// `BENCHMARK_MIDFIELD`. Panics on any rejected setup command: a benchmark
/// that boots with rejected orders would silently measure an idle crowd.
pub fn extend_with_benchmark_workload(world: &mut World, map: &mut GridMap) {
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
        let workers: Vec<UnitId> = team_kind_ids(world, team, UnitKind::Worker)
            .into_iter()
            .take(12)
            .collect();
        for (pair, source) in workers.chunks(2).zip(MapFixture::safe_source_ids(team)) {
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
        let army: Vec<UnitId> = [UnitKind::Guard, UnitKind::Rifleman]
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
/// authored blocker, Research Center or source — and team 2's cells are
/// the local point reflection of the west scan through the map center,
/// re-checked against the live map so a fixture retune fails loudly here
/// instead of spawning team 2 into terrain.
fn benchmark_staging_cells(map: &GridMap, team: TeamId) -> Vec<GridPos> {
    if team == TeamId(2) {
        let cells: Vec<GridPos> = benchmark_staging_cells(map, TeamId(1))
            .into_iter()
            .map(point_reflect)
            .collect();
        // The reflected cells are only staging candidates: check them
        // against the live map instead of assuming any symmetry.
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

#[cfg(test)]
mod tests;
