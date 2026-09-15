use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, World};

use crate::catalog::UnitKind;
use crate::commands::RejectReason;
use crate::economy::{WorkerTask, idle_worker_on_route_failure};
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
    pub kind: UnitKind,
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
    /// Revision at which a replan last failed to find a route to `goal`.
    /// While `map.revision()` matches this, the movement step skips
    /// re-running A* and waits for a map change instead of recomputing a
    /// path that is already known to be unreachable.
    pub last_failed_replan: Option<u64>,
}

#[derive(Clone, Debug)]
struct MovementSnapshot {
    entity: Entity,
    unit: Unit,
    position: SimPosition,
    order: Option<MoveOrder>,
    /// True when the unit holds a worker task that depends on its route
    /// (ToSource/ToDropoff/ToConstruction). Plain Move orders — even on
    /// villagers whose task is `Idle` — keep the HPA-470 preserve-and-wait
    /// policy instead.
    active_worker_task: bool,
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
                active_worker_task: world
                    .get::<WorkerTask>(entity)
                    .is_some_and(|task| !matches!(task, WorkerTask::Idle)),
            })
            .collect::<Vec<_>>()
    };

    for snapshot in &snapshots {
        let mut position = snapshot.position;
        position.previous = snapshot.position.current;
        let mut order = snapshot.order.clone();
        let mut worker_route_failed = false;

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
                } else if map.is_walkable(active_order.goal) {
                    // The cached route's next step is blocked, typically because
                    // separation nudged the unit off-route and the straight line
                    // to the next waypoint clips a blocked corner. Force a
                    // replan from the current cell; if a fresh route exists,
                    // adopt it and recover in one tick. If the goal is still
                    // unreachable from here, non-worker units preserve the
                    // existing order and wait for a map revision change rather
                    // than cancelling a still-valid order (workers idle through
                    // the shared cleanup instead). The failed revision is
                    // recorded so we do not re-run A* on every step while the
                    // map is unchanged.
                    if active_order.last_failed_replan != Some(map.revision()) {
                        if let Some(forced_order) =
                            compute_route(active_order.goal, snapshot.position.current, map)
                        {
                            order = Some(forced_order);
                        } else if snapshot.active_worker_task {
                            // The worker's required route is impossible: idle
                            // with typed feedback instead of waiting forever
                            // on a map change that may never come.
                            order = None;
                            worker_route_failed = true;
                        } else if let Some(preserved) = order.as_mut() {
                            preserved.last_failed_replan = Some(map.revision());
                        }
                    }
                } else if snapshot.active_worker_task {
                    // The goal cell itself is blocked: the required route can
                    // never complete.
                    order = None;
                    worker_route_failed = true;
                }
            } else {
                // A newer map revision triggered a replan that found no route
                // to the goal. Workers idle through the shared cleanup with
                // typed feedback; every other unit preserves the order and
                // records the failed revision so it waits for a later map
                // change that may reopen a path instead of dropping an order
                // that is still wanted.
                if snapshot.active_worker_task {
                    order = None;
                    worker_route_failed = true;
                } else if let Some(preserved) = order.as_mut() {
                    preserved.map_revision = map.revision();
                    preserved.last_failed_replan = Some(map.revision());
                }
            }
        }

        {
            let mut entity = world.entity_mut(snapshot.entity);
            entity.insert(position);
            if let Some(order) = order {
                entity.insert(order);
            } else {
                entity.remove::<MoveOrder>();
            }
        }
        if worker_route_failed {
            idle_worker_on_route_failure(world, snapshot.entity, RejectReason::Unreachable);
        }
    }
}

fn refresh_route(order: &MoveOrder, current: Vec2, map: &GridMap) -> Option<MoveOrder> {
    if order.map_revision == map.revision() {
        return Some(order.clone());
    }

    compute_route(order.goal, current, map)
}

fn compute_route(goal: GridPos, current: Vec2, map: &GridMap) -> Option<MoveOrder> {
    let path = map.find_path(map.world_to_cell(current), goal)?;
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
        goal,
        map_revision: map.revision(),
        last_failed_replan: None,
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
mod tests;
