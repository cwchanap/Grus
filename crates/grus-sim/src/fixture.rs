use bevy::prelude::{Entity, Or, With, World};

use crate::buildings::{Building, BuildingIndex, ConstructionState, spawn_completed_building};
use crate::catalog::{
    BuildingKind, ResourceKind, UnitKind, building_spec, resource_amount, unit_spec,
};
use crate::combat::{CombatEventTotal, CombatEvents, Health};
use crate::commands::{
    PlayerCommand, UnitCommand, UnitCommandKind, UnitIndex, apply_player_command, sorted_unit_ids,
    spawn_unit,
};
use crate::economy::{
    Carry, Dropoff, GatherProgress, ResourceIndex, ResourceSource, ResourceStockpile, TeamEconomy,
    WorkerTask, spawn_resource_source,
};
use crate::heat::HeatState;
use crate::ids::{BuildingId, IdAllocator, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{SIM_STEP_SECONDS, Unit};
use crate::research::ResearchState;
use crate::session::MatchSession;
use crate::underground::Underground;
use crate::visibility::VisibilityMap;
use crate::waves::{WaveState, step_waves, wave_deadline};

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

impl EntryZone {
    /// The authored map-edge anchor this zone spawns from — single-sourced
    /// in `ENTRY_ZONES`, so the director, bridge snapshot, and fixtures all
    /// share the one lookup.
    pub fn anchor(self) -> GridPos {
        ENTRY_ZONES
            .iter()
            .find(|(zone, _)| *zone == self)
            .map(|(_, anchor)| *anchor)
            .expect("every EntryZone has an authored anchor")
    }
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
    world.insert_resource(HeatState::default());
    world.insert_resource(ResearchState::default());
    world.insert_resource(Underground::default());

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

/// The seed's inverse and every reset path's one shared clear: all
/// gameplay entities (units, buildings, resource sources) despawn and all
/// match-scoped resources die — indices, economy, id allocation, fog,
/// combat/session transients, waves, heat, research, the underground and
/// the session itself. Absent state reads as neutral defaults until a
/// reseed inserts fresh ones: no `Underground` reads as depth zero, no
/// `HeatState` as full efficiency, no `MatchSession` as Playing (the
/// benchmark contract). Bridge-owned transients (command queue, feedback)
/// are not the sim's to clear.
pub fn clear_match_world(world: &mut World) {
    let stale: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Unit>, With<Building>, With<ResourceSource>)>>()
        .iter(world)
        .collect();
    for entity in stale {
        let _ = world.despawn(entity);
    }
    world.remove_resource::<UnitIndex>();
    world.remove_resource::<BuildingIndex>();
    world.remove_resource::<ResourceIndex>();
    world.remove_resource::<TeamEconomy>();
    world.remove_resource::<IdAllocator>();
    // Visibility dies with the match: every seed path re-inserts a fresh
    // fogged map.
    world.remove_resource::<VisibilityMap>();
    // Combat/session transients die with the match: the benchmark restarts
    // session-free (missing session = Playing), the normal reset re-inserts
    // Start.
    world.remove_resource::<CombatEvents>();
    world.remove_resource::<CombatEventTotal>();
    world.remove_resource::<WaveState>();
    // Heat dies with the match: a stale supply/demand between clear and
    // reseed would read wrong, while absence reads neutral full efficiency.
    world.remove_resource::<HeatState>();
    // Research dies with the match too: absence reads zero points until the
    // reseed inserts a fresh state — a restart can never carry progress.
    world.remove_resource::<ResearchState>();
    // The underground dies with the match the same way: absence reads depth
    // zero until the shared seed reseeds the empty default.
    world.remove_resource::<Underground>();
    world.remove_resource::<MatchSession>();
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

/// The benchmark fortress: the inclusive wall-rectangle bounds around the
/// central Research Center, chosen so the six safe sources, the four
/// worker cells, and the authored Barracks/Depot anchors all sit inside
/// the defended interior.
const BENCHMARK_WALL_MIN: GridPos = GridPos::new(52, 37);
const BENCHMARK_WALL_MAX: GridPos = GridPos::new(77, 60);

/// One gate centered on each wall face: Team-1-passable cells that are
/// also the horde's weakest breach points.
const BENCHMARK_GATES: [GridPos; 4] = [
    GridPos::new(64, 37),
    GridPos::new(64, 60),
    GridPos::new(52, 48),
    GridPos::new(77, 48),
];

/// The four gate-lane rally cells — each defender squad's accepted
/// defensive destination just inside its gate.
const BENCHMARK_RALLY_CELLS: [GridPos; 4] = [
    GridPos::new(64, 38),
    GridPos::new(64, 59),
    GridPos::new(53, 48),
    GridPos::new(76, 48),
];

/// Benchmark-only fuel: nine furnaces each burn FURNACE_COAL_PER_SECOND,
/// so 400 Coal outlasts the 60-second sample. Never touches the
/// normal-match starting stockpile.
const BENCHMARK_COAL: u32 = 400;

/// 24 Gun Turrets lining the interior wall edge on every face — their
/// 8-cell range clears the parapet.
const BENCHMARK_GUN_ANCHORS: [GridPos; 24] = [
    GridPos::new(53, 38),
    GridPos::new(57, 38),
    GridPos::new(61, 38),
    GridPos::new(67, 38),
    GridPos::new(71, 38),
    GridPos::new(73, 38),
    GridPos::new(75, 38),
    GridPos::new(53, 58),
    GridPos::new(57, 58),
    GridPos::new(61, 58),
    GridPos::new(65, 58),
    GridPos::new(69, 58),
    GridPos::new(71, 58),
    GridPos::new(73, 58),
    GridPos::new(75, 58),
    GridPos::new(53, 40),
    GridPos::new(53, 42),
    GridPos::new(53, 46),
    GridPos::new(53, 50),
    GridPos::new(75, 40),
    GridPos::new(75, 42),
    GridPos::new(75, 46),
    GridPos::new(75, 50),
    GridPos::new(75, 56),
];

/// 10 Flame Towers hugging the wall edge — range 3 reaches aliens pressing
/// the outer face.
const BENCHMARK_FLAME_ANCHORS: [GridPos; 10] = [
    GridPos::new(55, 38),
    GridPos::new(59, 38),
    GridPos::new(65, 38),
    GridPos::new(69, 38),
    GridPos::new(55, 58),
    GridPos::new(67, 58),
    GridPos::new(53, 44),
    GridPos::new(53, 53),
    GridPos::new(75, 44),
    GridPos::new(75, 53),
];

/// 6 Crystal Lances one row deeper — range 10 still clears the wall.
const BENCHMARK_LANCE_ANCHORS: [GridPos; 6] = [
    GridPos::new(55, 41),
    GridPos::new(62, 41),
    GridPos::new(72, 41),
    GridPos::new(60, 56),
    GridPos::new(56, 49),
    GridPos::new(73, 45),
];

/// Nine Coal Furnaces clustered in the defended south pocket, off the
/// west/north/east assault corridors that converge on the core.
const BENCHMARK_FURNACE_ANCHORS: [GridPos; 9] = [
    GridPos::new(55, 54),
    GridPos::new(57, 55),
    GridPos::new(60, 54),
    GridPos::new(62, 55),
    GridPos::new(63, 53),
    GridPos::new(67, 53),
    GridPos::new(68, 55),
    GridPos::new(71, 54),
    GridPos::new(73, 55),
];

/// Extends the seeded survival fixture to the fortified 200-unit stress
/// workload: a completed wall/gate ring with the 24/10/6 combat tower mix
/// and nine fueled furnaces — all authored through the shared
/// `spawn_completed_building` constructor — plus 46 defenders garrisoned
/// inside with accepted defensive orders, and 150 live aliens from
/// `step_waves` itself. The benchmark remains session-free, as the absent
/// MatchSession means Playing.
pub fn extend_with_benchmark_workload(world: &mut World, map: &mut GridMap) {
    // The closed wall/gate ring first, then the interior buildings — every
    // cell must still be free terrain when its footprint claims it.
    for y in BENCHMARK_WALL_MIN.y..=BENCHMARK_WALL_MAX.y {
        for x in BENCHMARK_WALL_MIN.x..=BENCHMARK_WALL_MAX.x {
            if x != BENCHMARK_WALL_MIN.x
                && x != BENCHMARK_WALL_MAX.x
                && y != BENCHMARK_WALL_MIN.y
                && y != BENCHMARK_WALL_MAX.y
            {
                continue;
            }
            let cell = GridPos::new(x, y);
            let kind = if BENCHMARK_GATES.contains(&cell) {
                BuildingKind::Gate
            } else {
                BuildingKind::Wall
            };
            spawn_completed_building(world, map, TeamId(1), kind, cell);
        }
    }
    for (kind, anchors) in [
        (BuildingKind::GunTurret, BENCHMARK_GUN_ANCHORS.as_slice()),
        (BuildingKind::FlameTower, BENCHMARK_FLAME_ANCHORS.as_slice()),
        (
            BuildingKind::CrystalLance,
            BENCHMARK_LANCE_ANCHORS.as_slice(),
        ),
        (
            BuildingKind::CoalFurnace,
            BENCHMARK_FURNACE_ANCHORS.as_slice(),
        ),
    ] {
        for &anchor in anchors {
            spawn_completed_building(world, map, TeamId(1), kind, anchor);
        }
    }
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&TeamId(1))
        .expect("the seeded human economy")
        .stockpile
        .coal = BENCHMARK_COAL;

    // Gathers before the reinforcements stage, so defender bodies never
    // reserve a source approach slot the workers need.
    let workers = sorted_unit_ids(world, TeamId(1), |unit| unit.kind == UnitKind::Worker);
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

    let mut cells = benchmark_staging_cells(map).into_iter();
    for kind in [UnitKind::Guard, UnitKind::Rifleman] {
        for _ in 0..BENCHMARK_REINFORCEMENTS / 2 {
            let cell = cells.next().expect("benchmark staging capacity");
            let unit_id = world.resource_mut::<IdAllocator>().allocate_unit();
            let unit = spawn_unit(
                world,
                unit_id,
                TeamId(1),
                map.cell_center(cell),
                kind,
                unit_spec(kind).speed,
            );
            // Same post-spawn bundle production gives every trained unit,
            // so the benchmark measures the real archetype layout.
            world.entity_mut(unit).insert((
                Carry::Empty,
                GatherProgress::default(),
                WorkerTask::Idle,
            ));
        }
    }

    // Four defensive squads, one per gate lane — the garrison holds the
    // wall instead of marching north.
    let defenders = sorted_unit_ids(world, TeamId(1), |unit| unit.kind != UnitKind::Worker);
    let squad_size = defenders.len().div_ceil(BENCHMARK_RALLY_CELLS.len());
    for (squad, rally) in defenders.chunks(squad_size).zip(BENCHMARK_RALLY_CELLS) {
        accept(
            world,
            map,
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: squad.to_vec(),
                kind: UnitCommandKind::AttackMove {
                    target: map.cell_center(rally),
                },
            }),
        );
    }

    world.insert_resource(WaveState {
        elapsed: wave_deadline(BENCHMARK_WAVE),
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

/// Reinforcements stage inside the fortress on deterministic row-major
/// cells: the interior scan returns only free cells, clear of authored
/// workers, towers, furnaces, and resources.
fn benchmark_staging_cells(map: &GridMap) -> Vec<GridPos> {
    let start = MapFixture::team_starts()[0];
    let mut cells = Vec::with_capacity(BENCHMARK_REINFORCEMENTS);
    'scan: for y in (BENCHMARK_WALL_MIN.y + 1)..BENCHMARK_WALL_MAX.y {
        for x in (BENCHMARK_WALL_MIN.x + 1)..BENCHMARK_WALL_MAX.x {
            let cell = GridPos::new(x, y);
            if map.is_walkable(cell) && !start.workers.contains(&cell) {
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
        "the fortress interior lost walkable cells"
    );
    cells
}

#[cfg(test)]
mod tests;
