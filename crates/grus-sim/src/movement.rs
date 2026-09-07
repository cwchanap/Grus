use bevy::math::Vec2;
use bevy::prelude::{Component, World};

use crate::ids::{TeamId, UnitId};
use crate::map::GridMap;

pub const SIM_STEP_SECONDS: f32 = 0.05;

#[derive(Clone, Copy, Component, Debug)]
pub struct Unit {
    pub id: UnitId,
    pub team: TeamId,
    pub speed: f32,
}

#[derive(Clone, Copy, Component, Debug)]
pub struct SimPosition {
    pub previous: Vec2,
    pub current: Vec2,
}

impl SimPosition {
    pub const fn new(position: Vec2) -> Self {
        Self {
            previous: position,
            current: position,
        }
    }
}

#[derive(Clone, Component, Debug)]
pub struct MoveOrder {
    pub waypoints: Vec<Vec2>,
    pub next: usize,
}

pub fn step_movement(_world: &mut World, _map: &GridMap, _delta_seconds: f32) {
    // RED phase: movement behavior is implemented after these tests fail.
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::*;
    use crate::commands::{UnitCommand, UnitCommandKind, apply_command, spawn_unit};
    use crate::fixture::MapFixture;
    use crate::map::GridPos;

    fn assert_vec2_near(actual: Vec2, expected: Vec2) {
        let error = actual.distance(expected);
        assert!(
            error < 0.0001,
            "expected {expected:?}, got {actual:?}, error {error}"
        );
    }

    #[test]
    fn fixed_step_moves_by_speed_times_delta() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let entity = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            2.0,
        );
        world.entity_mut(entity).insert(MoveOrder {
            waypoints: vec![Vec2::new(8.5, 1.5)],
            next: 0,
        });

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        let position = world.get::<SimPosition>(entity).unwrap();
        assert_vec2_near(position.previous, Vec2::new(1.5, 1.5));
        assert_vec2_near(position.current, Vec2::new(1.6, 1.5));
    }

    #[test]
    fn route_finishes_and_removes_move_order() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let entity = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            20.0,
        );
        world.entity_mut(entity).insert(MoveOrder {
            waypoints: vec![Vec2::new(2.5, 1.5)],
            next: 0,
        });

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        assert_vec2_near(
            world.get::<SimPosition>(entity).unwrap().current,
            Vec2::new(2.5, 1.5),
        );
        assert!(world.get::<MoveOrder>(entity).is_none());
    }

    #[test]
    fn units_do_not_enter_static_blocked_cells() {
        let mut world = World::new();
        let mut map = GridMap::new(16, 16);
        map.set_blocked_rect(GridPos::new(2, 1), GridPos::new(2, 1));
        let entity = spawn_unit(
            &mut world,
            UnitId(3),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            20.0,
        );
        world.entity_mut(entity).insert((
            SimPosition {
                previous: Vec2::ZERO,
                current: Vec2::new(1.5, 1.5),
            },
            MoveOrder {
                waypoints: vec![Vec2::new(2.5, 1.5)],
                next: 0,
            },
        ));

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        let position = world.get::<SimPosition>(entity).unwrap();
        assert_vec2_near(position.previous, Vec2::new(1.5, 1.5));
        assert_vec2_near(position.current, Vec2::new(1.5, 1.5));
        assert!(world.get::<MoveOrder>(entity).is_some());
    }

    #[test]
    fn neighboring_units_separate_instead_of_collapsing() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let first = spawn_unit(
            &mut world,
            UnitId(4),
            TeamId(1),
            Vec2::new(5.5, 5.5),
            2.0,
        );
        let second = spawn_unit(
            &mut world,
            UnitId(5),
            TeamId(1),
            Vec2::new(5.5, 5.5),
            2.0,
        );
        for entity in [first, second] {
            world.entity_mut(entity).insert(MoveOrder {
                waypoints: vec![Vec2::new(10.5, 5.5)],
                next: 0,
            });
        }

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        let first_position = world.get::<SimPosition>(first).unwrap().current;
        let second_position = world.get::<SimPosition>(second).unwrap().current;
        assert!(
            first_position.distance(second_position) > 0.01,
            "units collapsed at {first_position:?}"
        );
    }

    #[test]
    fn hundred_unit_group_finishes_representative_battlefield_route() {
        let fixture = MapFixture::battlefield();
        let mut world = World::new();
        let mut ids = Vec::new();

        for row in 0..10 {
            for column in 0..10 {
                let id = UnitId((row * 10 + column + 1) as u32);
                ids.push(id);
                spawn_unit(
                    &mut world,
                    id,
                    TeamId(1),
                    Vec2::new(10.5 + column as f32, 43.5 + row as f32),
                    12.0,
                );
            }
        }

        let outcome = apply_command(
            &mut world,
            &fixture.map,
            UnitCommand {
                issuer: TeamId(1),
                units: ids,
                kind: UnitCommandKind::Move {
                    target: fixture.right_spawn,
                },
            },
        );
        assert_eq!(outcome.accepted.len(), 100);
        assert!(outcome.rejected.is_empty());

        for _ in 0..600 {
            step_movement(&mut world, &fixture.map, SIM_STEP_SECONDS);
        }

        let mut moving_query = world.query::<&MoveOrder>();
        assert_eq!(moving_query.iter(&world).count(), 0);

        let mut position_query = world.query::<&SimPosition>();
        assert!(position_query
            .iter(&world)
            .all(|position| fixture.map.is_walkable(fixture.map.world_to_cell(position.current))));
    }
}
