use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Entity, Resource, World};

use crate::ids::{TeamId, UnitId};
use crate::map::{GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandRejectReason {
    UnknownUnit,
    NotOwned,
    Unreachable,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandOutcome {
    pub accepted: Vec<UnitId>,
    pub rejected: Vec<(UnitId, CommandRejectReason)>,
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
    speed: f32,
) -> Entity {
    let entity = world
        .spawn((Unit { id, team, speed }, SimPosition::new(position)))
        .id();
    let mut index = world.get_resource_or_insert_with(UnitIndex::default);
    assert!(
        index.0.insert(id, entity).is_none(),
        "duplicate UnitId {id:?}"
    );
    entity
}

pub fn apply_command(world: &mut World, map: &GridMap, command: UnitCommand) -> CommandOutcome {
    let UnitCommand {
        issuer,
        mut units,
        kind,
    } = command;
    units.sort_unstable();
    units.dedup();

    let mut outcome = CommandOutcome::default();

    match kind {
        UnitCommandKind::Stop => {
            for id in units {
                match owned_entity(world, id, issuer) {
                    Ok(entity) => {
                        world.entity_mut(entity).remove::<MoveOrder>();
                        outcome.accepted.push(id);
                    }
                    Err(reason) => outcome.rejected.push((id, reason)),
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
                let entity = match owned_entity(world, id, issuer) {
                    Ok(entity) => entity,
                    Err(reason) => {
                        // Rejected before any reservation is touched: the unit
                        // keeps its current cell and existing goal reserved.
                        outcome.rejected.push((id, reason));
                        continue;
                    }
                };

                if !target_is_walkable {
                    outcome
                        .rejected
                        .push((id, CommandRejectReason::Unreachable));
                    continue;
                }

                let Some(position) = world.get::<SimPosition>(entity).copied() else {
                    outcome
                        .rejected
                        .push((id, CommandRejectReason::UnknownUnit));
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
                    outcome
                        .rejected
                        .push((id, CommandRejectReason::Unreachable));
                    continue;
                };

                reserve_slot(&mut used_slots, slot);
                let waypoints = path
                    .into_iter()
                    .skip(1)
                    .map(|cell| map.cell_center(cell))
                    .collect::<Vec<_>>();

                if waypoints.is_empty() {
                    world.entity_mut(entity).remove::<MoveOrder>();
                } else {
                    world.entity_mut(entity).insert(MoveOrder {
                        waypoints,
                        next: 0,
                        goal: slot,
                        map_revision: map.revision(),
                        last_failed_replan: None,
                    });
                }
                outcome.accepted.push(id);
            }
        }
    }

    outcome
}

fn owned_entity(world: &World, id: UnitId, issuer: TeamId) -> Result<Entity, CommandRejectReason> {
    let entity = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(id))
        .ok_or(CommandRejectReason::UnknownUnit)?;
    let unit = world
        .get::<Unit>(entity)
        .ok_or(CommandRejectReason::UnknownUnit)?;

    if unit.team != issuer {
        return Err(CommandRejectReason::NotOwned);
    }

    Ok(entity)
}

/// Reference-counted reservation for a destination cell. A cell shared by
/// several units (e.g. two units with the same existing goal) stays reserved
/// until every one of them releases it.
fn reserve_slot(used: &mut HashMap<GridPos, usize>, cell: GridPos) {
    *used.entry(cell).or_default() += 1;
}

/// Release one reference to a reserved cell, removing it entirely once the
/// last unit that claimed it has let go.
fn release_slot(used: &mut HashMap<GridPos, usize>, cell: GridPos) {
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

    fn move_command(issuer: TeamId, units: Vec<UnitId>, target: Vec2) -> UnitCommand {
        UnitCommand {
            issuer,
            units,
            kind: UnitCommandKind::Move { target },
        }
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
        let map = open_map();
        let enemy = spawn_unit(&mut world, UnitId(9), TeamId(2), Vec2::new(2.5, 2.5), 6.0);
        world
            .entity_mut(enemy)
            .insert(test_order(&map, Vec2::new(3.5, 2.5)));

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(9)], Vec2::new(18.5, 18.5)),
        );

        assert_eq!(
            outcome.rejected,
            vec![(UnitId(9), CommandRejectReason::NotOwned)]
        );
        assert_eq!(
            world.get::<MoveOrder>(enemy).unwrap().waypoints,
            vec![Vec2::new(3.5, 2.5)]
        );
    }

    #[test]
    fn replacement_move_discards_the_previous_route() {
        let mut world = World::new();
        let map = open_map();
        let unit = spawn_unit(&mut world, UnitId(1), TeamId(1), Vec2::new(1.5, 1.5), 6.0);
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(3.5, 1.5)));

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(19.5, 19.5)),
        );

        assert_eq!(outcome.accepted, vec![UnitId(1)]);
        let order = world.get::<MoveOrder>(unit).expect("replacement order");
        assert_eq!(order.waypoints.last().copied(), Some(Vec2::new(19.5, 19.5)));
        assert_ne!(order.waypoints, vec![Vec2::new(3.5, 1.5)]);
    }

    #[test]
    fn stop_removes_an_active_move_order() {
        let mut world = World::new();
        let map = open_map();
        let unit = spawn_unit(&mut world, UnitId(2), TeamId(1), Vec2::new(1.5, 1.5), 6.0);
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(8.5, 1.5)));

        let outcome = apply_command(
            &mut world,
            &map,
            UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(2)],
                kind: UnitCommandKind::Stop,
            },
        );

        assert_eq!(outcome.accepted, vec![UnitId(2)]);
        assert!(world.get::<MoveOrder>(unit).is_none());
    }

    #[test]
    fn unreachable_units_return_terminal_rejection_without_an_order() {
        let mut world = World::new();
        let mut map = open_map();
        map.set_blocked_rect(GridPos::new(10, 10), GridPos::new(12, 12));
        let unit = spawn_unit(&mut world, UnitId(3), TeamId(1), Vec2::new(2.5, 2.5), 6.0);

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(3)], Vec2::new(11.5, 11.5)),
        );

        assert_eq!(
            outcome.rejected,
            vec![(UnitId(3), CommandRejectReason::Unreachable)]
        );
        assert!(world.get::<MoveOrder>(unit).is_none());
    }

    #[test]
    fn group_targets_distinct_walkable_destination_slots() {
        let mut world = World::new();
        let map = open_map();
        for id in 1..=4 {
            spawn_unit(
                &mut world,
                UnitId(id),
                TeamId(1),
                Vec2::new(2.5, id as f32 + 1.5),
                6.0,
            );
        }

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(
                TeamId(1),
                vec![UnitId(4), UnitId(2), UnitId(1), UnitId(3)],
                Vec2::new(18.5, 18.5),
            ),
        );

        assert_eq!(
            outcome.accepted,
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
        let map = open_map();
        let first = spawn_unit(&mut world, UnitId(1), TeamId(1), Vec2::new(2.5, 2.5), 20.0);
        let second = spawn_unit(&mut world, UnitId(2), TeamId(1), Vec2::new(2.5, 4.5), 20.0);

        // Command 1: move the first unit to the target.
        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
        );
        assert_eq!(outcome.accepted, vec![UnitId(1)]);
        for _ in 0..200 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
        }
        assert!(
            world.get::<MoveOrder>(first).is_none(),
            "first unit should have arrived"
        );

        // Command 2: move the second unit to the SAME target.
        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(2)], Vec2::new(18.5, 18.5)),
        );
        assert_eq!(outcome.accepted, vec![UnitId(2)]);
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
        let map = open_map();
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
                6.0,
            );
        }
        let unit = spawn_unit(&mut world, UnitId(1), TeamId(1), Vec2::new(2.5, 2.5), 6.0);

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(1)], Vec2::new(18.5, 18.5)),
        );

        assert_eq!(outcome.accepted, vec![UnitId(1)]);
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
        let unit = spawn_unit(&mut world, UnitId(4), TeamId(1), Vec2::new(2.5, 2.5), 6.0);
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(3.5, 2.5)));

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(4)], Vec2::new(11.5, 11.5)),
        );

        assert_eq!(
            outcome.rejected,
            vec![(UnitId(4), CommandRejectReason::Unreachable)]
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
        let unit = spawn_unit(&mut world, UnitId(5), TeamId(1), Vec2::new(2.5, 2.5), 6.0);
        world
            .entity_mut(unit)
            .insert(test_order(&map, Vec2::new(1.5, 2.5)));

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(5)], Vec2::new(5.5, 5.5)),
        );

        assert_eq!(
            outcome.rejected,
            vec![(UnitId(5), CommandRejectReason::Unreachable)]
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
        let a = spawn_unit(&mut world, UnitId(1), TeamId(1), Vec2::new(18.5, 2.5), 12.0);
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
        let b = spawn_unit(&mut world, UnitId(2), TeamId(1), Vec2::new(2.5, 2.5), 12.0);
        world.entity_mut(b).insert(MoveOrder {
            waypoints: vec![map.cell_center(target_cell)],
            next: 0,
            goal: target_cell,
            map_revision: map.revision(),
            last_failed_replan: None,
        });

        let outcome = apply_command(
            &mut world,
            &map,
            move_command(TeamId(1), vec![UnitId(1), UnitId(2)], target),
        );

        assert_eq!(outcome.accepted, vec![UnitId(1)]);
        assert_eq!(
            outcome.rejected,
            vec![(UnitId(2), CommandRejectReason::Unreachable)]
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
}
