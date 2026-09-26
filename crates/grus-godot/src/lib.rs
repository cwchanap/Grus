use std::collections::HashSet;

use bevy::prelude::*;
use godot::builtin::{GString, PackedInt32Array, VarDictionary, Vector2};
use godot::classes::{Engine, INode, Node, Node3D, SceneTree};
use godot::prelude::*;
use godot_bevy::BevyApp;
use godot_bevy::prelude::*;
use grus_sim::catalog::{ResourceKind, building_spec, unit_spec};
use grus_sim::{
    AGE_TWO_COST, AGE_TWO_SECONDS, Age, AiController, Building, BuildingId, BuildingIndex,
    BuildingKind, CombatEvent, CombatEvents, CombatTarget, CommandResult, Footprint, GridMap,
    GridPos, Health, IdAllocator, LastRouteReject, MapFixture, MatchPhase, MatchSession,
    PlayerCommand, ProductionJob, ProductionKind, ProductionQueue, RallyPoint, RejectReason,
    ResourceId, ResourceIndex, ResourceSource, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId,
    Unit, UnitCommand, UnitCommandKind, UnitId, UnitIndex, UnitKind, VisibilityMap, active_phase,
    apply_player_command, explored_by, gather_rate_for_age, idle_worker_ids, population_cap,
    population_used, produces, refresh_visibility, seed_skirmish, set_paused, spawn_unit,
    start_match, step_ai, step_combat, step_construction, step_economy, step_movement,
    step_production, validate_placement, visible_to,
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
    /// Last fog payload handed to GDScript, keyed by its visibility
    /// revision: the fog overlay and the minimap both fetch per revision
    /// change, and rebuilding the 12k-cell payload twice per revision is
    /// pure waste. `VarDictionary` is refcounted, so the cache hit clones a
    /// handle, not the payload.
    visibility_cache: Option<(i64, VarDictionary)>,
}

#[godot_api]
impl INode for GrusBridgeNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            visibility_cache: None,
        }
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
    fn attack_units(
        &self,
        packed_ids: PackedInt32Array,
        target_kind: GString,
        target_id: i32,
    ) -> bool {
        let units = decode_unit_ids(&packed_ids);
        let Some(target) = parse_target_kind(&target_kind, target_id) else {
            return false;
        };
        if units.is_empty() {
            return false;
        }
        queue_command(PlayerCommand::Attack {
            issuer: TeamId(1),
            units,
            target,
        })
    }

    #[func]
    fn attack_move_units(&self, packed_ids: PackedInt32Array, target: Vector2) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }
        queue_command(PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units,
            kind: UnitCommandKind::AttackMove {
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

    /// Session phase for the overlay: `phase` is one of
    /// "Start"/"Playing"/"Paused"/"Result"; `winner_team` is the winning
    /// TeamId once Result, else -1.
    #[func]
    fn session_snapshot(&self) -> VarDictionary {
        with_app(|app| {
            let phase = active_phase(app.world());
            let mut dict = VarDictionary::new();
            dict.set(
                "phase",
                &GString::from(match phase {
                    MatchPhase::Start => "Start",
                    MatchPhase::Playing => "Playing",
                    MatchPhase::Paused => "Paused",
                    MatchPhase::Result(_) => "Result",
                }),
            );
            dict.set(
                "winner_team",
                match phase {
                    MatchPhase::Result(result) => i64::from((result.0).0),
                    _ => -1,
                },
            );
            dict
        })
        .unwrap_or_default()
    }

    /// Explicit Start -> Playing. Gameplay commands and the fixed steps stay
    /// session-locked until this is called.
    #[func]
    fn start_match(&self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };
        start_match(app.world_mut());
        true
    }

    /// Test-only AI removal for scripted legacy gameplay smokes: succeeds
    /// only while the session sits in Start (never mid-match) and removes
    /// only the `AiController` — visibility, entities and session state are
    /// untouched. The HPA-473 scouting smoke deliberately never calls this:
    /// it asserts the AI stays present.
    #[func]
    fn disable_ai_for_test(&self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };
        disable_ai_in_world(app.world_mut())
    }

    /// Pause/Resume. Changes the session phase only — never
    /// `Engine.time_scale`, which stays the headless sim-speed control.
    #[func]
    fn set_paused(&self, paused: bool) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };
        set_paused(app.world_mut(), paused);
        true
    }

    /// Restart = the existing clear + reseed reset seam, returning the
    /// normal skirmish to Start.
    #[func]
    fn restart_match(&self) -> bool {
        self.reset_fixture()
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

    /// Sim-speed multiplier for headless gate runs. Scales Godot's
    /// `Engine.time_scale` (the 0.12 fixed-loop tick-rate authority) and
    /// `Time<Virtual>`; never touches `max_delta`.
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
        // godot-bevy 0.12's `godot_fixed_driver` takes its tick delta from
        // `_physics_process`, scaled by `Engine.time_scale`, and ignores
        // `Time<Virtual>`'s relative speed — so the engine-wide knob is the
        // tick-rate authority. The virtual-speed write stays to keep
        // Bevy-side render clocks consistent.
        Engine::singleton().set_time_scale(relative_speed);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed_f64(relative_speed);
        true
    }

    /// Current-tick combat events for cosmetics: drains and returns them while
    /// the session is Playing, plus the settle tick's buffer on Result —
    /// `strike()` records the killing blow and resolves the match in the same
    /// fixed tick, and that buffer must present once. Returns an empty array
    /// in Start/Paused. The guard is defense in depth: `step_combat` clears
    /// `CombatEvents` on every non-Result tick — Playing or frozen — so stale
    /// events can never survive a pause to replay as fresh effects; Result
    /// ticks hold the buffer until this drain reads it.
    #[func]
    fn drain_combat_events(&self) -> Array<VarDictionary> {
        let Some(mut app_node) = bevy_app_singleton() else {
            return Array::new();
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return Array::new();
        };
        take_presentable_events(app.world_mut())
            .into_iter()
            .map(combat_event_dict)
            .collect()
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

    /// Monotonic Team-1 visibility revision for cheap per-frame polling: fog
    /// and minimap fetch the packed snapshot only when this changes. `-1`
    /// when no visibility state exists (the visibility-free benchmark).
    #[func]
    fn visibility_revision(&self) -> i64 {
        with_app(|app| {
            app.world()
                .get_resource::<VisibilityMap>()
                .map(|visibility| visibility.revision() as i64)
                .unwrap_or(-1)
        })
        .unwrap_or(-1)
    }

    /// Packed Team-1 fog payload: map width/height, row-major cell states
    /// (0 Unexplored / 1 Explored / 2 Visible), and the revision observed
    /// during the read so a consumer can detect a mid-read change. Empty
    /// dict when visibility state is absent. Cached per revision: both
    /// GDScript consumers poll every frame and fetch only on change, so at
    /// most one rebuild lands per revision change.
    #[func]
    fn visibility_snapshot(&mut self) -> VarDictionary {
        with_app(|app| {
            let world = app.world();
            let Some(visibility) = world.get_resource::<VisibilityMap>() else {
                return VarDictionary::new();
            };
            let revision = visibility.revision() as i64;
            if let Some((cached_revision, cached)) = &self.visibility_cache
                && *cached_revision == revision
            {
                return cached.clone();
            }
            let Some(map) = world.get_resource::<GridMap>() else {
                return VarDictionary::new();
            };
            let states = visibility.packed_cell_states(TeamId(1), map.width(), map.height());
            let mut dict = VarDictionary::new();
            dict.set("width", i64::from(map.width()));
            dict.set("height", i64::from(map.height()));
            dict.set("revision", revision);
            dict.set(
                "states",
                &PackedInt32Array::from_iter(states.into_iter().map(i32::from)),
            );
            self.visibility_cache = Some((revision, dict.clone()));
            dict
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
            dict.set(
                "idle_worker_ids",
                &PackedInt32Array::from_iter(idle.iter().filter_map(|id| i32::try_from(id.0).ok())),
            );
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
    /// cell (-1/-1 when unset). Empty dict when the building does not exist
    /// or is a hidden enemy — fog must not leak health/queue/construction
    /// through stale ids.
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
            if building_hidden_from_local_player(world, entity) {
                return dict;
            }
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
                sync_view_visibility,
                sync_interpolated_unit_transforms,
                update_health_bars,
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                apply_pending_commands,
                advance_combat,
                advance_movement,
                advance_economy,
                advance_construction,
                advance_production,
                advance_visibility,
                advance_ai,
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

/// Closed-string target kind beside the other parse helpers: only
/// `"unit"` / `"building"` map to a `CombatTarget`; anything else rejects at
/// the bridge.
fn parse_target_kind(kind: &GString, target_id: i32) -> Option<CombatTarget> {
    parse_target_kind_str(kind.to_string().as_str(), target_id)
}

fn parse_target_kind_str(kind: &str, target_id: i32) -> Option<CombatTarget> {
    let Ok(raw_id) = u32::try_from(target_id) else {
        return None;
    };
    // Stable ids are 1-based; 0 matches decode_unit_ids' reject convention.
    if raw_id == 0 {
        return None;
    }
    match kind {
        "unit" => Some(CombatTarget::Unit(UnitId(raw_id))),
        "building" => Some(CombatTarget::Building(BuildingId(raw_id))),
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

/// Drains the current-tick combat events while the session is Playing, and
/// once on Result: `strike()` pushes the killing blow and resolves the Town
/// Center destruction to Result in the same fixed tick, so that settle
/// tick's buffer must still drain or the winning hit/death cosmetics never
/// present. Start/Paused stay blocked — defense in depth on top of
/// `step_combat` clearing the buffer on every non-Result tick, so nothing
/// frozen can ever replay as fresh cosmetics across a pause; Result ticks
/// hold the buffer until this drain reads it.
fn take_presentable_events(world: &mut World) -> Vec<CombatEvent> {
    if matches!(active_phase(world), MatchPhase::Start | MatchPhase::Paused) {
        return Vec::new();
    }
    let mut events = world
        .get_resource_mut::<CombatEvents>()
        .map(|mut events| std::mem::take(&mut events.0))
        .unwrap_or_default();
    // Presentation permission is derived here, before dictionaries reach
    // GDScript: a hidden enemy attacker must not expose its stable id as a
    // tracer origin (GDScript never re-looks-up live attackers). The event's
    // position is the target's — always a friendly (always visible) when the
    // attacker is a hidden enemy — so hit/death feedback stays intact. Id 0
    // matches every id-reject convention: no live unit carries it.
    for event in &mut events {
        if !attacker_presentable(world, event.attacker) {
            event.attacker = UnitId(0);
        }
    }
    events
}

/// The attacker's stable id may reach GDScript only when it exists and is
/// either friendly or currently visible to the local team. A missing attacker
/// (already despawned) is scrubbed too — dead men leak no positions.
fn attacker_presentable(world: &World, attacker: UnitId) -> bool {
    let Some(entity) = world
        .get_resource::<UnitIndex>()
        .and_then(|index| index.entity(attacker))
    else {
        return false;
    };
    let Some(unit) = world.get::<Unit>(entity) else {
        return false;
    };
    if unit.team == TeamId(1) {
        return true;
    }
    let Some(map) = world.get_resource::<GridMap>() else {
        return true;
    };
    let Some(position) = world.get::<SimPosition>(entity) else {
        return false;
    };
    visible_to(world, TeamId(1), map.world_to_cell(position.current))
}

/// A building is fog-hidden from the local player when it is an enemy whose
/// footprint is not currently Team-1 visible. Friendly buildings are always
/// presentable. Drives the `building_snapshot` empty dict and nothing else —
/// view rendering goes through `sync_view_visibility`.
fn building_hidden_from_local_player(world: &World, entity: Entity) -> bool {
    let Some(building) = world.get::<Building>(entity) else {
        return false;
    };
    if building.team == TeamId(1) {
        return false;
    }
    match world.get::<Footprint>(entity) {
        Some(footprint) => !visible_to(world, TeamId(1), *footprint),
        None => false,
    }
}

fn combat_event_dict(event: CombatEvent) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("attacker", i64::from(event.attacker.0));
    dict.set(
        "target_kind",
        &GString::from(match event.target {
            CombatTarget::Unit(_) => "unit",
            CombatTarget::Building(_) => "building",
        }),
    );
    dict.set(
        "target_id",
        i64::from(match event.target {
            CombatTarget::Unit(id) => id.0,
            CombatTarget::Building(id) => id.0,
        }),
    );
    dict.set("damage", i64::from(event.damage));
    dict.set("x", f64::from(event.position.x));
    dict.set("y", f64::from(event.position.y));
    dict.set("ranged", event.ranged);
    dict.set("killed", event.killed);
    dict
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
    // Normal Godot setup boots into Start; start_match opens gameplay.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    insert_fresh_visibility(world);
    // The Team-2 economic AI opponent ships with every normal setup; the
    // benchmark reset never inserts one.
    world.insert_resource(AiController::new(TeamId(2)));
}

/// Normal runtime gameplay runs fogged: a fresh `VisibilityMap` beside the
/// session plus one initial refresh so the Start screen already has correct
/// fog. The benchmark reset never calls this — it stays visibility-free.
fn insert_fresh_visibility(world: &mut World) {
    world.insert_resource(VisibilityMap::default());
    world.resource_scope(|world, map: Mut<GridMap>| refresh_visibility(world, &map));
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

/// Exclusive (main-thread) because the authoritative-visibility predicates
/// read the `World` while the pass writes Godot node state — a parametric
/// `&World` would conflict with `GodotAccess`'s main-thread guard. Stamps
/// identity metadata AND the initial authoritative visibility in the same
/// pass, so a newly instantiated view never spends a frame visible before
/// the sim truth lands (scene roots default hidden).
fn initialize_view_metadata(world: &mut World) {
    #[allow(clippy::type_complexity)]
    let mut units = world.query_filtered::<
        (Entity, &Unit, &SimPosition, &GodotNodeHandle),
        Without<ViewMetaInitialized>,
    >();
    let unit_batch: Vec<(Entity, UnitId, TeamId, UnitKind, Vec2, GodotNodeHandle)> = units
        .iter(world)
        .map(|(entity, unit, position, handle)| {
            (
                entity,
                unit.id,
                unit.team,
                unit.kind,
                position.current,
                *handle,
            )
        })
        .collect();
    let mut buildings = world.query_filtered::<(
        Entity,
        &Building,
        &Footprint,
        Option<&ResourceSource>,
        &GodotNodeHandle,
    ), Without<ViewMetaInitialized>>();
    #[allow(clippy::type_complexity)]
    let building_batch: Vec<(
        Entity,
        BuildingId,
        BuildingKind,
        TeamId,
        Footprint,
        Option<(ResourceId, ResourceKind)>,
        GodotNodeHandle,
    )> = buildings
        .iter(world)
        .map(|(entity, building, footprint, source, handle)| {
            (
                entity,
                building.id,
                building.kind,
                building.team,
                *footprint,
                source.map(|source| (source.id, source.kind)),
                *handle,
            )
        })
        .collect();
    let mut resources = world.query_filtered::<(
        Entity,
        &ResourceSource,
        &Footprint,
        &GodotNodeHandle,
    ), (Without<Building>, Without<ViewMetaInitialized>)>();
    let resource_batch: Vec<(Entity, ResourceId, ResourceKind, Footprint, GodotNodeHandle)> =
        resources
            .iter(world)
            .map(|(entity, source, footprint, handle)| {
                (entity, source.id, source.kind, *footprint, *handle)
            })
            .collect();

    for (entity, id, team, kind, position, handle) in unit_batch {
        let Some(mut node) = node_from_handle(handle) else {
            continue;
        };
        node.set_meta("unit_id", &i64::from(id.0).to_variant());
        node.set_meta("team_id", &i64::from(team.0).to_variant());
        node.set_meta("unit_kind", &debug_variant(kind));
        // Borrow, never clone: `world_to_cell` is pure coordinate math, and
        // the exclusive pass needs the map only while no &mut is held.
        let cell = world.resource::<GridMap>().world_to_cell(position);
        let visible = team == TeamId(1) || visible_to(world, TeamId(1), cell);
        node.set_visible(visible);
        world.entity_mut(entity).insert(ViewMetaInitialized);
    }
    for (entity, id, kind, team, footprint, source, handle) in building_batch {
        let Some(mut node) = node_from_handle(handle) else {
            continue;
        };
        node.set_meta("building_id", &i64::from(id.0).to_variant());
        node.set_meta("building_kind", &debug_variant(kind));
        node.set_meta("team_id", &i64::from(team.0).to_variant());
        // A completed Farm carries both its building identity and its
        // renewable Food source on the same entity.
        if let Some((source_id, source_kind)) = source {
            node.set_meta("resource_id", &i64::from(source_id.0).to_variant());
            node.set_meta("resource_kind", &debug_variant(source_kind));
            world.entity_mut(entity).insert(ResourceMetaInitialized);
        }
        // Enemy Farms follow enemy-building visibility: the building branch
        // owns the stamp, never the resource branch below.
        let visible = team == TeamId(1) || visible_to(world, TeamId(1), footprint);
        node.set_visible(visible);
        world.entity_mut(entity).insert(ViewMetaInitialized);
    }
    for (entity, id, kind, footprint, handle) in resource_batch {
        let Some(mut node) = node_from_handle(handle) else {
            continue;
        };
        node.set_meta("resource_id", &i64::from(id.0).to_variant());
        node.set_meta("resource_kind", &debug_variant(kind));
        // Standalone sources appear on first exploration and persist.
        let visible = explored_by(world, TeamId(1), footprint);
        node.set_visible(visible);
        world
            .entity_mut(entity)
            .insert((ViewMetaInitialized, ResourceMetaInitialized));
    }
}

/// The exclusive pass can't hold a query borrow while mutating entities, so
/// it batches first; this helper only unwraps node handles.
fn node_from_handle(handle: GodotNodeHandle) -> Option<Gd<Node3D>> {
    Gd::<Node3D>::try_from_instance_id(handle.instance_id()).ok()
}

/// Maintains every gameplay view's `Node3D.visible` from the authoritative
/// Team-1 visibility each Update: friendly units/buildings always visible,
/// enemies only while currently visible (enemy Farms included — they ride
/// the building branch), standalone resources once explored and staying.
/// Exclusive for the same reason as `initialize_view_metadata`; the initial
/// stamp happens there, this system only maintains the value.
fn sync_view_visibility(world: &mut World) {
    let mut units = world.query::<(&Unit, &SimPosition, &GodotNodeHandle)>();
    let unit_batch: Vec<(TeamId, Vec2, GodotNodeHandle)> = units
        .iter(world)
        .map(|(unit, position, handle)| (unit.team, position.current, *handle))
        .collect();
    let mut buildings = world.query::<(&Building, &Footprint, &GodotNodeHandle)>();
    let building_batch: Vec<(TeamId, Footprint, GodotNodeHandle)> = buildings
        .iter(world)
        .map(|(building, footprint, handle)| (building.team, *footprint, *handle))
        .collect();
    let mut resources = world.query_filtered::<
        (&Footprint, &GodotNodeHandle),
        (With<ResourceSource>, Without<Building>),
    >();
    let resource_batch: Vec<(Footprint, GodotNodeHandle)> = resources
        .iter(world)
        .map(|(footprint, handle)| (*footprint, *handle))
        .collect();

    for (team, position, handle) in unit_batch {
        // Borrow, never clone: `world_to_cell` is pure coordinate math.
        let cell = world.resource::<GridMap>().world_to_cell(position);
        set_view_visible(
            handle,
            team == TeamId(1) || visible_to(world, TeamId(1), cell),
        );
    }
    for (team, footprint, handle) in building_batch {
        set_view_visible(
            handle,
            team == TeamId(1) || visible_to(world, TeamId(1), footprint),
        );
    }
    for (footprint, handle) in resource_batch {
        set_view_visible(handle, explored_by(world, TeamId(1), footprint));
    }
}

/// Value-gated write: idle views don't re-raise Godot visibility flags.
fn set_view_visible(handle: GodotNodeHandle, visible: bool) {
    let Some(mut node) = node_from_handle(handle) else {
        return;
    };
    if node.is_visible() != visible {
        node.set_visible(visible);
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

/// Health bars ride the existing per-entity Godot node handles: every
/// `Changed<Health>` tick forwards the ratio to the view script, which owns
/// the bar's primitive geometry. No per-frame GDScript world polling.
fn update_health_bars(
    changed: Query<(&Health, &GodotNodeHandle), Changed<Health>>,
    mut godot: GodotAccess,
) {
    for (health, handle) in &changed {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        let ratio = if health.max == 0 {
            1.0
        } else {
            health.current as f32 / health.max as f32
        };
        node.call("set_health_ratio", &[ratio.to_variant()]);
    }
}

fn reset_fixture_world(world: &mut World) {
    clear_gameplay_world(world);
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    seed_skirmish(world, &mut map, &fixture);
    world.insert_resource(map);
    // A normal-skirmish restart returns the session to Start — fogged with a
    // fresh exploration state, same as a first boot.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    insert_fresh_visibility(world);
    // A fresh controller: scout progress, exploration-adjacent decision
    // state and the remembered enemy Town Center all restart clean.
    world.insert_resource(AiController::new(TeamId(2)));
    // The clear despawns the selector-bearing gameplay entities; reattach the
    // e2e selector surface to the reseeded fixture. Inert without BEVY_E2E=1.
    #[cfg(feature = "e2e")]
    e2e::attach_selectors(world);
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

/// The narrow test-disable seam behind `disable_ai_for_test`: Start-only and
/// controller-only.
fn disable_ai_in_world(world: &mut World) -> bool {
    if !matches!(
        world
            .get_resource::<MatchSession>()
            .map(|session| &session.phase),
        Some(MatchPhase::Start)
    ) {
        return false;
    }
    world.remove_resource::<AiController>().is_some()
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
    // Visibility dies with the match: the benchmark restart must stay
    // visibility-free, and a normal reset re-inserts a fresh fogged map.
    world.remove_resource::<VisibilityMap>();
    // The AI controller dies with the match: the benchmark stays AI-free and
    // a normal reset re-inserts a fresh Team-2 controller.
    world.remove_resource::<AiController>();
    // Combat/session transients die with the match: the benchmark restarts
    // session-free (missing session = Playing), the normal reset re-inserts
    // Start.
    world.remove_resource::<CombatEvents>();
    world.remove_resource::<MatchSession>();
    if let Some(mut pending) = world.get_resource_mut::<PendingCommands>() {
        pending.0.clear();
    }
    if let Some(mut feedback) = world.get_resource_mut::<CommandFeedback>() {
        feedback.revision = feedback.revision.wrapping_add(1);
        feedback.text = "Ready".to_string();
        feedback.last_reject_code = None;
    }
    if let Some(mut route_reject) = world.get_resource_mut::<LastRouteReject>() {
        route_reject.0.clear();
    }
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

// ponytail: each advance_* system takes one coarse Time<Fixed>::delta() per
// tick (speed 20 → 1.0 sim-s per 0.05 s tick); sub-step movement/economy
// inside the tick if a high-speed scenario ever shows tunneling or overshoot.
fn advance_combat(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_combat(world, &mut map, seconds);
    });
}

fn advance_movement(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, map: Mut<GridMap>| {
        step_movement(world, &map, seconds);
    });
}

fn advance_economy(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_economy(world, &mut map, seconds);
    });
}

fn advance_construction(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    step_construction(world, seconds);
}

fn advance_production(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_production(world, &mut map, seconds);
    });
}

/// Canonical post-production step: presentation and (later) AI consume the
/// same freshly computed Team-1 visibility the command validation of the
/// next tick will enforce.
fn advance_visibility(world: &mut World) {
    world.resource_scope(|world, map: Mut<GridMap>| refresh_visibility(world, &map));
}

/// The Team-2 AI fixed step, last mutation before the feedback drain: the
/// decision reads exactly the fog the presentation received this tick.
fn advance_ai(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_ai(world, &mut map, seconds);
    });
}

/// Drains the sim's latest Team-1 worker route-failure reject into the
/// existing feedback channel, so mid-step idles surface as typed codes with
/// a revision bump instead of disappearing silently. Team 2's entries stay
/// in the sim resource: the AI's private rejects must never surface as — or
/// overwrite — the human player's feedback.
fn drain_route_reject_feedback(world: &mut World) {
    let reason = world
        .get_resource_mut::<LastRouteReject>()
        .and_then(|mut slot| slot.0.remove(&TeamId(1)));
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
        // Value gate: idle units re-render the mirrored transform, so skip
        // both the mirror update and the node write until the value moves.
        if transform.translation.x == rendered.x && transform.translation.z == rendered.y {
            continue;
        }
        transform.translation.x = rendered.x;
        transform.translation.z = rendered.y;
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
    use grus_sim::{CombatEvent, CombatTarget, MatchResult, MoveOrder, WorkerTask};

    use super::*;

    #[test]
    fn parse_target_kind_accepts_only_the_closed_strings() {
        assert_eq!(
            parse_target_kind_str("unit", 5),
            Some(CombatTarget::Unit(UnitId(5)))
        );
        assert_eq!(
            parse_target_kind_str("building", 2),
            Some(CombatTarget::Building(BuildingId(2)))
        );
        // Everything else rejects at the bridge, including case drift,
        // unknown words, and out-of-range ids.
        assert_eq!(parse_target_kind_str("Unit", 5), None);
        assert_eq!(parse_target_kind_str("cactus", 5), None);
        assert_eq!(parse_target_kind_str("", 5), None);
        assert_eq!(parse_target_kind_str("unit", 0), None);
        assert_eq!(parse_target_kind_str("unit", -5), None);
        assert_eq!(
            parse_target_kind_str("unit", i32::MAX),
            Some(CombatTarget::Unit(UnitId(i32::MAX as u32)))
        );
    }

    #[test]
    fn combat_event_drain_blocks_frozen_but_releases_result() {
        let event = CombatEvent {
            attacker: UnitId(1),
            target: CombatTarget::Unit(UnitId(2)),
            damage: 3,
            position: Vec2::new(4.0, 5.0),
            ranged: true,
            killed: false,
        };
        let mut world = World::new();

        // A frozen session keeps the last Playing tick's events readable;
        // the drain must not take or replay them.
        for phase in [MatchPhase::Paused, MatchPhase::Start] {
            world.insert_resource(MatchSession { phase });
            world.insert_resource(CombatEvents(vec![event]));
            assert!(take_presentable_events(&mut world).is_empty());
            assert_eq!(world.resource::<CombatEvents>().0.len(), 1);
        }

        // Without a UnitIndex the attacker's existence (and therefore its
        // presentation permission) cannot be verified: the id is scrubbed
        // to 0 while the target-position feedback survives.
        let mut scrubbed = event;
        scrubbed.attacker = UnitId(0);

        // Result releases the settle tick's buffer once — `strike()` records
        // the killing blow and resolves the match in the same fixed tick —
        // and the drained buffer stays empty on later Result frames.
        world.insert_resource(MatchSession {
            phase: MatchPhase::Result(MatchResult(TeamId(2))),
        });
        assert_eq!(take_presentable_events(&mut world), vec![scrubbed]);
        assert!(world.resource::<CombatEvents>().0.is_empty());
        assert!(take_presentable_events(&mut world).is_empty());

        // Playing takes the events and leaves the buffer empty.
        world.insert_resource(MatchSession {
            phase: MatchPhase::Playing,
        });
        world.insert_resource(CombatEvents(vec![event]));
        assert_eq!(take_presentable_events(&mut world), vec![scrubbed]);
        assert!(world.resource::<CombatEvents>().0.is_empty());
        // A missing session reads as Playing (benchmark contract).
        world.remove_resource::<MatchSession>();
        world.insert_resource(CombatEvents(vec![event]));
        assert_eq!(take_presentable_events(&mut world), vec![scrubbed]);
    }

    /// Runtime setup fog: the fixture boots with a fresh `VisibilityMap`, an
    /// initial refresh (revision >= 1), the enemy start Unexplored and the
    /// own start Visible. Restarts re-fog; the benchmark stays free of
    /// visibility so its full-information contract cannot silently flip.
    #[test]
    fn runtime_setup_and_resets_manage_the_visibility_resource() {
        let mut world = World::new();
        setup_fixture(&mut world);

        let revision = world.resource::<VisibilityMap>().revision();
        assert!(
            revision >= 1,
            "initial refresh must have bumped the revision"
        );
        let own_town_center = GridPos::new(12, 46);
        let enemy_town_center = GridPos::new(112, 46);
        let states = world
            .resource::<VisibilityMap>()
            .packed_cell_states(TeamId(1), 128, 96);
        assert_eq!(
            states[(own_town_center.y * 128 + own_town_center.x) as usize],
            2
        );
        assert_eq!(
            states[(enemy_town_center.y * 128 + enemy_town_center.x) as usize],
            0,
            "the enemy start must boot Unexplored — runtime is not full-info"
        );

        // A normal restart re-inserts a fresh fogged map.
        reset_fixture_world(&mut world);
        assert!(world.get_resource::<VisibilityMap>().is_some());
        assert!(world.resource::<VisibilityMap>().revision() >= 1);

        // The benchmark reset must remove visibility: seed_skirmish and the
        // 200-villager benchmark run full-information.
        reset_benchmark_world(&mut world);
        assert!(world.get_resource::<VisibilityMap>().is_none());
    }

    /// Normal setup/reset insert a fresh Team-2 controller; the benchmark
    /// stays AI-free; the test-disable seam lifts only the controller and
    /// only from Start.
    #[test]
    fn the_team_two_ai_controller_follows_setup_reset_and_test_disable() {
        let mut world = World::new();
        setup_fixture(&mut world);
        assert_eq!(
            world.resource::<AiController>().team,
            TeamId(2),
            "normal setup ships the Team-2 AI opponent"
        );

        reset_fixture_world(&mut world);
        assert_eq!(
            world.resource::<AiController>().team,
            TeamId(2),
            "a restart re-inserts a fresh controller"
        );

        reset_benchmark_world(&mut world);
        assert!(
            world.get_resource::<AiController>().is_none(),
            "the benchmark fixture stays AI-free"
        );

        // The disable seam: no session at all must fail closed, Start must
        // succeed exactly once, and Playing/Paused must refuse.
        assert!(!disable_ai_in_world(&mut world), "no session: refuse");
        world.insert_resource(MatchSession {
            phase: MatchPhase::Start,
        });
        world.insert_resource(AiController::new(TeamId(2)));
        assert!(disable_ai_in_world(&mut world), "Start: disable succeeds");
        assert!(!disable_ai_in_world(&mut world), "already disabled: refuse");
        world.insert_resource(AiController::new(TeamId(2)));
        world.insert_resource(MatchSession {
            phase: MatchPhase::Playing,
        });
        assert!(!disable_ai_in_world(&mut world), "Playing: refuse");
        assert!(world.get_resource::<AiController>().is_some());
    }

    #[test]
    fn hidden_enemy_attackers_are_scrubbed_but_visible_ones_present() {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);
        // Runtime parity: presentation paths read the GridMap resource.
        world.insert_resource(map.clone());
        world.insert_resource(VisibilityMap::default());
        refresh_visibility(&mut world, &map);
        world.insert_resource(MatchSession {
            phase: MatchPhase::Playing,
        });

        let event_from = |attacker: UnitId| CombatEvent {
            attacker,
            target: CombatTarget::Unit(UnitId(1)),
            damage: 2,
            position: Vec2::new(11.5, 45.5),
            ranged: true,
            killed: false,
        };
        let enemy_at_home = UnitId(5);
        let missing = UnitId(999);
        world.insert_resource(CombatEvents(vec![
            event_from(UnitId(1)),
            event_from(enemy_at_home),
            event_from(missing),
        ]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(drained[0].attacker, UnitId(1), "friendly attacker stays");
        assert_eq!(
            drained[1].attacker,
            UnitId(0),
            "hidden enemy attacker must not expose its stable id"
        );
        assert_eq!(drained[1].position, event_from(enemy_at_home).position);
        assert_eq!(
            drained[2].attacker,
            UnitId(0),
            "missing attacker is scrubbed"
        );

        // March the enemy unit into Team-1 vision; its id becomes presentable.
        let enemy_entity = world.resource::<UnitIndex>().entity(enemy_at_home).unwrap();
        world
            .entity_mut(enemy_entity)
            .insert(SimPosition::new(map.cell_center(GridPos::new(22, 46))));
        refresh_visibility(&mut world, &map);
        world.insert_resource(CombatEvents(vec![event_from(enemy_at_home)]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(
            drained[0].attacker, enemy_at_home,
            "a visible enemy attacker may present"
        );
    }

    #[test]
    fn enemy_buildings_are_fog_hidden_and_friendly_ones_are_not() {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);
        world.insert_resource(VisibilityMap::default());
        refresh_visibility(&mut world, &map);

        let own = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .unwrap();
        let enemy = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(2))
            .unwrap();
        assert!(!building_hidden_from_local_player(&world, own));
        assert!(
            building_hidden_from_local_player(&world, enemy),
            "the far enemy Town Center starts hidden"
        );

        // A team-1 scout beside the enemy footprint clears it for presentation.
        let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        world
            .entity_mut(scout)
            .insert(SimPosition::new(map.cell_center(GridPos::new(108, 44))));
        refresh_visibility(&mut world, &map);
        assert!(!building_hidden_from_local_player(&world, enemy));
    }

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

        assert_eq!(idle_worker_ids(&world, TeamId(1)), vec![UnitId(1)]);
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

    /// The full initial-skirmish contract after setup/reset: seeded ids,
    /// indexes, stockpiles, a Start session, and no combat transients.
    fn assert_fixture_restored(world: &World) {
        let mut unit_ids: Vec<u32> = world
            .resource::<UnitIndex>()
            .iter()
            .map(|(id, _)| id.0)
            .collect();
        unit_ids.sort_unstable();
        assert_eq!(unit_ids, (1..=8).collect::<Vec<u32>>());

        let mut building_ids: Vec<u32> = world
            .resource::<BuildingIndex>()
            .iter()
            .map(|(id, _)| id.0)
            .collect();
        building_ids.sort_unstable();
        assert_eq!(building_ids, vec![1, 2]);

        assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);
        for team in [TeamId(1), TeamId(2)] {
            let stockpile = &world.resource::<TeamEconomy>().0[&team].stockpile;
            assert_eq!(
                (stockpile.food, stockpile.wood, stockpile.gold),
                (200, 300, 100)
            );
        }
        assert!(matches!(
            world.resource::<MatchSession>().phase,
            MatchPhase::Start
        ));
        assert!(world.get_resource::<CombatEvents>().is_none());
    }

    #[test]
    fn reset_and_restart_cycles_restore_the_fixture_once() {
        let mut world = World::new();
        setup_fixture(&mut world);
        assert_fixture_restored(&world);

        // Churn the running match: start, displace a unit, drain the
        // stockpile, settle a Result, and leave combat transients behind.
        start_match(&mut world);
        assert_eq!(world.resource::<MatchSession>().phase, MatchPhase::Playing);
        let unit = world.resource::<UnitIndex>().entity(UnitId(3)).unwrap();
        world
            .entity_mut(unit)
            .insert(SimPosition::new(Vec2::new(60.0, 60.0)));
        if let Some(mut economy) = world.get_resource_mut::<TeamEconomy>()
            && let Some(state) = economy.0.get_mut(&TeamId(1))
        {
            state.stockpile.food = 9999;
        }
        if let Some(mut session) = world.get_resource_mut::<MatchSession>() {
            session.phase = MatchPhase::Result(MatchResult(TeamId(2)));
        }
        world.insert_resource(CombatEvents(vec![CombatEvent {
            attacker: UnitId(1),
            target: CombatTarget::Unit(UnitId(2)),
            damage: 1,
            position: Vec2::ZERO,
            ranged: false,
            killed: false,
        }]));

        // Restart rides the reset seam; repeated cycles restore the initial
        // resources/entities/indexes exactly once, with no stale state.
        reset_fixture_world(&mut world);
        assert_fixture_restored(&world);
        reset_fixture_world(&mut world);
        assert_fixture_restored(&world);
    }

    /// Restart wipes everything a used match banked: pending human commands
    /// and explored fog. The AI's scout cursor, remembered Town Center and
    /// cadence accumulator ride the fresh `AiController` reseed — their deep
    /// reset is proven in grus-sim's `ai::tests`, which can read the private
    /// fields the bridge cannot.
    #[test]
    fn restart_clears_pending_commands_and_used_fog() {
        let mut world = World::new();
        setup_fixture(&mut world);
        start_match(&mut world);

        // A queued human command that never reached a fixed tick. The raw
        // test world has no app plugin, so insert the channel first.
        world.insert_resource(PendingCommands::default());
        world
            .resource_mut::<PendingCommands>()
            .0
            .push(PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units: vec![UnitId(1)],
                kind: UnitCommandKind::Move {
                    target: Vec2::new(60.5, 60.5),
                },
            }));

        // Use the fog: a scout beside the enemy start explores its Town
        // Center footprint.
        let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        let fixture = MapFixture::battlefield();
        let map = fixture.map;
        world
            .entity_mut(scout)
            .insert(SimPosition::new(map.cell_center(GridPos::new(108, 44))));
        let enemy_town_center_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(2))
            .unwrap();
        let enemy_town_center_footprint =
            *world.get::<Footprint>(enemy_town_center_entity).unwrap();
        refresh_visibility(&mut world, &map);
        assert!(explored_by(&world, TeamId(1), enemy_town_center_footprint));

        reset_fixture_world(&mut world);

        assert!(
            world.resource::<PendingCommands>().0.is_empty(),
            "restart must clear pending human commands"
        );
        assert!(matches!(
            world.resource::<MatchSession>().phase,
            MatchPhase::Start
        ));
        assert_eq!(world.resource::<AiController>().team, TeamId(2));
        let states = world
            .resource::<VisibilityMap>()
            .packed_cell_states(TeamId(1), 128, 96);
        let enemy_town_center = GridPos::new(112, 46);
        assert_eq!(
            states[(enemy_town_center.y * 128 + enemy_town_center.x) as usize],
            0,
            "restart must return the used fog to boot Unexplored"
        );
    }

    #[test]
    fn benchmark_world_moves_units_without_a_session() {
        let mut world = World::new();
        // A stale normal-skirmish session must not leak into the benchmark:
        // reset removes it, and the missing session reads as Playing.
        world.insert_resource(MatchSession {
            phase: MatchPhase::Result(MatchResult(TeamId(1))),
        });
        reset_benchmark_world(&mut world);
        assert!(world.get_resource::<MatchSession>().is_none());
        assert_eq!(world.resource::<UnitIndex>().iter().count(), 200);

        let command = PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: (1_u32..=100).map(UnitId).collect(),
            kind: UnitCommandKind::Move {
                target: MapFixture::battlefield().right_spawn,
            },
        });
        world.resource_scope(|world, mut map: Mut<GridMap>| {
            let result = apply_player_command(world, &mut map, command);
            assert_eq!(result.reject, None);
            assert!(result.rejected_units.is_empty());
        });

        let positions_before: Vec<Vec2> = (1_u32..=100)
            .map(|id| {
                let entity = world.resource::<UnitIndex>().entity(UnitId(id)).unwrap();
                world.get::<SimPosition>(entity).unwrap().current
            })
            .collect();
        let map = world.resource::<GridMap>().clone();
        for _ in 0..20 {
            step_movement(&mut world, &map, SIM_STEP_SECONDS);
        }
        for (index, id) in (1_u32..=100).enumerate() {
            let entity = world.resource::<UnitIndex>().entity(UnitId(id)).unwrap();
            let position = world.get::<SimPosition>(entity).unwrap().current;
            assert!(
                position.distance(positions_before[index]) > 0.5,
                "benchmark unit {id} never moved without Start interaction"
            );
        }
    }
}
