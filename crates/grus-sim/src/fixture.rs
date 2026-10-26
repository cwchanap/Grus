use bevy::prelude::World;

use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{
    BuildingKind, ResourceKind, UnitKind, building_spec, resource_amount, unit_spec,
};
use crate::combat::Health;
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, UnitIndex, apply_player_command, spawn_unit,
};
use crate::economy::{
    Carry, Dropoff, GatherProgress, ResourceStockpile, TeamEconomy, WorkerTask,
    spawn_resource_source,
};
use crate::ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{SIM_STEP_SECONDS, Unit};
use crate::waves::{WaveState, step_waves};

/// Authored human starting layout: Research Center anchor, four worker
/// spawn cells, Barracks/Depot build anchors, and six safe resource ids.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryZone {
    North,
    East,
    South,
    West,
}

/// The single authored blocker table for the battlefield — sparse ruins
/// around the central city, each pair a rectangle's minimum and INCLUSIVE
/// maximum cell. `MapFixture::battlefield` loops exactly this data through
/// `GridMap::set_blocked_rect`, and the Godot bridge hands the same
/// rectangles to the presentation layer: nothing may copy the coordinates.
/// Every rect stays clear of the central construction/gather ring, the
/// N/E/S/W access lanes, authored occupants, and benchmark staging region.
pub const RUINS: &[(GridPos, GridPos)] = &[
    (GridPos::new(48, 12), GridPos::new(51, 14)),
    (GridPos::new(64, 24), GridPos::new(66, 26)),
    (GridPos::new(78, 10), GridPos::new(81, 12)),
    (GridPos::new(16, 30), GridPos::new(19, 33)),
    (GridPos::new(56, 30), GridPos::new(58, 33)),
    (GridPos::new(34, 82), GridPos::new(37, 85)),
    (GridPos::new(84, 80), GridPos::new(88, 83)),
];

/// Authored alien entry anchors in the same order used for zone rotation.
pub const ENTRY_ZONES: [(EntryZone, GridPos); 4] = [
    (EntryZone::North, GridPos::new(64, 0)),
    (EntryZone::East, GridPos::new(127, 48)),
    (EntryZone::South, GridPos::new(64, 95)),
    (EntryZone::West, GridPos::new(0, 48)),
];

#[derive(Clone, Debug)]
pub struct MapFixture {
    pub map: GridMap,
    /// Authored finite resource nodes for the skirmish seed.
    pub resources: Vec<ResourceSpawn>,
}

impl MapFixture {
    /// The single authored human start anchors the central city: its
    /// (62,46) 4×4 footprint centers on the map midpoint, its four workers
    /// occupy free diagonal-corner cells, and its Barracks/Depot anchors
    /// hug the safe ring. Aliens enter through `ENTRY_ZONES` instead.
    pub fn team_starts() -> [TeamStart; 1] {
        [TeamStart {
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
        }]
    }

    /// The six safe sources ringing the human Research Center, followed by
    /// six eastern expansion deposits. The only resource-position table in
    /// the codebase; ids remain deterministic from 1 through 18.
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

/// The pure simulation survival seed: one completed human Research Center
/// (4×4 Footprint + `Dropoff` from birth), four Workers with fresh worker
/// components, one starting stockpile (200 Food / 300 Scrap / 150 Coal /
/// 0 Crystals), the wave clock, authored finite resource sources, and
/// `UnitIndex`/`BuildingIndex` `IdAllocator` counters above authored ids.
/// Every starting entity flows through the production constructors.
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
    world.insert_resource(economy);
    world.insert_resource(WaveState::default());

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

    world.insert_resource(IdAllocator::new(unit_counter + 1, 2, 19));
}

fn spawn(id: ResourceId, kind: ResourceKind, x: i32, y: i32) -> ResourceSpawn {
    ResourceSpawn {
        id,
        kind,
        cell: GridPos::new(x, y),
        amount: resource_amount(kind),
    }
}

const BENCHMARK_REINFORCEMENTS: usize = 46;
const BENCHMARK_WAVE: u64 = 100;

/// Human defenders march toward the northern approach while the shared wave
/// director sends aliens toward the central Research Center.
const BENCHMARK_MIDFIELD: GridPos = GridPos::new(64, 12);

/// Extends the seeded survival fixture to 50 human defenders and 150 live
/// aliens. Human reinforcements use the shared unit constructor and ordinary
/// AttackMove dispatcher; aliens come from `step_waves` itself. The benchmark
/// remains session-free, as the absent MatchSession means Playing.
pub fn extend_with_benchmark_workload(world: &mut World, map: &mut GridMap) {
    let mut cells = benchmark_staging_cells(map).into_iter();
    for kind in [UnitKind::Guard, UnitKind::Rifleman] {
        for _ in 0..BENCHMARK_REINFORCEMENTS / 2 {
            let cell = cells.next().expect("benchmark staging capacity");
            let unit_id = world.resource_mut::<IdAllocator>().allocate_unit();
            spawn_unit(
                world,
                unit_id,
                TeamId(1),
                map.cell_center(cell),
                kind,
                unit_spec(kind).speed,
            );
        }
    }

    let mut units = world
        .resource::<UnitIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Unit>(*entity)
                .filter(|unit| unit.team == TeamId(1))
                .map(|unit| (*id, unit.kind))
        })
        .collect::<Vec<_>>();
    units.sort_unstable_by_key(|(id, _)| *id);
    let workers: Vec<UnitId> = units
        .iter()
        .filter_map(|(id, kind)| (*kind == UnitKind::Worker).then_some(*id))
        .collect();
    for (worker, source) in workers
        .into_iter()
        .zip(MapFixture::safe_source_ids(TeamId(1)))
    {
        accept(
            world,
            map,
            PlayerCommand::Gather {
                issuer: TeamId(1),
                workers: vec![worker],
                source,
            },
        );
    }
    let defenders: Vec<UnitId> = units
        .into_iter()
        .filter_map(|(id, kind)| (kind != UnitKind::Worker).then_some(id))
        .collect();
    accept(
        world,
        map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: defenders,
            kind: UnitCommandKind::AttackMove {
                target: map.cell_center(BENCHMARK_MIDFIELD),
            },
        }),
    );

    world.insert_resource(WaveState {
        elapsed: f64::from(crate::catalog::WAVE_PREPARATION_SECONDS)
            + (BENCHMARK_WAVE - 1) as f64 * f64::from(crate::catalog::WAVE_INTERVAL_SECONDS),
        next_wave: BENCHMARK_WAVE,
    });
    step_waves(world, map, SIM_STEP_SECONDS);
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

/// Reinforcements stage on deterministic row-major cells in the existing
/// open western field, clear of the human start and authored deposits.
fn benchmark_staging_cells(map: &GridMap) -> Vec<GridPos> {
    let mut cells = Vec::with_capacity(BENCHMARK_REINFORCEMENTS);
    'scan: for y in 20..=75 {
        for x in 26..=53 {
            let cell = GridPos::new(x, y);
            if map.is_walkable(cell) {
                cells.push(cell);
                if cells.len() == BENCHMARK_REINFORCEMENTS {
                    break 'scan;
                }
            }
        }
    }
    assert_eq!(
        cells.len(),
        BENCHMARK_REINFORCEMENTS,
        "the staging region lost walkable cells"
    );
    cells
}

#[cfg(test)]
mod tests;
