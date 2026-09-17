use std::num::NonZeroU32;

use bevy::math::Vec2;
use bevy::prelude::World;

use super::*;
use crate::buildings::{Building, BuildingIndex, ConstructionState};
use crate::catalog::{Age, BuildingKind, ResourceKind, UnitKind, building_spec, unit_spec};
use crate::commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, apply_player_command,
    spawn_unit,
};
use crate::economy::{
    Carry, Dropoff, GatherProgress, LastRouteReject, ResourceIndex, ResourceSource,
    ResourceStockpile, TeamEconomy, WorkerTask,
};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::movement::{SIM_STEP_SECONDS, SimPosition, step_movement};
use crate::production::ProductionQueue;
use crate::session::tests as journeys;
use crate::session::{MatchPhase, MatchSession};

fn open_map() -> GridMap {
    GridMap::new(32, 64)
}

fn spawn_combatant(
    world: &mut World,
    id: UnitId,
    team: TeamId,
    position: Vec2,
    kind: UnitKind,
) -> Entity {
    spawn_unit(world, id, team, position, kind, unit_spec(kind).speed)
}

fn issue(world: &mut World, map: &mut GridMap, command: PlayerCommand) -> CommandResult {
    apply_player_command(world, map, command)
}

fn attack_command(issuer: TeamId, units: &[UnitId], target: CombatTarget) -> PlayerCommand {
    PlayerCommand::Attack {
        issuer,
        units: units.to_vec(),
        target,
    }
}

fn attack_move_command(issuer: TeamId, units: &[UnitId], target: Vec2) -> PlayerCommand {
    PlayerCommand::Units(UnitCommand {
        issuer,
        units: units.to_vec(),
        kind: UnitCommandKind::AttackMove { target },
    })
}

fn spawn_building(
    world: &mut World,
    map: &mut GridMap,
    id: BuildingId,
    team: TeamId,
    kind: BuildingKind,
    anchor: GridPos,
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
                team,
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: true,
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

fn events(world: &World) -> &[CombatEvent] {
    &world.resource::<CombatEvents>().0
}

#[test]
fn melee_attacker_in_range_hits_without_pathing() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );

    let outcome = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    assert_eq!(outcome.accepted_units, vec![UnitId(1)]);

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);
    assert_eq!(
        world.get::<AttackCooldown>(attacker).unwrap().0,
        1.0,
        "the catalogue cooldown is installed on strike"
    );
    assert!(
        world.get::<MoveOrder>(attacker).is_none(),
        "an in-range attacker never paths"
    );
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0],
        CombatEvent {
            attacker: UnitId(1),
            target: CombatTarget::Unit(UnitId(2)),
            damage: 10,
            position: Vec2::new(6.5, 5.5),
            ranged: false,
            killed: false,
        }
    );
}

#[test]
fn melee_attacker_out_of_range_pursues_without_striking() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(events(&world).is_empty(), "out of range means no hit");
    assert_eq!(world.get::<Health>(defender).unwrap().current, 100);
    let order = world.get::<MoveOrder>(attacker).expect("pursuit route");
    assert_eq!(order.goal, GridPos::new(7, 4), "pursues the target's cell");
}

#[test]
fn archer_strikes_at_ranged_range_without_moving() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Archer,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(10.5, 5.5),
        UnitKind::Cavalry,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // Cavalry takes the archer's base damage: its counter bonus names Spearman.
    assert_eq!(world.get::<Health>(defender).unwrap().current, 132);
    assert!(world.get::<MoveOrder>(attacker).is_none());
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].ranged, "archer hits are ranged");
    assert_eq!(recorded[0].damage, 8);
}

#[test]
fn counter_bonus_applies_only_to_the_named_kind() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let cavalry = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Cavalry,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // Spearman base 10 + counter bonus 10 vs Cavalry.
    assert_eq!(world.get::<Health>(cavalry).unwrap().current, 120);
    assert_eq!(events(&world)[0].damage, 20);
}

#[test]
fn cooldown_gates_repeat_strikes_until_elapsed() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);

    // Events are current-tick-only: the next step is empty and the target is
    // still on cooldown.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(events(&world).is_empty());
    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);

    let mut restrike_at = None;
    for tick in 1..=40 {
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        if !events(&world).is_empty() {
            restrike_at = Some(tick);
            break;
        }
    }
    let restrike_at = restrike_at.expect("the cooldown must elapse within ~1s");
    assert!(
        (15..=25).contains(&restrike_at),
        "second strike waits roughly one cooldown, got tick {restrike_at}"
    );
    assert_eq!(world.get::<Health>(defender).unwrap().current, 80);
}

#[test]
fn frozen_ticks_clear_combat_events_so_nothing_replays_after_resume() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(events(&world).len(), 1, "the Playing tick records its hit");

    // Draining the presentation buffer is bookkeeping, not gameplay: a
    // frozen tick must still clear it, or the last Playing tick's events
    // survive and replay on the first resumed frame.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Paused,
    });
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        events(&world).is_empty(),
        "a frozen tick must drain CombatEvents"
    );

    // Resume replays nothing: frozen ticks never ticked the strike
    // cooldown, so the first Playing tick is still on cooldown.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Playing,
    });
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        events(&world).is_empty(),
        "resume must not replay stale hits"
    );
    assert_eq!(world.get::<Health>(defender).unwrap().current, 90);
    // The resumed tick ticks the cooldown (one SIM_STEP_SECONDS off) but is
    // still far from ready: no replayed strike.
    assert_eq!(world.get::<AttackCooldown>(attacker).unwrap().0, 0.95);
}

#[test]
fn killing_blow_atomically_destroys_a_building() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(11.5, 48.5),
        UnitKind::Spearman,
    );
    let town_center = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(1),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // Base damage only: buildings never take a counter bonus.
    assert_eq!(world.get::<Health>(town_center).unwrap().current, 790);
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].target, CombatTarget::Building(BuildingId(1)));
    assert_eq!(recorded[0].damage, 10);
    assert_eq!(recorded[0].position, Vec2::new(12.0, 48.5));
    assert!(!recorded[0].killed);

    // The killing blow runs the atomic destruction transaction. The
    // attacker's cooldown from the first strike is cleared so the blow
    // lands now.
    let attacker = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
    world.get_mut::<AttackCooldown>(attacker).unwrap().0 = 0.0;
    world.get_mut::<Health>(town_center).unwrap().current = 10;
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);
    assert!(world.get::<Health>(town_center).is_none(), "despawned");
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .is_none(),
        "the stable index entry is removed"
    );
    for cell in Footprint::new(GridPos::new(12, 46), 4, 4).cells() {
        assert!(map.is_walkable(cell), "footprint cell {cell:?} freed");
    }
    assert!(
        !target_eligible(&world, TeamId(1), CombatTarget::Building(BuildingId(1))),
        "the destroyed building leaves through the eligibility seam"
    );

    // The attacker's direct order ends on the vanished target through
    // target_eligible — no global CombatOrder scan.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(world.get::<CombatOrder>(attacker).is_none());
}

#[test]
fn destroying_a_dropoff_reroutes_the_carrying_worker() {
    let mut world = World::new();
    let mut map = open_map();
    let storehouse = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(1),
        BuildingKind::Storehouse,
        GridPos::new(8, 10),
    );
    world
        .entity_mut(storehouse)
        .insert(Dropoff { team: TeamId(1) });
    let backup = spawn_building(
        &mut world,
        &mut map,
        BuildingId(2),
        TeamId(1),
        BuildingKind::Storehouse,
        GridPos::new(16, 10),
    );
    world.entity_mut(backup).insert(Dropoff { team: TeamId(1) });

    let worker = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(7.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(worker).insert((
        Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(5).unwrap(),
        },
        GatherProgress::default(),
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(1),
            slot: GridPos::new(7, 10),
        },
    ));
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(2),
        Vec2::new(7.5, 9.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(storehouse).unwrap().current = 10;
    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(2),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .is_none()
    );
    let task = world.get::<WorkerTask>(worker).cloned().unwrap();
    match task {
        WorkerTask::ToDropoff {
            source,
            dropoff,
            slot,
        } => {
            assert_eq!(source, ResourceId(1), "the task's source is preserved");
            assert_eq!(
                dropoff,
                BuildingId(2),
                "rerouted immediately to the surviving same-team drop-off"
            );
            assert!(
                world
                    .get::<Footprint>(backup)
                    .unwrap()
                    .is_immediately_adjacent(slot),
                "new slot {slot:?} sits on the backup's perimeter"
            );
            let order = world
                .get::<MoveOrder>(worker)
                .expect("a route to the new drop-off");
            assert_eq!(order.goal, slot);
        }
        other => panic!("expected a rerouted ToDropoff, got {other:?}"),
    }
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(5).unwrap(),
        }),
        "the reroute preserves Carry"
    );
}

#[test]
fn destroying_the_last_reachable_dropoff_idles_the_worker_preserving_carry() {
    let mut world = World::new();
    let mut map = open_map();
    let town_center = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(1),
        BuildingKind::TownCenter,
        GridPos::new(12, 10),
    );
    world
        .entity_mut(town_center)
        .insert(Dropoff { team: TeamId(1) });
    let storehouse = spawn_building(
        &mut world,
        &mut map,
        BuildingId(2),
        TeamId(1),
        BuildingKind::Storehouse,
        GridPos::new(24, 10),
    );
    world
        .entity_mut(storehouse)
        .insert(Dropoff { team: TeamId(1) });

    let worker = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(4.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(worker).insert((
        Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
        WorkerTask::ToDropoff {
            source: ResourceId(1),
            dropoff: BuildingId(1),
            slot: GridPos::new(4, 10),
        },
    ));
    // Box the worker in so neither the destroyed drop-off's freed footprint
    // nor the surviving storehouse is pathable.
    for cell in [
        GridPos::new(3, 9),
        GridPos::new(4, 9),
        GridPos::new(5, 9),
        GridPos::new(3, 10),
        GridPos::new(5, 10),
        GridPos::new(3, 11),
        GridPos::new(4, 11),
        GridPos::new(5, 11),
    ] {
        map.set_blocked(cell, true);
    }
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(2),
        Vec2::new(11.5, 13.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(town_center).unwrap().current = 10;
    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(2),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(world.get::<WorkerTask>(worker), Some(&WorkerTask::Idle));
    assert!(world.get::<MoveOrder>(worker).is_none());
    assert_eq!(
        world.get::<Carry>(worker),
        Some(&Carry::Holding {
            kind: ResourceKind::Wood,
            amount: NonZeroU32::new(10).unwrap(),
        }),
        "the idle never discards Carry"
    );
    assert_eq!(
        world.resource::<LastRouteReject>().0,
        Some(RejectReason::Unreachable),
        "typed route failure recorded for bridge feedback"
    );
}

#[test]
fn destroying_a_construction_site_cancels_every_tasking_worker() {
    let mut world = World::new();
    let mut map = open_map();
    let site = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(1),
        BuildingKind::Barracks,
        GridPos::new(12, 10),
    );
    world
        .get_mut::<Building>(site)
        .unwrap()
        .construction
        .complete = false;

    let walker = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(11.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(walker).insert((
        GatherProgress::default(),
        WorkerTask::ToConstruction {
            building: BuildingId(1),
            slot: GridPos::new(11, 10),
        },
        MoveOrder {
            waypoints: vec![map.cell_center(GridPos::new(11, 10))],
            next: 0,
            goal: GridPos::new(11, 10),
            map_revision: map.revision(),
            last_failed_replan: None,
        },
    ));
    let builder = spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(11.5, 14.5),
        UnitKind::Villager,
    );
    world.entity_mut(builder).insert((
        GatherProgress::default(),
        WorkerTask::Constructing {
            building: BuildingId(1),
        },
    ));
    world
        .get_mut::<Building>(site)
        .unwrap()
        .construction
        .active_builder = Some(UnitId(3));

    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(2),
        Vec2::new(11.5, 9.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(site).unwrap().current = 10;
    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(2),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(world.get::<WorkerTask>(walker), Some(&WorkerTask::Idle));
    assert!(world.get::<MoveOrder>(walker).is_none());
    assert_eq!(world.get::<WorkerTask>(builder), Some(&WorkerTask::Idle));
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .is_none()
    );
    for cell in Footprint::new(GridPos::new(12, 10), 3, 3).cells() {
        assert!(map.is_walkable(cell), "site cell {cell:?} freed");
    }
}

#[test]
fn destroying_a_farm_idles_its_workers_in_every_phase_preserving_carry() {
    let mut world = World::new();
    let mut map = open_map();
    let farm = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(1),
        BuildingKind::Farm,
        GridPos::new(8, 10),
    );
    let farm_source = ResourceId(1);
    world.entity_mut(farm).insert(ResourceSource {
        id: farm_source,
        kind: ResourceKind::Food,
        remaining: None,
        assigned_worker: Some(UnitId(3)),
    });
    world
        .get_resource_or_insert_with(ResourceIndex::default)
        .insert(farm_source, farm);
    let town_center = spawn_building(
        &mut world,
        &mut map,
        BuildingId(2),
        TeamId(1),
        BuildingKind::TownCenter,
        GridPos::new(16, 10),
    );
    world
        .entity_mut(town_center)
        .insert(Dropoff { team: TeamId(1) });

    // Moving to the farm, gathering from it, and returning a Farm-sourced
    // load to the (surviving) Town Center.
    let to_farm = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(1),
        Vec2::new(7.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(to_farm).insert((
        Carry::Empty,
        GatherProgress::default(),
        WorkerTask::ToSource {
            source: farm_source,
            slot: GridPos::new(7, 10),
        },
    ));
    let gatherer = spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(6.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(gatherer).insert((
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(5).unwrap(),
        },
        GatherProgress(0.5),
        WorkerTask::Gathering {
            source: farm_source,
        },
    ));
    let returning = spawn_combatant(
        &mut world,
        UnitId(4),
        TeamId(1),
        Vec2::new(15.5, 10.5),
        UnitKind::Villager,
    );
    world.entity_mut(returning).insert((
        Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        },
        GatherProgress::default(),
        WorkerTask::ToDropoff {
            source: farm_source,
            dropoff: BuildingId(2),
            slot: GridPos::new(15, 10),
        },
    ));

    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(2),
        Vec2::new(7.5, 9.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(farm).unwrap().current = 10;
    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(2),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    for worker in [to_farm, gatherer, returning] {
        assert_eq!(
            world.get::<WorkerTask>(worker),
            Some(&WorkerTask::Idle),
            "the destroyed Farm idles its worker in every phase"
        );
        assert!(world.get::<MoveOrder>(worker).is_none());
    }
    assert_eq!(
        world.get::<Carry>(gatherer),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(5).unwrap(),
        })
    );
    assert_eq!(
        world.get::<Carry>(returning),
        Some(&Carry::Holding {
            kind: ResourceKind::Food,
            amount: NonZeroU32::new(10).unwrap(),
        }),
        "Carry is preserved through the idle"
    );
    assert!(
        world
            .resource::<ResourceIndex>()
            .entity(farm_source)
            .is_none(),
        "the Farm's resource identity leaves the ResourceIndex"
    );
    assert!(world.get::<Building>(farm).is_none(), "the Farm despawned");
    for cell in Footprint::new(GridPos::new(8, 10), 2, 2).cells() {
        assert!(map.is_walkable(cell), "farm cell {cell:?} freed");
    }
}

#[test]
fn destroying_a_producer_drops_its_queue_without_refund() {
    let mut world = World::new();
    let mut map = open_map();
    let barracks = spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(1),
        BuildingKind::Barracks,
        GridPos::new(12, 10),
    );
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile {
            food: 1000,
            wood: 1000,
            gold: 1000,
        },
        Age::Age1,
    );
    world.insert_resource(economy);
    let result = issue(
        &mut world,
        &mut map,
        PlayerCommand::EnqueueUnit {
            issuer: TeamId(1),
            building: BuildingId(1),
            kind: UnitKind::Spearman,
        },
    );
    assert_eq!(result.reject, None, "enqueue accepted");
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.food,
        940,
        "the charge happened at acceptance"
    );

    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(2),
        Vec2::new(11.5, 9.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(barracks).unwrap().current = 10;
    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(2),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(
        world.get::<ProductionQueue>(barracks).is_none(),
        "the queue disappears with the building"
    );
    assert!(
        world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .is_none()
    );
    assert_eq!(
        world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile.food,
        940,
        "destruction refunds nothing"
    );
}

#[test]
fn attack_rejects_missing_friendly_and_dead_targets() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    // A friendly teammate exists for the friendly-fire rejection below.
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(1),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );
    let dead = spawn_combatant(
        &mut world,
        UnitId(4),
        TeamId(2),
        Vec2::new(7.5, 5.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(dead).unwrap().current = 0;

    let missing = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(9))),
    );
    let enemy_own_team = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(3))),
    );
    let dead_target = issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(4))),
    );

    assert_eq!(
        missing.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::TargetMissing)]
    );
    assert_eq!(
        enemy_own_team.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::InvalidTarget)]
    );
    assert_eq!(
        dead_target.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::InvalidTarget)]
    );
    assert!(world.get::<CombatOrder>(attacker).is_none());
}

#[test]
fn attack_move_rejects_unreachable_destinations() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    map.set_blocked(GridPos::new(10, 5), true);

    let blocked = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(10.5, 5.5)),
    );
    let out_of_bounds = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(500.5, 500.5)),
    );

    assert_eq!(
        blocked.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::Unreachable)]
    );
    assert_eq!(
        out_of_bounds.rejected_units,
        vec![(UnitId(1), crate::commands::RejectReason::Unreachable)]
    );
    assert!(world.get::<CombatOrder>(attacker).is_none());
}

#[test]
fn attack_move_rejects_walled_off_destinations_without_disturbing_prior_orders() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let enemy = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );

    // Prior intent: a direct Attack plus an active route, and no WorkerTask
    // (a cancellation would install `Idle`, so `None` must stay `None`).
    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    world.entity_mut(attacker).insert(MoveOrder {
        waypoints: vec![map.cell_center(GridPos::new(6, 5))],
        next: 0,
        goal: GridPos::new(6, 5),
        map_revision: map.revision(),
        last_failed_replan: None,
    });
    let order_before = world.get::<CombatOrder>(attacker).cloned().unwrap();
    let route_before = world.get::<MoveOrder>(attacker).cloned().unwrap();
    let worker_before = world.get::<WorkerTask>(attacker).cloned();

    // Wall the attacker in; the destination cell itself stays walkable, so
    // only the reachability probe can tell the two cases apart.
    for cell in [
        GridPos::new(4, 4),
        GridPos::new(5, 4),
        GridPos::new(6, 4),
        GridPos::new(4, 5),
        GridPos::new(6, 5),
        GridPos::new(4, 6),
        GridPos::new(5, 6),
        GridPos::new(6, 6),
    ] {
        map.set_blocked(cell, true);
    }

    let rejected = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 5.5)),
    );
    assert_eq!(
        rejected.rejected_units,
        vec![(UnitId(1), RejectReason::Unreachable)]
    );
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned().unwrap(),
        order_before,
        "the prior CombatOrder must survive the rejection"
    );
    assert_eq!(
        world.get::<MoveOrder>(attacker).unwrap().goal,
        route_before.goal,
        "the prior MoveOrder must survive the rejection"
    );
    assert_eq!(
        world.get::<MoveOrder>(attacker).unwrap().waypoints,
        route_before.waypoints,
        "the prior route must survive the rejection"
    );
    assert!(
        world.get::<WorkerTask>(attacker) == worker_before.as_ref(),
        "a rejected AttackMove must not touch the unit's activity"
    );
    assert_eq!(
        world.get::<Health>(enemy).unwrap().current,
        unit_spec(UnitKind::Villager).max_health,
        "the rejected command must not touch the target"
    );

    // Reopen one wall cell: the same destination is reachable and must be
    // accepted, cancelling the prior activity and installing both the order
    // and the opening route.
    map.set_blocked(GridPos::new(5, 4), false);
    let accepted = issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 5.5)),
    );
    assert_eq!(accepted.accepted_units, vec![UnitId(1)]);
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 5),
            target: None,
            last_target_cell: None,
        }),
        "a reachable destination is accepted after the reorder"
    );
    assert_eq!(
        world.get::<MoveOrder>(attacker).unwrap().goal,
        GridPos::new(20, 5)
    );
    assert_eq!(
        world.get::<WorkerTask>(attacker),
        Some(&WorkerTask::Idle),
        "acceptance cancels the prior activity"
    );
}

#[test]
fn attack_move_targets_the_nearest_enemy_within_radius() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(7.5, 12.5),
        UnitKind::Villager,
    );
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(2),
        Vec2::new(12.5, 12.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 12),
            target: Some(CombatTarget::Unit(UnitId(2))),
            last_target_cell: Some(GridPos::new(7, 12)),
        }),
        "the nearest enemy in radius is acquired first"
    );
}

#[test]
fn attack_move_tie_breaks_by_lower_stable_id() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(3),
        TeamId(2),
        Vec2::new(8.5, 14.5),
        UnitKind::Villager,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 10.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    match world.get::<CombatOrder>(attacker) {
        Some(CombatOrder::AttackMove {
            target: Some(CombatTarget::Unit(id)),
            ..
        }) => assert_eq!(
            *id,
            UnitId(2),
            "equidistant ties resolve to the lower stable id"
        ),
        other => panic!("expected an acquired unit target, got {other:?}"),
    }
}

#[test]
fn attack_move_ignores_enemies_beyond_the_radius() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(13.5, 12.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(events(&world).is_empty());
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 12),
            target: None,
            last_target_cell: None,
        }),
        "an enemy beyond ATTACK_MOVE_RADIUS is never acquired"
    );
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("resumes destination");
    assert_eq!(order.goal, GridPos::new(20, 12));
}

#[test]
fn attack_move_acquires_buildings() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(16.5, 48.5),
        UnitKind::Spearman,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 52.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // Already inside attack range of the footprint: strike immediately, on
    // the footprint's closest point — never its center.
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].target, CombatTarget::Building(BuildingId(1)));
    assert_eq!(recorded[0].position, Vec2::new(16.0, 48.5));
}

#[test]
fn attack_move_diverts_to_an_acquired_stationary_unit() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(7.5, 12.5),
        UnitKind::Villager,
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // The destination route is replaced by pursuit the moment the target is
    // acquired — even though the target never moves. The occupied target
    // cell resolves to its nearest free ring slot.
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("pursues the acquired target");
    assert_eq!(
        order.goal,
        GridPos::new(6, 11),
        "diverts from the destination route to engage the stationary enemy"
    );
}

#[test]
fn attack_move_diverts_to_an_acquired_building() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 48.5),
        UnitKind::Spearman,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 52.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    let order = world
        .get::<MoveOrder>(attacker)
        .expect("pursues the acquired building");
    assert_eq!(
        order.goal,
        GridPos::new(11, 48),
        "diverts from the destination route to the nearest perimeter slot"
    );

    // The diverted pursuit leg is kept while it is active: a static building
    // never triggers a replan mid-route.
    let paths = map.path_call_count();
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(
        map.path_call_count(),
        paths,
        "the diverted pursuit leg is reused, not replanned"
    );
    assert_eq!(
        world.get::<MoveOrder>(attacker).unwrap().goal,
        GridPos::new(11, 48)
    );
}

fn run_symmetric_duel(left: UnitKind, right: UnitKind) -> Option<UnitKind> {
    let mut world = World::new();
    let mut map = open_map();
    let left_position = Vec2::new(7.5, 16.5);
    let right_position = Vec2::new(14.5, 16.5);
    spawn_combatant(&mut world, UnitId(1), TeamId(1), left_position, left);
    spawn_combatant(&mut world, UnitId(2), TeamId(2), right_position, right);
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_move_command(TeamId(1), &[UnitId(1)], right_position)
        )
        .rejected_units
        .is_empty()
    );
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_move_command(TeamId(2), &[UnitId(2)], left_position)
        )
        .rejected_units
        .is_empty()
    );

    for _ in 0..2_000 {
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        let left_alive = world.resource::<UnitIndex>().entity(UnitId(1)).is_some();
        let right_alive = world.resource::<UnitIndex>().entity(UnitId(2)).is_some();
        match (left_alive, right_alive) {
            (true, false) => return Some(left),
            (false, true) => return Some(right),
            _ => {}
        }
    }
    None
}

#[test]
fn spearman_survives_the_symmetric_cavalry_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Spearman, UnitKind::Cavalry),
        Some(UnitKind::Spearman)
    );
}

#[test]
fn archer_survives_the_symmetric_spearman_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Archer, UnitKind::Spearman),
        Some(UnitKind::Archer)
    );
}

#[test]
fn cavalry_survives_the_symmetric_archer_duel() {
    assert_eq!(
        run_symmetric_duel(UnitKind::Cavalry, UnitKind::Archer),
        Some(UnitKind::Cavalry)
    );
}

#[test]
fn dead_later_attacker_is_skipped_in_the_same_combat_step() {
    let mut world = World::new();
    let mut map = open_map();
    let left = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let right = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Spearman,
    );
    world.get_mut::<Health>(left).unwrap().current = 10;
    world.get_mut::<Health>(right).unwrap().current = 10;
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2)))
        )
        .accepted_units
            == vec![UnitId(1)]
    );
    assert!(
        issue(
            &mut world,
            &mut map,
            attack_command(TeamId(2), &[UnitId(2)], CombatTarget::Unit(UnitId(1)))
        )
        .accepted_units
            == vec![UnitId(2)]
    );

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    // The lower stable id acts first and atomically destroys the later
    // attacker, which is then skipped instead of acting on a stale entity.
    assert!(world.resource::<UnitIndex>().entity(UnitId(2)).is_none());
    assert!(world.get::<Health>(right).is_none(), "destroyed atomically");
    let recorded = events(&world);
    assert_eq!(recorded.len(), 1, "the dead later attacker never acted");
    assert_eq!(recorded[0].attacker, UnitId(1));
    assert!(recorded[0].killed);
    assert_eq!(
        world.get::<Health>(left).unwrap().current,
        10,
        "the survivor took no damage this step"
    );

    // Next step the survivor's direct order ends on its dead target.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(world.get::<CombatOrder>(left).is_none());
}

#[test]
fn direct_attack_ends_when_the_target_dies() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    world.get_mut::<Health>(defender).unwrap().current = 10;

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        world.get::<CombatOrder>(attacker).is_none(),
        "a direct attack ends on its dead target"
    );
}

#[test]
fn attack_move_clears_the_dead_target_and_resumes_destination() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Archer,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    world.get_mut::<Health>(defender).unwrap().current = 8;

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 5.5)),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);

    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(
        world.get::<CombatOrder>(attacker).cloned(),
        Some(CombatOrder::AttackMove {
            destination: GridPos::new(20, 5),
            target: None,
            last_target_cell: None,
        }),
        "the dead target is cleared"
    );
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("resumes destination");
    assert_eq!(order.goal, GridPos::new(20, 5));
}

#[test]
fn attack_move_resumes_destination_when_the_pursued_target_dies_mid_route() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 12.5),
        UnitKind::Spearman,
    );
    let target = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(7.5, 12.5),
        UnitKind::Villager,
    );
    world.get_mut::<Health>(target).unwrap().current = 5;

    issue(
        &mut world,
        &mut map,
        attack_move_command(TeamId(1), &[UnitId(1)], Vec2::new(20.5, 12.5)),
    );
    // Acquire the villager; its move then triggers a pursuit replan, so a
    // real pursuit leg (beside the villager, not the destination) is active.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    world
        .entity_mut(target)
        .insert(SimPosition::new(Vec2::new(7.5, 13.5)));
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(
        world.get::<MoveOrder>(attacker).unwrap().goal,
        GridPos::new(6, 12),
        "a pursuit leg toward the villager is active"
    );

    // The pursuit leg ended at range (it must not outlive engagement), so
    // the killing blow lands with no stale leg present at all.
    world
        .entity_mut(attacker)
        .insert(SimPosition::new(Vec2::new(6.5, 13.5)));
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(events(&world)[0].killed);
    assert!(
        world.get::<MoveOrder>(attacker).is_none(),
        "the pursuit leg ended when range was reached"
    );

    // The dead target frees the unit immediately — the stale pursuit leg is
    // replaced instead of being waited out.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    let order = world
        .get::<MoveOrder>(attacker)
        .expect("resumes the destination");
    assert_eq!(order.goal, GridPos::new(20, 12));
}

#[test]
fn destroy_unit_releases_worker_state_and_index_entry() {
    let mut world = World::new();
    let mut map = open_map();
    spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(5.5, 5.5),
        UnitKind::Spearman,
    );
    let worker = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(6.5, 5.5),
        UnitKind::Villager,
    );
    let source = crate::economy::spawn_resource_source(
        &mut world,
        &mut map,
        ResourceId(1),
        ResourceKind::Wood,
        GridPos::new(9, 5),
        100,
    );
    world
        .get_mut::<ResourceSource>(source)
        .unwrap()
        .assigned_worker = Some(UnitId(2));
    world.entity_mut(worker).insert((
        WorkerTask::Gathering {
            source: ResourceId(1),
        },
        Carry::Holding {
            kind: crate::catalog::ResourceKind::Wood,
            amount: std::num::NonZeroU32::new(5).unwrap(),
        },
    ));
    world.get_mut::<Health>(worker).unwrap().current = 10;

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    assert!(events(&world)[0].killed);
    assert!(
        world.resource::<UnitIndex>().entity(UnitId(2)).is_none(),
        "the dead worker leaves the stable index"
    );
    assert!(
        world.get::<Health>(worker).is_none(),
        "the entity is despawned"
    );
    assert_eq!(
        world.get::<ResourceSource>(source).unwrap().assigned_worker,
        None,
        "the source reservation dies with the worker"
    );
}

#[test]
fn unit_pursuit_refreshes_only_when_the_target_moves_or_the_route_ends() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 5.5),
        UnitKind::Spearman,
    );
    let target = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(8.5, 5.5),
        UnitKind::Spearman,
    );

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    let initial_paths = map.path_call_count();
    assert!(initial_paths > 0, "the first pursuit paths once");
    let goal = world.get::<MoveOrder>(attacker).unwrap().goal;

    // Same target cell and an active route: pursuit is reused, no new A*.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(map.path_call_count(), initial_paths);
    assert_eq!(world.get::<MoveOrder>(attacker).unwrap().goal, goal);

    // A target cell change triggers a replan.
    world
        .entity_mut(target)
        .insert(SimPosition::new(Vec2::new(8.5, 9.5)));
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        map.path_call_count() > initial_paths,
        "a moved target replans pursuit"
    );

    // A route that ended is reassigned even against a stationary target.
    let paths_before_end = map.path_call_count();
    world.entity_mut(attacker).remove::<MoveOrder>();
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        map.path_call_count() > paths_before_end,
        "an ended pursuit route is reassigned"
    );
}

#[test]
fn building_pursuit_repaths_only_when_the_route_ends_and_picks_the_nearest_perimeter() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(8.5, 48.5),
        UnitKind::Spearman,
    );
    spawn_building(
        &mut world,
        &mut map,
        BuildingId(1),
        TeamId(2),
        BuildingKind::TownCenter,
        GridPos::new(12, 46),
    );

    issue(
        &mut world,
        &mut map,
        attack_command(
            TeamId(1),
            &[UnitId(1)],
            CombatTarget::Building(BuildingId(1)),
        ),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);

    let goal = world
        .get::<MoveOrder>(attacker)
        .expect("pursuit route")
        .goal;
    assert_eq!(
        goal,
        GridPos::new(11, 48),
        "the nearest walkable immediate-perimeter cell wins at the combat call site"
    );
    let paths = map.path_call_count();
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(
        map.path_call_count(),
        paths,
        "a static building never triggers a pursuit replan while the route is active"
    );

    world.entity_mut(attacker).remove::<MoveOrder>();
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(map.path_call_count() > paths, "a ended route is reassigned");
    assert_eq!(world.get::<MoveOrder>(attacker).unwrap().goal, goal);
}

#[test]
fn ranged_pursuit_drops_the_leg_and_stops_closing_at_range() {
    let mut world = World::new();
    let mut map = open_map();
    let attacker = spawn_combatant(
        &mut world,
        UnitId(1),
        TeamId(1),
        Vec2::new(4.5, 5.5),
        UnitKind::Archer,
    );
    let defender = spawn_combatant(
        &mut world,
        UnitId(2),
        TeamId(2),
        Vec2::new(16.5, 5.5),
        UnitKind::Spearman,
    );
    let range = unit_spec(UnitKind::Archer).combat.unwrap().attack_range;

    issue(
        &mut world,
        &mut map,
        attack_command(TeamId(1), &[UnitId(1)], CombatTarget::Unit(UnitId(2))),
    );
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        world.get::<MoveOrder>(attacker).is_some(),
        "an out-of-range archer pursues"
    );

    // March until combat first sees attack range: the pursuit leg must end
    // exactly there so the archer stops closing and fights from range.
    let mut settled = None;
    for _ in 0..200 {
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        let position = world.get::<SimPosition>(attacker).unwrap().current;
        if position.distance(world.get::<SimPosition>(defender).unwrap().current) <= range {
            settled = Some(position);
            break;
        }
    }
    let settled = settled.expect("the archer must reach attack range");
    // The first combat tick that sees the in-range position ends the leg.
    step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
    assert!(
        world.get::<MoveOrder>(attacker).is_none(),
        "the pursuit leg is dropped the moment range is reached"
    );
    let health_at_range = world.get::<Health>(defender).unwrap().current;
    assert!(
        health_at_range < unit_spec(UnitKind::Spearman).max_health,
        "the archer already fired on reaching range"
    );

    // Holding position: no closing into melee, and the next shot still
    // lands once the cooldown elapses (20 = 8 base + 12 counter vs
    // Spearman).
    let mut rearmed = false;
    for _ in 0..30 {
        step_combat(&mut world, &mut map, SIM_STEP_SECONDS);
        step_movement(&mut world, &map, SIM_STEP_SECONDS);
        assert_eq!(
            world.get::<SimPosition>(attacker).unwrap().current,
            settled,
            "the attacker must not close past its first in-range position"
        );
        assert!(world.get::<MoveOrder>(attacker).is_none());
        if !events(&world).is_empty() {
            rearmed = true;
            break;
        }
    }
    assert!(rearmed, "the settled archer keeps attacking on cooldown");
    assert_eq!(
        world.get::<Health>(defender).unwrap().current,
        health_at_range - 20
    );
}

#[test]
fn bounded_victory_journey_destroys_the_enemy_town_center_through_real_systems() {
    let (mut world, mut map) = journeys::live_match();

    journeys::run_army_journey(&mut world, &mut map, TeamId(1));

    // Combat-level journey facts: the winning blow is a recorded combat
    // event against team 2's authored Town Center, and the passive opponent
    // never scratched the army.
    let events = &world.resource::<CombatEvents>().0;
    assert!(
        events
            .iter()
            .any(|event| event.target == CombatTarget::Building(BuildingId(2)) && event.killed),
        "the Town Center's killing blow is a recorded combat event"
    );
    assert_eq!(
        journeys::units_of_kind(&world, TeamId(1), UnitKind::Spearman).len(),
        4,
        "the passive opponent never damages the army"
    );

    journeys::assert_result_freezes_and_locks(&mut world, &mut map, TeamId(1));
}
