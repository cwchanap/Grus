//! Building entities, authoritative placement validation, and construction
//! stepping. One assigned builder advances construction; accepted replacement
//! commands pause or retask it, rejected ones preserve the current work.

use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, Resource, World};

use crate::catalog::{BuildingKind, ResourceKind, UnitKind, building_spec};
use crate::combat::Health;
use crate::commands::{CommandResult, RejectReason, UnitIndex, approach_slots, owned_unit_entity};
use crate::economy::{
    Dropoff, ResourceIndex, ResourceSource, TeamEconomy, WorkerTask, cancel_unit_activity,
};
use crate::ids::{BuildingId, IdAllocator, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{ProductionQueue, is_producer};
use crate::session::gameplay_active;
use crate::visibility::{explored_by, visible_to};

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

    pub(crate) fn remove(&mut self, id: BuildingId) {
        self.0.remove(&id);
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
/// kind unlocked/buildable → footprint in bounds → every footprint cell
/// explored → footprint cells walkable and free of any unit's current cell
/// or claimed `MoveOrder` goal the issuer can see, with hidden enemy
/// *building* cells treated as free ground → affordability → reachable
/// reserved immediate-perimeter builder slot, evaluated on the
/// post-placement map (footprint cells already blocked). The explored check
/// runs before the occupancy scan, and the scan itself only counts
/// occupancy the issuer can observe — hidden enemy units, their move
/// goals, and hidden enemy building footprints never produce `Occupied`,
/// so the preview cannot map them. Terrain, resources and own buildings
/// stay honest regardless of vision: static or own-map state, and those
/// cells can never accept a building anyway. A hidden unit standing inside
/// an accepted footprint is displaced at apply time; a hidden enemy
/// building overlaps nothing because the apply path re-checks real
/// occupancy. Mutates nothing; apply the returned plan only after every
/// check passes.
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

    // 4. every footprint cell explored — immediately after bounds and before
    // the all-unit occupancy set below, so an `Occupied` reject can never
    // leak a hidden enemy unit, building, or claimed move goal.
    if !footprint
        .cells()
        .iter()
        .all(|cell| explored_by(world, issuer, *cell))
    {
        return Err(RejectReason::Unexplored);
    }

    // 5. footprint cells walkable and free of any observable unit's current
    // cell or claimed MoveOrder goal: placement would block the cells and
    // permanently entomb a unit standing inside the footprint (find_path
    // needs a walkable start), or strand a unit whose goal lies inside on a
    // preserved order that can never replan onto blocked cells. Only
    // occupancy the issuer can see counts — a hidden enemy's position or
    // goal must never surface as an `Occupied` reject through the preview;
    // apply handles that collision by displacement instead. Own units are
    // always observable to their issuer. The builder's own goal is exempt
    // because acceptance cancels that order.
    let mut unit_cells: HashSet<GridPos> = HashSet::new();
    if let Some(index) = world.get_resource::<UnitIndex>() {
        for (_, unit_entity) in index.iter() {
            let unit_team = world.get::<Unit>(*unit_entity).map(|unit| unit.team);
            if let Some(position) = world.get::<SimPosition>(*unit_entity) {
                let cell = map.world_to_cell(position.current);
                if unit_team == Some(issuer) || visible_to(world, issuer, cell) {
                    unit_cells.insert(cell);
                }
            }
            if *unit_entity != entity
                && let Some(order) = world.get::<MoveOrder>(*unit_entity)
                && (unit_team == Some(issuer) || visible_to(world, issuer, order.goal))
            {
                unit_cells.insert(order.goal);
            }
        }
    }
    // Cells blocked by a *hidden* enemy building are treated as free: a
    // building is dynamic state the issuer cannot remember (no last-seen
    // ghosts), so an `Occupied` from its footprint would map it exactly —
    // the preview must answer explored-hidden ground like identical empty
    // ground. Terrain, resources and own buildings are static or own state
    // and stay honest. The apply path re-checks real occupancy, because
    // two real buildings can never overlap.
    let mut hidden_enemy_cells: HashSet<GridPos> = HashSet::new();
    if let Some(index) = world.get_resource::<BuildingIndex>() {
        for (_, building_entity) in index.iter() {
            let owner = world
                .get::<Building>(*building_entity)
                .map(|building| building.team);
            if owner == Some(issuer) {
                continue;
            }
            if let Some(building_footprint) = world.get::<Footprint>(*building_entity) {
                for cell in building_footprint.cells() {
                    if !visible_to(world, issuer, cell) {
                        hidden_enemy_cells.insert(cell);
                    }
                }
            }
        }
    }
    if !footprint.cells().iter().all(|cell| {
        (map.is_walkable(*cell) || hidden_enemy_cells.contains(cell)) && !unit_cells.contains(cell)
    }) {
        return Err(RejectReason::Occupied);
    }

    // 6. affordable.
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

    // 7. reachable immediate-perimeter builder slot, checked against the
    // post-placement map: acceptance blocks the footprint, so a route that
    // only exists through those cells would charge for a site the builder
    // then cannot reach — the first movement replan fails and it idles.
    let mut occupied_map = map.clone();
    for cell in footprint.cells() {
        occupied_map.set_blocked(cell, true);
    }
    let (slot, route) =
        reachable_builder_slot(world, &occupied_map, issuer, entity, footprint, None)?;
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

    // The knowledge-aware validator deliberately passes footprints over
    // hidden enemy building cells (fog privacy), but two real buildings can
    // never overlap: the committed command re-checks real occupancy and
    // rejects instead. A one-shot command rejection is the accepted
    // shared-map channel — unlike the mouse-motion preview, an actual
    // command never maps hidden state on its own.
    if plan
        .footprint
        .cells()
        .iter()
        .any(|cell| !map.is_walkable(*cell))
    {
        result.reject = Some(RejectReason::Occupied);
        return result;
    }

    cancel_unit_activity(world, plan.builder);

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

    // A hidden enemy unit can stand inside an accepted footprint — its cell
    // was excluded from occupancy — so push occupants out before the cells
    // block; otherwise it would be entombed on an unwalkable start cell that
    // `find_path` can never route from. A hidden move goal inside the
    // footprint is likewise retargeted to open ground so the preserved
    // order can never strand on blocked cells.
    displace_footprint_occupants(world, map, plan.footprint);
    retarget_footprint_goals(world, map, plan.footprint);

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
            // Sites use full owning-building health from placement;
            // construction progress does not scale it.
            Health {
                current: spec.max_health,
                max: spec.max_health,
            },
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
    let (slot, route) = match reachable_builder_slot(
        world,
        map,
        issuer,
        builder_entity,
        footprint,
        previous_entity,
    ) {
        Ok(slot) => slot,
        Err(reason) => {
            result.reject = Some(reason);
            return result;
        }
    };

    // The site's previous builder is paused before the new one takes over:
    // one active builder per building, never two.
    if let Some(previous_entity) = previous_entity {
        cancel_unit_activity(world, previous_entity);
    }

    cancel_unit_activity(world, builder_entity);
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
/// footprint's immediate perimeter that no live unit the issuer can observe
/// stands on or moves to — a hidden unit never reserves a slot, matching the
/// occupancy rule above (and the collision resolves through ordinary
/// separation on arrival). `goal_released` names a unit whose `MoveOrder` an
/// accepted command cancels (a construction site's previous builder on
/// takeover): its goal is not seeded because acceptance frees it, while its
/// current cell still reserves. Returns the slot and its route waypoints.
fn reachable_builder_slot(
    world: &World,
    map: &GridMap,
    issuer: TeamId,
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
    // reassignment — and only when the issuer can observe the occupant, so a
    // hidden enemy never blocks an approach slot through this command either.
    // `goal_released` frees one more goal ahead of acceptance:
    // the order carrying it is cancelled once the command applies, so only
    // that unit's current cell still reserves. When the new order replaces
    // the old one, `cancel_unit_activity` drops the old route (releasing
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
        let observable = world
            .get::<Unit>(entity)
            .is_some_and(|unit| unit.team == issuer);
        if let Some(position) = world.get::<SimPosition>(entity) {
            let cell = map.world_to_cell(position.current);
            if observable || visible_to(world, issuer, cell) {
                used.insert(cell);
            }
        }
        if Some(entity) != goal_released
            && let Some(order) = world.get::<MoveOrder>(entity)
            && (observable || visible_to(world, issuer, order.goal))
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

/// Pushes every unit standing inside `footprint` onto the nearest open cell
/// outside it. Runs just before the footprint blocks: validation only rejects
/// occupancy the issuer can see, so a hidden enemy unit can be inside an
/// accepted footprint, and leaving it there would entomb it.
fn displace_footprint_occupants(world: &mut World, map: &GridMap, footprint: Footprint) {
    let footprint_cells: HashSet<GridPos> = footprint.cells().into_iter().collect();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        let Some(cell) = world
            .get::<SimPosition>(entity)
            .map(|position| map.world_to_cell(position.current))
        else {
            continue;
        };
        if !footprint_cells.contains(&cell) {
            continue;
        }
        let Some(open) = nearest_open_cell(map, cell, &footprint_cells) else {
            continue;
        };
        let center = map.cell_center(open);
        if let Some(mut position) = world.get_mut::<SimPosition>(entity) {
            // Teleport semantics: `previous` collapses onto `current` so the
            // interpolated Godot view does not slide across the displacement.
            position.previous = center;
            position.current = center;
        }
    }
}

/// Retargets every `MoveOrder` whose goal lies inside `footprint` to the
/// nearest open cell outside it. Validation only rejects goals the issuer
/// can see, so a hidden enemy route can end inside an accepted footprint;
/// leaving it would strand the unit on an order that can never replan onto
/// blocked cells. The next map revision replans the route to the new goal.
fn retarget_footprint_goals(world: &mut World, map: &GridMap, footprint: Footprint) {
    let footprint_cells: HashSet<GridPos> = footprint.cells().into_iter().collect();
    let entities: Vec<Entity> = world
        .get_resource::<UnitIndex>()
        .map(|index| index.iter().map(|(_, entity)| *entity).collect())
        .unwrap_or_default();
    for entity in entities {
        let new_goal = world.get::<MoveOrder>(entity).and_then(|order| {
            footprint_cells
                .contains(&order.goal)
                .then(|| nearest_open_cell(map, order.goal, &footprint_cells).unwrap_or(order.goal))
        });
        if let Some(goal) = new_goal
            && let Some(mut order) = world.get_mut::<MoveOrder>(entity)
        {
            order.goal = goal;
            order.last_failed_replan = None;
        }
    }
}

/// The nearest walkable cell outside `excluded`, searched by expanding
/// Chebyshev rings row-major — same ring order as the AI target picker.
fn nearest_open_cell(map: &GridMap, cell: GridPos, excluded: &HashSet<GridPos>) -> Option<GridPos> {
    for radius in 1_i32..8 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let candidate = GridPos::new(cell.x + dx, cell.y + dy);
                if map.is_walkable(candidate) && !excluded.contains(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Advances construction one fixed tick. A worker transitions
/// `ToConstruction → Constructing` once its current cell equals the stored
/// slot; each building then advances only while its single `active_builder` is
/// the worker in `Constructing` state, so a second builder can never double
/// the rate. Completion clears the assignment, idles the builder, grants a
/// Storehouse its `Dropoff` marker, and grants a Farm its renewable
/// `ResourceSource` under a fresh runtime `ResourceId` — each exactly once.
pub fn step_construction(world: &mut World, seconds: f32) {
    if !gameplay_active(world) {
        return;
    }
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
