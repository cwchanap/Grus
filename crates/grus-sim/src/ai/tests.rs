//! Focused AI tests: every policy step is exercised as a pure decision, plus
//! bounded headless journeys (grant-free economy, worker raids, population
//! stalls) that run the real fixed-step chain with the real 1 Hz cadence.

use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::ConstructionState;
use crate::combat::{CombatOrder, CombatTarget};
use crate::commands::spawn_unit;
use crate::economy::{Carry, GatherProgress, ResourceStockpile};
use crate::movement::SimPosition;
use crate::session::{MatchPhase, MatchSession};
use crate::visibility::{VisibilityMap, refresh_visibility};
use crate::{
    SIM_STEP_SECONDS, seed_skirmish, step_combat, step_construction, step_economy, step_movement,
    step_production,
};

/// Tick budget for every bounded journey loop; a correct AI settles far
/// inside it, only a broken policy runs out.
const JOURNEY_BUDGET: u32 = 40_000;

/// A fogged, playing world with a Team-2 controller over the authored
/// skirmish — the normal runtime shape, minus Godot.
fn ai_world(team: TeamId) -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    world.insert_resource(AiController::new(team));
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    (world, map)
}

/// One canonical fixed-step match tick in the production order: combat,
/// movement, economy, construction, production, visibility, AI.
fn step_tick(world: &mut World, map: &mut GridMap) {
    step_combat(world, map, SIM_STEP_SECONDS);
    step_movement(world, map, SIM_STEP_SECONDS);
    step_economy(world, map, SIM_STEP_SECONDS);
    step_construction(world, SIM_STEP_SECONDS);
    step_production(world, map, SIM_STEP_SECONDS);
    refresh_visibility(world, map);
    step_ai(world, map, SIM_STEP_SECONDS);
}

fn run_until(world: &mut World, map: &mut GridMap, what: &str, condition: impl Fn(&World) -> bool) {
    for _ in 0..JOURNEY_BUDGET {
        if condition(world) {
            return;
        }
        step_tick(world, map);
    }
    panic!("{what} did not happen inside {JOURNEY_BUDGET} ticks");
}

/// Pure decision pass: takes the controller out (its private state is
/// module-local), composes the ordered commands, puts it back.
fn decide(world: &mut World) -> Vec<PlayerCommand> {
    let mut controller = world.remove_resource::<AiController>().unwrap();
    let plan = MapFixture::team_plan(controller.team);
    let commands = decide_ai_commands(world, &mut controller, &plan);
    world.insert_resource(controller);
    commands
}

fn decide_and_apply(world: &mut World, map: &mut GridMap) -> Vec<PlayerCommand> {
    let commands = decide(world);
    for command in commands.clone() {
        apply_player_command(world, map, command);
    }
    commands
}

fn spawn_villager(
    world: &mut World,
    map: &GridMap,
    id: UnitId,
    team: TeamId,
    cell: GridPos,
) -> Entity {
    let entity = spawn_unit(
        world,
        id,
        team,
        map.cell_center(cell),
        UnitKind::Villager,
        unit_spec(UnitKind::Villager).speed,
    );
    world
        .entity_mut(entity)
        .insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    entity
}

fn spawn_military(
    world: &mut World,
    map: &GridMap,
    id: UnitId,
    team: TeamId,
    cell: GridPos,
    kind: UnitKind,
) -> Entity {
    spawn_unit(
        world,
        id,
        team,
        map.cell_center(cell),
        kind,
        unit_spec(kind).speed,
    )
}

fn grant(world: &mut World, team: TeamId, food: u32, wood: u32, gold: u32) {
    let mut economy = world.get_resource_mut::<TeamEconomy>().unwrap();
    let state = economy.0.get_mut(&team).unwrap();
    state.stockpile = ResourceStockpile {
        food: state.stockpile.food + food,
        wood: state.stockpile.wood + wood,
        gold: state.stockpile.gold + gold,
    };
}

fn villagers_of(world: &World, team: TeamId) -> Vec<UnitId> {
    villager_ids(world, team)
}

fn units_of_kind(world: &World, team: TeamId, kind: UnitKind) -> Vec<UnitId> {
    let mut ids: Vec<UnitId> = world
        .resource::<UnitIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Unit>(*entity)
                .is_some_and(|unit| unit.team == team && unit.kind == kind)
                .then_some(*id)
        })
        .collect();
    ids.sort_unstable();
    ids
}

fn own_buildings_of(world: &World, team: TeamId, kind: BuildingKind) -> Vec<BuildingId> {
    let mut ids: Vec<BuildingId> = world
        .resource::<BuildingIndex>()
        .iter()
        .filter_map(|(id, entity)| {
            world
                .get::<Building>(*entity)
                .is_some_and(|building| building.team == team && building.kind == kind)
                .then_some(*id)
        })
        .collect();
    ids.sort_unstable();
    ids
}

fn stockpile(world: &World, team: TeamId) -> ResourceStockpile {
    world.resource::<TeamEconomy>().0[&team].stockpile
}

fn age_of(world: &World, team: TeamId) -> Age {
    world.resource::<TeamEconomy>().0[&team].age
}

// ---- Cadence / gate ---------------------------------------------------------

#[test]
fn frozen_phases_never_accumulate_decisions() {
    for phase in [
        MatchPhase::Start,
        MatchPhase::Paused,
        MatchPhase::Result(crate::session::MatchResult(TeamId(1))),
    ] {
        let (mut world, mut map) = ai_world(TeamId(2));
        world.insert_resource(MatchSession { phase });

        // Far more than one decision's worth of frozen time.
        for _ in 0..40 {
            step_ai(&mut world, &mut map, SIM_STEP_SECONDS);
        }
        let controller = world.resource::<AiController>();
        assert_eq!(
            controller.decision_accumulator, 0.0,
            "phase {phase:?}: frozen time must not bank into the accumulator"
        );
        // And nothing happened: the seed stockpiles are untouched.
        let stockpile = stockpile(&world, TeamId(2));
        assert_eq!(
            (stockpile.food, stockpile.wood, stockpile.gold),
            (200, 300, 100)
        );
    }
}

#[test]
fn step_ai_without_a_controller_is_a_no_op() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    step_ai(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(stockpile(&world, TeamId(2)).food, 200);
}

#[test]
fn playing_ticks_fire_one_decision_per_second() {
    let (mut world, mut map) = ai_world(TeamId(2));

    // 19 ticks = 0.95 s: under cadence, no decision yet.
    for _ in 0..19 {
        step_ai(&mut world, &mut map, SIM_STEP_SECONDS);
    }
    assert!(
        world.resource::<AiController>().decision_accumulator > 0.0,
        "Playing time banks into the accumulator"
    );
    assert_eq!(stockpile(&world, TeamId(2)).food, 200, "no decision yet");

    // The 20th tick crosses 1.0 s: the first decision fires (the paid
    // replacement villager charges 50 food).
    step_ai(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(stockpile(&world, TeamId(2)).food, 150);
    assert!(
        world.resource::<AiController>().decision_accumulator < AI_DECISION_SECONDS,
        "cadence resets after firing"
    );
}

// ---- Scouting ---------------------------------------------------------------

#[test]
fn scout_uses_the_lowest_idle_military_unit() {
    let (mut world, _map) = ai_world(TeamId(2));
    let spearman = spawn_military(
        &mut world,
        &_map,
        UnitId(20),
        TeamId(2),
        GridPos::new(108, 44),
        UnitKind::Spearman,
    );

    let commands = decide(&mut world);
    let scout = commands.iter().find_map(|command| match command {
        PlayerCommand::Units(units) => Some(units),
        _ => None,
    });
    assert_eq!(
        scout.map(|units| units.units.as_slice()),
        Some([UnitId(20)].as_slice()),
        "the lowest stable-ID idle military unit scouts"
    );
    assert!(
        world.get::<MoveOrder>(spearman).is_none(),
        "pure decide mutates nothing"
    );
}

#[test]
fn scout_falls_back_to_a_surplus_idle_villager_with_plain_move() {
    let (mut world, map) = ai_world(TeamId(2));
    // Fifth villager: above the worker floor. The four seeded villagers stay
    // idle too, so the deficit guard must block the scout until allocation
    // has claimed them — run the allocation first by deciding with full
    // staffing: grant a fifth villager and staff the split manually.
    spawn_villager(
        &mut world,
        &map,
        UnitId(20),
        TeamId(2),
        GridPos::new(110, 52),
    );
    // Staff the whole split (2 food, 1 wood, 1 gold of the 4 seeded workers):
    // seed targets for total=5 are F2/W1/G1.
    for (id, source) in [
        (UnitId(5), ResourceId(7)),
        (UnitId(6), ResourceId(7)),
        (UnitId(7), ResourceId(9)),
        (UnitId(8), ResourceId(12)),
    ] {
        let entity = world.resource::<UnitIndex>().entity(id).unwrap();
        world
            .entity_mut(entity)
            .insert(WorkerTask::Gathering { source });
    }

    let commands = decide(&mut world);
    let scout = commands.iter().find_map(|command| match command {
        PlayerCommand::Units(units) => Some((units.units[0], units.kind.clone())),
        _ => None,
    });
    let (id, kind) = scout.expect("a surplus idle villager scouts once staffed");
    assert_eq!(id, UnitId(20), "the lowest stable-ID idle villager scouts");
    assert!(
        matches!(kind, UnitCommandKind::Move { .. }),
        "villagers scout with plain Move, got {kind:?}"
    );
}

#[test]
fn scout_never_fires_below_the_worker_floor_or_with_an_allocation_deficit() {
    let (mut world, _map) = ai_world(TeamId(2));
    // Four seeded villagers, all idle: below the floor AND with a deficit.
    let commands = decide(&mut world);
    assert!(
        commands
            .iter()
            .all(|command| !matches!(command, PlayerCommand::Units(_))),
        "no unit scouts at or below the worker floor: {commands:?}"
    );
}

#[test]
fn scout_traverses_the_authored_route_once_and_discovers_the_expansion() {
    let (mut world, mut map) = ai_world(TeamId(2));
    let plan = MapFixture::team_plan(TeamId(2));
    assert_eq!(plan.scout_route.len(), 5);

    // A dedicated military scout walks the full route, one leg per decision.
    spawn_military(
        &mut world,
        &map,
        UnitId(20),
        TeamId(2),
        GridPos::new(112, 44),
        UnitKind::Spearman,
    );
    for (leg, expected) in plan.scout_route.iter().enumerate() {
        let commands = decide_and_apply(&mut world, &mut map);
        let moved = commands.iter().any(
            |command| matches!(command, PlayerCommand::Units(units) if units.units == [UnitId(20)]),
        );
        assert!(moved, "leg {leg}: the scout did not advance");
        // March the scout to the leg cell by hand (bounded, no walking wait):
        // teleport and drop the delivered leg's route so the unit is idle
        // again for the next decision.
        let entity = world.resource::<UnitIndex>().entity(UnitId(20)).unwrap();
        world
            .entity_mut(entity)
            .insert(SimPosition::new(map.cell_center(*expected)))
            .remove::<MoveOrder>();
        refresh_visibility(&mut world, &map);
    }

    // Route exhausted: no more scout commands, ever.
    let entity = world.resource::<UnitIndex>().entity(UnitId(20)).unwrap();
    world.entity_mut(entity).remove::<CombatOrder>();
    for _ in 0..3 {
        let commands = decide_and_apply(&mut world, &mut map);
        assert!(
            commands
                .iter()
                .all(|command| !matches!(command, PlayerCommand::Units(_))),
            "the route must not wrap into an endless patrol"
        );
    }

    // And the walk actually discovered the mirrored expansion.
    let expansion = MapFixture::expansion_resources()
        .into_iter()
        .find(|spawn| spawn.cell.x > 64)
        .unwrap();
    assert!(
        crate::visibility::explored_by(&world, TeamId(2), expansion.cell),
        "the scout route must discover the team's expansion resources"
    );
}

// ---- Allocation -------------------------------------------------------------

#[test]
fn allocation_assigns_the_lowest_idle_villager_to_the_lowest_known_source() {
    for (team, first_villager, first_food, barracks_anchor, range_anchor) in [
        (
            TeamId(1),
            UnitId(1),
            ResourceId(1),
            GridPos::new(17, 51),
            GridPos::new(17, 38),
        ),
        (
            TeamId(2),
            UnitId(5),
            ResourceId(7),
            GridPos::new(108, 51),
            GridPos::new(108, 38),
        ),
    ] {
        let (mut world, _map) = ai_world(team);
        let commands = decide(&mut world);

        // The gather assignment: lowest villager to the team's lowest-ID
        // known food source (Food is the first deficit in the split).
        let gather = commands.iter().find_map(|command| match command {
            PlayerCommand::Gather {
                workers, source, ..
            } => Some((workers[0], *source)),
            _ => None,
        });
        assert_eq!(
            gather,
            Some((first_villager, first_food)),
            "team {team:?}: lowest idle villager gathers the lowest known food source"
        );

        // The mirrored growth slots come from the one authored table.
        let barracks = commands.iter().find_map(|command| match command {
            PlayerCommand::PlaceBuilding { kind, anchor, .. }
                if *kind == BuildingKind::Barracks =>
            {
                Some(*anchor)
            }
            _ => None,
        });
        assert_eq!(
            barracks,
            Some(barracks_anchor),
            "team {team:?} barracks slot"
        );
        let range = commands.iter().find_map(|command| match command {
            PlayerCommand::PlaceBuilding { kind, anchor, .. }
                if *kind == BuildingKind::ArcheryRange =>
            {
                Some(*anchor)
            }
            _ => None,
        });
        assert_eq!(
            range,
            Some(range_anchor),
            "team {team:?} archery range slot"
        );
    }
}

#[test]
fn allocation_never_offers_unexplored_or_enemy_farm_sources() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    // Runtime fog with the initial reveal only: the expansions stay unknown.
    world.insert_resource(VisibilityMap::default());
    refresh_visibility(&mut world, &map);
    world.insert_resource(AiController::new(TeamId(2)));
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });

    // An enemy Farm sitting in the ResourceIndex must never surface as a
    // candidate either — build one for Team 1 with a food source attached.
    let farm_entity = world
        .spawn((
            Building {
                id: BuildingId(90),
                team: TeamId(1),
                kind: BuildingKind::Farm,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(GridPos::new(40, 40), 2, 2),
        ))
        .id();
    world
        .get_resource_or_insert_with(crate::economy::ResourceIndex::default)
        .insert(ResourceId(90), farm_entity);
    world.entity_mut(farm_entity).insert(ResourceSource {
        id: ResourceId(90),
        kind: ResourceKind::Food,
        remaining: None,
        assigned_worker: None,
    });

    let sources = known_sources(&world, TeamId(2));
    assert!(
        sources.iter().all(|source| source.id != ResourceId(90)),
        "an enemy Farm must never be a gather candidate"
    );
    assert!(
        sources
            .iter()
            .all(|source| source.id.0 <= 12 || source.farm),
        "unexplored expansion sources must not be known: {:?}",
        sources.iter().map(|source| source.id).collect::<Vec<_>>()
    );
    assert!(
        sources.iter().any(|source| source.id == ResourceId(7)),
        "own explored safe sources stay candidates"
    );

    // Every issued gather targets a known source only.
    let commands = decide(&mut world);
    for command in &commands {
        if let PlayerCommand::Gather { source, .. } = command {
            assert!(
                sources.iter().any(|known| known.id == *source),
                "gather target {source:?} is not a known source"
            );
        }
    }
}

// ---- Growth -----------------------------------------------------------------

#[test]
fn growth_places_barracks_range_and_discovered_storehouse_on_authored_slots() {
    let (mut world, mut map) = ai_world(TeamId(2));
    let plan = MapFixture::team_plan(TeamId(2));
    grant(&mut world, TeamId(2), 0, 2000, 0);

    // Unknown expansion (fog on, no scouting yet) and no production core:
    // the storehouse step must stay quiet while Barracks/Range fire.
    let commands = decide(&mut world);
    assert!(
        commands.iter().all(|command| !matches!(
            command,
            PlayerCommand::PlaceBuilding {
                kind: BuildingKind::Storehouse,
                ..
            }
        )),
        "no storehouse before discovery/core: {commands:?}"
    );
    for command in commands.clone() {
        apply_player_command(&mut world, &mut map, command);
    }
    assert_eq!(
        own_buildings_of(&world, TeamId(2), BuildingKind::Barracks).len(),
        1,
        "one Barracks placed on the authored slot"
    );

    // Full-information world (no VisibilityMap): the expansion counts as
    // discovered, and the core stands — the expansion storehouse fires first.
    // An extra idle villager is its builder: the seeded four are busy
    // building and gathering after the first decision applied.
    world.remove_resource::<VisibilityMap>();
    spawn_villager(
        &mut world,
        &map,
        UnitId(30),
        TeamId(2),
        GridPos::new(110, 58),
    );
    let commands = decide(&mut world);
    let storehouses: Vec<GridPos> = commands
        .iter()
        .filter_map(|command| match command {
            PlayerCommand::PlaceBuilding {
                kind: BuildingKind::Storehouse,
                anchor,
                ..
            } => Some(*anchor),
            _ => None,
        })
        .collect();
    assert_eq!(
        storehouses,
        vec![plan.expansion_storehouse_slots[0]],
        "the discovered expansion storehouse is placed first"
    );

    // With the expansion slot satisfied, the safe storehouse opens once the
    // production core exists — verified at the step level so the shared
    // builder pool is not consumed by the allocation step first.
    spawn_villager(
        &mut world,
        &map,
        UnitId(31),
        TeamId(2),
        GridPos::new(105, 58),
    );
    for command in commands {
        apply_player_command(&mut world, &mut map, command);
    }
    let mut controller = world.remove_resource::<AiController>().unwrap();
    let mut claimed = Vec::new();
    let command = place_storehouse(&world, &mut controller, &plan, &mut claimed);
    world.insert_resource(controller);
    assert!(
        matches!(command,
        Some(PlayerCommand::PlaceBuilding { kind: BuildingKind::Storehouse, anchor, .. })
            if anchor == plan.safe_storehouse_slots[0]),
        "the safe storehouse opens once the production core exists: {command:?}"
    );
}

#[test]
fn growth_skips_satisfied_or_incomplete_authored_slots() {
    let (mut world, mut map) = ai_world(TeamId(2));
    grant(&mut world, TeamId(2), 0, 2000, 0);
    // The Barracks site now exists (incomplete). A fresh decision must not
    // place a second one.
    decide_and_apply(&mut world, &mut map);
    let commands = decide(&mut world);
    assert!(
        commands.iter().all(|command| !matches!(
            command,
            PlayerCommand::PlaceBuilding {
                kind: BuildingKind::Barracks,
                ..
            }
        )),
        "an incomplete authored Barracks must not be re-placed: {commands:?}"
    );
}

#[test]
fn house_fires_on_population_pressure_and_stops_at_satisfied_slots() {
    let (mut world, _map) = ai_world(TeamId(2));
    let plan = MapFixture::team_plan(TeamId(2));
    // Free capacity is 6 of 10: no house yet.
    let commands = decide(&mut world);
    assert!(commands.iter().all(|command| !matches!(
        command,
        PlayerCommand::PlaceBuilding {
            kind: BuildingKind::House,
            ..
        }
    )));

    // Manufacture pressure: fill the population to its cap.
    grant(&mut world, TeamId(2), 0, 500, 0);
    for index in 0..6_u32 {
        spawn_unit(
            &mut world,
            UnitId(100 + index),
            TeamId(2),
            Vec2::new(110.5, 40.5 + index as f32),
            UnitKind::Villager,
            6.0,
        );
    }
    let commands = decide(&mut world);
    let house = commands.iter().find_map(|command| match command {
        PlayerCommand::PlaceBuilding {
            kind: BuildingKind::House,
            anchor,
            builder,
            ..
        } => Some((*anchor, *builder)),
        _ => None,
    });
    let (anchor, builder) = house.expect("population pressure must place a House");
    assert_eq!(
        anchor, plan.house_slots[0],
        "the first authored slot is used"
    );
    assert!(
        villagers_of(&world, TeamId(2)).contains(&builder)
            && idle_worker_ids(&world, TeamId(2)).contains(&builder),
        "the builder is a truly idle villager"
    );

    // Once a House stands at every authored slot, no more are placed.
    for (index, slot) in plan.house_slots.iter().enumerate() {
        let id = BuildingId(50 + index as u32);
        let entity = world
            .spawn((
                Building {
                    id,
                    team: TeamId(2),
                    kind: BuildingKind::House,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(*slot, 2, 2),
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(id, entity);
    }
    let commands = decide(&mut world);
    assert!(commands.iter().all(|command| !matches!(
        command,
        PlayerCommand::PlaceBuilding {
            kind: BuildingKind::House,
            ..
        }
    )));
}

#[test]
fn farm_fires_only_when_food_capacity_drops_below_target() {
    let (mut world, _map) = ai_world(TeamId(2));
    let plan = MapFixture::team_plan(TeamId(2));
    grant(&mut world, TeamId(2), 0, 500, 0);

    // Both safe food sources known: 2 standalone x 2 workers = 4 capacity,
    // at or above every seed-level split target — no farm.
    let food_sources = known_sources(&world, TeamId(2))
        .iter()
        .filter(|source| source.kind == ResourceKind::Food)
        .count();
    assert_eq!(food_sources, 2, "both safe food sources are known");

    // Deplete both food sources: capacity 0 drops below the target.
    for source_id in [ResourceId(7), ResourceId(8)] {
        let entity = world
            .resource::<crate::economy::ResourceIndex>()
            .entity(source_id)
            .unwrap();
        world.despawn(entity);
        world
            .get_resource_mut::<crate::economy::ResourceIndex>()
            .unwrap()
            .remove(source_id);
    }

    let commands = decide(&mut world);
    let farm = commands.iter().find_map(|command| match command {
        PlayerCommand::PlaceBuilding {
            kind: BuildingKind::Farm,
            anchor,
            ..
        } => Some(*anchor),
        _ => None,
    });
    assert_eq!(
        farm,
        Some(plan.farm_slots[0]),
        "insufficient known food supply places the first authored Farm"
    );
}

// ---- Age 2 / army -----------------------------------------------------------

#[test]
fn age_two_requires_workers_core_and_affordability() {
    let (mut world, map) = ai_world(TeamId(2));
    // Seed: 4 villagers, no core -> no age attempt.
    assert!(
        decide(&mut world)
            .iter()
            .all(|command| !matches!(command, PlayerCommand::EnqueueAgeUp { .. }))
    );

    // Workers + core but unaffordable -> still no attempt.
    for index in 0..4_u32 {
        spawn_villager(
            &mut world,
            &map,
            UnitId(20 + index),
            TeamId(2),
            GridPos::new(110, 52 + index as i32),
        );
    }
    let entity = world.spawn((
        Building {
            id: BuildingId(10),
            team: TeamId(2),
            kind: BuildingKind::Barracks,
            construction: ConstructionState {
                progress_seconds: 0.0,
                complete: true,
                active_builder: None,
            },
        },
        Footprint::new(GridPos::new(108, 51), 3, 3),
    ));
    let barracks_entity = entity.id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(10), barracks_entity);
    assert!(
        decide(&mut world)
            .iter()
            .all(|command| !matches!(command, PlayerCommand::EnqueueAgeUp { .. }))
    );

    // Affordable -> the attempt fires against the Town Center.
    grant(&mut world, TeamId(2), 300, 0, 200);
    let age = decide(&mut world)
        .into_iter()
        .find(|command| matches!(command, PlayerCommand::EnqueueAgeUp { .. }));
    assert!(
        age.is_some(),
        "affordable, staffed, cored: the age attempt fires"
    );
}

#[test]
fn round_robin_rotates_and_respects_queues_and_ages() {
    let (mut world, mut map) = ai_world(TeamId(2));
    grant(&mut world, TeamId(2), 3000, 3000, 2000);

    // Two completed producers with empty queues (no Stable yet).
    let plan = MapFixture::team_plan(TeamId(2));
    for (index, (kind, anchor)) in [
        (BuildingKind::Barracks, plan.barracks_anchor),
        (BuildingKind::ArcheryRange, plan.archery_range_anchor),
    ]
    .into_iter()
    .enumerate()
    {
        let id = BuildingId(10 + index as u32);
        let entity = world
            .spawn((
                Building {
                    id,
                    team: TeamId(2),
                    kind,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(anchor, 3, 3),
            ))
            .id();
        world
            .get_resource_or_insert_with(BuildingIndex::default)
            .insert(id, entity);
    }

    // Age 1: the rotation starts at the Spearman...
    let first = decide_and_apply(&mut world, &mut map);
    assert!(first.iter().any(|command| matches!(
        command,
        PlayerCommand::EnqueueUnit {
            kind: UnitKind::Spearman,
            ..
        }
    )));
    // ...and a decision whose Barracks queue is no longer empty skips to the
    // Archer through the Archery Range.
    let second = decide(&mut world);
    assert!(second.iter().any(|command| matches!(
        command,
        PlayerCommand::EnqueueUnit {
            kind: UnitKind::Archer,
            ..
        }
    )));
    assert!(second.iter().all(|command| !matches!(
        command,
        PlayerCommand::EnqueueUnit {
            kind: UnitKind::Spearman,
            ..
        }
    )));

    // The Cavalry leg stays locked until Age 2 even with the cursor upon it
    // and a Stable standing: point the cursor at the Cavalry leg directly.
    let entity = world
        .spawn((
            Building {
                id: BuildingId(12),
                team: TeamId(2),
                kind: BuildingKind::Stable,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
                    active_builder: None,
                },
            },
            Footprint::new(plan.stable_anchor, 3, 3),
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(BuildingId(12), entity);
    // Drain the Barracks/Range queues so only the age gate can block.
    for producer in [BuildingId(10), BuildingId(11)] {
        let producer_entity = world.resource::<BuildingIndex>().entity(producer).unwrap();
        if let Some(mut queue) = world.get_mut::<ProductionQueue>(producer_entity) {
            queue.jobs.clear();
        }
    }
    world.resource_mut::<AiController>().next_army_kind = 2;
    assert!(decide(&mut world).iter().all(|command| !matches!(
        command,
        PlayerCommand::EnqueueUnit {
            kind: UnitKind::Cavalry,
            ..
        }
    )));
    // The locked decision's rotation falls through to the unlocked legs and
    // moves the cursor; pin it back to the Cavalry leg for the unlock proof.
    world.resource_mut::<AiController>().next_army_kind = 2;
    world
        .get_resource_mut::<TeamEconomy>()
        .unwrap()
        .0
        .get_mut(&TeamId(2))
        .unwrap()
        .age = Age::Age2;
    assert!(decide(&mut world).iter().any(|command| matches!(
        command,
        PlayerCommand::EnqueueUnit {
            kind: UnitKind::Cavalry,
            ..
        }
    )));
}

#[test]
fn stable_places_on_the_authored_slot_once_age_two_lands() {
    let (mut world, _map) = ai_world(TeamId(2));
    let plan = MapFixture::team_plan(TeamId(2));
    grant(&mut world, TeamId(2), 0, 1000, 0);
    world
        .get_resource_mut::<TeamEconomy>()
        .unwrap()
        .0
        .get_mut(&TeamId(2))
        .unwrap()
        .age = Age::Age2;

    let commands = decide(&mut world);
    assert!(
        commands.iter().any(|command| matches!(command,
        PlayerCommand::PlaceBuilding { kind: BuildingKind::Stable, anchor, .. }
            if *anchor == plan.stable_anchor)),
        "Age 2 unlocks the authored Stable slot: {commands:?}"
    );
}

// ---- Defense / memory / attack ----------------------------------------------

/// Two worlds whose AI own/observed state is identical but whose hidden
/// enemies sit at different unseen cells must decide identical command
/// lists. Making a threat visible is then allowed to change the output.
#[test]
fn decisions_are_invariant_to_hidden_enemy_positions() {
    let mut command_lists = Vec::new();
    for (enemy_cell, extra_enemy) in [(GridPos::new(5, 5), false), (GridPos::new(60, 60), true)] {
        let (mut world, map) = ai_world(TeamId(2));
        spawn_military(
            &mut world,
            &map,
            UnitId(50),
            TeamId(1),
            enemy_cell,
            UnitKind::Spearman,
        );
        if extra_enemy {
            spawn_military(
                &mut world,
                &map,
                UnitId(51),
                TeamId(1),
                GridPos::new(3, 90),
                UnitKind::Archer,
            );
        }
        assert!(
            !crate::visibility::explored_by(&world, TeamId(2), enemy_cell),
            "the enemy cell must be hidden for the invariance premise"
        );
        command_lists.push(decide(&mut world));
    }
    assert_eq!(
        command_lists[0], command_lists[1],
        "hidden enemy positions must not influence the decision"
    );
}

#[test]
fn visible_base_threat_pulls_idle_military_into_defense() {
    let (mut world, map) = ai_world(TeamId(2));
    spawn_military(
        &mut world,
        &map,
        UnitId(20),
        TeamId(2),
        GridPos::new(108, 44),
        UnitKind::Spearman,
    );
    let villager = spawn_villager(
        &mut world,
        &map,
        UnitId(21),
        TeamId(2),
        GridPos::new(110, 48),
    );

    // No threat: no defense command.
    assert!(
        decide(&mut world)
            .iter()
            .all(|command| !matches!(command, PlayerCommand::Units(units)
            if matches!(units.kind, UnitCommandKind::AttackMove { .. })))
    );

    // A spearman inside the base radius and currently visible: the defender
    // is pulled into one AttackMove at the threat's position; the villager
    // is never drafted.
    spawn_military(
        &mut world,
        &map,
        UnitId(30),
        TeamId(1),
        GridPos::new(108, 50),
        UnitKind::Spearman,
    );
    refresh_visibility(&mut world, &map);
    let defense = decide(&mut world).into_iter().find(|command| {
        matches!(command, PlayerCommand::Units(units)
            if matches!(units.kind, UnitCommandKind::AttackMove { .. }))
    });
    let Some(PlayerCommand::Units(units)) = defense else {
        panic!("a visible base threat must trigger the defense step");
    };
    assert_eq!(
        units.units,
        vec![UnitId(20)],
        "only the idle military defends"
    );
    let threat_position = world
        .get::<SimPosition>(world.resource::<UnitIndex>().entity(UnitId(30)).unwrap())
        .unwrap()
        .current;
    assert_eq!(
        units.kind,
        UnitCommandKind::AttackMove {
            target: threat_position
        },
        "the defense marches at the nearest visible threat"
    );
    assert!(
        world.get::<MoveOrder>(villager).is_none(),
        "villagers are never drafted into defense by decide"
    );
}

#[test]
fn remembered_town_center_is_written_only_while_visible_and_retained() {
    let (mut world, map) = ai_world(TeamId(2));

    // Before any observation: no memory.
    decide(&mut world);
    assert!(
        world
            .resource::<AiController>()
            .remembered_enemy_town_center
            .is_none()
    );

    // A scout vantage next to the enemy Town Center: the sighting writes
    // the anchor cell.
    spawn_military(
        &mut world,
        &map,
        UnitId(20),
        TeamId(2),
        GridPos::new(16, 44),
        UnitKind::Spearman,
    );
    refresh_visibility(&mut world, &map);
    decide(&mut world);
    assert_eq!(
        world
            .resource::<AiController>()
            .remembered_enemy_town_center,
        Some(GridPos::new(12, 46)),
        "a genuinely visible enemy Town Center is remembered at its anchor"
    );

    // Vision lost: the memory is retained (it is the only enemy memory).
    let scout_entity = world.resource::<UnitIndex>().entity(UnitId(20)).unwrap();
    world
        .entity_mut(scout_entity)
        .insert(SimPosition::new(map.cell_center(GridPos::new(90, 44))))
        .remove::<MoveOrder>();
    refresh_visibility(&mut world, &map);
    decide(&mut world);
    assert_eq!(
        world
            .resource::<AiController>()
            .remembered_enemy_town_center,
        Some(GridPos::new(12, 46)),
        "the remembered cell survives the loss of current vision"
    );
}

#[test]
fn attack_fires_grouped_at_threshold_and_skips_engaged_units() {
    let (mut world, map) = ai_world(TeamId(2));

    // Five military: below the threshold, no attack (the scout may take one
    // on a plain Move, never an AttackMove).
    for index in 0..5_u32 {
        spawn_military(
            &mut world,
            &map,
            UnitId(20 + index),
            TeamId(2),
            GridPos::new(110, 52 + index as i32),
            UnitKind::Spearman,
        );
    }
    assert!(decide(&mut world).iter().all(|command| !matches!(command,
        PlayerCommand::Units(units) if matches!(units.kind, UnitCommandKind::AttackMove { .. }))));

    // Six military with one already engaged: the grouped attack carries the
    // five available ones and marches at the remembered Town Center — or,
    // before any sighting, the far end of the authored route.
    let sixth = spawn_military(
        &mut world,
        &map,
        UnitId(30),
        TeamId(2),
        GridPos::new(112, 60),
        UnitKind::Spearman,
    );
    world.entity_mut(sixth).insert(CombatOrder::Attack {
        target: CombatTarget::Unit(UnitId(1)),
        last_target_cell: None,
    });
    let plan = MapFixture::team_plan(TeamId(2));
    let attack = decide(&mut world).into_iter().find(|command| {
        matches!(command, PlayerCommand::Units(units)
            if matches!(units.kind, UnitCommandKind::AttackMove { .. }))
    });
    let Some(PlayerCommand::Units(units)) = attack else {
        panic!("the threshold army must issue one grouped AttackMove");
    };
    // UnitId(20) is absent twice over: the earlier scout step claimed it for
    // the next route leg, and the engaged sixth unit holds a CombatOrder.
    assert_eq!(
        units.units,
        vec![UnitId(21), UnitId(22), UnitId(23), UnitId(24)],
        "the engaged and already-claimed units are skipped, the idle ones march"
    );
    let far_route_cell = *plan.scout_route.last().unwrap();
    assert_eq!(
        units.kind,
        UnitCommandKind::AttackMove {
            target: Vec2::new(far_route_cell.x as f32 + 0.5, far_route_cell.y as f32 + 0.5)
        },
        "without a sighting the attack advances along the authored route"
    );

    // Once the enemy Town Center has genuinely been seen, it is the target.
    world
        .resource_mut::<AiController>()
        .remembered_enemy_town_center = Some(GridPos::new(12, 46));
    // Re-arm: after the previous decision the marchers hold Move/Combat
    // orders only once applied — decide is pure, so the same five are still
    // available and the attack now targets the remembered cell.
    let attack = decide(&mut world).into_iter().find(|command| {
        matches!(command, PlayerCommand::Units(units)
            if matches!(units.kind, UnitCommandKind::AttackMove { .. }))
    });
    assert!(
        matches!(
            attack,
            Some(PlayerCommand::Units(units))
                if units.kind == UnitCommandKind::AttackMove { target: Vec2::new(12.5, 46.5) }
        ),
        "the remembered Town Center cell is the attack target"
    );
}

// ---- Idempotence ------------------------------------------------------------

#[test]
fn decisions_never_recommand_busy_villagers() {
    let (mut world, mut map) = ai_world(TeamId(2));
    grant(&mut world, TeamId(2), 0, 2000, 0);

    // First decision applies real commands; run a few ticks so tasks land.
    decide_and_apply(&mut world, &mut map);
    for _ in 0..10 {
        step_tick(&mut world, &mut map);
    }

    let busy: Vec<UnitId> = villagers_of(&world, TeamId(2))
        .into_iter()
        .filter(|id| {
            let entity = world.resource::<UnitIndex>().entity(*id).unwrap();
            world.get::<WorkerTask>(entity) != Some(&WorkerTask::Idle)
                || world.get::<MoveOrder>(entity).is_some()
        })
        .collect();

    let commands = decide(&mut world);
    for command in &commands {
        match command {
            PlayerCommand::Gather { workers, .. } => {
                for worker in workers {
                    assert!(
                        !busy.contains(worker),
                        "a busy villager was re-gathered: {worker:?}"
                    );
                }
            }
            PlayerCommand::PlaceBuilding { builder, .. } => {
                assert!(
                    !busy.contains(builder),
                    "a busy villager was re-tasked to build"
                );
            }
            PlayerCommand::Units(units) => {
                for unit in &units.units {
                    assert!(!busy.contains(unit), "a busy unit was re-ordered");
                }
            }
            _ => {}
        }
    }
}

// ---- Bounded journeys -------------------------------------------------------

/// The grant-free proof: gather -> deposit -> build -> train -> Age 2, all
/// through ordinary AI commands from the authored 200/300/100 seed.
#[test]
fn economy_journey_reaches_age_two_without_grants() {
    let team = TeamId(2);
    let (mut world, mut map) = ai_world(team);
    let plan = MapFixture::team_plan(team);

    // Gather + deposit: food climbs back above the seed after the first paid
    // villager, which only real deposits can do.
    run_until(&mut world, &mut map, "food deposits", |world| {
        stockpile(world, team).food > 260
    });
    // Worker replacement runs (paid villager training).
    run_until(&mut world, &mut map, "worker replacement", |world| {
        villagers_of(world, team).len() >= 6
    });
    // Base growth on the authored slots.
    run_until(&mut world, &mut map, "the Barracks build", |world| {
        own_buildings_of(world, team, BuildingKind::Barracks)
            .iter()
            .any(|id| {
                world
                    .resource::<BuildingIndex>()
                    .entity(*id)
                    .is_some_and(|entity| {
                        world.get::<Building>(entity).unwrap().construction.complete
                    })
            })
    });
    // Army training through the real queue.
    run_until(&mut world, &mut map, "army training", |world| {
        !units_of_kind(world, team, UnitKind::Spearman).is_empty()
    });
    // Scouting actually discovered the expansion (route leg 2 sits on it).
    assert!(
        crate::visibility::explored_by(&world, team, plan.expansion_storehouse_slots[0]),
        "the AI must scout its expansion before depending on it"
    );
    run_until(&mut world, &mut map, "the expansion storehouse", |world| {
        !own_buildings_of(world, team, BuildingKind::Storehouse).is_empty()
    });
    // Age 2, still with zero grants anywhere in this journey.
    run_until(&mut world, &mut map, "Age 2", |world| {
        age_of(world, team) == Age::Age2
    });
}

/// A real raid: enemy combat kills AI workers, later income drops until the
/// paid replacement lands — nothing is replenished for free.
/// A real raid: enemy combat kills the AI's food gatherers; gross food
/// deposits drop until paid replacements land — nothing is replenished free.
#[test]
fn raid_reduces_income_until_paid_replacement_lands() {
    let team = TeamId(2);
    let (mut world, mut map) = ai_world(team);

    run_until(&mut world, &mut map, "worker growth", |world| {
        villagers_of(world, team).len() >= 6
    });

    // Gross food deposits over a window (positive deltas only — spending
    // noise excluded) is the honest income measure for this economy.
    fn food_deposits(world: &mut World, map: &mut GridMap, team: TeamId, ticks: u32) -> u32 {
        let mut last = stockpile(world, team).food;
        let mut gained = 0;
        for _ in 0..ticks {
            step_tick(world, map);
            let now = stockpile(world, team).food;
            gained += now.saturating_sub(last);
            last = now;
        }
        gained
    }

    // Baseline window before the raid.
    let income_before = food_deposits(&mut world, &mut map, team, 300);

    // The raid: enemy spearmen kill every AI food gatherer up close.
    let food_workers: Vec<UnitId> = villagers_of(&world, team)
        .into_iter()
        .filter(|id| {
            let entity = world.resource::<UnitIndex>().entity(*id).unwrap();
            task_source(&world, entity)
                .and_then(|source| {
                    let source_entity = world
                        .resource::<crate::economy::ResourceIndex>()
                        .entity(source)?;
                    world
                        .get::<ResourceSource>(source_entity)
                        .map(|state| state.kind)
                })
                .is_some_and(|kind| kind == ResourceKind::Food)
        })
        .collect();
    assert!(
        !food_workers.is_empty(),
        "the raid scenario needs food gatherers to raid"
    );
    let victims = villagers_of(&world, team);
    let mut raiders = Vec::new();
    for (index, victim) in food_workers.clone().into_iter().enumerate() {
        let victim_entity = world.resource::<UnitIndex>().entity(victim).unwrap();
        let position = world.get::<SimPosition>(victim_entity).unwrap().current;
        let raider = spawn_military(
            &mut world,
            &map,
            UnitId(900 + index as u32),
            TeamId(1),
            map.world_to_cell(position),
            UnitKind::Spearman,
        );
        world.entity_mut(raider).insert(CombatOrder::Attack {
            target: CombatTarget::Unit(victim),
            last_target_cell: None,
        });
        raiders.push(raider);
    }
    // The raiders' own sight: one refresh so their targets are visible to
    // team 1 before the first combat step (a stale map would clear the fresh
    // orders as hidden-target attacks).
    refresh_visibility(&mut world, &map);
    run_until(&mut world, &mut map, "the raid kills", |world| {
        villagers_of(world, team).len() <= victims.len() - food_workers.len()
    });
    // Pull the raiders out so replacements are not slaughtered too.
    for (index, raider) in raiders.iter().enumerate() {
        world
            .entity_mut(*raider)
            .insert(SimPosition::new(Vec2::new(5.5, 5.5 + index as f32)));
    }

    // Deposits crater while the gatherers are dead: the window is shorter
    // than one training-plus-walk cycle, so no replacement has landed yet.
    let income_after = food_deposits(&mut world, &mut map, team, 250);
    assert!(
        income_before > 0 && income_after * 4 < income_before,
        "the raid must crater food income: before {income_before}, after {income_after}"
    );

    // Replacement is paid, never free: some recovery tick charges exactly 50
    // food (the villager cost — nothing else costs exactly 50 food), and the
    // worker count recovers.
    let mut saw_paid_replacement = false;
    let mut recovered = false;
    for _ in 0..JOURNEY_BUDGET {
        let before_tick = stockpile(&world, team).food;
        step_tick(&mut world, &mut map);
        let delta = before_tick as i64 - stockpile(&world, team).food as i64;
        if delta == 50 {
            saw_paid_replacement = true;
        }
        if villagers_of(&world, team).len() >= victims.len() {
            recovered = true;
            break;
        }
    }
    assert!(recovered, "the AI must replace its lost workers");
    assert!(
        saw_paid_replacement,
        "worker replacement must go through paid production"
    );
}

/// Population stall: with the cap full the AI places the next authored House
/// and production resumes past the old cap.
#[test]
fn population_stall_builds_a_house_and_recovers() {
    let team = TeamId(2);
    let (mut world, mut map) = ai_world(team);
    // Grants are fine here — the stall is the scenario under test.
    grant(&mut world, team, 3000, 3000, 2000);

    run_until(&mut world, &mut map, "the population stall", |world| {
        population_cap(world, team) > 10
            || (crate::production::population_used(world, team)
                >= crate::production::population_cap(world, team))
    });
    run_until(&mut world, &mut map, "the stall recovery House", |world| {
        crate::production::population_cap(world, team) >= 20
    });
    assert!(
        !own_buildings_of(&world, team, BuildingKind::House).is_empty(),
        "the recovery must come from a real AI-placed House"
    );
    run_until(
        &mut world,
        &mut map,
        "production past the old cap",
        |world| crate::production::population_used(world, team) > 10,
    );
}
