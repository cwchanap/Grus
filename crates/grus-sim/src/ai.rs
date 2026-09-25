//! Feature-local scripted economic AI. One `AiController` resource per
//! opposing team decides at a fixed 1 Hz cadence and acts only through
//! ordinary `PlayerCommand` application — no second simulation, no behavior
//! tree, no persistent planner or squad state. Every policy step is a pure
//! decision function over the live `World`, the controller/decision state and
//! the authored `AiMapPlan`, returning zero or one command;
//! `decide_ai_commands` composes them in a fixed order and `step_ai` is only
//! the Playing/cadence gate plus the apply loop. Decision output stays
//! private: results are dropped, never surfaced through the human
//! command-feedback channel. Every candidate enumeration is sorted by stable
//! ID before a choice, so decisions never depend on HashMap order.

use std::collections::HashMap;

use bevy::math::Vec2;
use bevy::prelude::{Entity, Resource, World};

use crate::buildings::{Building, BuildingIndex, validate_placement};
use crate::catalog::{
    AGE_TWO_COST, Age, BuildingKind, Cost, MAX_POPULATION, ResourceKind, UnitKind, building_spec,
    unit_spec,
};
use crate::combat::{CombatOrder, CombatTarget};
use crate::commands::{
    CommandResult, PlayerCommand, RejectReason, UnitCommand, UnitCommandKind, UnitIndex,
    apply_player_command,
};
use crate::economy::{ResourceIndex, ResourceSource, TeamEconomy, WorkerTask, idle_worker_ids};
use crate::fixture::{AiMapPlan, MapFixture};
use crate::ids::{BuildingId, ResourceId, TeamId, UnitId};
use crate::map::{Footprint, GridMap, GridPos};
use crate::movement::{MoveOrder, SimPosition, Unit};
use crate::production::{
    ProductionKind, ProductionQueue, population_cap, population_used, produces,
};
use crate::session::gameplay_active;
use crate::visibility::{explored_by, visible_to};

/// Decision cadence in seconds. The gate runs before the accumulator, so
/// Start/Paused/Result never bank time and a resume never fires a catch-up
/// decision burst.
pub const AI_DECISION_SECONDS: f32 = 1.0;

/// HPA-473 starting values — HPA-474 may tune them without touching the
/// architecture.
const TARGET_WORKERS: u32 = 8;
/// Villager scouting unlocks only above this worker floor.
const MIN_SCOUT_WORKER_FLOOR: u32 = 4;
/// Standalone sources host up to this many workers; a Farm serves exactly one.
const WORKERS_PER_STANDALONE_SOURCE: u32 = 2;
/// Population headroom below which the next authored House is placed.
const POPULATION_HEADROOM: u32 = 2;
/// Radius (world units == cells) around the own Town Center inside which a
/// currently visible enemy unit counts as a base threat.
const DEFENSE_RADIUS: f32 = 12.0;
/// Live military count at which one grouped AttackMove marches.
const ATTACK_THRESHOLD: u32 = 6;
/// Rejected scout legs retry at later decisions, but a leg that keeps
/// rejecting is skipped so a permanent blocker cannot stall the rest of the
/// route.
const MAX_SCOUT_LEG_FAILURES: u32 = 3;
/// Round-robin army composition cursor order.
const ARMY_ROTATION: [UnitKind; 3] = [UnitKind::Spearman, UnitKind::Archer, UnitKind::Cavalry];

/// Per-team AI state. Persisted across decisions on purpose: the cadence
/// accumulator, scout route progress, the only retained enemy memory (the
/// last genuinely observed enemy Town Center cell) and the round-robin army
/// cursor. Restart removes the controller wholesale, which resets all of it.
#[derive(Debug, Resource)]
pub struct AiController {
    pub team: TeamId,
    decision_accumulator: f32,
    scout_route_index: usize,
    /// Rejections of the currently issued scout leg; reaching
    /// `MAX_SCOUT_LEG_FAILURES` skips the leg instead of stalling the route.
    scout_leg_failures: u32,
    /// The only retained enemy memory: the last cell of an enemy Town Center
    /// genuinely seen by this team. Written only while one is visible;
    /// restart removes the controller wholesale, which drops it.
    remembered_enemy_town_center: Option<GridPos>,
    /// Authored anchors whose placement command rejected `Occupied` at
    /// apply time — a hidden enemy building stands there (the
    /// knowledge-aware validator let the footprint through the preview).
    /// `place_at_slot` skips them so one unseen blocker cannot stall AI
    /// growth forever; restart drops the controller and this with it.
    blocked_anchors: Vec<GridPos>,
    next_army_kind: usize,
}

impl AiController {
    pub fn new(team: TeamId) -> Self {
        Self {
            team,
            decision_accumulator: 0.0,
            scout_route_index: 0,
            scout_leg_failures: 0,
            remembered_enemy_town_center: None,
            blocked_anchors: Vec::new(),
            next_army_kind: 0,
        }
    }
}

/// One decided command plus the controller transition it commits only when
/// its `CommandResult` accepts it: a rejected command must never consume
/// persistent progress (a scout leg skipped forever without being walked, an
/// army kind rotated past without being trained).
#[derive(Debug)]
pub struct PlannedCommand {
    /// The ordinary player command to apply.
    pub command: PlayerCommand,
    pub(crate) commit: Option<AiCommit>,
}

/// A controller transition deferred until its paired command is accepted.
#[derive(Clone, Copy, Debug)]
pub(crate) enum AiCommit {
    /// A scout Move was issued to `unit`: advance the route cursor on
    /// acceptance; on rejection count a leg failure and skip the leg after
    /// `MAX_SCOUT_LEG_FAILURES` so a permanent blocker cannot stall the route.
    ScoutLeg { unit: UnitId },
    /// An army enqueue was issued: on acceptance store the next rotation
    /// cursor.
    ArmyCursor(usize),
    /// A building placement was issued at `anchor`: an `Occupied` apply-time
    /// reject means a hidden enemy building stands there — remember the
    /// anchor so the AI stops offering it every decision.
    PlaceAnchor { anchor: GridPos },
}

impl PlannedCommand {
    /// A command carrying no controller transition.
    fn plain(command: PlayerCommand) -> Self {
        Self {
            command,
            commit: None,
        }
    }

    /// A command whose controller transition applies only on acceptance.
    fn on_accept(command: PlayerCommand, commit: AiCommit) -> Self {
        Self {
            command,
            commit: Some(commit),
        }
    }
}

/// The AI fixed step: a Playing-gated 1 Hz decision whose commands apply
/// through the one command dispatcher. Runs after the visibility refresh so a
/// decision reads the same freshly computed fog the presentation receives.
pub fn step_ai(world: &mut World, map: &mut GridMap, seconds: f32) {
    if !gameplay_active(world) {
        // Before the accumulator: frozen phases must not bank time.
        return;
    }
    let Some(mut controller) = world.remove_resource::<AiController>() else {
        return;
    };
    controller.decision_accumulator += seconds;
    if controller.decision_accumulator >= AI_DECISION_SECONDS {
        controller.decision_accumulator -= AI_DECISION_SECONDS;
        let plan = MapFixture::team_plan(controller.team);
        for planned in decide_ai_commands(world, &mut controller, &plan, map) {
            // Rejects never reach the human `CommandFeedback` channel, but
            // they do gate the controller transition the command carried.
            let result = apply_player_command(world, map, planned.command);
            commit_outcome(&mut controller, planned.commit, &result);
        }
    }
    world.insert_resource(controller);
}

/// Commits one deferred controller transition for an applied command: the
/// scout cursor and army rotation move only on acceptance, and a scout leg
/// that keeps rejecting is skipped so a permanent blocker cannot stall the
/// route. `step_ai` and the test harness share this seam.
fn commit_outcome(controller: &mut AiController, commit: Option<AiCommit>, result: &CommandResult) {
    match commit {
        Some(AiCommit::ScoutLeg { unit }) => {
            if result.accepted_units.contains(&unit) {
                controller.scout_route_index += 1;
                controller.scout_leg_failures = 0;
            } else {
                controller.scout_leg_failures += 1;
                if controller.scout_leg_failures >= MAX_SCOUT_LEG_FAILURES {
                    controller.scout_route_index += 1;
                    controller.scout_leg_failures = 0;
                }
            }
        }
        Some(AiCommit::ArmyCursor(next)) if result.reject.is_none() => {
            controller.next_army_kind = next;
        }
        Some(AiCommit::PlaceAnchor { anchor })
            if result.reject == Some(RejectReason::Occupied)
                && !controller.blocked_anchors.contains(&anchor) =>
        {
            controller.blocked_anchors.push(anchor);
        }
        _ => {}
    }
}

/// The one ordered policy step: a pure decision over the pre-decision world
/// (the `GridMap` is static bounds/blockers data, explicitly readable).
type PolicyStep =
    fn(&World, &mut AiController, &AiMapPlan, &GridMap, &mut Vec<UnitId>) -> Option<PlannedCommand>;

/// Ordered composition of the pure policy steps. Every step sees the same
/// pre-decision world; `claimed` keeps two steps of this same decision from
/// commanding the same villager. The order is the contract: visible-threat
/// defense, population-stall avoidance (House), worker replacement, scouting,
/// idle-worker allocation, base growth (Barracks, Archery Range, Storehouse,
/// Farm), the Age-2 attempt, the Stable, the round-robin army and the grouped
/// attack.
pub fn decide_ai_commands(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
) -> Vec<PlannedCommand> {
    // The memory step writes controller state, not a command: while an enemy
    // Town Center is actually visible, its cell is the retained memory.
    update_remembered_town_center(world, controller);
    let mut commands = Vec::new();
    let mut claimed = Vec::new();
    for step in [
        defend_visible_threats as PolicyStep,
        place_house,
        queue_replacement_worker,
        scout,
        allocate_idle_worker,
        place_barracks,
        place_archery_range,
        place_storehouse,
        place_farm,
        attempt_age_two,
        place_stable,
        train_round_robin,
        attack,
    ] {
        if let Some(command) = step(world, controller, plan, map, &mut claimed) {
            commands.push(command);
        }
    }
    commands
}

/// Defend visible threats: a currently visible enemy unit inside the base
/// radius pulls every available (idle, unclaimed) military unit into one
/// direct Attack on the nearest threat — nearest by distance to the Town
/// Center, ties broken by stable ID. The threat is visible right now, so
/// the direct entity target is legitimate; a hidden enemy never reaches
/// this step, so hidden threats cannot change the decision. Units already
/// holding a Move/Combat order keep theirs; the decision never retasks an
/// engaged unit.
fn defend_visible_threats(
    world: &World,
    controller: &mut AiController,
    _plan: &AiMapPlan,
    _map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let (_, town_center_entity) = town_center(world, controller.team)?;
    let home = town_center_cell_center(world, town_center_entity)?;
    let (threat_id, _) = visible_threats(world, controller.team, home)
        .into_iter()
        .min_by(|(a_id, a_position), (b_id, b_position)| {
            a_position
                .distance_squared(home)
                .total_cmp(&b_position.distance_squared(home))
                .then(a_id.cmp(b_id))
        })?;
    let defenders = available_military_ids(world, controller.team, claimed);
    if defenders.is_empty() {
        return None;
    }
    claimed.extend_from_slice(&defenders);
    Some(PlannedCommand::plain(PlayerCommand::Attack {
        issuer: controller.team,
        units: defenders,
        target: CombatTarget::Unit(threat_id),
    }))
}

/// The retained enemy memory: while an enemy Town Center is currently
/// visible its anchor cell is recorded; the cell persists after vision is
/// lost. Lowest stable ID wins when more than one is visible.
fn update_remembered_town_center(world: &World, controller: &mut AiController) {
    let mut visible_centers: Vec<(BuildingId, GridPos)> = world
        .get_resource::<BuildingIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let building = world.get::<Building>(*entity)?;
                    if building.team == controller.team
                        || building.kind != BuildingKind::TownCenter
                        || !building.construction.complete
                    {
                        return None;
                    }
                    let footprint = world.get::<Footprint>(*entity)?;
                    visible_to(world, controller.team, *footprint)
                        .then_some((*id, footprint.anchor))
                })
                .collect()
        })
        .unwrap_or_default();
    visible_centers.sort_by_key(|(id, _)| *id);
    if let Some((_, anchor)) = visible_centers.into_iter().next() {
        controller.remembered_enemy_town_center = Some(anchor);
    }
}

/// Keep a small worker pool: queue a Villager at the Town Center while below
/// the target, only against an empty queue and only when affordable — a
/// killed worker is replaced through the normal paid production path, never
/// for free.
fn queue_replacement_worker(
    world: &World,
    controller: &mut AiController,
    _plan: &AiMapPlan,
    _map: &GridMap,
    _claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    if villager_ids(world, controller.team).len() as u32 >= TARGET_WORKERS {
        return None;
    }
    let (town_center_id, town_center_entity) = town_center(world, controller.team)?;
    if !queue_is_empty(world, town_center_entity)
        || !can_afford(world, controller.team, unit_spec(UnitKind::Villager).cost)
    {
        return None;
    }
    Some(PlannedCommand::plain(PlayerCommand::EnqueueUnit {
        issuer: controller.team,
        building: town_center_id,
        kind: UnitKind::Villager,
    }))
}

/// Scout early, before any expansion-dependent growth: the lowest stable-ID
/// idle military unit takes the next route leg; without one, an idle surplus
/// villager does (worker floor and allocation deficit respected — an active
/// economic worker is never retasked, and a unit already walking holds a
/// `MoveOrder` and is skipped). Both use ordinary Move: combatants may use
/// AttackMove, but a scouting combatant that attacked what it saw would turn
/// route patrol into an unrequested raid. The route is traversed once; the
/// index never wraps, so the surplus worker returns to the economy. The
/// cursor is a deferred commit: it advances only when the Move is accepted,
/// so a rejected leg is retried rather than silently skipped.
fn scout(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    _map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    if controller.scout_route_index >= plan.scout_route.len() {
        return None;
    }
    let cell = plan.scout_route[controller.scout_route_index];
    let target = Vec2::new(cell.x as f32 + 0.5, cell.y as f32 + 0.5);

    let scout_id = match idle_military_id(world, controller.team, claimed) {
        Some(id) => id,
        None => villager_scout_id(world, controller.team, claimed)?,
    };
    claimed.push(scout_id);
    Some(PlannedCommand::on_accept(
        PlayerCommand::Units(UnitCommand {
            issuer: controller.team,
            units: vec![scout_id],
            kind: UnitCommandKind::Move { target },
        }),
        AiCommit::ScoutLeg { unit: scout_id },
    ))
}

/// A villager may scout only above the worker floor and only when the economy
/// has no unsatisfied allocation deficit — i.e. it is genuinely surplus.
fn villager_scout_id(world: &World, team: TeamId, claimed: &[UnitId]) -> Option<UnitId> {
    if villager_ids(world, team).len() as u32 <= MIN_SCOUT_WORKER_FLOOR
        || has_allocation_deficit(world, team)
    {
        return None;
    }
    idle_worker_ids(world, team)
        .into_iter()
        .find(|id| !claimed.contains(id))
}

/// Assign one idle villager per decision toward a simple Food/Wood/Gold
/// split. Existing `WorkerTask`/assignment state is the source of truth;
/// candidates are explored standalone sources and own completed Farms, and
/// the kind's nearest known source to the own Town Center wins (ties by
/// stable ID) — an explored enemy-base fringe source never outranks a home
/// source just because it has a lower ID.
fn allocate_idle_worker(
    world: &World,
    controller: &mut AiController,
    _plan: &AiMapPlan,
    _map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let idle = idle_worker_ids(world, controller.team)
        .into_iter()
        .find(|id| !claimed.contains(id))?;
    let home = town_center(world, controller.team)
        .and_then(|(_, entity)| town_center_cell_center(world, entity));
    let sources = known_sources(world, controller.team);
    let counts = task_kind_counts(world, controller.team);
    let total = villager_ids(world, controller.team).len() as u32;

    for (kind, target) in split_targets(total) {
        if counts.get(&kind).copied().unwrap_or(0) >= target {
            continue;
        }
        let Some(source) = sources
            .iter()
            .filter(|source| source.kind == kind)
            .filter(|source| source_has_capacity(world, controller.team, source))
            .min_by(|a, b| {
                source_distance(a, home)
                    .total_cmp(&source_distance(b, home))
                    .then(a.id.cmp(&b.id))
            })
        else {
            continue;
        };
        claimed.push(idle);
        return Some(PlannedCommand::plain(PlayerCommand::Gather {
            issuer: controller.team,
            workers: vec![idle],
            source: source.id,
        }));
    }
    None
}

/// Distance from a known source to the own Town Center; without a Town
/// Center every source ties and stable ID decides.
fn source_distance(source: &KnownSource, home: Option<Vec2>) -> f32 {
    home.map_or(0.0, |home| source.center.distance_squared(home))
}

/// Avoid population stalls: with low free capacity and cap below 100, place
/// the next authored House using a real villager command.
fn place_house(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let cap = population_cap(world, controller.team);
    // Destruction can drive the cap below the live population; saturate so
    // the rebuild path still sees zero free capacity instead of panicking.
    if cap >= MAX_POPULATION
        || cap.saturating_sub(population_used(world, controller.team)) >= POPULATION_HEADROOM
    {
        return None;
    }
    let anchors: Vec<GridPos> = plan
        .house_slots
        .iter()
        .copied()
        .filter(|slot| !has_building_at(world, controller.team, BuildingKind::House, *slot))
        .collect();
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::House,
        &anchors,
    )
}

/// Grow the production core: one Barracks on the authored slot.
fn place_barracks(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    if any_own_building(world, controller.team, BuildingKind::Barracks) {
        return None;
    }
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::Barracks,
        &[plan.barracks_anchor],
    )
}

/// Grow the production core: one Archery Range on the authored slot.
fn place_archery_range(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    if any_own_building(world, controller.team, BuildingKind::ArcheryRange) {
        return None;
    }
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::ArcheryRange,
        &[plan.archery_range_anchor],
    )
}

/// Add Farms only when known food supply is insufficient: food worker
/// capacity (two per standalone source, one per Farm) below the split target.
fn place_farm(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let total = villager_ids(world, controller.team).len() as u32;
    let food_target = total / 2;
    let capacity: u32 = known_sources(world, controller.team)
        .iter()
        .filter(|source| source.kind == ResourceKind::Food)
        .map(|source| {
            if source.farm {
                1
            } else {
                WORKERS_PER_STANDALONE_SOURCE
            }
        })
        .sum();
    if capacity >= food_target {
        return None;
    }
    let anchors: Vec<GridPos> = plan
        .farm_slots
        .iter()
        .copied()
        .filter(|slot| !has_building_at(world, controller.team, BuildingKind::Farm, *slot))
        .collect();
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::Farm,
        &anchors,
    )
}

/// A Storehouse near a discovered expansion: the authored expansion slot
/// itself must be explored first — scouting actually has to reach the
/// expansion before late-game logistics depend on it. Safe slots are home
/// ground and open once the production core exists.
fn place_storehouse(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    // Expansion candidates first — a discovered expansion is preferred — then
    // the safe home slots once the production core exists.
    let mut anchors: Vec<GridPos> = plan
        .expansion_storehouse_slots
        .iter()
        .copied()
        .filter(|slot| {
            !has_building_at(world, controller.team, BuildingKind::Storehouse, *slot)
                && slot_explored(world, controller.team, *slot)
        })
        .collect();
    if any_own_building(world, controller.team, BuildingKind::Barracks) {
        anchors.extend(plan.safe_storehouse_slots.iter().copied().filter(|slot| {
            !has_building_at(world, controller.team, BuildingKind::Storehouse, *slot)
        }));
    }
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::Storehouse,
        &anchors,
    )
}

/// Advance after the worker target is met, the Age-1 production core stands,
/// the Town Center queue is free and the shared catalogue cost is affordable.
fn attempt_age_two(
    world: &World,
    controller: &mut AiController,
    _plan: &AiMapPlan,
    _map: &GridMap,
    _claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let state = world
        .get_resource::<TeamEconomy>()?
        .0
        .get(&controller.team)?;
    if state.age != Age::Age1
        || state.age_up_started
        || (villager_ids(world, controller.team).len() as u32) < TARGET_WORKERS
        || !has_completed_building(world, controller.team, BuildingKind::Barracks)
        || !has_completed_building(world, controller.team, BuildingKind::ArcheryRange)
    {
        return None;
    }
    let (town_center_id, town_center_entity) = town_center(world, controller.team)?;
    if !queue_is_empty(world, town_center_entity)
        || !can_afford(world, controller.team, AGE_TWO_COST)
    {
        return None;
    }
    Some(PlannedCommand::plain(PlayerCommand::EnqueueAgeUp {
        issuer: controller.team,
        building: town_center_id,
    }))
}

/// Complete production: the Stable unlocks at Age 2.
fn place_stable(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    let age = world
        .get_resource::<TeamEconomy>()?
        .0
        .get(&controller.team)?
        .age;
    if age != Age::Age2 || any_own_building(world, controller.team, BuildingKind::Stable) {
        return None;
    }
    place_at_slot(
        world,
        controller,
        claimed,
        map,
        BuildingKind::Stable,
        &[plan.stable_anchor],
    )
}

/// Train the army through the existing producer queues: one round-robin
/// enqueue per decision against an empty queue, respecting unlock age,
/// affordability, and population headroom (producer compatibility comes
/// from the shared catalogue).
fn train_round_robin(
    world: &World,
    controller: &mut AiController,
    _plan: &AiMapPlan,
    _map: &GridMap,
    _claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    // Decision-side population gate: with zero free slots the enqueue would
    // be a guaranteed apply-time rejection, so leave the rotation cursor
    // untouched until headroom exists (mirrors `place_house`'s saturation).
    if population_cap(world, controller.team)
        .saturating_sub(population_used(world, controller.team))
        < 1
    {
        return None;
    }
    let team_age = world
        .get_resource::<TeamEconomy>()?
        .0
        .get(&controller.team)?
        .age;
    for offset in 0..ARMY_ROTATION.len() {
        let index = (controller.next_army_kind + offset) % ARMY_ROTATION.len();
        let kind = ARMY_ROTATION[index];
        if team_age < unit_spec(kind).required_age {
            continue;
        }
        let Some((producer_id, _)) = free_producer(world, controller.team, kind) else {
            continue;
        };
        if !can_afford(world, controller.team, unit_spec(kind).cost) {
            continue;
        }
        // The rotation cursor is a deferred commit: it moves only when the
        // enqueue is accepted, so a rejected command retries the same kind
        // instead of silently rotating past it.
        return Some(PlannedCommand::on_accept(
            PlayerCommand::EnqueueUnit {
                issuer: controller.team,
                building: producer_id,
                kind,
            },
            AiCommit::ArmyCursor((index + 1) % ARMY_ROTATION.len()),
        ));
    }
    None
}

/// Attack: once at least the threshold of military units is live, the
/// available (idle, unclaimed) ones march as one grouped AttackMove toward
/// the nearest walkable ground cell beside the remembered enemy Town Center
/// anchor (the anchor itself is a blocked footprint cell while the building
/// stands) — or, if none has ever been seen, the far end of the authored
/// route (enemy ground; scouting progress in `scout_route_index` is not
/// consumed). The target is always a ground cell, never an entity. Units
/// already holding a Move/Combat order are skipped, so an active push is
/// never reissued; losses simply drop the live count below the threshold
/// until ordinary production rebuilds the force and a later decision
/// regroups it — there is no persistent squad state to repair.
fn attack(
    world: &World,
    controller: &mut AiController,
    plan: &AiMapPlan,
    map: &GridMap,
    claimed: &mut Vec<UnitId>,
) -> Option<PlannedCommand> {
    if own_military_count(world, controller.team) < ATTACK_THRESHOLD {
        return None;
    }
    let attackers = available_military_ids(world, controller.team, claimed);
    if attackers.is_empty() {
        return None;
    }
    let target_cell = match controller.remembered_enemy_town_center {
        Some(remembered) => nearest_walkable_cell(map, remembered)?,
        None => *plan.scout_route.last()?,
    };
    claimed.extend_from_slice(&attackers);
    Some(PlannedCommand::plain(PlayerCommand::Units(UnitCommand {
        issuer: controller.team,
        units: attackers,
        kind: UnitCommandKind::AttackMove {
            target: map.cell_center(target_cell),
        },
    })))
}

/// The nearest walkable ground cell to `cell`: rings expand outward by
/// Chebyshev distance and each ring is visited row-major (dy outer, dx
/// inner), so the result is the nearest ring (Chebyshev) with a row-major
/// tie-break — not the Euclidean-nearest cell. A remembered footprint
/// anchor is itself blocked while the building stands; an AttackMove there
/// would reject `Unreachable` for every attacker.
fn nearest_walkable_cell(map: &GridMap, cell: GridPos) -> Option<GridPos> {
    for radius in 0i32..8 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if radius > 0 && dx.abs() != radius && dy.abs() != radius {
                    continue; // interior cells were covered by a smaller ring
                }
                let candidate = GridPos::new(cell.x + dx, cell.y + dy);
                if map.is_walkable(candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

// ---- Pure read helpers ------------------------------------------------------

/// Food/Wood/Gold split targets for a villager population of `total`; the
/// sum is clamped below `total` so an unallocated villager always remains
/// for building and scouting duty.
fn split_targets(total: u32) -> [(ResourceKind, u32); 3] {
    let mut targets = [
        (ResourceKind::Food, total / 2),
        (ResourceKind::Wood, total / 3),
        (ResourceKind::Gold, 1),
    ];
    let sum: u32 = targets.iter().map(|(_, target)| target).sum();
    if sum >= total {
        targets[2].1 = targets[2].1.saturating_sub(sum - total + 1);
    }
    targets
}

/// True while some kind is below its split target and still has source
/// capacity — i.e. an idle villager would be put to work, not surplus.
fn has_allocation_deficit(world: &World, team: TeamId) -> bool {
    let counts = task_kind_counts(world, team);
    let total = villager_ids(world, team).len() as u32;
    let capacity = source_capacity_by_kind(world, team);
    split_targets(total).iter().any(|(kind, target)| {
        let assigned = counts.get(kind).copied().unwrap_or(0);
        assigned < *target && capacity.get(kind).copied().unwrap_or(0) > assigned
    })
}

/// Stable-ID-sorted villagers of one team.
fn villager_ids(world: &World, team: TeamId) -> Vec<UnitId> {
    let mut ids: Vec<UnitId> = world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let unit = world.get::<Unit>(*entity)?;
                    (unit.team == team && unit.kind == UnitKind::Villager).then_some(*id)
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

/// Stable-ID-sorted combatants of one team holding no route and no combat
/// intent and not claimed earlier in this same decision.
fn available_military_ids(world: &World, team: TeamId, claimed: &[UnitId]) -> Vec<UnitId> {
    let mut ids: Vec<UnitId> = world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let unit = world.get::<Unit>(*entity)?;
                    if unit.team != team
                        || claimed.contains(id)
                        || unit_spec(unit.kind).combat.is_none()
                        || world.get::<MoveOrder>(*entity).is_some()
                        || world.get::<CombatOrder>(*entity).is_some()
                    {
                        return None;
                    }
                    Some(*id)
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

/// Lowest stable-ID combatant with no route and no combat intent.
fn idle_military_id(world: &World, team: TeamId, claimed: &[UnitId]) -> Option<UnitId> {
    available_military_ids(world, team, claimed)
        .into_iter()
        .next()
}

/// Live combatant count of one team — the attack threshold's population.
fn own_military_count(world: &World, team: TeamId) -> u32 {
    world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter(|(_, entity)| {
                    world.get::<Unit>(**entity).is_some_and(|unit| {
                        unit.team == team && unit_spec(unit.kind).combat.is_some()
                    })
                })
                .count() as u32
        })
        .unwrap_or(0)
}

/// Currently visible enemy units within the base radius of `home`, as
/// (stable ID, current position). Hidden enemies are never enumerated —
/// their positions cannot influence any decision.
fn visible_threats(world: &World, team: TeamId, home: Vec2) -> Vec<(UnitId, Vec2)> {
    let mut threats: Vec<(UnitId, Vec2)> = world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let unit = world.get::<Unit>(*entity)?;
                    if unit.team == team {
                        return None;
                    }
                    let position = world.get::<SimPosition>(*entity)?.current;
                    let cell = GridPos::new(position.x.floor() as i32, position.y.floor() as i32);
                    if !visible_to(world, team, cell)
                        || position.distance_squared(home) > DEFENSE_RADIUS * DEFENSE_RADIUS
                    {
                        return None;
                    }
                    Some((*id, position))
                })
                .collect()
        })
        .unwrap_or_default();
    threats.sort_unstable_by_key(|(id, _)| *id);
    threats
}

/// The completed Town Center's footprint anchor as a world-space center.
fn town_center_cell_center(world: &World, entity: Entity) -> Option<Vec2> {
    let anchor = world.get::<Footprint>(entity)?.anchor;
    Some(Vec2::new(anchor.x as f32 + 2.0, anchor.y as f32 + 2.0))
}

/// One gather candidate: an explored standalone source or an own completed
/// Farm. Enemy Farms are enemy buildings — never candidates, visible or not.
struct KnownSource {
    id: ResourceId,
    kind: ResourceKind,
    entity: Entity,
    farm: bool,
    center: Vec2,
}

fn known_sources(world: &World, team: TeamId) -> Vec<KnownSource> {
    let mut sources: Vec<KnownSource> = world
        .get_resource::<ResourceIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let source = world.get::<ResourceSource>(*entity)?;
                    let footprint = world.get::<Footprint>(*entity)?;
                    let center = Vec2::new(
                        footprint.anchor.x as f32 + f32::from(footprint.width) / 2.0,
                        footprint.anchor.y as f32 + f32::from(footprint.height) / 2.0,
                    );
                    if let Some(building) = world.get::<Building>(*entity) {
                        (building.team == team).then_some(KnownSource {
                            id: *id,
                            kind: source.kind,
                            entity: *entity,
                            farm: true,
                            center,
                        })
                    } else {
                        explored_by(world, team, *footprint).then_some(KnownSource {
                            id: *id,
                            kind: source.kind,
                            entity: *entity,
                            farm: false,
                            center,
                        })
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    sources.sort_by_key(|source| source.id);
    sources
}

/// A Farm serves exactly one assigned worker; a standalone source hosts a
/// small fixed number.
fn source_has_capacity(world: &World, team: TeamId, source: &KnownSource) -> bool {
    if source.farm {
        world
            .get::<ResourceSource>(source.entity)
            .is_some_and(|state| state.assigned_worker.is_none())
    } else {
        workers_tasked_to(world, team, source.id) < WORKERS_PER_STANDALONE_SOURCE
    }
}

/// Own workers currently tasked to a specific source. Enemy workers are never
/// counted: their task state is hidden enemy state, and hidden enemy state
/// must not influence any decision.
fn workers_tasked_to(world: &World, team: TeamId, source: ResourceId) -> u32 {
    world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter(|(_, entity)| {
                    world
                        .get::<Unit>(**entity)
                        .is_some_and(|unit| unit.team == team)
                        && task_source(world, **entity) == Some(source)
                })
                .count() as u32
        })
        .unwrap_or(0)
}

fn task_source(world: &World, entity: Entity) -> Option<ResourceId> {
    match world.get::<WorkerTask>(entity)? {
        WorkerTask::ToSource { source, .. }
        | WorkerTask::Gathering { source }
        | WorkerTask::ToDropoff { source, .. } => Some(*source),
        _ => None,
    }
}

fn task_kind_counts(world: &World, team: TeamId) -> HashMap<ResourceKind, u32> {
    let mut counts = HashMap::new();
    let Some(index) = world.get_resource::<UnitIndex>() else {
        return counts;
    };
    for (_, entity) in index.iter() {
        let Some(unit) = world.get::<Unit>(*entity) else {
            continue;
        };
        if unit.team != team {
            continue;
        }
        let Some(source) = task_source(world, *entity) else {
            continue;
        };
        let Some(source_entity) = world
            .get_resource::<ResourceIndex>()
            .and_then(|index| index.entity(source))
        else {
            continue;
        };
        let Some(state) = world.get::<ResourceSource>(source_entity) else {
            continue;
        };
        *counts.entry(state.kind).or_default() += 1;
    }
    counts
}

/// Total gather capacity per kind from the known sources.
fn source_capacity_by_kind(world: &World, team: TeamId) -> HashMap<ResourceKind, u32> {
    let mut capacity = HashMap::new();
    for source in known_sources(world, team) {
        let per = if source.farm {
            1
        } else {
            WORKERS_PER_STANDALONE_SOURCE
        };
        *capacity.entry(source.kind).or_default() += per;
    }
    capacity
}

/// The team's lowest-ID completed Town Center.
fn town_center(world: &World, team: TeamId) -> Option<(BuildingId, Entity)> {
    let mut centers: Vec<(BuildingId, Entity)> = world
        .get_resource::<BuildingIndex>()?
        .iter()
        .filter_map(|(id, entity)| {
            let building = world.get::<Building>(*entity)?;
            (building.team == team
                && building.kind == BuildingKind::TownCenter
                && building.construction.complete)
                .then_some((*id, *entity))
        })
        .collect();
    centers.sort_by_key(|(id, _)| *id);
    centers.into_iter().next()
}

/// Lowest-ID completed own producer of `kind` with an empty queue.
fn free_producer(world: &World, team: TeamId, kind: UnitKind) -> Option<(BuildingId, Entity)> {
    let mut producers: Vec<(BuildingId, Entity)> = world
        .get_resource::<BuildingIndex>()?
        .iter()
        .filter_map(|(id, entity)| {
            let building = world.get::<Building>(*entity)?;
            (building.team == team
                && building.construction.complete
                && produces(building.kind, ProductionKind::Unit(kind)))
            .then_some((*id, *entity))
        })
        .collect();
    producers.sort_by_key(|(id, _)| *id);
    producers
        .into_iter()
        .find(|(_, entity)| queue_is_empty(world, *entity))
}

fn queue_is_empty(world: &World, entity: Entity) -> bool {
    world
        .get::<ProductionQueue>(entity)
        .is_none_or(|queue| queue.jobs.is_empty())
}

fn can_afford(world: &World, team: TeamId, cost: Cost) -> bool {
    world
        .get_resource::<TeamEconomy>()
        .and_then(|economy| economy.0.get(&team))
        .is_some_and(|state| {
            state.stockpile.food >= cost.food
                && state.stockpile.wood >= cost.wood
                && state.stockpile.gold >= cost.gold
        })
}

/// An own building of `kind` already stands (complete or not) at `anchor`:
/// the authored slot is satisfied or under construction.
fn has_building_at(world: &World, team: TeamId, kind: BuildingKind, anchor: GridPos) -> bool {
    world.get_resource::<BuildingIndex>().is_some_and(|index| {
        index.iter().any(|(_, entity)| {
            world.get::<Building>(*entity).is_some_and(|building| {
                building.team == team
                    && building.kind == kind
                    && world
                        .get::<Footprint>(*entity)
                        .is_some_and(|footprint| footprint.anchor == anchor)
            })
        })
    })
}

fn any_own_building(world: &World, team: TeamId, kind: BuildingKind) -> bool {
    world.get_resource::<BuildingIndex>().is_some_and(|index| {
        index.iter().any(|(_, entity)| {
            world
                .get::<Building>(*entity)
                .is_some_and(|building| building.team == team && building.kind == kind)
        })
    })
}

fn has_completed_building(world: &World, team: TeamId, kind: BuildingKind) -> bool {
    world.get_resource::<BuildingIndex>().is_some_and(|index| {
        index.iter().any(|(_, entity)| {
            world.get::<Building>(*entity).is_some_and(|building| {
                building.team == team && building.kind == kind && building.construction.complete
            })
        })
    })
}

/// True when every cell of a 2×2 authored slot has been explored — the
/// "scouting actually discovered it" gate.
fn slot_explored(world: &World, team: TeamId, anchor: GridPos) -> bool {
    Footprint::new(anchor, 2, 2)
        .cells()
        .iter()
        .all(|cell| explored_by(world, team, *cell))
}

/// Shared placement tail: a real idle builder (never claimed earlier in this
/// decision), catalogue affordability, then the first candidate anchor that
/// survives the real `validate_placement` for that builder — a slot already
/// occupied by something observable no longer starves every later candidate
/// of a retry each decision, and an anchor that rejected `Occupied` at apply
/// time is skipped until restart. The apply-time validator remains the
/// authority; the command carries a deferred commit so an apply reject is
/// remembered instead of retried forever.
fn place_at_slot(
    world: &World,
    controller: &AiController,
    claimed: &mut Vec<UnitId>,
    map: &GridMap,
    kind: BuildingKind,
    anchors: &[GridPos],
) -> Option<PlannedCommand> {
    let builder = idle_worker_ids(world, controller.team)
        .into_iter()
        .find(|id| !claimed.contains(id))?;
    if !can_afford(world, controller.team, building_spec(kind).cost) {
        return None;
    }
    let anchor = anchors.iter().copied().find(|anchor| {
        !controller.blocked_anchors.contains(anchor)
            && validate_placement(world, map, controller.team, builder, kind, *anchor).is_ok()
    })?;
    claimed.push(builder);
    Some(PlannedCommand::on_accept(
        PlayerCommand::PlaceBuilding {
            issuer: controller.team,
            builder,
            kind,
            anchor,
        },
        AiCommit::PlaceAnchor { anchor },
    ))
}

#[cfg(test)]
mod tests;
