use std::collections::HashSet;
use std::time::{Duration, Instant};

use bevy::prelude::World;
use grus_sim::catalog::building_spec;
use grus_sim::{
    AttackCooldown, Building, BuildingId, BuildingIndex, BuildingKind, CombatEvents, CombatTarget,
    Footprint, GridMap, GridPos, Health, HeatState, MapFixture, MatchPhase, MatchResult,
    MatchSession, SIM_STEP_SECONDS, TeamId, UnitIndex, UnitKind, VisibilityMap, WaveState,
    WorkerTask, active_phase, seed_skirmish, sorted_unit_ids, spawn_completed_building,
    start_match, step_heat, step_tick, surface_efficiency, wave_deadline,
};

const TEAM: TeamId = TeamId(1);
const SURVIVAL_DEADLINE_WAVE: u64 = 11;
const FINAL_WAVE: u64 = 40;

fn ring_cells(anchor: GridPos) -> Vec<GridPos> {
    let min = GridPos::new(anchor.x - 4, anchor.y - 3);
    let max = GridPos::new(anchor.x + 7, anchor.y + 7);
    let mut cells = Vec::new();
    for x in min.x..=max.x {
        cells.push(GridPos::new(x, min.y));
        cells.push(GridPos::new(x, max.y));
    }
    for y in (min.y + 1)..max.y {
        cells.push(GridPos::new(min.x, y));
        cells.push(GridPos::new(max.x, y));
    }
    cells
}

fn gate_cell(anchor: GridPos) -> GridPos {
    GridPos::new(anchor.x + 1, anchor.y + 7)
}

fn turret_anchors(anchor: GridPos) -> [GridPos; 2] {
    [
        GridPos::new(anchor.x - 3, anchor.y + 1),
        GridPos::new(anchor.x + 5, anchor.y + 1),
    ]
}

fn authored_cells(anchor: GridPos) -> Vec<GridPos> {
    let mut cells = ring_cells(anchor);
    let spec = building_spec(BuildingKind::GunTurret);
    for turret in turret_anchors(anchor) {
        cells.extend(Footprint::new(turret, spec.width, spec.height).cells());
    }
    cells
}

fn research_center_alive(world: &World) -> bool {
    world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index.iter().any(|(_, entity)| {
                world.get::<Building>(*entity).is_some_and(|building| {
                    building.team == TEAM
                        && building.kind == BuildingKind::ResearchCenter
                        && world
                            .get::<Health>(*entity)
                            .is_some_and(|health| health.current > 0)
                })
            })
        })
        .unwrap_or(false)
}

fn survival_checkpoint_met(phase: MatchPhase, next_wave: u64, core_alive: bool) -> bool {
    next_wave > SURVIVAL_DEADLINE_WAVE && matches!(phase, MatchPhase::Playing) && core_alive
}

const WAVE_CLOCK_TOLERANCE_SECONDS: f64 = 1.0;

fn wave_schedule_on_track(wave: WaveState, simulated_seconds: f64) -> bool {
    let clock_advancing = (wave.elapsed - simulated_seconds).abs() <= WAVE_CLOCK_TOLERANCE_SECONDS;
    let no_skipped_launch = wave.next_wave <= 1
        || wave_deadline(wave.next_wave - 1) <= wave.elapsed + WAVE_CLOCK_TOLERANCE_SECONDS;
    let launch_not_overdue = wave.elapsed <= wave_deadline(wave.next_wave);
    clock_advancing && no_skipped_launch && launch_not_overdue
}

fn report(world: &World, simulated_seconds: f64, wall_clock: Duration) {
    let wave = *world.resource::<WaveState>();
    eprintln!(
        "survival-balance: phase {:?}; wave next={} elapsed={:.2} s; \
         simulated={simulated_seconds:.2} s; wall-clock {wall_clock:.2?}",
        active_phase(world),
        wave.next_wave,
        wave.elapsed
    );
}

fn reference_defense() -> (World, GridMap, [BuildingId; 2]) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);

    world.insert_resource(VisibilityMap::default());
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    start_match(&mut world);
    assert_eq!(
        active_phase(&world),
        MatchPhase::Playing,
        "start_match must move Start to Playing"
    );

    let start = MapFixture::team_starts()[0];
    let anchor = start.research_center_anchor;
    let authored = authored_cells(anchor);

    let unique: HashSet<GridPos> = authored.iter().copied().collect();
    assert_eq!(
        unique.len(),
        authored.len(),
        "authored cells overlap each other"
    );
    let core = Footprint::new(
        anchor,
        building_spec(BuildingKind::ResearchCenter).width,
        building_spec(BuildingKind::ResearchCenter).height,
    );
    for cell in &authored {
        assert!(
            map.is_walkable(*cell),
            "authored cell {cell:?} is not free terrain"
        );
        assert!(
            !core.cells().contains(cell),
            "authored cell {cell:?} overlaps the Research Center"
        );
        assert!(
            !start.workers.contains(cell),
            "authored cell {cell:?} overlaps a Worker spawn"
        );
    }
    for spawn in fixture
        .resources
        .iter()
        .filter(|spawn| start.safe_sources.contains(&spawn.id))
    {
        assert!(
            !authored.contains(&spawn.cell),
            "authored cells cover safe source {:?} at {:?}",
            spawn.id,
            spawn.cell
        );
    }

    let gate = gate_cell(anchor);
    for cell in ring_cells(anchor) {
        let kind = if cell == gate {
            BuildingKind::Gate
        } else {
            BuildingKind::Wall
        };
        spawn_completed_building(&mut world, &mut map, TEAM, kind, cell);
    }
    let turrets = turret_anchors(anchor).map(|turret| {
        spawn_completed_building(&mut world, &mut map, TEAM, BuildingKind::GunTurret, turret)
    });

    for id in turrets {
        let entity = world
            .resource::<BuildingIndex>()
            .entity(id)
            .expect("authored turret is indexed");
        assert!(
            world.get::<AttackCooldown>(entity).is_some(),
            "authored Gun Turret {id:?} lacks AttackCooldown"
        );
    }
    (world, map, turrets)
}

#[test]
fn reference_defense_layout_is_legal_and_fully_heated() {
    let (mut world, _map, _turrets) = reference_defense();

    let workers = sorted_unit_ids(&world, TEAM, |unit| unit.kind == UnitKind::Worker);
    assert_eq!(workers.len(), 4, "the seed provides four starting Workers");
    for id in workers {
        let entity = world
            .resource::<UnitIndex>()
            .entity(id)
            .expect("worker is indexed");
        assert!(
            matches!(world.get::<WorkerTask>(entity), Some(WorkerTask::Idle)),
            "worker {id:?} must stay idle"
        );
    }

    let index = world.resource::<BuildingIndex>();
    assert_eq!(index.iter().count(), 45);
    let count = |kind| {
        index
            .iter()
            .filter(|(_, entity)| {
                world
                    .get::<Building>(**entity)
                    .is_some_and(|building| building.kind == kind)
            })
            .count()
    };
    assert_eq!(count(BuildingKind::Wall), 41);
    assert_eq!(count(BuildingKind::Gate), 1);
    assert_eq!(count(BuildingKind::GunTurret), 2);
    assert_eq!(count(BuildingKind::ResearchCenter), 1);

    step_heat(&mut world, SIM_STEP_SECONDS);
    let heat = *world.resource::<HeatState>();
    assert_eq!(
        heat.supply,
        building_spec(BuildingKind::ResearchCenter).heat_supply
    );
    assert_eq!(
        heat.demand,
        building_spec(BuildingKind::ResearchCenter).heat_demand
            + 2 * building_spec(BuildingKind::GunTurret).heat_demand
    );
    assert_eq!(surface_efficiency(&world), 1.0);
}

#[test]
fn wave_deadline_checkpoint_requires_live_core_and_playing() {
    assert!(survival_checkpoint_met(
        MatchPhase::Playing,
        SURVIVAL_DEADLINE_WAVE + 1,
        true
    ));
    assert!(!survival_checkpoint_met(
        MatchPhase::Result(MatchResult(TeamId(2))),
        SURVIVAL_DEADLINE_WAVE + 1,
        false
    ));
    assert!(!survival_checkpoint_met(
        MatchPhase::Result(MatchResult(TeamId(2))),
        SURVIVAL_DEADLINE_WAVE + 1,
        true
    ));
    assert!(!survival_checkpoint_met(
        MatchPhase::Playing,
        SURVIVAL_DEADLINE_WAVE + 1,
        false
    ));
    assert!(!survival_checkpoint_met(
        MatchPhase::Playing,
        SURVIVAL_DEADLINE_WAVE,
        true
    ));
    assert!(!survival_checkpoint_met(
        MatchPhase::Paused,
        SURVIVAL_DEADLINE_WAVE + 1,
        true
    ));
}

#[test]
fn wave_schedule_check_flags_stalled_or_skipped_launches() {
    let on_schedule = |elapsed: f64, next_wave: u64| {
        wave_schedule_on_track(WaveState { elapsed, next_wave }, elapsed)
    };
    assert!(on_schedule(250.0, 1));
    assert!(on_schedule(1_300.0, 12));
    assert!(on_schedule(1_383.75, 12));
    assert!(on_schedule(1_400.0, 13));
    assert!(!on_schedule(1_400.05, 12));
    assert!(!on_schedule(1_500.0, 12));
    assert!(!on_schedule(1_350.0, 14));
    assert!(!wave_schedule_on_track(
        WaveState {
            elapsed: 1_300.0,
            next_wave: 12
        },
        1_500.0
    ));
}

#[test]
#[ignore = "up to 84,000 fixed ticks; CI runs it in --release explicitly"]
fn fixed_defense_survives_wave_10_and_falls_before_wave_40() {
    let (mut world, mut map, turrets) = reference_defense();
    step_heat(&mut world, SIM_STEP_SECONDS);
    assert_eq!(
        surface_efficiency(&world),
        1.0,
        "the authored defense must run at full surface efficiency"
    );

    let wall_clock = Instant::now();
    let mut simulated_seconds = 0.0_f64;
    let tick_cap = (wave_deadline(FINAL_WAVE) / f64::from(SIM_STEP_SECONDS)).ceil() as usize;
    let mut checkpoint = false;
    let mut turrets_fired: HashSet<BuildingId> = HashSet::new();
    let mut defeat: Option<(WaveState, MatchResult)> = None;

    for _ in 0..tick_cap {
        step_tick(&mut world, &mut map, SIM_STEP_SECONDS);
        simulated_seconds += f64::from(SIM_STEP_SECONDS);

        if turrets_fired.len() < turrets.len()
            && let Some(events) = world.get_resource::<CombatEvents>()
        {
            for event in &events.0 {
                if let CombatTarget::Building(id) = event.attacker
                    && turrets.contains(&id)
                {
                    turrets_fired.insert(id);
                }
            }
        }

        let wave = *world.resource::<WaveState>();
        let phase = active_phase(&world);

        assert!(
            wave_schedule_on_track(wave, simulated_seconds),
            "wave schedule stalled at {simulated_seconds:.1} simulated s: \
             wave clock {:.2} s, next wave {} of {FINAL_WAVE}",
            wave.elapsed,
            wave.next_wave
        );

        if !checkpoint && wave.next_wave > SURVIVAL_DEADLINE_WAVE {
            let core_alive = research_center_alive(&world);
            assert!(
                survival_checkpoint_met(phase, wave.next_wave, core_alive),
                "premature end at the wave-{SURVIVAL_DEADLINE_WAVE} deadline boundary: \
                 phase {phase:?}, core alive {core_alive} at \
                 {simulated_seconds:.1} simulated s"
            );
            assert_eq!(
                turrets_fired.len(),
                turrets.len(),
                "each authored Gun Turret must have fired by the wave-{SURVIVAL_DEADLINE_WAVE} deadline"
            );
            checkpoint = true;
        }

        match phase {
            MatchPhase::Playing => {}
            MatchPhase::Result(result) => {
                defeat = Some((wave, result));
                break;
            }
            phase => {
                report(&world, simulated_seconds, wall_clock.elapsed());
                panic!("session must stay Playing until Result; found {phase:?}");
            }
        }

        if wave.next_wave > FINAL_WAVE {
            report(&world, simulated_seconds, wall_clock.elapsed());
            panic!(
                "stalemate: wave {FINAL_WAVE} launched at {:.1} simulated s \
                 with the Research Center alive",
                wave.elapsed
            );
        }
    }

    let Some((wave_at_defeat, result)) = defeat else {
        report(&world, simulated_seconds, wall_clock.elapsed());
        panic!(
            "no Defeat inside {tick_cap} ticks ({simulated_seconds:.1} simulated s): \
             stalemate or missing wave progress"
        );
    };
    let failure_wave = wave_at_defeat.next_wave.saturating_sub(1);
    eprintln!(
        "survival-balance baseline: defeat during wave {failure_wave} at \
         {simulated_seconds:.2} simulated s (wave clock {:.2} s); \
         wall-clock {:.2?}",
        wave_at_defeat.elapsed,
        wall_clock.elapsed()
    );

    assert_eq!(
        result.0,
        TeamId(2),
        "unexpected Victory at {simulated_seconds:.1} simulated s — passive research \
         outlasted the horde"
    );
    assert!(
        checkpoint,
        "premature Defeat during wave {failure_wave} at {simulated_seconds:.1} simulated s — \
         the defense must survive to the wave-{SURVIVAL_DEADLINE_WAVE} deadline"
    );
    assert!(
        wave_at_defeat.next_wave <= FINAL_WAVE,
        "Defeat during wave {failure_wave} settled after wave {FINAL_WAVE} launched"
    );
    assert_eq!(
        turrets_fired.len(),
        turrets.len(),
        "both authored Gun Turrets must have fired"
    );
    report(&world, simulated_seconds, wall_clock.elapsed());
}
