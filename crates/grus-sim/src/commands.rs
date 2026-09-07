use std::collections::HashMap;

use bevy::math::Vec2;
use bevy::prelude::{Entity, Resource, World};

use crate::ids::{TeamId, UnitId};
use crate::map::GridMap;
use crate::movement::{SimPosition, Unit};

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

pub fn apply_command(_world: &mut World, _map: &GridMap, _command: UnitCommand) -> CommandOutcome {
    // RED phase: command validation and routing are implemented after the behavior tests fail.
    CommandOutcome::default()
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::*;
    use crate::map::GridPos;
    use crate::movement::MoveOrder;

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
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(destinations.len(), 4);
        assert!(destinations.iter().all(|cell| map.is_walkable(*cell)));
    }
}
