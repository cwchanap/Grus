use std::collections::{HashMap, HashSet};

use bevy::math::Vec2;
use bevy::prelude::{Entity, Resource, World};

use crate::buildings::{apply_place_building, apply_resume_construction};
use crate::catalog::{BuildingKind, UnitKind, unit_spec};
use crate::combat::{
    AttackCooldown, CombatOrder, CombatTarget, Health, resolve_target, target_eligible,
};
use crate::economy::{apply_gather, cancel_unit_activity};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{apply_enqueue_age_up, apply_enqueue_unit, apply_set_rally};
use crate::session::gameplay_active;

#[derive(Clone, Debug, PartialEq)]
pub enum UnitCommandKind {
    Move { target: Vec2 },
    AttackMove { target: Vec2 },
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitCommand {
    pub issuer: TeamId,
    pub units: Vec<UnitId>,
    pub kind: UnitCommandKind,
}

/// Top-level player command. Grows only when its owning behavior lands.
#[derive(Clone, Debug, PartialEq)]
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
    Attack {
        issuer: TeamId,
        units: Vec<UnitId>,
        target: CombatTarget,
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
    NotCombatant,
    TargetMissing,
    InvalidTarget,
    SessionLocked,
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

    pub(crate) fn remove(&mut self, id: UnitId) {
        self.0.remove(&id);
    }
}

/// Spawns one unit registered in `UnitIndex`, carrying its catalogue
/// `Health` at full value and a ready `AttackCooldown`.
pub fn spawn_unit(
    world: &mut World,
    id: UnitId,
    team: TeamId,
    position: Vec2,
    kind: UnitKind,
    speed: f32,
) -> Entity {
    let spec = unit_spec(kind);
    let entity = world
        .spawn((
            Unit {
                id,
                team,
                kind,
                speed,
            },
            SimPosition::new(position),
            Health {
                current: spec.max_health,
                max: spec.max_health,
            },
            AttackCooldown::default(),
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
    // One gate for every gameplay command kind: an explicit session outside
    // Playing locks orders; an absent session (pure sim, benchmark) stays open.
    if !gameplay_active(world) {
        return CommandResult {
            reject: Some(RejectReason::SessionLocked),
            ..CommandResult::default()
        };
    }
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
        PlayerCommand::Attack {
            issuer,
            units,
            target,
        } => apply_attack(world, issuer, units, target),
    }
}

/// Direct Attack: validates per unit (owned → combatant → target exists →
/// target eligible through the one `target_eligible` seam) before touching
/// any state; an accepted unit then cancels its prior activity through the
/// shared helper and receives a fresh `CombatOrder::Attack`.
fn apply_attack(
    world: &mut World,
    issuer: TeamId,
    mut units: Vec<UnitId>,
    target: CombatTarget,
) -> CommandResult {
    units.sort_unstable();
    units.dedup();
    let mut outcome = CommandResult::default();

    for id in units {
        let entity = match owned_unit_entity(world, id, issuer) {
            Ok(entity) => entity,
            Err(reason) => {
                outcome.rejected_units.push((id, reason));
                continue;
            }
        };
        if world
            .get::<Unit>(entity)
            .is_none_or(|unit| unit_spec(unit.kind).combat.is_none())
        {
            outcome
                .rejected_units
                .push((id, RejectReason::NotCombatant));
            continue;
        }
        if resolve_target(world, target).is_none() {
            outcome
                .rejected_units
                .push((id, RejectReason::TargetMissing));
            continue;
        }
        if !target_eligible(world, issuer, target) {
            outcome
                .rejected_units
                .push((id, RejectReason::InvalidTarget));
            continue;
        }
        // Fully validated: cancel the unit's previous activity before
        // installing the combat order.
        cancel_unit_activity(world, entity);
        world.entity_mut(entity).insert(CombatOrder::Attack {
            target,
            last_target_cell: None,
        });
        outcome.accepted_units.push(id);
    }

    outcome
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
        UnitCommandKind::AttackMove { target } => {
            // Per-unit: owned → combatant → walkable and reachable
            // destination. Reachability is probed once at accept time
            // through the exact assignment seam the install uses; an
            // unreachable destination is rejected with the unit's prior
            // state fully untouched (no cancelled intent, no per-tick A*
            // retry), and only a reachable destination cancels the prior
            // activity and installs the combat order plus its opening
            // route. Enemy acquisition happens in `step_combat`.
            let destination = map.world_to_cell(target);
            for id in units {
                let entity = match owned_unit_entity(world, id, issuer) {
                    Ok(entity) => entity,
                    Err(reason) => {
                        outcome.rejected_units.push((id, reason));
                        continue;
                    }
                };
                if world
                    .get::<Unit>(entity)
                    .is_none_or(|unit| unit_spec(unit.kind).combat.is_none())
                {
                    outcome
                        .rejected_units
                        .push((id, RejectReason::NotCombatant));
                    continue;
                }
                if !map.is_walkable(destination) {
                    outcome.rejected_units.push((id, RejectReason::Unreachable));
                    continue;
                }
                let Some(position) = world.get::<SimPosition>(entity).copied() else {
                    outcome.rejected_units.push((id, RejectReason::UnknownUnit));
                    continue;
                };
                let start = map.world_to_cell(position.current);
                let Some((goal, waypoints)) =
                    plan_move_route(world, map, entity, start, destination)
                else {
                    outcome.rejected_units.push((id, RejectReason::Unreachable));
                    continue;
                };
                // Reachable: cancel prior activity, then install the probed
                // route and order. The order anchors the planner's accepted
                // goal — a neighbor cell when the click was already claimed —
                // so `resume_destination` recognizes this leg as its own
                // instead of re-planning toward the clicked cell every tick.
                cancel_unit_activity(world, entity);
                if !waypoints.is_empty() {
                    world.entity_mut(entity).insert(MoveOrder {
                        waypoints,
                        next: 0,
                        goal,
                        map_revision: map.revision(),
                        last_failed_replan: None,
                    });
                }
                world.entity_mut(entity).insert(CombatOrder::AttackMove {
                    destination: goal,
                    target: None,
                    last_target_cell: None,
                });
                outcome.accepted_units.push(id);
            }
        }
        UnitCommandKind::Stop => {
            for id in units {
                match owned_unit_entity(world, id, issuer) {
                    Ok(entity) => {
                        // Stop is unconditional activity cancellation.
                        cancel_unit_activity(world, entity);
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

                // Accepted replacement: cancel the unit's previous activity
                // (which also drops its old order) before installing the new
                // route. Rejected units above never reach this.
                cancel_unit_activity(world, entity);
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
    let (goal, waypoints) = plan_move_route(world, map, entity, start, target)?;
    if !waypoints.is_empty() {
        world.entity_mut(entity).insert(MoveOrder {
            waypoints,
            next: 0,
            goal,
            map_revision: map.revision(),
            last_failed_replan: None,
        });
    }
    Some(goal)
}

/// Plans one unit's route toward `target` — the Move-command destination
/// generation and reservation seeding, so the plan cannot claim a cell
/// another live unit stands on or already has a `MoveOrder` goal for —
/// without mutating anything. Returns the assigned goal cell and its
/// waypoints, or `None` when no reachable free slot exists. Callers probe
/// reachability with this, then install through `assign_move_toward` or
/// their own `MoveOrder` insert.
fn plan_move_route(
    world: &World,
    map: &GridMap,
    entity: Entity,
    start: GridPos,
    target: GridPos,
) -> Option<(GridPos, Vec<Vec2>)> {
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
                (slot, waypoints)
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
mod tests;
