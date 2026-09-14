use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Entity, Resource, World};

use crate::buildings::{apply_place_building, apply_resume_construction};
use crate::catalog::{BuildingKind, UnitKind};
use crate::economy::{apply_gather, cancel_worker_activity};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{apply_enqueue_age_up, apply_enqueue_unit, apply_set_rally};

#[derive(Clone, Debug)]
pub enum UnitCommandKind {
    Move { target: Vec2 },
    Stop,
}

#[derive(Clone, Debug)]
pub struct UnitCommand {
    pub issuer: TeamId,
    pub units: Vec<UnitId>,
    pub kind: UnitCommandKind,
}

/// Top-level player command. Grows only when its owning behavior lands.
#[derive(Clone, Debug)]
pub enum PlayerCommand {
    Units(UnitCommand),
    PlaceBuilding {
        issuer: TeamId,
        builder: UnitId,
        kind: BuildingKind,
        anchor: GridPos,
    },
    ResumeConstruction {
        issuer: TeamId,
        builder: UnitId,
        building: BuildingId,
    },
    Gather {
        issuer: TeamId,
        workers: Vec<UnitId>,
        source: ResourceId,
    },
    EnqueueUnit {
        issuer: TeamId,
        building: BuildingId,
        kind: UnitKind,
    },
    EnqueueAgeUp {
        issuer: TeamId,
        building: BuildingId,
    },
    SetRally {
        issuer: TeamId,
        building: BuildingId,
        target: GridPos,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectReason {
    UnknownUnit,
    NotOwned,
    NotVillager,
    Unreachable,
    Crowded,
    SourceMissing,
    BuildingMissing,
    Locked,
    InsufficientResources,
    OutOfBounds,
    Occupied,
    WrongProducer,
    FarmOccupied,
    PopulationFull,
    NoSpawnSpace,
}

/// Typed result of one player command. Batch unit commands report per-unit
/// outcomes; single building/production commands use `reject`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandResult {
    pub accepted_units: Vec<UnitId>,
    pub rejected_units: Vec<(UnitId, RejectReason)>,
    pub reject: Option<RejectReason>,
}

#[derive(Debug, Default, Resource)]
pub struct UnitIndex(HashMap<UnitId, Entity>);

impl UnitIndex {
    pub fn entity(&self, id: UnitId) -> Option<Entity> {
        self.0.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&UnitId, &Entity)> {
        self.0.iter()
    }
}

pub fn spawn_unit(
    world: &mut World,
    id: UnitId,
    team: TeamId,
    position: Vec2,
    kind: UnitKind,
    speed: f32,
) -> Entity {
    let entity = world
        .spawn((
            Unit {
                id,
                team,
                kind,
                speed,
            },
            SimPosition::new(position),
        ))
        .id();
    let mut index = world.get_resource_or_insert_with(UnitIndex::default);
    assert!(
        index.0.insert(id, entity).is_none(),
        "duplicate UnitId {id:?}"
    );
    entity
}

pub fn apply_player_command(
    world: &mut World,
    map: &mut GridMap,
    command: PlayerCommand,
) -> CommandResult {
    match command {
        PlayerCommand::Units(units) => apply_unit_command(world, map, units),
        PlayerCommand::PlaceBuilding {
            issuer,
            builder,
            kind,
            anchor,
        } => apply_place_building(world, map, issuer, builder, kind, anchor),
        PlayerCommand::ResumeConstruction {
            issuer,
            builder,
            building,
        } => apply_resume_construction(world, map, issuer, builder, building),
        PlayerCommand::Gather {
            issuer,
            workers,
            source,
        } => apply_gather(world, map, issuer, workers, source),
        PlayerCommand::EnqueueUnit {
            issuer,
            building,
            kind,
        } => apply_enqueue_unit(world, issuer, building, kind),
        PlayerCommand::EnqueueAgeUp { issuer, building } => {
            apply_enqueue_age_up(world, issuer, building)
        }
        PlayerCommand::SetRally {
            issuer,
            building,
            target,
        } => apply_set_rally(world, issuer, building, target),
    }
}

fn apply_unit_command(world: &mut World, map: &mut GridMap, command: UnitCommand) -> CommandResult {
    let UnitCommand {
        issuer,
        mut units,
        kind,
    } = command;
    units.sort_unstable();
    units.dedup();

    let mut outcome = CommandResult::default();

    match kind {
        UnitCommandKind::Stop => {
            for id in units {
                match owned_unit_entity(world, id, issuer) {
                    Ok(entity) => {
                        // Stop is unconditional activity cancellation.
                        cancel_worker_activity(world, entity);
                        outcome.accepted_units.push(id);
                    }
                    Err(reason) => outcome.rejected_units.push((id, reason)),
                }
            }
        }
        UnitCommandKind::Move { target } => {
            let target_cell = map.world_to_cell(target);
            let target_is_walkable = map.is_walkable(target_cell);
            let command_units: HashSet<UnitId> = units.iter().copied().collect();

            // Reference-counted reservations seeded from EVERY live unit's
            // current cell and existing MoveOrder goal, including units in this
            // command. A commanded unit that is later rejected keeps its
            // reservation so an accepted sibling cannot be assigned the cell it
            // is standing on or already moving to; a commanded unit's old
            // reservation is released only when an accepted replacement is
            // assigned (and restored if assignment fails).
            let mut used_slots: HashMap<GridPos, usize> = HashMap::new();
            // Candidate slot generation excludes only non-commanded
            // reservations, so a commanded unit's own current cell (e.g. a unit
            // already at the target) remains a selectable candidate that the
            // live `slot_taken` check protects until the unit is reassigned.
            let mut non_commanded_used: HashSet<GridPos> = HashSet::new();
            let all_entities: Vec<Entity> = world
                .get_resource::<UnitIndex>()
                .map(|index| index.iter().map(|(_, entity)| *entity).collect())
                .unwrap_or_default();
            for entity in &all_entities {
                let is_commanded = world
                    .get::<Unit>(*entity)
                    .map(|unit| command_units.contains(&unit.id))
                    .unwrap_or(false);
                if let Some(position) = world.get::<SimPosition>(*entity) {
                    let cell = map.world_to_cell(position.current);
                    reserve_slot(&mut used_slots, cell);
                    if !is_commanded {
                        non_commanded_used.insert(cell);
                    }
                }
                if let Some(order) = world.get::<MoveOrder>(*entity) {
                    reserve_slot(&mut used_slots, order.goal);
                    if !is_commanded {
                        non_commanded_used.insert(order.goal);
                    }
                }
            }
            // Generate destination slots after seeding reservations so the cap
            // counts only unreserved candidates. A fixed 4*units.len() list built
            // before reservations would exhaust a one-unit command whose first
            // few candidates are already occupied/targeted, rejecting the move
            // even when a neighboring cell is open and pathable.
            let slots = if target_is_walkable {
                destination_slots(map, target_cell, units.len(), &non_commanded_used)
            } else {
                Vec::new()
            };

            for id in units {
                let entity = match owned_unit_entity(world, id, issuer) {
                    Ok(entity) => entity,
                    Err(reason) => {
                        // Rejected before any reservation is touched: the unit
                        // keeps its current cell and existing goal reserved.
                        outcome.rejected_units.push((id, reason));
                        continue;
                    }
                };

                if !target_is_walkable {
                    outcome.rejected_units.push((id, RejectReason::Unreachable));
                    continue;
                }

                let Some(position) = world.get::<SimPosition>(entity).copied() else {
                    outcome.rejected_units.push((id, RejectReason::UnknownUnit));
                    continue;
                };
                let start = map.world_to_cell(position.current);
                let old_goal = world.get::<MoveOrder>(entity).map(|order| order.goal);

                // Tentatively release this unit's old reservations so a later
                // sibling may reuse the cell it is leaving. If no replacement
                // slot is accepted below, restore them so a rejected unit keeps
                // its place reserved against the rest of the command.
                release_slot(&mut used_slots, start);
                if let Some(goal) = old_goal {
                    release_slot(&mut used_slots, goal);
                }

                let route = slots.iter().copied().find_map(|slot| {
                    if slot_taken(&used_slots, &slot) {
                        return None;
                    }
                    map.find_path(start, slot).map(|path| (slot, path))
                });

                let Some((slot, path)) = route else {
                    reserve_slot(&mut used_slots, start);
                    if let Some(goal) = old_goal {
                        reserve_slot(&mut used_slots, goal);
                    }
                    outcome.rejected_units.push((id, RejectReason::Unreachable));
                    continue;
                };

                reserve_slot(&mut used_slots, slot);
                let waypoints = path
                    .into_iter()
                    .skip(1)
                    .map(|cell| map.cell_center(cell))
                    .collect::<Vec<_>>();

                // Accepted replacement: cancel the worker's previous task
                // (which also drops its old order) before installing the new
                // route. Rejected units above never reach this.
                cancel_worker_activity(world, entity);
                if !waypoints.is_empty() {
                    world.entity_mut(entity).insert(MoveOrder {
                        waypoints,
                        next: 0,
                        goal: slot,
                        map_revision: map.revision(),
                        last_failed_replan: None,
                    });
                }
                outcome.accepted_units.push(id);
            }
        }
    }

    outcome
}

pub(crate) fn owned_unit_entity(
    world: &World,
    id: UnitId,
    issuer: TeamId,
) -> Result<Entity, RejectReason> {
    let entity = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(id))
        .ok_or(RejectReason::UnknownUnit)?;
    let unit = world.get::<Unit>(entity).ok_or(RejectReason::UnknownUnit)?;

    if unit.team != issuer {
        return Err(RejectReason::NotOwned);
    }

    Ok(entity)
}

/// Assigns one unit a normal Move toward `target` using the Move command's
/// destination generation and reservation seeding, so the assignment cannot
/// claim a cell another live unit stands on or already has a `MoveOrder`
/// goal for. Returns the assigned goal cell, or `None` when no reachable
/// free slot exists (the caller leaves the unit where it is). Used by rally
/// routing; the batch Move command keeps its own per-unit restore logic.
pub(crate) fn assign_move_toward(
    world: &mut World,
    map: &GridMap,
    entity: Entity,
    start: GridPos,
    target: GridPos,
) -> Option<GridPos> {
    if !map.is_walkable(target) {
        return None;
    }
    let mut used_slots: HashMap<GridPos, usize> = HashMap::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for other in entities {
        if other == entity {
            continue;
        }
        if let Some(position) = world.get::<SimPosition>(other) {
            reserve_slot(&mut used_slots, map.world_to_cell(position.current));
        }
        if let Some(order) = world.get::<MoveOrder>(other) {
            reserve_slot(&mut used_slots, order.goal);
        }
    }
    let reserved: HashSet<GridPos> = used_slots.keys().copied().collect();
    destination_slots(map, target, 1, &reserved)
        .into_iter()
        .find_map(|slot| {
            if used_slots.contains_key(&slot) {
                return None;
            }
            map.find_path(start, slot).map(|path| {
                let waypoints: Vec<Vec2> = path
                    .into_iter()
                    .skip(1)
                    .map(|cell| map.cell_center(cell))
                    .collect();
                if !waypoints.is_empty() {
                    world.entity_mut(entity).insert(MoveOrder {
                        waypoints,
                        next: 0,
                        goal: slot,
                        map_revision: map.revision(),
                        last_failed_replan: None,
                    });
                }
                slot
            })
        })
}

/// Immediate-perimeter candidate generator for gather/build/drop-off tasking.
/// Returns at most `count` walkable, unreserved cells from the target
/// footprint's immediate perimeter and never scans a wider ring. Move keeps
/// its own `destination_slots` ring generation.
pub(crate) fn approach_slots(
    map: &GridMap,
    footprint: Footprint,
    used: &HashSet<GridPos>,
    count: usize,
) -> Vec<GridPos> {
    footprint
        .perimeter_cells()
        .into_iter()
        .filter(|cell| map.is_walkable(*cell) && !used.contains(cell))
        .take(count)
        .collect()
}

/// Reference-counted reservation for a destination cell. A cell shared by
/// several units (e.g. two units with the same existing goal) stays reserved
/// until every one of them releases it.
pub(crate) fn reserve_slot(used: &mut HashMap<GridPos, usize>, cell: GridPos) {
    *used.entry(cell).or_default() += 1;
}

/// Release one reference to a reserved cell, removing it entirely once the
/// last unit that claimed it has let go.
pub(crate) fn release_slot(used: &mut HashMap<GridPos, usize>, cell: GridPos) {
    if let Some(count) = used.get_mut(&cell) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            used.remove(&cell);
        }
    }
}

fn slot_taken(used: &HashMap<GridPos, usize>, cell: &GridPos) -> bool {
    used.contains_key(cell)
}

fn destination_slots(
    map: &GridMap,
    target: GridPos,
    unit_count: usize,
    used_slots: &HashSet<GridPos>,
) -> Vec<GridPos> {
    let desired = unit_count.max(1).saturating_mul(4);
    let mut slots = Vec::with_capacity(desired);
    if !used_slots.contains(&target) {
        slots.push(target);
    }

    let max_radius = map.width().max(map.height());
    for radius in 1..=max_radius {
        let min_x = target.x - radius;
        let max_x = target.x + radius;
        let min_y = target.y - radius;
        let max_y = target.y + radius;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if x != min_x && x != max_x && y != min_y && y != max_y {
                    continue;
                }
                let cell = GridPos::new(x, y);
                if map.is_walkable(cell) && !used_slots.contains(&cell) {
                    slots.push(cell);
                    if slots.len() >= desired {
                        return slots;
                    }
                }
            }
        }
    }

    slots
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::*;
    use crate::movement::{SIM_STEP_SECONDS, step_movement};

    fn open_map() -> GridMap {
        GridMap::new(24, 24)
    }

    fn move_command(issuer: TeamId, units: Vec<UnitId>, target: Vec2) -> PlayerCommand {
        PlayerCommand::Units(UnitCommand {
            issuer,
            units,
            kind: UnitCommandKind::Move { target },
        })
    }

    fn stop_command(issuer: TeamId, units: Vec<UnitId>) -> PlayerCommand {
        PlayerCommand::Units(UnitCommand {
            issuer,
            units,
            kind: UnitCommandKind::Stop,
        })
    }

    fn test_order(map: &GridMap, waypoint: Vec2) -> MoveOrder {
        MoveOrder {
            waypoints: vec![waypoint],
            next: 0,
            goal: map.world_to_cell(waypoint),
            map_revision: map.revision(),
            last_failed_replan: None,
        }
    }

    #[test]
    fn enemy_units_are_rejected_without_mutating_their_order() {
        let mut world = World::new();
        let mut map = open_map();
        let enemy = spawn_unit(
            &mut world,
            UnitId(9),
            TeamId(2),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(enemy)
            .insert(test_order(&map, Vec2::new(3.5, 2.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(9)], Vec2::new(18.5, 18.5)),
        );

        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(9), RejectReason::NotOwned)]
        );
        assert_eq!(
            world.get::<MoveOrder>(enemy).unwrap().waypoints,
            vec![Vec2::new(3.5, 2.5)]
        );
    }

    #[test]
    fn replacement_move_discards_the_previous_route() {
        let mut world = World::new();
        let mut map = open_map();
        let unit = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(3.5, 1.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(19.5, 19.5)),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        let order = world.get::<MoveOrder>(unit).expect("replacement order");
        assert_eq!(order.waypoints.last().copied(), Some(Vec2::new(19.5, 19.5)));
        assert_ne!(order.waypoints, vec![Vec2::new(3.5, 1.5)]);
    }

    #[test]
    fn stop_removes_an_active_move_order() {
        let mut world = World::new();
        let mut map = open_map();
        let unit = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(8.5, 1.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            stop_command(TeamId(1), vec![UnitId(2)]),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(2)]);
        assert!(world.get::<MoveOrder>(unit).is_none());
    }

    #[test]
    fn unreachable_units_return_terminal_rejection_without_an_order() {
        let mut world = World::new();
        let mut map = open_map();
        map.set_blocked_rect(GridPos::new(10, 10), GridPos::new(12, 12));
        let unit = spawn_unit(
            &mut world,
            UnitId(3),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(3)], Vec2::new(11.5, 11.5)),
        );

        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(3), RejectReason::Unreachable)]
        );
        assert!(world.get::<MoveOrder>(unit).is_none());
    }

    #[test]
    fn group_targets_distinct_walkable_destination_slots() {
        let mut world = World::new();
        let mut map = open_map();
        for id in 1..=4 {
            spawn_unit(
                &mut world,
                UnitId(id),
                TeamId(1),
                Vec2::new(2.5, id as f32 + 1.5),
                UnitKind::Villager,
                6.0,
            );
        }

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(
                TeamId(1),
                vec![UnitId(4), UnitId(2), UnitId(1), UnitId(3)],
                Vec2::new(18.5, 18.5),
            ),
        );

        assert_eq!(
            outcome.accepted_units,
            vec![UnitId(1), UnitId(2), UnitId(3), UnitId(4)]
        );
        let index = world.resource::<UnitIndex>();
        let entities = (1..=4)
            .map(|id| index.entity(UnitId(id)).unwrap())
            .collect::<Vec<_>>();
        let destinations = entities
            .into_iter()
            .map(|entity| {
                let point = world
                    .get::<MoveOrder>(entity)
                    .unwrap()
                    .waypoints
                    .last()
                    .copied()
                    .unwrap();
                map.world_to_cell(point)
            })
            .collect::<HashSet<_>>();

        assert_eq!(destinations.len(), 4);
        assert!(destinations.iter().all(|cell| map.is_walkable(*cell)));
    }

    #[test]
    fn sequential_commands_do_not_collapse_onto_occupied_destination() {
        let mut world = World::new();
        let mut map = open_map();
        let first = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            20.0,
        );
        let second = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(2.5, 4.5),
            UnitKind::Villager,
            20.0,
        );

        // Command 1: move the first unit to the target.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        for _ in 0..200 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
        }
        assert!(
            world.get::<MoveOrder>(first).is_none(),
            "first unit should have arrived"
        );

        // Command 2: move the second unit to the SAME target.
        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(2)], Vec2::new(18.5, 18.5)),
        );
        assert_eq!(outcome.accepted_units, vec![UnitId(2)]);
        for _ in 0..200 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
        }
        assert!(
            world.get::<MoveOrder>(second).is_none(),
            "second unit should have arrived"
        );

        let first_pos = world.get::<SimPosition>(first).unwrap().current;
        let second_pos = world.get::<SimPosition>(second).unwrap().current;
        assert!(
            first_pos.distance(second_pos) >= 0.9,
            "units collapsed at first={first_pos:?} second={second_pos:?}"
        );
    }

    #[test]
    fn destination_slots_keep_scanning_past_reserved_candidates() {
        let mut world = World::new();
        let mut map = open_map();
        // Reserve the four closest destination candidates (the target cell and
        // the first three ring-1 cells in slot order) with units that are not part
        // of the command, so a fixed 4*units.len() slot list would be exhausted
        // and the one-unit command would be rejected as unreachable.
        let target_cell = map.world_to_cell(Vec2::new(18.5, 18.5));
        let reserved = [
            target_cell,
            GridPos::new(17, 17),
            GridPos::new(18, 17),
            GridPos::new(19, 17),
        ];
        for (i, cell) in reserved.iter().enumerate() {
            spawn_unit(
                &mut world,
                UnitId(100 + i as u32),
                TeamId(2),
                map.cell_center(*cell),
                UnitKind::Villager,
                6.0,
            );
        }
        let unit = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        let order = world.get::<MoveOrder>(unit).expect("order accepted");
        assert!(
            !reserved.contains(&order.goal),
            "unit settled on a reserved candidate {:?}",
            order.goal
        );
        assert!(map.is_walkable(order.goal));
    }

    #[test]
    fn unreachable_move_preserves_an_active_order() {
        let mut world = World::new();
        let mut map = open_map();
        map.set_blocked_rect(GridPos::new(10, 10), GridPos::new(12, 12));
        let unit = spawn_unit(
            &mut world,
            UnitId(4),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(3.5, 2.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(4)], Vec2::new(11.5, 11.5)),
        );

        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(4), RejectReason::Unreachable)]
        );
        assert_eq!(
            world.get::<MoveOrder>(unit).unwrap().waypoints,
            vec![Vec2::new(3.5, 2.5)]
        );
    }

    #[test]
    fn unreachable_route_preserves_an_active_order() {
        let mut world = World::new();
        let mut map = open_map();
        // Wall off the unit from the target region so the target is walkable
        // but no destination slot is reachable.
        map.set_blocked_rect(GridPos::new(0, 3), GridPos::new(23, 3));
        let unit = spawn_unit(
            &mut world,
            UnitId(5),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(1.5, 2.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(5)], Vec2::new(5.5, 5.5)),
        );

        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(5), RejectReason::Unreachable)]
        );
        assert_eq!(
            world.get::<MoveOrder>(unit).unwrap().waypoints,
            vec![Vec2::new(1.5, 2.5)]
        );
    }

    #[test]
    fn rejected_unit_keeps_its_destination_reserved_from_accepted_sibling() {
        let mut world = World::new();
        let mut map = open_map();
        let target = Vec2::new(18.5, 18.5);
        let target_cell = map.world_to_cell(target);

        // Unit A is on the right side and can reach the target.
        let a = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(18.5, 2.5),
            UnitKind::Villager,
            12.0,
        );
        // Unit B is walled into a pocket on the left so it is Unreachable, but
        // it already holds a MoveOrder to the target cell. B must keep that goal
        // reserved so the accepted A is not sent to the same cell.
        for cell in [
            GridPos::new(3, 2),
            GridPos::new(2, 3),
            GridPos::new(1, 2),
            GridPos::new(2, 1),
        ] {
            map.set_blocked(cell, true);
        }
        let b = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            12.0,
        );
        world.entity_mut(b).insert(MoveOrder {
            waypoints: vec![map.cell_center(target_cell)],
            next: 0,
            goal: target_cell,
            map_revision: map.revision(),
            last_failed_replan: None,
        });

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1), UnitId(2)], target),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(2), RejectReason::Unreachable)]
        );

        // A must not have been assigned the target cell B is already moving to.
        let a_goal = world.get::<MoveOrder>(a).expect("A accepted an order").goal;
        assert_ne!(
            a_goal, target_cell,
            "accepted unit took the rejected unit's goal"
        );

        // B keeps its existing order to the target cell unchanged.
        let b_order = world.get::<MoveOrder>(b).expect("B keeps its order");
        assert_eq!(b_order.goal, target_cell);
        assert_eq!(b_order.waypoints, vec![map.cell_center(target_cell)]);
    }

    #[test]
    fn approach_slots_stay_on_the_immediate_perimeter() {
        let mut map = GridMap::new(8, 8);
        let footprint = Footprint::new(GridPos::new(3, 3), 2, 2);
        for cell in footprint.cells() {
            map.set_blocked(cell, true);
        }

        let used = std::collections::HashSet::new();
        let slots = approach_slots(&map, footprint, &used, 8);

        assert!(!slots.is_empty());
        assert!(slots.iter().all(|slot| map.is_walkable(*slot)));
        assert!(
            slots
                .iter()
                .all(|slot| footprint.is_immediately_adjacent(*slot))
        );
        assert!(slots.iter().all(|slot| !footprint.cells().contains(slot)));
    }

    #[test]
    fn stop_rejects_foreign_units_and_preserves_their_orders() {
        let mut world = World::new();
        let mut map = open_map();
        let enemy = spawn_unit(
            &mut world,
            UnitId(9),
            TeamId(2),
            Vec2::new(2.5, 2.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(enemy)
            .insert(test_order(&map, Vec2::new(3.5, 2.5)));

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            stop_command(TeamId(1), vec![UnitId(9)]),
        );

        assert_eq!(outcome.accepted_units, Vec::<UnitId>::new());
        assert_eq!(
            outcome.rejected_units,
            vec![(UnitId(9), RejectReason::NotOwned)]
        );
        assert!(
            world.get::<MoveOrder>(enemy).is_some(),
            "a rejected Stop must not cancel the unit's activity"
        );
    }

    #[test]
    fn move_never_assigns_a_non_commanded_units_existing_goal() {
        let mut world = World::new();
        let mut map = open_map();
        let mover = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        let other = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(4.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        let claimed = GridPos::new(12, 12);
        world
            .entity_mut(other)
            .insert(test_order(&map, map.cell_center(claimed)));
        let claimed_center = map.cell_center(claimed);

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1)], claimed_center),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(1)]);
        let order = world.get::<MoveOrder>(mover).expect("replacement order");
        assert_ne!(
            order.goal, claimed,
            "the mover must not be sent onto a goal a non-commanded unit already claims"
        );
        assert_eq!(
            world.get::<MoveOrder>(other).unwrap().goal,
            claimed,
            "the uncommanded unit keeps its own route"
        );
    }

    #[test]
    fn group_move_never_targets_a_cell_another_commanded_unit_stands_on() {
        let mut world = World::new();
        let mut map = open_map();
        let a = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(10.5, 10.5),
            UnitKind::Villager,
            6.0,
        );
        let b = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(12.5, 10.5),
            UnitKind::Villager,
            6.0,
        );
        let b_cell = GridPos::new(12, 10);

        let outcome = apply_player_command(
            &mut world,
            &mut map,
            move_command(TeamId(1), vec![UnitId(1), UnitId(2)], Vec2::new(12.5, 10.5)),
        );

        assert_eq!(outcome.accepted_units, vec![UnitId(1), UnitId(2)]);
        assert!(outcome.rejected_units.is_empty());
        let goal_a = world
            .get::<MoveOrder>(a)
            .expect("the displaced unit moves")
            .goal;
        assert_ne!(
            goal_a, b_cell,
            "the mover is never sent onto a cell a commanded sibling still holds"
        );
        assert!(
            world.get::<MoveOrder>(b).is_none(),
            "the unit already standing on the target keeps its place"
        );
    }
}
