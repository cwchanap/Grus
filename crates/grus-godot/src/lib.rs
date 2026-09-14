use std::collections::HashSet;

use bevy::prelude::*;
use godot::builtin::{GString, PackedInt32Array, VarDictionary, Vector2};
use godot::classes::{Engine, INode, Node, Node3D, SceneTree};
use godot::prelude::*;
use godot_bevy::BevyApp;
use godot_bevy::prelude::*;
use grus_sim::catalog::{building_spec, unit_spec};
use grus_sim::{
    AGE_TWO_COST, AGE_TWO_SECONDS, Age, Building, BuildingId, BuildingIndex, BuildingKind,
    CommandResult, Footprint, GridMap, GridPos, IdAllocator, LastRouteReject, MapFixture,
    MoveOrder, PlayerCommand, ProductionJob, ProductionKind, ProductionQueue, RallyPoint,
    RejectReason, ResourceId, ResourceIndex, ResourceSource, SIM_STEP_SECONDS, SimPosition,
    TeamEconomy, TeamId, Unit, UnitCommand, UnitCommandKind, UnitId, UnitIndex, UnitKind,
    WorkerTask, apply_player_command, gather_rate_for_age, population_cap, population_used,
    produces, seed_skirmish, spawn_unit, step_construction, step_economy, step_movement,
    step_production, validate_placement,
};

#[cfg(feature = "e2e")]
mod e2e;

#[derive(Default, Resource)]
struct PendingCommands(Vec<PlayerCommand>);

#[derive(Resource)]
struct CommandFeedback {
    revision: u64,
    text: String,
    last_reject_code: Option<RejectReason>,
}

impl Default for CommandFeedback {
    fn default() -> Self {
        Self {
            revision: 0,
            text: "Ready".to_string(),
            last_reject_code: None,
        }
    }
}

#[derive(Component)]
struct ViewMetaInitialized;

#[derive(Component)]
struct ResourceMetaInitialized;

#[derive(Component)]
struct GameplayViewRequested;

#[derive(GodotClass)]
#[class(base=Node)]
struct GrusBridgeNode {
    base: Base<Node>,
}

#[godot_api]
impl INode for GrusBridgeNode {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl GrusBridgeNode {
    #[func]
    fn move_units(&self, packed_ids: PackedInt32Array, target: Vector2) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }

        queue_command(PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units,
            kind: UnitCommandKind::Move {
                target: Vec2::new(target.x, target.y),
            },
        }))
    }

    #[func]
    fn stop_units(&self, packed_ids: PackedInt32Array) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }

        queue_command(PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units,
            kind: UnitCommandKind::Stop,
        }))
    }

    #[func]
    fn gather(&self, packed_workers: PackedInt32Array, source_id: i32) -> bool {
        let workers = decode_unit_ids(&packed_workers);
        if workers.is_empty() {
            return false;
        }
        let Ok(source) = u32::try_from(source_id) else {
            return false;
        };
        queue_command(PlayerCommand::Gather {
            issuer: TeamId(1),
            workers,
            source: ResourceId(source),
        })
    }

    #[func]
    fn place_building(&self, builder_id: i32, kind: GString, anchor_x: i32, anchor_y: i32) -> bool {
        let Some(kind) = parse_building_kind(&kind) else {
            return false;
        };
        let Ok(builder) = u32::try_from(builder_id) else {
            return false;
        };
        queue_command(PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(builder),
            kind,
            anchor: GridPos::new(anchor_x, anchor_y),
        })
    }

    #[func]
    fn resume_construction(&self, builder_id: i32, building_id: i32) -> bool {
        let Ok(builder) = u32::try_from(builder_id) else {
            return false;
        };
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(PlayerCommand::ResumeConstruction {
            issuer: TeamId(1),
            builder: UnitId(builder),
            building: BuildingId(building),
        })
    }

    #[func]
    fn enqueue_unit(&self, building_id: i32, kind: GString) -> bool {
        let Some(kind) = parse_unit_kind(&kind) else {
            return false;
        };
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(PlayerCommand::EnqueueUnit {
            issuer: TeamId(1),
            building: BuildingId(building),
            kind,
        })
    }

    #[func]
    fn enqueue_age_up(&self, building_id: i32) -> bool {
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(PlayerCommand::EnqueueAgeUp {
            issuer: TeamId(1),
            building: BuildingId(building),
        })
    }

    #[func]
    fn set_rally(&self, building_id: i32, target_x: i32, target_y: i32) -> bool {
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(PlayerCommand::SetRally {
            issuer: TeamId(1),
            building: BuildingId(building),
            target: GridPos::new(target_x, target_y),
        })
    }

    #[func]
    fn benchmark_move_all(&self) -> bool {
        let fixture = MapFixture::battlefield();
        let player_queued = queue_command(PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: (1_u32..=100).map(UnitId).collect(),
            kind: UnitCommandKind::Move {
                target: fixture.right_spawn,
            },
        }));
        let enemy_queued = queue_command(PlayerCommand::Units(UnitCommand {
            issuer: TeamId(2),
            units: (101_u32..=200).map(UnitId).collect(),
            kind: UnitCommandKind::Move {
                target: fixture.left_spawn,
            },
        }));

        player_queued && enemy_queued
    }

    #[func]
    fn reset_fixture(&self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };

        reset_fixture_world(app.world_mut());
        true
    }

    #[func]
    fn reset_benchmark_fixture(&self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };

        reset_benchmark_world(app.world_mut());
        true
    }

    /// Virtual-time multiplier for headless gate runs. Never touches `max_delta`.
    #[func]
    fn set_sim_speed(&self, relative_speed: f64) -> bool {
        if !relative_speed.is_finite() || relative_speed <= 0.0 {
            return false;
        }
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed_f64(relative_speed);
        true
    }

    #[func]
    fn command_feedback_revision(&self) -> i64 {
        with_app(|app| {
            app.world()
                .get_resource::<CommandFeedback>()
                .map(|feedback| feedback.revision as i64)
                .unwrap_or_default()
        })
        .unwrap_or_default()
    }

    #[func]
    fn command_feedback(&self) -> GString {
        with_app(|app| {
            app.world()
                .get_resource::<CommandFeedback>()
                .map(|feedback| GString::from(feedback.text.as_str()))
                .unwrap_or_else(|| GString::from("Bridge unavailable"))
        })
        .unwrap_or_else(|| GString::from("Bridge unavailable"))
    }

    /// Typed code of the latest rejection, `-1` when nothing was rejected.
    /// Discriminant order follows `RejectReason` declaration order.
    #[func]
    fn last_reject_code(&self) -> i32 {
        with_app(|app| {
            app.world()
                .get_resource::<CommandFeedback>()
                .and_then(|feedback| feedback.last_reject_code)
                .map(|reason| reason as i32)
                .unwrap_or(-1)
        })
        .unwrap_or(-1)
    }

    /// Team 1 economy state: stockpile, age, population, idle villagers, the
    /// current gather rate, and the latest rejection code. `age` is 1 or 2;
    /// `last_reject_code` follows `RejectReason` discriminant order (-1 =
    /// none); `gather_rate` is the team's current resources/second.
    #[func]
    fn economy_snapshot(&self) -> VarDictionary {
        with_app(|app| {
            let world = app.world();
            let mut dict = VarDictionary::new();
            let state = world
                .get_resource::<TeamEconomy>()
                .and_then(|economy| economy.0.get(&TeamId(1)));
            let (food, wood, gold, age) = match state {
                Some(state) => (
                    i64::from(state.stockpile.food),
                    i64::from(state.stockpile.wood),
                    i64::from(state.stockpile.gold),
                    match state.age {
                        Age::Age1 => 1,
                        Age::Age2 => 2,
                    },
                ),
                None => (0, 0, 0, 1),
            };
            dict.set("food", food);
            dict.set("wood", wood);
            dict.set("gold", gold);
            dict.set("age", age);
            dict.set(
                "population_used",
                i64::from(population_used(world, TeamId(1))),
            );
            dict.set(
                "population_cap",
                i64::from(population_cap(world, TeamId(1))),
            );
            dict.set(
                "gather_rate",
                f64::from(gather_rate_for_age(
                    state.map(|state| state.age).unwrap_or(Age::Age1),
                )),
            );
            let idle = idle_worker_ids(world, TeamId(1));
            dict.set("idle_workers", idle.len() as i64);
            dict.set("idle_worker_ids", &PackedInt32Array::from_iter(idle));
            dict.set(
                "last_reject_code",
                world
                    .get_resource::<CommandFeedback>()
                    .and_then(|feedback| feedback.last_reject_code)
                    .map(|reason| reason as i32)
                    .unwrap_or(-1),
            );
            dict
        })
        .unwrap_or_default()
    }

    /// One building's state by stable id: completion, construction and queue
    /// progress (0..1), queue head label, blocked code (0 = none), and rally
    /// cell (-1/-1 when unset). Empty dict when the building does not exist.
    #[func]
    fn building_snapshot(&self, building_id: i32) -> VarDictionary {
        with_app(|app| {
            let world = app.world();
            let mut dict = VarDictionary::new();
            let Ok(raw_id) = u32::try_from(building_id) else {
                return dict;
            };
            let Some(entity) = world
                .get_resource::<BuildingIndex>()
                .and_then(|index| index.entity(BuildingId(raw_id)))
            else {
                return dict;
            };
            let Some(building) = world.get::<Building>(entity) else {
                return dict;
            };

            dict.set("id", i64::from(building.id.0));
            dict.set("kind", &debug_variant(building.kind));
            dict.set("team_id", i64::from(building.team.0));
            dict.set("complete", building.construction.complete);
            let spec = building_spec(building.kind);
            let construction_progress = if building.construction.complete {
                1.0
            } else if spec.build_seconds == 0 {
                0.0
            } else {
                building.construction.progress_seconds / spec.build_seconds as f32
            };
            dict.set(
                "construction_progress",
                f64::from(construction_progress.clamp(0.0, 1.0)),
            );

            let queue = world.get::<ProductionQueue>(entity);
            match queue.and_then(|queue| queue.jobs.front()) {
                Some(job) => {
                    dict.set(
                        "queue_label",
                        &GString::from(queue_head_label(job).as_str()),
                    );
                    let required = job_seconds(job);
                    let progress = queue.map_or(0.0, |queue| queue.progress_seconds);
                    dict.set(
                        "queue_progress",
                        f64::from(if required > 0.0 {
                            (progress / required).clamp(0.0, 1.0)
                        } else {
                            0.0
                        }),
                    );
                }
                None => {
                    // Seeded producers have no queue component until the first
                    // accepted enqueue heals it; report an empty queue.
                    dict.set("queue_label", &GString::from(""));
                    dict.set("queue_progress", 0.0_f64);
                }
            }
            dict.set(
                "blocked_reason",
                queue
                    .and_then(|queue| queue.blocked)
                    .map(|reason| reason as i32)
                    .unwrap_or(0),
            );
            match world.get::<RallyPoint>(entity) {
                Some(rally) => {
                    dict.set("rally_x", i64::from(rally.0.x));
                    dict.set("rally_y", i64::from(rally.0.y));
                }
                None => {
                    dict.set("rally_x", -1_i64);
                    dict.set("rally_y", -1_i64);
                }
            }
            dict
        })
        .unwrap_or_default()
    }

    /// Read-only placement validation through the authoritative
    /// `validate_placement`; mutates nothing. Reports the spec footprint plus
    /// validity and reject code (-1 = valid).
    #[func]
    fn placement_preview(
        &self,
        builder_id: i32,
        kind: GString,
        anchor_x: i32,
        anchor_y: i32,
    ) -> VarDictionary {
        with_app(|app| {
            let mut dict = VarDictionary::new();
            let Some(kind) = parse_building_kind(&kind) else {
                return dict;
            };
            let world = app.world();
            let spec = building_spec(kind);
            dict.set("anchor_x", i64::from(anchor_x));
            dict.set("anchor_y", i64::from(anchor_y));
            dict.set("width", i64::from(spec.width));
            dict.set("height", i64::from(spec.height));

            let outcome = u32::try_from(builder_id)
                .map(UnitId)
                .map_err(|_| RejectReason::UnknownUnit)
                .and_then(|builder| {
                    let map = world
                        .get_resource::<GridMap>()
                        .ok_or(RejectReason::UnknownUnit)?;
                    validate_placement(
                        world,
                        map,
                        TeamId(1),
                        builder,
                        kind,
                        GridPos::new(anchor_x, anchor_y),
                    )
                    .map(|_| ())
                });
            match outcome {
                Ok(()) => {
                    dict.set("valid", true);
                    dict.set("reject_code", -1_i32);
                }
                Err(reason) => {
                    dict.set("valid", false);
                    dict.set("reject_code", reason as i32);
                }
            }
            dict
        })
        .unwrap_or_default()
    }

    /// Catalogue action data computed from the Rust catalogue, so GDScript
    /// never hardcodes gameplay costs or unlocks. Keys:
    /// - "units": {kind -> {food, wood, gold, seconds, producer, age}} —
    ///   train cost, train seconds, the producing building kind name, and the
    ///   required age (1|2).
    /// - "buildings": {kind -> {food, wood, gold, seconds, width, height,
    ///   age, population}} — build cost, build seconds, footprint, required
    ///   age, and population capacity contribution.
    /// - "age_up": {food, wood, gold, seconds} for the one-time Age 2 job.
    #[func]
    fn catalogue_snapshot(&self) -> VarDictionary {
        let mut units = VarDictionary::new();
        for kind in UnitKind::ALL {
            let spec = unit_spec(kind);
            let mut entry = VarDictionary::new();
            entry.set("food", i64::from(spec.cost.food));
            entry.set("wood", i64::from(spec.cost.wood));
            entry.set("gold", i64::from(spec.cost.gold));
            entry.set("seconds", i64::from(spec.train_seconds));
            entry.set("producer", &producer_kind_name(kind));
            entry.set("age", age_number(spec.required_age));
            units.set(&kind_name(kind), &entry.to_variant());
        }
        let mut buildings = VarDictionary::new();
        for kind in BuildingKind::ALL {
            let spec = building_spec(kind);
            let mut entry = VarDictionary::new();
            entry.set("food", i64::from(spec.cost.food));
            entry.set("wood", i64::from(spec.cost.wood));
            entry.set("gold", i64::from(spec.cost.gold));
            entry.set("seconds", i64::from(spec.build_seconds));
            entry.set("width", i64::from(spec.width));
            entry.set("height", i64::from(spec.height));
            entry.set("age", age_number(spec.required_age));
            entry.set("population", i64::from(spec.population_capacity));
            buildings.set(&kind_name(kind), &entry.to_variant());
        }
        let mut age_up = VarDictionary::new();
        age_up.set("food", i64::from(AGE_TWO_COST.food));
        age_up.set("wood", i64::from(AGE_TWO_COST.wood));
        age_up.set("gold", i64::from(AGE_TWO_COST.gold));
        age_up.set("seconds", i64::from(AGE_TWO_SECONDS));
        let mut catalogue = VarDictionary::new();
        catalogue.set("units", &units.to_variant());
        catalogue.set("buildings", &buildings.to_variant());
        catalogue.set("age_up", &age_up.to_variant());
        catalogue
    }
}

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotAssetsPlugin)
        .add_plugins(GodotTransformSyncPlugin::default())
        .add_plugins(GodotPackedScenePlugin)
        .insert_resource(Time::<Fixed>::from_seconds(f64::from(SIM_STEP_SECONDS)))
        .init_resource::<PendingCommands>()
        .init_resource::<CommandFeedback>()
        .init_resource::<LastRouteReject>()
        .add_systems(Startup, setup_fixture)
        .add_systems(
            Update,
            (
                attach_missing_gameplay_views,
                initialize_view_metadata,
                stamp_late_resource_metadata,
                sync_interpolated_unit_transforms,
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                apply_pending_commands,
                advance_movement,
                advance_economy,
                advance_construction,
                advance_production,
                drain_route_reject_feedback,
            )
                .chain(),
        );
    #[cfg(feature = "e2e")]
    app.add_plugins(bevy_e2e::BevyE2EPlugin);
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::GrusE2ePlugin);
}

fn decode_unit_ids(packed_ids: &PackedInt32Array) -> Vec<UnitId> {
    packed_ids
        .as_slice()
        .iter()
        .filter_map(|id| u32::try_from(*id).ok())
        .filter(|id| *id != 0)
        .map(UnitId)
        .collect()
}

fn parse_unit_kind(name: &GString) -> Option<UnitKind> {
    match name.to_string().as_str() {
        "Villager" => Some(UnitKind::Villager),
        "Spearman" => Some(UnitKind::Spearman),
        "Archer" => Some(UnitKind::Archer),
        "Cavalry" => Some(UnitKind::Cavalry),
        _ => None,
    }
}

fn parse_building_kind(name: &GString) -> Option<BuildingKind> {
    match name.to_string().as_str() {
        "House" => Some(BuildingKind::House),
        "Storehouse" => Some(BuildingKind::Storehouse),
        "Farm" => Some(BuildingKind::Farm),
        "Barracks" => Some(BuildingKind::Barracks),
        "ArcheryRange" => Some(BuildingKind::ArcheryRange),
        "Stable" => Some(BuildingKind::Stable),
        // Town Centers are seeded, never placed.
        _ => None,
    }
}

fn debug_variant(value: impl std::fmt::Debug) -> Variant {
    GString::from(format!("{value:?}").as_str()).to_variant()
}

fn kind_name(value: impl std::fmt::Debug) -> GString {
    GString::from(format!("{value:?}").as_str())
}

fn age_number(age: Age) -> i64 {
    match age {
        Age::Age1 => 1,
        Age::Age2 => 2,
    }
}

/// The building kind that trains the given unit, per the fixed producer
/// compatibility table (Town Center trains villagers, Barracks spearmen,
/// Archery Range archers, Stable cavalry).
fn producer_kind_name(kind: UnitKind) -> GString {
    for building in BuildingKind::ALL {
        if produces(building, ProductionKind::Unit(kind)) {
            return kind_name(building);
        }
    }
    GString::from("")
}

fn bevy_app_singleton() -> Option<Gd<BevyApp>> {
    Engine::singleton()
        .get_main_loop()?
        .try_cast::<SceneTree>()
        .ok()?
        .get_root()?
        .try_get_node_as::<BevyApp>("BevyAppSingleton")
}

fn queue_command(command: PlayerCommand) -> bool {
    let Some(mut app_node) = bevy_app_singleton() else {
        return false;
    };
    let mut app_node = app_node.bind_mut();
    let Some(app) = app_node.get_app_mut() else {
        return false;
    };
    let Some(mut pending) = app.world_mut().get_resource_mut::<PendingCommands>() else {
        return false;
    };
    pending.0.push(command);
    true
}

fn with_app<T>(read: impl FnOnce(&App) -> T) -> Option<T> {
    let app_node = bevy_app_singleton()?;
    let app_node = app_node.bind();
    app_node.get_app().map(read)
}

fn setup_fixture(world: &mut World) {
    // A bridge reset (e.g. the benchmark fixture) can run before the first
    // fixed tick; its fresh world must not be double-seeded.
    if world.get_resource::<UnitIndex>().is_some() {
        return;
    }
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    seed_skirmish(world, &mut map, &fixture);
    world.insert_resource(map);
}

#[allow(clippy::type_complexity)]
fn attach_missing_gameplay_views(
    mut commands: Commands,
    units: Query<(Entity, &SimPosition), (With<Unit>, Without<GameplayViewRequested>)>,
    farm_buildings: Query<
        (Entity, &Footprint),
        (
            With<Building>,
            With<ResourceSource>,
            Without<GameplayViewRequested>,
        ),
    >,
    plain_buildings: Query<
        (Entity, &Footprint),
        (
            With<Building>,
            Without<ResourceSource>,
            Without<GameplayViewRequested>,
        ),
    >,
    standalone_resources: Query<
        (Entity, &Footprint),
        (
            With<ResourceSource>,
            Without<Building>,
            Without<GameplayViewRequested>,
        ),
    >,
    map: Res<GridMap>,
) {
    for (entity, position) in &units {
        attach_unit_view(
            &mut commands,
            entity,
            Transform::from_xyz(position.current.x, 0.0, position.current.y),
        );
    }
    for (entity, footprint) in &farm_buildings {
        attach_view(
            &mut commands,
            entity,
            building_view_transform(footprint),
            "res://scenes/building_view.tscn",
        );
    }
    for (entity, footprint) in &plain_buildings {
        attach_view(
            &mut commands,
            entity,
            building_view_transform(footprint),
            "res://scenes/building_view.tscn",
        );
    }
    for (entity, footprint) in &standalone_resources {
        let center = map.cell_center(footprint.anchor);
        attach_view(
            &mut commands,
            entity,
            Transform::from_xyz(center.x, 0.0, center.y),
            "res://scenes/resource_view.tscn",
        );
    }
}

/// The building view's unit box is scaled by the footprint so the rendered
/// building matches the blocked cells and the placement preview exactly.
/// `Footprint::anchor` is the top-left cell, so the transform centers on
/// anchor + half the footprint in each axis.
fn building_view_transform(footprint: &Footprint) -> Transform {
    Transform::from_xyz(
        footprint.anchor.x as f32 + f32::from(footprint.width) / 2.0,
        0.0,
        footprint.anchor.y as f32 + f32::from(footprint.height) / 2.0,
    )
    .with_scale(Vec3::new(
        f32::from(footprint.width),
        1.0,
        f32::from(footprint.height),
    ))
}

fn attach_view(commands: &mut Commands, entity: Entity, transform: Transform, path: &str) {
    commands.entity(entity).insert((
        transform,
        TransformSyncMetadata::default(),
        Node3DMarker,
        GodotScene::from_path(path),
        GameplayViewRequested,
    ));
}

/// Unit views opt out of godot-bevy's stock transform sync (no
/// `TransformSyncMetadata`/`Node3DMarker`): under 0.12 the stock path copies
/// Bevy→Godot only once per physics tick in FixedLast, which would fight the
/// per-render-frame interpolated writes in `sync_interpolated_unit_transforms`.
/// Grus is the single writer for unit view transforms. `Transform` stays —
/// `GodotScene` instantiation reads it for initial placement. Buildings and
/// resources are static and keep the stock sync path via `attach_view`.
fn attach_unit_view(commands: &mut Commands, entity: Entity, transform: Transform) {
    commands.entity(entity).insert((
        transform,
        GodotScene::from_path("res://scenes/unit_view.tscn"),
        GameplayViewRequested,
    ));
}

fn initialize_view_metadata(
    mut commands: Commands,
    units: Query<(Entity, &Unit, &GodotNodeHandle), Without<ViewMetaInitialized>>,
    buildings: Query<
        (Entity, &Building, Option<&ResourceSource>, &GodotNodeHandle),
        Without<ViewMetaInitialized>,
    >,
    resources: Query<(Entity, &ResourceSource, &GodotNodeHandle), Without<ViewMetaInitialized>>,
    mut godot: GodotAccess,
) {
    for (entity, unit, handle) in &units {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        node.set_meta("unit_id", &i64::from(unit.id.0).to_variant());
        node.set_meta("team_id", &i64::from(unit.team.0).to_variant());
        node.set_meta("unit_kind", &debug_variant(unit.kind));
        commands.entity(entity).insert(ViewMetaInitialized);
    }
    for (entity, building, source, handle) in &buildings {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        node.set_meta("building_id", &i64::from(building.id.0).to_variant());
        node.set_meta("building_kind", &debug_variant(building.kind));
        node.set_meta("team_id", &i64::from(building.team.0).to_variant());
        // A completed Farm carries both its building identity and its
        // renewable Food source on the same entity.
        if let Some(source) = source {
            node.set_meta("resource_id", &i64::from(source.id.0).to_variant());
            node.set_meta("resource_kind", &debug_variant(source.kind));
            commands.entity(entity).insert(ResourceMetaInitialized);
        }
        commands.entity(entity).insert(ViewMetaInitialized);
    }
    for (entity, source, handle) in &resources {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        node.set_meta("resource_id", &i64::from(source.id.0).to_variant());
        node.set_meta("resource_kind", &debug_variant(source.kind));
        commands
            .entity(entity)
            .insert((ViewMetaInitialized, ResourceMetaInitialized));
    }
}

/// A placed Farm only gains `ResourceSource` at construction completion,
/// after its one-shot view metadata ran — so its view must be re-stamped
/// with resource metadata when the source appears.
fn stamp_late_resource_metadata(
    mut commands: Commands,
    sources: Query<(Entity, &ResourceSource, &GodotNodeHandle), Without<ResourceMetaInitialized>>,
    mut godot: GodotAccess,
) {
    for (entity, source, handle) in &sources {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        node.set_meta("resource_id", &i64::from(source.id.0).to_variant());
        node.set_meta("resource_kind", &debug_variant(source.kind));
        commands.entity(entity).insert(ResourceMetaInitialized);
    }
}

fn reset_fixture_world(world: &mut World) {
    clear_gameplay_world(world);
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    seed_skirmish(world, &mut map, &fixture);
    world.insert_resource(map);
}

fn reset_benchmark_world(world: &mut World) {
    clear_gameplay_world(world);
    let fixture = MapFixture::battlefield();
    world.insert_resource(fixture.map.clone());
    for spawn in fixture.units_200() {
        spawn_unit(
            world,
            spawn.id,
            spawn.team,
            spawn.position,
            UnitKind::Villager,
            12.0,
        );
    }
}

fn clear_gameplay_world(world: &mut World) {
    let mut entities = HashSet::new();
    {
        let mut query = world.query_filtered::<Entity, With<Unit>>();
        entities.extend(query.iter(world));
    }
    {
        let mut query = world.query_filtered::<Entity, With<Building>>();
        entities.extend(query.iter(world));
    }
    {
        let mut query = world.query_filtered::<Entity, With<ResourceSource>>();
        entities.extend(query.iter(world));
    }
    for entity in entities {
        let _ = world.despawn(entity);
    }

    world.remove_resource::<UnitIndex>();
    world.remove_resource::<BuildingIndex>();
    world.remove_resource::<ResourceIndex>();
    world.remove_resource::<TeamEconomy>();
    world.remove_resource::<IdAllocator>();
    if let Some(mut pending) = world.get_resource_mut::<PendingCommands>() {
        pending.0.clear();
    }
    if let Some(mut feedback) = world.get_resource_mut::<CommandFeedback>() {
        feedback.revision = feedback.revision.wrapping_add(1);
        feedback.text = "Ready".to_string();
        feedback.last_reject_code = None;
    }
    if let Some(mut route_reject) = world.get_resource_mut::<LastRouteReject>() {
        route_reject.0 = None;
    }
}

fn idle_worker_ids(world: &World, team: TeamId) -> Vec<i32> {
    let mut ids: Vec<i32> = world
        .get_resource::<UnitIndex>()
        .map(|index| {
            index
                .iter()
                .filter_map(|(id, entity)| {
                    let unit = world.get::<Unit>(*entity)?;
                    if unit.team != team || unit.kind != UnitKind::Villager {
                        return None;
                    }
                    if world.get::<WorkerTask>(*entity) != Some(&WorkerTask::Idle) {
                        return None;
                    }
                    // A villager with a route (e.g. just rallied or moved) is
                    // traveling, not idle.
                    if world.get::<MoveOrder>(*entity).is_some() {
                        return None;
                    }
                    i32::try_from(id.0).ok()
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort_unstable();
    ids
}

fn apply_pending_commands(world: &mut World) {
    let commands = {
        let mut pending = world.resource_mut::<PendingCommands>();
        std::mem::take(&mut pending.0)
    };

    if commands.is_empty() {
        return;
    }

    let mut latest_feedback: Option<(String, Option<RejectReason>)> = None;
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        for command in commands {
            let result = apply_player_command(world, &mut map, command);
            let reject_code = result
                .reject
                .or_else(|| result.rejected_units.first().map(|(_, reason)| *reason));
            latest_feedback = Some((format_command_result(&result), reject_code));
        }
    });

    if let Some((text, reject_code)) = latest_feedback {
        let mut feedback = world.resource_mut::<CommandFeedback>();
        feedback.revision = feedback.revision.wrapping_add(1);
        feedback.text = text;
        feedback.last_reject_code = reject_code;
    }
}

fn format_command_result(result: &CommandResult) -> String {
    // Single-command rejections (Place/Enqueue/SetRally/Gather-global) name
    // the typed reject code; without this they would read as accepted.
    if let Some(reason) = result.reject {
        return format!("Command rejected ({reason:?})");
    }

    let unreachable = result
        .rejected_units
        .iter()
        .filter(|(_, reason)| *reason == RejectReason::Unreachable)
        .count();
    let not_owned = result
        .rejected_units
        .iter()
        .filter(|(_, reason)| *reason == RejectReason::NotOwned)
        .count();
    let unknown = result
        .rejected_units
        .iter()
        .filter(|(_, reason)| *reason == RejectReason::UnknownUnit)
        .count();

    if result.rejected_units.is_empty() {
        // Single-object commands (PlaceBuilding, ResumeConstruction, enqueue,
        // rally) accept nothing per-unit; a bare "accepted" must not read as
        // "0 unit(s)" and clobber the controller's optimistic status.
        return if result.accepted_units.is_empty() {
            "Command accepted".to_string()
        } else {
            format!(
                "Command accepted for {} unit(s)",
                result.accepted_units.len()
            )
        };
    }

    if result.accepted_units.is_empty() && unreachable == result.rejected_units.len() {
        return format!("Destination unreachable for {unreachable} unit(s)");
    }

    // Per-unit rejects outside the three counted kinds (e.g. FarmOccupied,
    // Crowded, NotVillager) still name the first uncounted typed code instead
    // of collapsing into a zero-count summary.
    let uncounted = result.rejected_units.len() - unreachable - not_owned - unknown;
    let mut text = format!(
        "Command: {} accepted, {} unreachable, {} not owned, {} missing",
        result.accepted_units.len(),
        unreachable,
        not_owned,
        unknown
    );
    if let Some((_, reason)) = result.rejected_units.iter().find(|(_, reason)| {
        !matches!(
            reason,
            RejectReason::Unreachable | RejectReason::NotOwned | RejectReason::UnknownUnit
        )
    }) {
        text += &format!(", {uncounted} rejected ({reason:?})");
    }
    text
}

fn advance_movement(world: &mut World) {
    world.resource_scope(|world, map: Mut<GridMap>| {
        step_movement(world, &map, SIM_STEP_SECONDS);
    });
}

fn advance_economy(world: &mut World) {
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_economy(world, &mut map, SIM_STEP_SECONDS);
    });
}

fn advance_construction(world: &mut World) {
    step_construction(world, SIM_STEP_SECONDS);
}

fn advance_production(world: &mut World) {
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_production(world, &mut map, SIM_STEP_SECONDS);
    });
}

/// Drains the sim's latest worker route-failure reject into the existing
/// feedback channel, so mid-step idles surface as typed codes with a
/// revision bump instead of disappearing silently.
fn drain_route_reject_feedback(world: &mut World) {
    let reason = world
        .get_resource_mut::<LastRouteReject>()
        .and_then(|mut slot| slot.0.take());
    let Some(reason) = reason else {
        return;
    };
    let mut feedback = world.resource_mut::<CommandFeedback>();
    feedback.revision = feedback.revision.wrapping_add(1);
    feedback.text = format!("Worker route impossible ({reason:?})");
    feedback.last_reject_code = Some(reason);
}

fn job_seconds(job: &ProductionJob) -> f32 {
    match job.kind {
        ProductionKind::Unit(kind) => unit_spec(kind).train_seconds as f32,
        ProductionKind::Age2 => AGE_TWO_SECONDS as f32,
    }
}

fn queue_head_label(job: &ProductionJob) -> String {
    match job.kind {
        ProductionKind::Unit(kind) => format!("{kind:?}"),
        ProductionKind::Age2 => "Age2".to_string(),
    }
}

/// godot-bevy 0.12 pins `Time<Fixed>::overstep_fraction()` to 0 (Godot owns the
/// fixed-step accumulator) and copies Bevy→Godot transforms only once per tick,
/// so the stock path renders unit views one tick stale and unsmoothed. Restore
/// the 0.11 contract: every Update, write the interpolated transform directly
/// to each unit view's Node3D, using Godot's own between-ticks fraction.
fn sync_interpolated_unit_transforms(
    mut units: Query<(&SimPosition, &mut Transform, &GodotNodeHandle), With<Unit>>,
    mut godot: GodotAccess,
) {
    let alpha = Engine::singleton().get_physics_interpolation_fraction() as f32;
    for (position, mut transform, handle) in &mut units {
        let rendered = position.previous.lerp(position.current, alpha);
        if transform.translation.x != rendered.x || transform.translation.z != rendered.y {
            transform.translation.x = rendered.x;
            transform.translation.z = rendered.y;
        }
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        let mut node_transform = node.get_transform();
        node_transform.origin.x = rendered.x;
        node_transform.origin.y = 0.0;
        node_transform.origin.z = rendered.y;
        node.set_transform(node_transform);
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::World;
    use grus_sim::GatherProgress;
    use grus_sim::map::{GridMap, GridPos};

    use super::*;

    #[test]
    fn idle_workers_exclude_units_with_move_orders() {
        let mut world = World::new();
        let map = GridMap::new(16, 16);
        let idle = spawn_unit(
            &mut world,
            UnitId(1),
            TeamId(1),
            Vec2::new(1.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        world
            .entity_mut(idle)
            .insert((WorkerTask::Idle, GatherProgress::default()));
        let moving = spawn_unit(
            &mut world,
            UnitId(2),
            TeamId(1),
            Vec2::new(2.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        world.entity_mut(moving).insert((
            WorkerTask::Idle,
            GatherProgress::default(),
            MoveOrder {
                waypoints: vec![map.cell_center(GridPos::new(8, 8))],
                next: 0,
                goal: GridPos::new(8, 8),
                map_revision: map.revision(),
                last_failed_replan: None,
            },
        ));
        let gathering = spawn_unit(
            &mut world,
            UnitId(3),
            TeamId(1),
            Vec2::new(3.5, 1.5),
            UnitKind::Villager,
            6.0,
        );
        world.entity_mut(gathering).insert((
            WorkerTask::Gathering {
                source: ResourceId(1),
            },
            GatherProgress::default(),
        ));
        spawn_unit(
            &mut world,
            UnitId(4),
            TeamId(1),
            Vec2::new(4.5, 1.5),
            UnitKind::Spearman,
            6.0,
        );

        assert_eq!(idle_worker_ids(&world, TeamId(1)), vec![1]);
    }

    #[test]
    fn building_view_transform_centers_and_scales_to_the_footprint() {
        // A 4×4 Town Center at (12, 46) renders around (14, 48) — not the
        // anchor's cell center — and the unit box scales to the footprint.
        let town_center = building_view_transform(&Footprint::new(GridPos::new(12, 46), 4, 4));
        assert_eq!(town_center.translation, Vec3::new(14.0, 0.0, 48.0));
        assert_eq!(town_center.scale, Vec3::new(4.0, 1.0, 4.0));

        let house = building_view_transform(&Footprint::new(GridPos::new(13, 10), 2, 2));
        assert_eq!(house.translation, Vec3::new(14.0, 0.0, 11.0));
        assert_eq!(house.scale, Vec3::new(2.0, 1.0, 2.0));
    }

    #[test]
    fn command_feedback_names_typed_rejects_and_bare_accepts() {
        // Single-object commands accept nothing per-unit: a bare "accepted"
        // rather than "0 unit(s)".
        assert_eq!(
            format_command_result(&CommandResult::default()),
            "Command accepted"
        );
        assert_eq!(
            format_command_result(&CommandResult {
                reject: Some(RejectReason::Occupied),
                ..CommandResult::default()
            }),
            "Command rejected (Occupied)"
        );
        assert_eq!(
            format_command_result(&CommandResult {
                accepted_units: vec![UnitId(1), UnitId(2)],
                ..CommandResult::default()
            }),
            "Command accepted for 2 unit(s)"
        );
        assert_eq!(
            format_command_result(&CommandResult {
                rejected_units: vec![
                    (UnitId(1), RejectReason::Unreachable),
                    (UnitId(2), RejectReason::Unreachable),
                ],
                ..CommandResult::default()
            }),
            "Destination unreachable for 2 unit(s)"
        );
        // Rejects outside the counted kinds still surface their typed code.
        assert_eq!(
            format_command_result(&CommandResult {
                rejected_units: vec![
                    (UnitId(1), RejectReason::FarmOccupied),
                    (UnitId(2), RejectReason::FarmOccupied),
                ],
                ..CommandResult::default()
            }),
            "Command: 0 accepted, 0 unreachable, 0 not owned, 0 missing, 2 rejected (FarmOccupied)"
        );
    }
}
