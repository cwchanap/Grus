use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, World};

use crate::ids::{TeamId, UnitId};
use crate::map::{GridMap, GridPos};

pub const SIM_STEP_SECONDS: f32 = 0.05;
const WAYPOINT_EPSILON: f32 = 0.0001;
const SEPARATION_DISTANCE: f32 = 0.6;
const MAX_SEPARATION_STEP: f32 = 0.08;

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
    pub goal: GridPos,
    pub map_revision: u64,
}

#[derive(Clone, Debug)]
struct MovementSnapshot {
    entity: Entity,
    unit: Unit,
    position: SimPosition,
    order: Option<MoveOrder>,
}

pub fn step_movement(world: &mut World, map: &GridMap, delta_seconds: f32) {
    let snapshots = {
        let mut query = world.query::<(Entity, &Unit, &SimPosition, Option<&MoveOrder>)>();
        query
            .iter(world)
            .map(|(entity, unit, position, order)| MovementSnapshot {
                entity,
                unit: *unit,
                position: *position,
                order: order.cloned(),
            })
            .collect::<Vec<_>>()
    };

    for snapshot in &snapshots {
        let mut position = snapshot.position;
        position.previous = snapshot.position.current;
        let mut order = snapshot.order.clone();

        if let Some(active_order) = snapshot.order.as_ref() {
            if let Some(mut candidate_order) =
                refresh_route(active_order, snapshot.position.current, map)
            {
                let route_candidate = advance_along_route(
                    snapshot.position.current,
                    &mut candidate_order,
                    snapshot.unit.speed * delta_seconds.max(0.0),
                );

                if map.is_walkable(map.world_to_cell(route_candidate)) {
                    let separation = separation_for(snapshot, &snapshots);
                    let separated = route_candidate + separation;
                    position.current = if map.is_walkable(map.world_to_cell(separated)) {
                        separated
                    } else {
                        route_candidate
                    };

                    order = if candidate_order.next >= candidate_order.waypoints.len() {
                        None
                    } else {
                        Some(candidate_order)
                    };
                }
            } else {
                order = None;
            }
        }

        let mut entity = world.entity_mut(snapshot.entity);
        entity.insert(position);
        if let Some(order) = order {
            entity.insert(order);
        } else {
            entity.remove::<MoveOrder>();
        }
    }
}

fn refresh_route(order: &MoveOrder, current: Vec2, map: &GridMap) -> Option<MoveOrder> {
    if order.map_revision == map.revision() {
        return Some(order.clone());
    }

    let path = map.find_path(map.world_to_cell(current), order.goal)?;
    let waypoints = path
        .into_iter()
        .skip(1)
        .map(|cell| map.cell_center(cell))
        .collect::<Vec<_>>();
    if waypoints.is_empty() {
        return None;
    }

    Some(MoveOrder {
        waypoints,
        next: 0,
        goal: order.goal,
        map_revision: map.revision(),
    })
}

fn advance_along_route(mut current: Vec2, order: &mut MoveOrder, mut remaining: f32) -> Vec2 {
    while order.next < order.waypoints.len() {
        let target = order.waypoints[order.next];
        let offset = target - current;
        let distance = offset.length();

        if distance <= WAYPOINT_EPSILON {
            current = target;
            order.next += 1;
            continue;
        }

        if remaining + WAYPOINT_EPSILON >= distance {
            current = target;
            remaining = (remaining - distance).max(0.0);
            order.next += 1;
            if remaining <= WAYPOINT_EPSILON {
                break;
            }
        } else {
            current += offset / distance * remaining;
            break;
        }
    }

    current
}

fn separation_for(snapshot: &MovementSnapshot, all: &[MovementSnapshot]) -> Vec2 {
    let mut separation = Vec2::ZERO;

    for other in all {
        if other.entity == snapshot.entity {
            continue;
        }

        let offset = snapshot.position.current - other.position.current;
        let distance = offset.length();
        if distance >= SEPARATION_DISTANCE {
            continue;
        }

        let direction = if distance <= WAYPOINT_EPSILON {
            if snapshot.unit.id < other.unit.id {
                Vec2::NEG_X
            } else {
                Vec2::X
            }
        } else {
            offset / distance
        };
        let strength = 1.0 - (distance / SEPARATION_DISTANCE);
        separation += direction * strength;
    }

    let length = separation.length();
    if length > MAX_SEPARATION_STEP {
        separation / length * MAX_SEPARATION_STEP
    } else {
        separation
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;

    use super::*;
    use crate::commands::{UnitCommand, UnitCommandKind, apply_command, spawn_unit};
    use crate::fixture::MapFixture;

    fn assert_vec2_near(actual: Vec2, expected: Vec2) {
        let error = actual.distance(expected);
        assert!(
            error < 0.0001,
            "expected {expected:?}, got {actual:?}, error {error}"
        );
    }

    fn test_order(map: &GridMap, waypoint: Vec2) -> MoveOrder {
        MoveOrder {
            waypoints: vec![waypoint],
            next: 0,
            goal: map.world_to_cell(waypoint),
            map_revision: map.revision(),
        }
    }

    #[test]
    fn fixed_step_moves_by_speed_times_delta() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let entity = spawn_unit(&mut world, UnitId(1), TeamId(1), Vec2::new(1.5, 1.5), 2.0);
        world
            .entity_mut(entity)
            .insert(test_order(&map, Vec2::new(8.5, 1.5)));

        step_movement(&mut world, &map, SIM_STEP_SECONDS);

        let position = world.get::<SimPosition>(entity).unwrap();
        assert_vec2_near(position.previous, Vec2::new(1.5, 1.5));
        assert_vec2_near(position.current, Vec2::new(1.6, 1.5));
    }

    #[test]
    fn route_finishes_and_removes_move_order() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let entity = spawn_unit(&mut world, UnitId(2), TeamId(1), Vec2::new(1.5, 1.5), 20.0);
        world
            .entity_mut(entity)
            .insert(test_order(&map, Vec2::new(2.5, 1.5)));

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
        let entity = spawn_unit(&mut world, UnitId(3), TeamId(1), Vec2::new(1.5, 1.5), 20.0);
        world.entity_mut(entity).insert((
            SimPosition {
                previous: Vec2::ZERO,
                current: Vec2::new(1.5, 1.5),
            },
            test_order(&map, Vec2::new(2.5, 1.5)),
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
        let first = spawn_unit(&mut world, UnitId(4), TeamId(1), Vec2::new(5.5, 5.5), 2.0);
        let second = spawn_unit(&mut world, UnitId(5), TeamId(1), Vec2::new(5.5, 5.5), 2.0);
        for entity in [first, second] {
            world
                .entity_mut(entity)
                .insert(test_order(&map, Vec2::new(10.5, 5.5)));
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
        assert!(position_query.iter(&world).all(|position| {
            fixture
                .map
                .is_walkable(fixture.map.world_to_cell(position.current))
        }));
    }
}
