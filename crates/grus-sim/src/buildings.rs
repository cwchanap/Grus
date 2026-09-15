//! Building entities, authoritative placement validation, and construction
//! stepping. One assigned builder advances construction; accepted replacement
//! commands pause or retask it, rejected ones preserve the current work.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::catalog::{BuildingKind, ResourceKind, UnitKind, building_spec};
use crate::commands::{CommandResult, RejectReason, UnitIndex, approach_slots, owned_unit_entity};
use crate::economy::{
    Dropoff, ResourceIndex, ResourceSource, TeamEconomy, WorkerTask, cancel_worker_activity,
};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{ProductionQueue, is_producer};

#[derive(Component, Debug)]
pub struct Building {
    pub id: BuildingId,
    pub team: TeamId,
    pub kind: BuildingKind,
    pub construction: ConstructionState,
}

#[derive(Clone, Copy, Debug)]
pub struct ConstructionState {
    pub progress_seconds: f32,
    pub complete: bool,
    pub active_builder: Option<UnitId>,
}

/// Stable lookup from `BuildingId` to its entity, mirroring `UnitIndex`.
#[derive(Debug, Default, Resource)]
pub struct BuildingIndex(HashMap<BuildingId, Entity>);

impl BuildingIndex {
    pub fn entity(&self, id: BuildingId) -> Option<Entity> {
        self.0.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&BuildingId, &Entity)> {
        self.0.iter()
    }

    pub(crate) fn insert(&mut self, id: BuildingId, entity: Entity) {
        self.0.insert(id, entity);
    }
}

/// Everything an accepted placement needs to apply: the builder entity, the
/// authored footprint, and the immediate-perimeter slot plus route the builder
/// was assigned.
pub struct PlacementPlan {
    pub builder: Entity,
    pub footprint: Footprint,
    pub slot: GridPos,
    pub route: Vec<Vec2>,
}

/// Authoritative placement validation. Checks, in order: owned villager →
/// kind unlocked/buildable → footprint in bounds → footprint cells walkable
/// and free of any live unit's current cell or claimed `MoveOrder` goal →
/// affordability → reachable reserved immediate-perimeter builder slot,
/// evaluated on the post-placement map (footprint cells already blocked).
/// Mutates nothing; apply the returned plan only after every check passes.
pub fn validate_placement(
    world: &World,
    map: &GridMap,
    issuer: TeamId,
    builder: UnitId,
    kind: BuildingKind,
    anchor: GridPos,
) -> Result<PlacementPlan, RejectReason> {
    // 1. owned villager.
    let entity = owned_unit_entity(world, builder, issuer)?;
    if !world
        .get::<Unit>(entity)
        .is_some_and(|unit| unit.kind == UnitKind::Villager)
    {
        return Err(RejectReason::NotVillager);
    }

    // 2. kind unlocked/buildable. Town Centers are seeded, never placed.
    let spec = building_spec(kind);
    let team_age = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .map(|state| state.age);
    if kind == BuildingKind::TownCenter || team_age.is_some_and(|age| age < spec.required_age) {
        return Err(RejectReason::Locked);
    }

    let footprint = Footprint::new(anchor, spec.width, spec.height);

    // 3. footprint in bounds.
    if !footprint.cells().iter().all(|cell| map.in_bounds(*cell)) {
        return Err(RejectReason::OutOfBounds);
    }

    // 4. footprint cells walkable and free of any live unit's current cell or
    // claimed MoveOrder goal: placement would block the cells and permanently
    // entomb a unit standing inside the footprint (find_path needs a walkable
    // start), or strand a unit whose goal lies inside on a preserved order
    // that can never replan onto blocked cells. The builder's own goal is
    // exempt because acceptance cancels that order.
    let mut unit_cells: HashSet<GridPos> = HashSet::new();
    if let Some(index) = world.get_resource::<UnitIndex>() {
        for (_, unit_entity) in index.iter() {
            if let Some(position) = world.get::<SimPosition>(*unit_entity) {
                unit_cells.insert(map.world_to_cell(position.current));
            }
            if *unit_entity != entity
                && let Some(order) = world.get::<MoveOrder>(*unit_entity)
            {
                unit_cells.insert(order.goal);
            }
        }
    }
    if !footprint
        .cells()
        .iter()
        .all(|cell| map.is_walkable(*cell) && !unit_cells.contains(cell))
    {
        return Err(RejectReason::Occupied);
    }

    // 5. affordable.
    let affordable = world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&issuer))
        .is_some_and(|state| {
            state.stockpile.food >= spec.cost.food
                && state.stockpile.wood >= spec.cost.wood
                && state.stockpile.gold >= spec.cost.gold
        });
    if !affordable {
        return Err(RejectReason::InsufficientResources);
    }

    // 6. reachable immediate-perimeter builder slot, checked against the
    // post-placement map: acceptance blocks the footprint, so a route that
    // only exists through those cells would charge for a site the builder
    // then cannot reach — the first movement replan fails and it idles.
    let mut occupied_map = map.clone();
    for cell in footprint.cells() {
        occupied_map.set_blocked(cell, true);
    }
    let (slot, route) = reachable_builder_slot(world, &occupied_map, entity, footprint, None)?;
    Ok(PlacementPlan {
        builder: entity,
        footprint,
        slot,
        route,
    })
}

/// Applies an accepted `PlaceBuilding`: validates first, then cancels the
/// builder's old activity, deducts the cost once, allocates the `BuildingId`,
/// blocks the footprint, spawns `Building + Footprint`, and routes the builder
/// to its stored approach slot.
pub(crate) fn apply_place_building(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    builder: UnitId,
    kind: BuildingKind,
    anchor: GridPos,
) -> CommandResult {
    let mut result = CommandResult::default();
    let plan = match validate_placement(world, map, issuer, builder, kind, anchor) {
        Ok(plan) => plan,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };

    cancel_worker_activity(world, plan.builder);

    let spec = building_spec(kind);
    {
        let mut economy = world
            .get_resource_mut::<TeamEconomy>()
            .expect("validated economy");
        let state = economy.0.get_mut(&issuer).expect("validated team");
        state.stockpile.food -= spec.cost.food;
        state.stockpile.wood -= spec.cost.wood;
        state.stockpile.gold -= spec.cost.gold;
    }

    let id = world.resource_mut::<IdAllocator>().allocate_building();

    for cell in plan.footprint.cells() {
        map.set_blocked(cell, true);
    }

    let building_entity = world
        .spawn((
            Building {
                id,
                team: issuer,
                kind,
                construction: ConstructionState {
                    progress_seconds: 0.0,
                    complete: false,
                    active_builder: Some(builder),
                },
            },
            plan.footprint,
        ))
        .id();
    let mut index = world.get_resource_or_insert_with(BuildingIndex::default);
    index.insert(id, building_entity);

    world
        .entity_mut(plan.builder)
        .insert(WorkerTask::ToConstruction {
            building: id,
            slot: plan.slot,
        });
    if !plan.route.is_empty() {
        world.entity_mut(plan.builder).insert(MoveOrder {
            waypoints: plan.route,
            next: 0,
            goal: plan.slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }

    result
}

/// Applies an accepted `ResumeConstruction` on an incomplete owned building:
/// validates first, then retasks the builder to an immediate-perimeter slot of
/// the site. Accumulated progress is untouched.
pub(crate) fn apply_resume_construction(
    world: &mut World,
    map: &mut GridMap,
    issuer: TeamId,
    builder: UnitId,
    building: BuildingId,
) -> CommandResult {
    let mut result = CommandResult::default();
    let (builder_entity, building_entity) = match validate_resume(world, issuer, builder, building)
    {
        Ok(entities) => entities,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };
    let footprint = world
        .get::<Footprint>(building_entity)
        .copied()
        .expect("building footprint");
    // Look up the site's current builder before route validation: an accepted
    // takeover cancels its order below, so its soon-released goal must not
    // reserve a slot the replacement needs, while its current cell is still
    // occupied and stays reserved. Validation remains side-effect free — a
    // rejection here never reached the cancellation.
    let previous_entity = world
        .get::<Building>(building_entity)
        .and_then(|state| state.construction.active_builder)
        .filter(|previous| *previous != builder)
        .and_then(|previous| {
            world
                .get_resource::<UnitIndex>()
                .and_then(|index| index.entity(previous))
        });
    let (slot, route) =
        match reachable_builder_slot(world, map, builder_entity, footprint, previous_entity) {
            Ok(slot) => slot,
            Err(reason) => {
                result.reject = Some(reason);
                return result;
            }
        };

    // The site's previous builder is paused before the new one takes over:
    // one active builder per building, never two.
    if let Some(previous_entity) = previous_entity {
        cancel_worker_activity(world, previous_entity);
    }

    cancel_worker_activity(world, builder_entity);
    world
        .entity_mut(builder_entity)
        .insert(WorkerTask::ToConstruction { building, slot });
    if !route.is_empty() {
        world.entity_mut(builder_entity).insert(MoveOrder {
            waypoints: route,
            next: 0,
            goal: slot,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }
    if let Some(mut state) = world.get_mut::<Building>(building_entity) {
        state.construction.active_builder = Some(builder);
    }

    result
}

/// Resume validation: owned villager → building exists → owned → incomplete.
fn validate_resume(
    world: &World,
    issuer: TeamId,
    builder: UnitId,
    building: BuildingId,
) -> Result<(Entity, Entity), RejectReason> {
    let entity = owned_unit_entity(world, builder, issuer)?;
    if !world
        .get::<Unit>(entity)
        .is_some_and(|unit| unit.kind == UnitKind::Villager)
    {
        return Err(RejectReason::NotVillager);
    }

    let building_entity = world
        .get_resource::<BuildingIndex>()
        .and_then(|index| index.entity(building))
        .ok_or(RejectReason::BuildingMissing)?;
    let state = world
        .get::<Building>(building_entity)
        .ok_or(RejectReason::BuildingMissing)?;
    if state.team != issuer {
        return Err(RejectReason::NotOwned);
    }
    if state.construction.complete {
        return Err(RejectReason::Locked);
    }
    Ok((entity, building_entity))
}

/// Picks the builder's approach slot: the first reachable walkable cell on the
/// footprint's immediate perimeter that no live unit stands on or moves to.
/// `goal_released` names a unit whose `MoveOrder` an accepted command cancels
/// (a construction site's previous builder on takeover): its goal is not
/// seeded because acceptance frees it, while its current cell still reserves.
/// Returns the slot and its route waypoints.
fn reachable_builder_slot(
    world: &World,
    map: &GridMap,
    builder: Entity,
    footprint: Footprint,
    goal_released: Option<Entity>,
) -> Result<(GridPos, Vec<Vec2>), RejectReason> {
    let position = world
        .get::<SimPosition>(builder)
        .ok_or(RejectReason::UnknownUnit)?;
    let start = map.world_to_cell(position.current);

    // Same reservation seam as Move: seed every live unit's current cell and
    // existing MoveOrder goal, excluding the builder's own current cell and
    // old goal exactly as Move releases a commanded unit's reservations before
    // reassignment. `goal_released` frees one more goal ahead of acceptance:
    // the order carrying it is cancelled once the command applies, so only
    // that unit's current cell still reserves. When the new order replaces
    // the old one, `cancel_worker_activity` drops the old route (releasing
    // its goal) and the new order claims the slot; on rejection nothing here
    // mutated.
    let mut used = HashSet::new();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        if entity == builder {
            continue;
        }
        if let Some(position) = world.get::<SimPosition>(entity) {
            used.insert(map.world_to_cell(position.current));
        }
        if Some(entity) != goal_released
            && let Some(order) = world.get::<MoveOrder>(entity)
        {
            used.insert(order.goal);
        }
    }
    for slot in approach_slots(map, footprint, &used, footprint.perimeter_cells().len()) {
        if let Some(path) = map.find_path(start, slot) {
            let route = path
                .into_iter()
                .skip(1)
                .map(|cell| map.cell_center(cell))
                .collect();
            return Ok((slot, route));
        }
    }
    Err(RejectReason::Unreachable)
}

/// Advances construction one fixed tick. A worker transitions
/// `ToConstruction → Constructing` once its current cell equals the stored
/// slot; each building then advances only while its single `active_builder` is
/// the worker in `Constructing` state, so a second builder can never double
/// the rate. Completion clears the assignment, idles the builder, grants a
/// Storehouse its `Dropoff` marker, and grants a Farm its renewable
/// `ResourceSource` under a fresh runtime `ResourceId` — each exactly once.
pub fn step_construction(world: &mut World, seconds: f32) {
    // Arrival transitions happen before advancement so movement completion is
    // visible to construction in the same fixed tick.
    let mut arrivals = Vec::new();
    {
        let mut query = world.query::<(Entity, &WorkerTask, &SimPosition)>();
        for (entity, task, position) in query.iter(world) {
            if let WorkerTask::ToConstruction { building, slot } = task
                && worker_at_slot(position, *slot)
            {
                arrivals.push((entity, *building));
            }
        }
    }
    for (entity, building) in arrivals {
        world
            .entity_mut(entity)
            .insert(WorkerTask::Constructing { building });
    }

    let buildings: Vec<(Entity, BuildingId, BuildingKind, TeamId, ConstructionState)> = {
        let mut query = world.query::<(Entity, &Building)>();
        query
            .iter(world)
            .map(|(entity, building)| {
                (
                    entity,
                    building.id,
                    building.kind,
                    building.team,
                    building.construction,
                )
            })
            .collect()
    };

    let mut completions = Vec::new();
    for (building_entity, id, kind, team, state) in buildings {
        if state.complete {
            continue;
        }
        let Some(builder) = state.active_builder else {
            continue;
        };
        let Some(builder_entity) = world
            .get_resource::<UnitIndex>()
            .and_then(|index| index.entity(builder))
        else {
            continue;
        };
        let constructing_this = matches!(
            world.get::<WorkerTask>(builder_entity),
            Some(WorkerTask::Constructing { building }) if *building == id
        );
        if !constructing_this {
            continue;
        }

        let progress = state.progress_seconds + seconds;
        if progress >= building_spec(kind).build_seconds as f32 {
            completions.push((building_entity, builder_entity, kind, team));
        } else if let Some(mut building) = world.get_mut::<Building>(building_entity) {
            building.construction.progress_seconds = progress;
        }
    }

    for (building_entity, builder_entity, kind, team) in completions {
        if let Some(mut building) = world.get_mut::<Building>(building_entity) {
            building.construction.complete = true;
            building.construction.active_builder = None;
        }
        world.entity_mut(builder_entity).insert(WorkerTask::Idle);
        if kind == BuildingKind::Storehouse {
            world.entity_mut(building_entity).insert(Dropoff { team });
        }
        if is_producer(kind) {
            // Completed producers own their FIFO production queue from birth.
            world
                .entity_mut(building_entity)
                .insert(ProductionQueue::default());
        }
        if kind == BuildingKind::Farm {
            // The completed Farm gains a renewable Food source on the same
            // building entity under a fresh runtime `ResourceId`.
            let id = world.resource_mut::<IdAllocator>().allocate_resource();
            world.entity_mut(building_entity).insert(ResourceSource {
                id,
                kind: ResourceKind::Food,
                remaining: None,
                assigned_worker: None,
            });
            world
                .get_resource_or_insert_with(ResourceIndex::default)
                .insert(id, building_entity);
        }
    }
}

/// A worker has reached its stored slot when its current cell equals the slot.
/// Cell equality (not exact-center distance) absorbs post-arrival separation
/// nudges of up to `MAX_SEPARATION_STEP`.
pub(crate) fn worker_at_slot(position: &SimPosition, slot: GridPos) -> bool {
    position.current.x.floor() as i32 == slot.x && position.current.y.floor() as i32 == slot.y
}

#[cfg(test)]
mod tests;
