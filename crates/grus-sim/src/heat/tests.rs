use bevy::prelude::{Entity, Or, With, World};

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{BuildingKind, MIN_SURFACE_EFFICIENCY, building_spec};
use crate::combat::{Health, destroy_building};
use crate::economy::TeamEconomy;
use crate::fixture::{MapFixture, seed_skirmish};
use crate::ids::{BuildingId, TeamId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{SIM_STEP_SECONDS, Unit};
use crate::session::{MatchPhase, MatchResult, MatchSession};

fn battlefield() -> (World, GridMap) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    let mut world = World::new();
    seed_skirmish(&mut world, &mut map, &fixture);
    (world, map)
}

/// Directly seeds a Team-1 building (completed unless `complete` is false),
/// blocking its footprint and registering the index — no placement
/// pipeline, exactly like the combat tests' constructor.
fn spawn_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    kind: BuildingKind,
    anchor: GridPos,
    complete: bool,
) -> Entity {
    let spec = building_spec(kind);
    let footprint = Footprint::new(anchor, spec.width, spec.height);
    for cell in footprint.cells() {
        map.set_blocked(cell, true);
    }
    let entity = world
        .spawn((
            Building {
                id,
                team: TeamId(1),
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete,
                    active_builder: None,
                },
            },
            footprint,
            Health {
                current: spec.max_health,
                max: spec.max_health,
            },
        ))
        .id();
    world
        .get_resource_or_insert_with(BuildingIndex::default)
        .insert(id, entity);
    entity
}

fn heat(world: &World) -> HeatState {
    *world.resource::<HeatState>()
}

fn team_coal(world: &World) -> u32 {
    world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.coal
}

fn set_team_coal(world: &mut World, coal: u32) {
    world
        .resource_mut::<TeamEconomy>()
        .0
        .get_mut(&TeamId(1))
        .unwrap()
        .stockpile
        .coal = coal;
}

/// Seeds two completed furnaces on free cells beside the Research Center.
fn seed_two_furnaces(world: &mut World, map: &mut GridMap) {
    spawn_building(
        world,
        map,
        BuildingId(2),
        BuildingKind::CoalFurnace,
        GridPos::new(68, 46),
        true,
    );
    spawn_building(
        world,
        map,
        BuildingId(3),
        BuildingKind::CoalFurnace,
        GridPos::new(68, 49),
        true,
    );
}

/// `seed_skirmish` inserts a default `HeatState` beside the wave clock; the
/// unstepped state already reads full efficiency.
#[test]
fn seed_skirmish_inserts_a_default_heat_state() {
    let (world, _map) = battlefield();
    let state = world.resource::<HeatState>();
    assert_eq!(
        (state.supply, state.demand, state.burn_remainder),
        (0, 0, 0.0)
    );
    assert_eq!(state.efficiency(), 1.0);
    assert_eq!(surface_efficiency(&world), 1.0);
}

/// The first scheduled tick even in Start reports the seeded 15/10 full
/// efficiency snapshot and burns no Coal.
#[test]
fn first_start_phase_tick_reports_seed_heat_without_burning() {
    let (mut world, mut _map) = battlefield();
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });

    step_heat(&mut world, SIM_STEP_SECONDS);

    let state = heat(&world);
    assert_eq!((state.supply, state.demand), (15, 10));
    assert_eq!(state.efficiency(), 1.0);
    assert_eq!(surface_efficiency(&world), 1.0);
    assert_eq!(state.burn_remainder, 0.0);
    assert_eq!(team_coal(&world), 150);
}

/// A world without `HeatState` and an inserted-but-unstepped default both
/// read the neutral full efficiency, never 0.0.
#[test]
fn missing_or_default_heat_state_reads_full_efficiency() {
    let mut world = World::new();
    assert_eq!(surface_efficiency(&world), 1.0);

    world.insert_resource(HeatState::default());
    let state = heat(&world);
    assert_eq!(
        (state.supply, state.demand, state.burn_remainder),
        (0, 0, 0.0)
    );
    assert_eq!(state.efficiency(), 1.0);
    assert_eq!(surface_efficiency(&world), 1.0);
}

/// Completed buildings add exact demand; construction sites never do.
#[test]
fn completed_buildings_add_demand_but_sites_do_not() {
    let (mut world, mut map) = battlefield();
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Bunkhouse,
        GridPos::new(68, 46),
        true,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(3),
        BuildingKind::Bunkhouse,
        GridPos::new(71, 46),
        false,
    );

    step_heat(&mut world, SIM_STEP_SECONDS);

    let state = heat(&world);
    assert_eq!((state.supply, state.demand), (15, 11));
    // Supply 15 over demand 11 is a surplus: efficiency clamps at full.
    assert_eq!(state.efficiency(), 1.0);
}

/// Zero demand reads full efficiency even when supply is zero.
#[test]
fn zero_demand_reads_full_efficiency() {
    let mut world = World::new();

    step_heat(&mut world, SIM_STEP_SECONDS);

    let state = heat(&world);
    assert_eq!((state.supply, state.demand), (0, 0));
    assert_eq!(state.efficiency(), 1.0);
    assert_eq!(surface_efficiency(&world), 1.0);
}

/// A severe deficit floors surface efficiency at the catalogue minimum; a
/// surplus clamps at full efficiency.
#[test]
fn severe_deficit_floors_surface_efficiency() {
    let deficit = HeatState {
        supply: 1,
        demand: 10,
        burn_remainder: 0.0,
    };
    assert_eq!(deficit.efficiency(), MIN_SURFACE_EFFICIENCY);

    let surplus = HeatState {
        supply: 40,
        demand: 10,
        burn_remainder: 0.0,
    };
    assert_eq!(surplus.efficiency(), 1.0);
}

/// Two fueled furnaces at 20 Hz pool 0.5 Coal/s each and debit exactly one
/// whole Coal per simulated second while contributing their supply.
#[test]
fn furnaces_burn_half_a_coal_per_second_each_at_20hz() {
    let (mut world, mut map) = battlefield();
    seed_two_furnaces(&mut world, &mut map);

    // Nineteen ticks stay fractional (0.95 Coal pooled); the 20th completes
    // the first whole unit.
    for _ in 0..19 {
        step_heat(&mut world, SIM_STEP_SECONDS);
        assert_eq!(
            team_coal(&world),
            150,
            "fractional burn must not debit early"
        );
    }
    step_heat(&mut world, SIM_STEP_SECONDS);
    assert!(heat(&world).burn_remainder.abs() < 1e-4);
    assert_eq!(team_coal(&world), 149);

    for _ in 0..20 {
        step_heat(&mut world, SIM_STEP_SECONDS);
    }
    assert_eq!(team_coal(&world), 148);
    let state = heat(&world);
    assert_eq!((state.supply, state.demand), (15 + 2 * 12, 10));
    assert_eq!(state.efficiency(), 1.0);
}

/// A tick's final consumption may supply that tick, but a coal-empty next
/// tick supplies and burns nothing, carrying no fractional debt.
#[test]
fn depleting_the_stockpile_stops_supply_the_next_tick() {
    let (mut world, mut map) = battlefield();
    seed_two_furnaces(&mut world, &mut map);
    set_team_coal(&mut world, 1);

    // The 20th tick debits the last whole unit; every tick up to and
    // including it stays fully fueled.
    for tick in 1..=20 {
        step_heat(&mut world, SIM_STEP_SECONDS);
        assert_eq!(
            heat(&world).supply,
            15 + 2 * 12,
            "tick {tick} must stay fueled"
        );
    }
    assert_eq!(team_coal(&world), 0);
    assert_eq!(heat(&world).burn_remainder, 0.0);

    step_heat(&mut world, SIM_STEP_SECONDS);
    let state = heat(&world);
    assert_eq!((state.supply, state.demand), (15, 10));
    assert_eq!(state.burn_remainder, 0.0);
    assert_eq!(team_coal(&world), 0);
}

/// Refueling re-enables the furnaces on the next heat tick from a clean
/// remainder: no phantom debt resumes the burn early.
#[test]
fn refueling_reenables_furnaces_from_a_clean_remainder() {
    let (mut world, mut map) = battlefield();
    seed_two_furnaces(&mut world, &mut map);
    set_team_coal(&mut world, 1);
    for _ in 0..20 {
        step_heat(&mut world, SIM_STEP_SECONDS);
    }
    assert_eq!(team_coal(&world), 0);

    set_team_coal(&mut world, 5);
    step_heat(&mut world, SIM_STEP_SECONDS);
    let state = heat(&world);
    assert_eq!(state.supply, 15 + 2 * 12);
    assert_eq!(
        team_coal(&world),
        5,
        "the first fractional tick must not debit"
    );
    assert!((state.burn_remainder - 0.05).abs() < 1e-6);
}

/// A purchase that empties the stockpile mid-fraction leaves no stale debt:
/// the next Playing tick clears the remainder even though the burn branch
/// is skipped, so after a refuel the first whole-Coal debit lands exactly
/// on schedule — never early.
#[test]
fn purchase_emptied_stockpile_refuels_without_early_debt() {
    let (mut world, mut map) = battlefield();
    seed_two_furnaces(&mut world, &mut map);

    // Nineteen fueled ticks pool 0.95 Coal without debiting.
    for _ in 0..19 {
        step_heat(&mut world, SIM_STEP_SECONDS);
    }
    assert_eq!(team_coal(&world), 150);

    // A 20-Coal purchase spends the last stockpiled Coal between ticks
    // while the fractional pool is hot; later unfueled ticks skip the burn
    // branch, which must still drop the stale fraction.
    set_team_coal(&mut world, 0);
    step_heat(&mut world, SIM_STEP_SECONDS);
    let state = heat(&world);
    assert_eq!(state.burn_remainder, 0.0);
    assert_eq!(
        state.supply, 15,
        "an empty stockpile supplies no furnace heat"
    );
    assert_eq!(team_coal(&world), 0);

    // Refuel: the next 19 ticks stay fractional, the 20th debits exactly
    // one whole unit — on schedule, with no phantom 0.95 debt.
    set_team_coal(&mut world, 5);
    for _ in 0..19 {
        step_heat(&mut world, SIM_STEP_SECONDS);
        assert_eq!(team_coal(&world), 5, "fractional burn must not debit early");
    }
    step_heat(&mut world, SIM_STEP_SECONDS);
    assert_eq!(team_coal(&world), 4);
    assert!(heat(&world).burn_remainder.abs() < 1e-4);
}

/// Paused and Result steps still refresh supply/demand — fueled furnace
/// output included — but never touch Coal or the burn remainder.
#[test]
fn non_playing_ticks_recompute_the_snapshot_without_burning() {
    for phase in [
        MatchPhase::Paused,
        MatchPhase::Result(MatchResult(TeamId(1))),
    ] {
        let (mut world, mut map) = battlefield();
        spawn_building(
            &mut world,
            &mut map,
            BuildingId(2),
            BuildingKind::CoalFurnace,
            GridPos::new(68, 46),
            true,
        );
        world.insert_resource(MatchSession { phase });
        // A leftover fractional pool from earlier Playing ticks stays put.
        world.resource_mut::<HeatState>().burn_remainder = 0.3;

        step_heat(&mut world, SIM_STEP_SECONDS);

        let state = heat(&world);
        assert_eq!((state.supply, state.demand), (15 + 12, 10), "{phase:?}");
        assert!((state.burn_remainder - 0.3).abs() < 1e-6, "{phase:?}");
        assert_eq!(team_coal(&world), 150, "{phase:?}");
    }
}

/// Destruction leaves the totals on the next heat tick, and the reset path
/// replaces `HeatState`, clearing any pooled burn fraction.
#[test]
fn destruction_updates_totals_and_reset_clears_the_remainder() {
    let (mut world, mut map) = battlefield();
    let bunker = spawn_building(
        &mut world,
        &mut map,
        BuildingId(2),
        BuildingKind::Bunkhouse,
        GridPos::new(68, 46),
        true,
    );
    step_heat(&mut world, SIM_STEP_SECONDS);
    assert_eq!(heat(&world).demand, 11);

    destroy_building(&mut world, &mut map, bunker);
    step_heat(&mut world, SIM_STEP_SECONDS);
    assert_eq!(heat(&world).demand, 10);

    world.resource_mut::<HeatState>().burn_remainder = 0.7;

    // Bridge-style clear before a reseed: gameplay entities and the
    // seed-owned indexes die, then `seed_skirmish` re-inserts the state.
    let stale: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Unit>, With<Building>)>>()
        .iter(&world)
        .collect();
    for entity in stale {
        world.despawn(entity);
    }
    world.remove_resource::<crate::commands::UnitIndex>();
    world.remove_resource::<BuildingIndex>();
    world.remove_resource::<crate::economy::ResourceIndex>();
    world.remove_resource::<crate::ids::IdAllocator>();
    let fixture = MapFixture::battlefield();
    let mut fresh_map = fixture.map.clone();
    seed_skirmish(&mut world, &mut fresh_map, &fixture);
    let state = world.resource::<HeatState>();
    assert_eq!(
        (state.supply, state.demand, state.burn_remainder),
        (0, 0, 0.0)
    );
}
