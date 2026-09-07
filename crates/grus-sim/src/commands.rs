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
            let slots = if target_is_walkable {
                destination_slots(map, target_cell, units.len())
            } else {
                Vec::new()
            };
            let mut used_slots = HashSet::new();

            for id in units {
                let entity = match owned_entity(world, id, issuer) {
                    Ok(entity) => entity,
                    Err(reason) => {
                        outcome.rejected.push((id, reason));
                        continue;
                    }
                };

                if !target_is_walkable {
                    world.entity_mut(entity).remove::<MoveOrder>();
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

                let route = slots.iter().copied().find_map(|slot| {
                    if used_slots.contains(&slot) {
                        return None;
                    }
                    map.find_path(start, slot).map(|path| (slot, path))
                });

                let Some((slot, path)) = route else {
                    world.entity_mut(entity).remove::<MoveOrder>();
                    outcome
                        .rejected
                        .push((id, CommandRejectReason::Unreachable));
                    continue;
                };

                used_slots.insert(slot);
                let waypoints = path
                    .into_iter()
                    .skip(1)
                    .map(|cell| map.cell_center(cell))
                    .collect::<Vec<_>>();

                if waypoints.is_empty() {
                    world.entity_mut(entity).remove::<MoveOrder>();
                } else {
                    world
                        .entity_mut(entity)
                        .insert(MoveOrder { waypoints, next: 0 });
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

fn destination_slots(map: &GridMap, target: GridPos, unit_count: usize) -> Vec<GridPos> {
    let desired = unit_count.max(1).saturating_mul(4);
    let mut slots = Vec::with_capacity(desired);
    slots.push(target);

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
                if map.is_walkable(cell) {
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

    #[test]
    fn enemy_units_are_rejected_without_mutating_their_order() {
        let mut world = World::new();
        let map = open_map();
        let enemy = spawn_unit(&mut world, UnitId(9), TeamId(2), Vec2::new(2.5, 2.5), 6.0);
        world.entity_mut(enemy).insert(MoveOrder {
            waypoints: vec![Vec2::new(3.5, 2.5)],
            next: 0,
        });

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
        world.entity_mut(unit).insert(MoveOrder {
            waypoints: vec![Vec2::new(3.5, 1.5)],
            next: 0,
        });

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
        world.entity_mut(unit).insert(MoveOrder {
            waypoints: vec![Vec2::new(8.5, 1.5)],
            next: 0,
        });

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
}
