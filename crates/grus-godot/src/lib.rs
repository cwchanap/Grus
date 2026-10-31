use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use godot::builtin::{GString, PackedInt32Array, VarDictionary, Vector2};
use godot::classes::{Engine, INode, Node, Node3D, SceneTree};
use godot::prelude::*;
use godot_bevy::BevyApp;
use godot_bevy::prelude::*;
use grus_sim::catalog::{ResourceKind, building_spec, unit_spec};
use grus_sim::{
    BASE_GATHER_RATE, Building, BuildingId, BuildingIndex, BuildingKind, CombatEvent,
    CombatEventTotal, CombatEvents, CombatTarget, CommandResult, EntryZone, Footprint, GridMap,
    GridPos, Health, HeatState, IdAllocator, LastRouteReject, MapFixture, MatchPhase, MatchSession,
    PlayerCommand, ProductionJob, ProductionKind, ProductionQueue, RUINS, RallyPoint, RejectReason,
    ResourceId, ResourceIndex, ResourceSource, SIM_STEP_SECONDS, SimPosition, TeamEconomy, TeamId,
    Unit, UnitCommand, UnitCommandKind, UnitId, UnitIndex, UnitKind, VisibilityMap, WaveState,
    WorkerTask, active_phase, apply_player_command, explored_by, extend_with_benchmark_workload,
    idle_worker_ids, live_alien_ids, population_cap, population_used, produces, refresh_visibility,
    resolve_target, seed_skirmish, set_paused, start_match, step_tick, surface_efficiency,
    target_eligible, validate_placement, visible_to,
};

#[cfg(feature = "e2e")]
mod e2e;

#[derive(Default, Resource)]
struct PendingCommands(Vec<PendingCommand>);

struct PendingCommand {
    command: PlayerCommand,
    placement_request_id: Option<i64>,
}

#[derive(Default, Resource)]
struct PlacementCommandResults(HashMap<i64, PlacementCommandResult>);

struct PlacementCommandResult {
    accepted: bool,
    feedback: String,
}

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
    /// Monotonic ticket counter behind `next_placement_request_id`; node
    /// state outlives a controller reload, so ids stay unique while their
    /// results remain pending.
    next_placement_request_id: i64,
}

#[godot_api]
impl INode for GrusBridgeNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            visibility_cache: None,
            next_placement_request_id: 1,
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

        queue_command(
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units,
                kind: UnitCommandKind::Move {
                    target: Vec2::new(target.x, target.y),
                },
            }),
            None,
        )
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
        queue_command(
            PlayerCommand::Attack {
                issuer: TeamId(1),
                units,
                target,
            },
            None,
        )
    }

    #[func]
    fn attack_move_units(&self, packed_ids: PackedInt32Array, target: Vector2) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }
        queue_command(
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units,
                kind: UnitCommandKind::AttackMove {
                    target: Vec2::new(target.x, target.y),
                },
            }),
            None,
        )
    }

    #[func]
    fn stop_units(&self, packed_ids: PackedInt32Array) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }

        queue_command(
            PlayerCommand::Units(UnitCommand {
                issuer: TeamId(1),
                units,
                kind: UnitCommandKind::Stop,
            }),
            None,
        )
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
        queue_command(
            PlayerCommand::Gather {
                issuer: TeamId(1),
                workers,
                source: ResourceId(source),
            },
            None,
        )
    }

    #[func]
    fn place_building(&self, builder_id: i32, kind: GString, anchor_x: i32, anchor_y: i32) -> bool {
        queue_place_building(builder_id, &kind, anchor_x, anchor_y, None)
    }

    /// Queue a building placement and retain its authoritative result until
    /// the requesting presentation code consumes it. The shared feedback
    /// surface only keeps the latest result per fixed tick, so this separate
    /// ticket prevents another command from masking this placement's result.
    #[func]
    fn place_building_request(
        &self,
        builder_id: i32,
        kind: GString,
        anchor_x: i32,
        anchor_y: i32,
        request_id: i64,
    ) -> bool {
        request_id > 0
            && queue_place_building(builder_id, &kind, anchor_x, anchor_y, Some(request_id))
    }

    /// Placement request tickets handed to GDScript callers. The counter
    /// lives on this autoload node — the same lifetime as the world-side
    /// results map — so a reloaded controller cannot reuse an id whose
    /// result is still pending there.
    #[func]
    fn next_placement_request_id(&mut self) -> i64 {
        let id = self.next_placement_request_id;
        self.next_placement_request_id += 1;
        id
    }

    /// Consume one request-scoped placement result. An empty dictionary means
    /// the fixed-step command has not completed yet.
    #[func]
    fn take_placement_command_result(&mut self, request_id: i64) -> VarDictionary {
        let Some(mut app_node) = bevy_app_singleton() else {
            return VarDictionary::new();
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return VarDictionary::new();
        };
        let Some(mut results) = app
            .world_mut()
            .get_resource_mut::<PlacementCommandResults>()
        else {
            return VarDictionary::new();
        };
        let Some(result) = results.0.remove(&request_id) else {
            return VarDictionary::new();
        };
        let mut dict = VarDictionary::new();
        dict.set("complete", true);
        dict.set("accepted", result.accepted);
        dict.set("feedback", &GString::from(result.feedback.as_str()));
        dict
    }

    #[func]
    fn resume_construction(&self, builder_id: i32, building_id: i32) -> bool {
        let Ok(builder) = u32::try_from(builder_id) else {
            return false;
        };
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(
            PlayerCommand::ResumeConstruction {
                issuer: TeamId(1),
                builder: UnitId(builder),
                building: BuildingId(building),
            },
            None,
        )
    }

    #[func]
    fn enqueue_unit(&self, building_id: i32, kind: GString) -> bool {
        let Some(kind) = parse_unit_kind(&kind) else {
            return false;
        };
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(
            PlayerCommand::EnqueueUnit {
                issuer: TeamId(1),
                building: BuildingId(building),
                kind,
            },
            None,
        )
    }

    #[func]
    fn set_rally(&self, building_id: i32, target_x: i32, target_y: i32) -> bool {
        let Ok(building) = u32::try_from(building_id) else {
            return false;
        };
        queue_command(
            PlayerCommand::SetRally {
                issuer: TeamId(1),
                building: BuildingId(building),
                target: GridPos::new(target_x, target_y),
            },
            None,
        )
    }

    #[func]
    fn reset_fixture(&mut self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };

        reset_fixture_world(app.world_mut());
        // A fresh map can reuse a revision the bridge already cached from
        // the previous match (both boot at 1), and the cached payload would
        // then outlive the reset — drop it so the next fetch rebuilds from
        // the reseeded map.
        self.visibility_cache = None;
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

    /// Read-only wave state for smoke assertions and the HUD/minimap. Zone
    /// names and fixture anchors are present only during the 30-second
    /// warning; individual alien positions remain behind normal fog views.
    #[func]
    fn wave_snapshot(&self) -> VarDictionary {
        with_app(|app| wave_snapshot_dict(app.world())).unwrap_or_default()
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
    fn restart_match(&mut self) -> bool {
        self.reset_fixture()
    }

    #[func]
    fn reset_benchmark_fixture(&mut self) -> bool {
        let Some(mut app_node) = bevy_app_singleton() else {
            return false;
        };
        let mut app_node = app_node.bind_mut();
        let Some(app) = app_node.get_app_mut() else {
            return false;
        };

        reset_benchmark_world(app.world_mut());
        // Same stale-cache hazard as the fixture reset: the benchmark's fresh
        // map can reuse a revision the bridge already cached from the
        // previous match, and the cached payload would then outlive the
        // reset — drop it so the next fetch rebuilds from the reseeded map.
        self.visibility_cache = None;
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

    /// Monotonic count of strikes recorded in the world: the benchmark's
    /// liveness checkpoint diffs it between windows instead of calling
    /// `drain_combat_events`, whose buffer the battlefield controller
    /// already drains every rendered frame for cosmetics. 0 when no combat
    /// has run in the current world; reset paths drop it with the match.
    #[func]
    fn combat_event_total(&self) -> i64 {
        with_app(|app| {
            app.world()
                .get_resource::<CombatEventTotal>()
                .map(|total| total.0 as i64)
                .unwrap_or(0)
        })
        .unwrap_or(0)
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
    /// when no visibility state exists.
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

    /// The authored obstacle rectangles: flat quads
    /// `[min_x, min_y, max_x, max_y, ...]` of grid cells with INCLUSIVE
    /// maximum bounds — the same `fixture::RUINS` table the simulation
    /// blocks through `GridMap::set_blocked_rect`. Static data, so the
    /// snapshot is safe to read before the first fixed tick; the
    /// presentation layer derives mesh centers and sizes from this payload
    /// and never keeps a second coordinate list.
    #[func]
    fn obstacle_rects(&self) -> PackedInt32Array {
        let mut rects = PackedInt32Array::new();
        for (min, max) in RUINS {
            rects.push(min.x);
            rects.push(min.y);
            rects.push(max.x);
            rects.push(max.y);
        }
        rects
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

    /// Team 1 economy state: stockpile, population, idle workers, the
    /// current gather rate, current surface heat, and the latest rejection
    /// code.
    /// `last_reject_code` follows `RejectReason` discriminant order (-1 =
    /// none); `gather_rate` is the team's current resources/second;
    /// `heat_supply`/`heat_demand`/`surface_efficiency` mirror the sim's
    /// heat totals with the one neutral-default efficiency accessor (an
    /// absent state reads 0/0 at full efficiency).
    #[func]
    fn economy_snapshot(&self) -> VarDictionary {
        with_app(|app| {
            let world = app.world();
            let mut dict = VarDictionary::new();
            let state = world
                .get_resource::<TeamEconomy>()
                .and_then(|economy| economy.0.get(&TeamId(1)));
            let stockpile = state.map(|state| state.stockpile).unwrap_or_default();
            dict.set("food", i64::from(stockpile.food));
            dict.set("scrap", i64::from(stockpile.scrap));
            dict.set("coal", i64::from(stockpile.coal));
            dict.set("crystals", i64::from(stockpile.crystals));
            dict.set(
                "population_used",
                i64::from(population_used(world, TeamId(1))),
            );
            dict.set(
                "population_cap",
                i64::from(population_cap(world, TeamId(1))),
            );
            dict.set("gather_rate", f64::from(BASE_GATHER_RATE));
            let (heat_supply, heat_demand, efficiency) = heat_fields(world);
            dict.set("heat_supply", i64::from(heat_supply));
            dict.set("heat_demand", i64::from(heat_demand));
            dict.set("surface_efficiency", f64::from(efficiency));
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

    /// One unit's `WorkerTask` for command round-trip assertions: the task
    /// variant name plus the resource/building ids it names (-1 when the
    /// task names none). Empty dict for a missing unit, a unit with no
    /// worker task, or a fog-hidden enemy — task data must not leak
    /// through stale ids.
    #[func]
    fn unit_task_snapshot(&self, unit_id: i32) -> VarDictionary {
        with_app(|app| {
            let world = app.world();
            let mut dict = VarDictionary::new();
            let Ok(raw_id) = u32::try_from(unit_id) else {
                return dict;
            };
            let Some(entity) = world
                .get_resource::<UnitIndex>()
                .and_then(|index| index.entity(UnitId(raw_id)))
            else {
                return dict;
            };
            if unit_hidden_from_local_player(world, entity) {
                return dict;
            }
            let Some(task) = world.get::<WorkerTask>(entity) else {
                return dict;
            };
            let (name, source, building) = match task {
                WorkerTask::Idle => ("Idle", -1, -1),
                WorkerTask::ToSource { source, .. } => ("ToSource", i64::from(source.0), -1),
                WorkerTask::Gathering { source } => ("Gathering", i64::from(source.0), -1),
                WorkerTask::ToDropoff {
                    source, dropoff, ..
                } => ("ToDropoff", i64::from(source.0), i64::from(dropoff.0)),
                WorkerTask::ToConstruction { building, .. } => {
                    ("ToConstruction", -1, i64::from(building.0))
                }
                WorkerTask::Constructing { building } => {
                    ("Constructing", -1, i64::from(building.0))
                }
            };
            dict.set("task", &GString::from(name));
            dict.set("source", source);
            dict.set("building", building);
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
    /// - "units": {kind -> {food, scrap, coal, crystals, seconds,
    ///   producer}} — train cost (all four currencies, always present),
    ///   train seconds, and the producing building kind name.
    /// - "buildings": {kind -> {food, scrap, coal, crystals, seconds,
    ///   width, height, population}} — build cost, build seconds,
    ///   footprint, and population capacity contribution.
    #[func]
    fn catalogue_snapshot(&self) -> VarDictionary {
        catalogue_snapshot_dict()
    }
}

/// The one four-field cost serialization: every cost dict carries all four
/// currencies so GDScript never has to guess which fields exist.
fn cost_dict(cost: grus_sim::catalog::Cost) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("food", i64::from(cost.food));
    dict.set("scrap", i64::from(cost.scrap));
    dict.set("coal", i64::from(cost.coal));
    dict.set("crystals", i64::from(cost.crystals));
    dict
}

/// Catalogue payload behind `catalogue_snapshot`, extracted so the bridge
/// method stays a thin `#[func]` wrapper. It builds Godot dictionaries, so
/// it can only run with the engine loaded — exercised through the GDScript
/// smokes, not unit tests.
fn catalogue_snapshot_dict() -> VarDictionary {
    let mut units = VarDictionary::new();
    for kind in UnitKind::HUMAN {
        let spec = unit_spec(kind);
        let mut entry = cost_dict(spec.cost);
        entry.set("seconds", i64::from(spec.train_seconds));
        entry.set("producer", &producer_kind_name(kind));
        units.set(&kind_name(kind), &entry.to_variant());
    }
    let mut buildings = VarDictionary::new();
    for kind in BuildingKind::ALL {
        let spec = building_spec(kind);
        let mut entry = cost_dict(spec.cost);
        entry.set("seconds", i64::from(spec.build_seconds));
        entry.set("width", i64::from(spec.width));
        entry.set("height", i64::from(spec.height));
        entry.set("population", i64::from(spec.population_capacity));
        buildings.set(&kind_name(kind), &entry.to_variant());
    }
    let mut catalogue = VarDictionary::new();
    catalogue.set("units", &units.to_variant());
    catalogue.set("buildings", &buildings.to_variant());
    catalogue
}

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotAssetsPlugin)
        .add_plugins(GodotTransformSyncPlugin::default())
        .add_plugins(GodotPackedScenePlugin)
        .insert_resource(Time::<Fixed>::from_seconds(f64::from(SIM_STEP_SECONDS)))
        .init_resource::<PendingCommands>()
        .init_resource::<PlacementCommandResults>()
        .init_resource::<CommandFeedback>()
        .init_resource::<LastRouteReject>()
        .add_systems(Startup, setup_fixture)
        .add_systems(
            Update,
            (
                attach_missing_gameplay_views,
                initialize_view_metadata,
                sync_view_visibility,
                sync_interpolated_unit_transforms,
                update_health_bars,
                update_cold_tints,
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                apply_pending_commands,
                advance_sim,
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
    parse_unit_kind_str(name.to_string().as_str())
}

/// Engine-free `&str` core so unit tests can drive the parser (a `GString`
/// needs a live Godot engine, which unit tests do not have). The accepted
/// spellings come from the same `kind_name_str` the outbound serializers
/// use, so the wire name can never drift between directions.
fn parse_unit_kind_str(name: &str) -> Option<UnitKind> {
    UnitKind::HUMAN
        .into_iter()
        .find(|kind| kind_name_str(kind) == name)
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
    parse_building_kind_str(name.to_string().as_str())
}

fn parse_building_kind_str(name: &str) -> Option<BuildingKind> {
    // Every catalogue name parses — the seeded-only ResearchCenter included;
    // its placement rejection lives in the sim (`Locked`), not the parser.
    BuildingKind::ALL
        .into_iter()
        .find(|kind| kind_name_str(kind) == name)
}

/// The single engine-free kind wire name: outbound serializers
/// (`kind_name`, `debug_variant`) and both inbound `parse_*_kind_str`
/// cores share it, so the accepted spellings are defined exactly once.
fn kind_name_str(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

fn debug_variant(value: impl std::fmt::Debug) -> Variant {
    GString::from(kind_name_str(value).as_str()).to_variant()
}

fn kind_name(value: impl std::fmt::Debug) -> GString {
    GString::from(kind_name_str(value).as_str())
}

/// The building kind that trains the given unit, per the fixed producer
/// compatibility table (Research Center trains Workers, Barracks Guards
/// and Riflemen).
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

/// Shared placement queueing for `place_building`/`place_building_request`:
/// parses the kind and builder once and pushes the pending command, with
/// the result ticket attached only for the request-scoped variant.
fn queue_place_building(
    builder_id: i32,
    kind: &GString,
    anchor_x: i32,
    anchor_y: i32,
    request_id: Option<i64>,
) -> bool {
    let Some(kind) = parse_building_kind(kind) else {
        return false;
    };
    let Ok(builder) = u32::try_from(builder_id) else {
        return false;
    };
    queue_command(
        PlayerCommand::PlaceBuilding {
            issuer: TeamId(1),
            builder: UnitId(builder),
            kind,
            anchor: GridPos::new(anchor_x, anchor_y),
        },
        request_id,
    )
}

fn queue_command(command: PlayerCommand, placement_request_id: Option<i64>) -> bool {
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
    pending.0.push(PendingCommand {
        command,
        placement_request_id,
    });
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
    // attacker is a hidden enemy — so hit/death feedback stays intact. Unit(0)
    // is the one absent identity, with no original kind to leak.
    for event in &mut events {
        if !attacker_presentable(world, event.attacker) {
            event.attacker = CombatTarget::Unit(UnitId(0));
        }
    }
    events
}

/// The attacker's stable identity may reach GDScript only when it exists and
/// is either friendly or currently visible to the local team. Missing or
/// fog-hidden units/buildings are scrubbed to the same absent identity.
/// Enemy attackers reuse the sim's one eligibility rule (foreign, live,
/// currently visible) instead of a parallel per-kind copy.
fn attacker_presentable(world: &World, attacker: CombatTarget) -> bool {
    resolve_target(world, attacker).is_some_and(|(_, team, health)| {
        health.current > 0 && (team == TeamId(1) || target_eligible(world, TeamId(1), attacker))
    })
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

/// A unit is fog-hidden from the local player when it is an enemy whose
/// current cell is not Team-1 visible. Friendly units are always
/// presentable — the `unit_task_snapshot` mirror of
/// `building_hidden_from_local_player`.
fn unit_hidden_from_local_player(world: &World, entity: Entity) -> bool {
    let Some(unit) = world.get::<Unit>(entity) else {
        return true;
    };
    if unit.team == TeamId(1) {
        return false;
    }
    let (Some(map), Some(position)) = (
        world.get_resource::<GridMap>(),
        world.get::<SimPosition>(entity),
    ) else {
        return true;
    };
    !visible_to(world, TeamId(1), map.world_to_cell(position.current))
}

fn combat_event_dict(event: CombatEvent) -> VarDictionary {
    let mut dict = VarDictionary::new();
    let (attacker_kind, attacker_id) = combat_attacker_kind_and_id(event.attacker);
    dict.set("attacker_kind", &GString::from(attacker_kind));
    dict.set("attacker", i64::from(attacker_id));
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

fn combat_attacker_kind_and_id(attacker: CombatTarget) -> (&'static str, u32) {
    match attacker {
        CombatTarget::Unit(id) => ("unit", id.0),
        CombatTarget::Building(id) => ("building", id.0),
    }
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
    seed_match_world(world);
    // Normal Godot setup boots into Start; start_match opens gameplay.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
}

/// The one shared match seed path for boot, restart and the benchmark:
/// seeds the authored skirmish, inserts the map, and inserts fresh Team-1
/// fog with an initial refresh. Deliberately does NOT insert a
/// `MatchSession` — normal callers add `Start`; the benchmark stays
/// session-free so the missing session (Playing forever) can never freeze
/// the load.
fn seed_match_world(world: &mut World) {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    seed_skirmish(world, &mut map, &fixture);
    world.insert_resource(map);
    insert_fresh_visibility(world);
}

/// Normal runtime gameplay runs fogged: a fresh `VisibilityMap` beside the
/// session plus one initial refresh so the Start screen already has correct
/// fog. Every seed path calls this through `seed_match_world`.
fn insert_fresh_visibility(world: &mut World) {
    world.insert_resource(VisibilityMap::default());
    world.resource_scope(|world, map: Mut<GridMap>| refresh_visibility(world, &map));
}

#[allow(clippy::type_complexity)]
fn attach_missing_gameplay_views(
    mut commands: Commands,
    units: Query<(Entity, &SimPosition), (With<Unit>, Without<GameplayViewRequested>)>,
    buildings: Query<(Entity, &Footprint), (With<Building>, Without<GameplayViewRequested>)>,
    standalone_resources: Query<
        (Entity, &Footprint),
        (With<ResourceSource>, Without<GameplayViewRequested>),
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
    for (entity, footprint) in &buildings {
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
        &GodotNodeHandle,
    ), Without<ViewMetaInitialized>>();
    let building_batch: Vec<(
        Entity,
        BuildingId,
        BuildingKind,
        TeamId,
        Footprint,
        GodotNodeHandle,
    )> = buildings
        .iter(world)
        .map(|(entity, building, footprint, handle)| {
            (
                entity,
                building.id,
                building.kind,
                building.team,
                *footprint,
                *handle,
            )
        })
        .collect();
    let mut resources = world.query_filtered::<(
        Entity,
        &ResourceSource,
        &Footprint,
        &GodotNodeHandle,
    ), Without<ViewMetaInitialized>>();
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
    for (entity, id, kind, team, footprint, handle) in building_batch {
        let Some(mut node) = node_from_handle(handle) else {
            continue;
        };
        node.set_meta("building_id", &i64::from(id.0).to_variant());
        node.set_meta("building_kind", &debug_variant(kind));
        node.set_meta("team_id", &i64::from(team.0).to_variant());
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
        world.entity_mut(entity).insert(ViewMetaInitialized);
    }
}

/// The exclusive pass can't hold a query borrow while mutating entities, so
/// it batches first; this helper only unwraps node handles.
fn node_from_handle(handle: GodotNodeHandle) -> Option<Gd<Node3D>> {
    Gd::<Node3D>::try_from_instance_id(handle.instance_id()).ok()
}

/// Maintains every gameplay view's `Node3D.visible` from the authoritative
/// Team-1 visibility each Update: friendly units/buildings always visible,
/// enemies only while currently visible, standalone resources once
/// explored and staying.
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
    let mut resources =
        world.query_filtered::<(&Footprint, &GodotNodeHandle), With<ResourceSource>>();
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

/// Engine-free heat triple behind `economy_snapshot`'s heat fields: the
/// live supply/demand totals and the exact fractional efficiency. A world
/// without `HeatState` reports 0/0 at full efficiency — the sim's own
/// neutral default, never a fake deficit.
fn heat_fields(world: &World) -> (u32, u32, f32) {
    let heat = world
        .get_resource::<HeatState>()
        .copied()
        .unwrap_or_default();
    (heat.supply, heat.demand, surface_efficiency(world))
}

/// The catalogue-derived cold condition for one building view: Team 1,
/// construction complete, nonzero catalogue heat demand, and the live
/// surface efficiency below 1. Zero-demand kinds (Wall, Gate, Furnace)
/// can never read cold by construction.
fn building_is_cold(world: &World, team: TeamId, kind: BuildingKind, complete: bool) -> bool {
    team == TeamId(1)
        && complete
        && building_spec(kind).heat_demand > 0
        && surface_efficiency(world) < 1.0
}

/// Pushes each building view's cold state from Rust every Update — the
/// per-view projection beside `update_health_bars`, never per-building
/// GDScript snapshot polling. Re-evaluating the value every frame covers
/// late-attached views and heating recovery; the view script caches the
/// flag and skips redundant material writes.
fn update_cold_tints(world: &mut World) {
    let mut query = world.query::<(&Building, &GodotNodeHandle)>();
    let batch: Vec<(TeamId, BuildingKind, bool, GodotNodeHandle)> = query
        .iter(world)
        .map(|(building, handle)| {
            (
                building.team,
                building.kind,
                building.construction.complete,
                *handle,
            )
        })
        .collect();
    for (team, kind, complete, handle) in batch {
        set_view_cold(handle, building_is_cold(world, team, kind, complete));
    }
}

/// Value-gated write; the view caches the flag so warm buildings skip the
/// material write entirely.
fn set_view_cold(handle: GodotNodeHandle, cold: bool) {
    let Some(mut node) = node_from_handle(handle) else {
        return;
    };
    node.call("set_cold", &[cold.to_variant()]);
}

fn reset_fixture_world(world: &mut World) {
    clear_gameplay_world(world);
    seed_match_world(world);
    // A normal-skirmish restart returns the session to Start — fogged with a
    // fresh exploration state, same as a first boot.
    world.insert_resource(MatchSession {
        phase: MatchPhase::Start,
    });
    // The clear despawns the selector-bearing gameplay entities; reattach the
    // e2e selector surface to the reseeded fixture. Inert without BEVY_E2E=1.
    #[cfg(feature = "e2e")]
    e2e::attach_selectors(world);
}

fn reset_benchmark_world(world: &mut World) {
    clear_gameplay_world(world);
    // The benchmark always measures the authored load fixture through the
    // one ordinary start seed path.
    seed_match_world(world);
    // The benchmark extends the normal seed with the pure mixed workload —
    // roster, staging and initial orders belong to grus-sim — and inserts
    // no session: the missing session reads as Playing forever, which is
    // what keeps the load test from freezing on a settled match.
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        extend_with_benchmark_workload(world, &mut map);
    });
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
    // Visibility dies with the match: every seed path re-inserts a fresh
    // fogged map through `seed_match_world`.
    world.remove_resource::<VisibilityMap>();
    // Combat/session transients die with the match: the benchmark restarts
    // session-free (missing session = Playing), the normal reset re-inserts
    // Start.
    world.remove_resource::<CombatEvents>();
    world.remove_resource::<CombatEventTotal>();
    world.remove_resource::<WaveState>();
    // Heat dies with the match: a stale supply/demand between clear and
    // reseed would read wrong, while absence reads neutral full efficiency.
    world.remove_resource::<HeatState>();
    world.remove_resource::<MatchSession>();
    if let Some(mut pending) = world.get_resource_mut::<PendingCommands>() {
        pending.0.clear();
    }
    if let Some(mut results) = world.get_resource_mut::<PlacementCommandResults>() {
        results.0.clear();
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
        for pending in commands {
            let result = apply_player_command(world, &mut map, pending.command);
            let reject_code = result
                .reject
                .or_else(|| result.rejected_units.first().map(|(_, reason)| *reason));
            let feedback = format_command_result(&result);
            if let Some(request_id) = pending.placement_request_id
                && let Some(mut results) = world.get_resource_mut::<PlacementCommandResults>()
            {
                results.0.insert(
                    request_id,
                    PlacementCommandResult {
                        accepted: result.reject.is_none(),
                        feedback: feedback.clone(),
                    },
                );
            }
            latest_feedback = Some((feedback, reject_code));
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

    // Per-unit rejects outside the three counted kinds (e.g. Crowded,
    // NotWorker) still name the first uncounted typed code instead of
    // collapsing into a zero-count summary.
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

// ponytail: advance_sim takes one coarse Time<Fixed>::delta() per tick
// (speed 20 → 1.0 sim-s per 0.05 s tick); sub-step movement/economy inside
// the tick if a high-speed scenario ever shows tunneling or overshoot.
/// The one fixed-step gameplay system: reads the fixed delta and runs the
/// canonical grus-sim `step_tick` order inside a single scoped `GridMap`
/// borrow. Command application runs before it and the route-reject drain
/// after it; everything between them is `step_tick`'s contract.
fn advance_sim(world: &mut World) {
    let seconds = world.resource::<Time<Fixed>>().delta().as_secs_f32();
    world.resource_scope(|world, mut map: Mut<GridMap>| {
        step_tick(world, &mut map, seconds);
    });
}

fn wave_snapshot_dict(world: &World) -> VarDictionary {
    let state = world
        .get_resource::<WaveState>()
        .copied()
        .unwrap_or_default();
    let incoming_zones: Array<VarDictionary> = state
        .incoming_zones()
        .unwrap_or_default()
        .into_iter()
        .map(|zone| wave_zone_dict(zone, zone.anchor()))
        .collect();
    let live_aliens = live_alien_ids(world, TeamId(2)).len();

    let mut dict = VarDictionary::new();
    dict.set(
        "wave",
        i64::try_from(state.next_wave.saturating_sub(1)).unwrap_or(i64::MAX),
    );
    dict.set(
        "next_wave",
        i64::try_from(state.next_wave).unwrap_or(i64::MAX),
    );
    dict.set("seconds_remaining", state.remaining_seconds());
    dict.set("incoming_zones", &incoming_zones);
    dict.set(
        "live_aliens",
        i64::try_from(live_aliens).unwrap_or(i64::MAX),
    );
    dict
}

fn wave_zone_dict(zone: EntryZone, anchor: GridPos) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("name", &kind_name(zone));
    dict.set("anchor_x", i64::from(anchor.x));
    dict.set("anchor_y", i64::from(anchor.y));
    dict
}

/// Drains the sim's latest Team-1 worker route-failure reject into the
/// existing feedback channel, so mid-step idles surface as typed codes with
/// a revision bump instead of disappearing silently. Team 2's entries stay
/// in the sim resource: only the human player's rejects surface as — or
/// overwrite — the feedback channel.
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
    }
}

fn queue_head_label(job: &ProductionJob) -> String {
    match job.kind {
        ProductionKind::Unit(kind) => format!("{kind:?}"),
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
    use grus_sim::seed_skirmish;
    use grus_sim::step_heat;
    use grus_sim::{
        AttackCooldown, CombatEvent, CombatTarget, ConstructionState, MatchResult, MoveOrder,
        WorkerTask, sorted_unit_ids, spawn_unit,
    };

    use super::*;

    /// Enqueue parsing accepts only human trainable units; aliens remain in
    /// the full catalogue for simulation and presentation. Building parsing
    /// still walks the complete building table. The tests run engine-free.
    #[test]
    fn unit_kind_parser_accepts_only_the_human_catalogue_names() {
        for kind in UnitKind::HUMAN {
            let name = kind_name_str(kind);
            assert_eq!(parse_unit_kind_str(&name), Some(kind), "{name} must parse");
        }
        for kind in [
            UnitKind::Swarmer,
            UnitKind::Spitter,
            UnitKind::Brute,
            UnitKind::Behemoth,
        ] {
            let name = kind_name_str(kind);
            assert_eq!(parse_unit_kind_str(&name), None, "{name} must not enqueue");
        }
        for retired in [
            "Villager", "Spearman", "Archer", "Cavalry", "worker", "Worker ", "", "Cactus",
        ] {
            assert_eq!(
                parse_unit_kind_str(retired),
                None,
                "retired/foreign name {retired} must reject"
            );
        }
    }

    /// Building names behave the same: every `BuildingKind::ALL` entry
    /// parses — including the seeded-only ResearchCenter, whose placement
    /// rejection lives in the sim (`Locked`), not the parser.
    #[test]
    fn building_kind_parser_accepts_exactly_the_catalogue_names() {
        for kind in BuildingKind::ALL {
            let name = kind_name_str(kind);
            assert_eq!(
                parse_building_kind_str(&name),
                Some(kind),
                "{name} must parse"
            );
        }
        for retired in [
            "House",
            "Storehouse",
            "TownCenter",
            "Farm",
            "ArcheryRange",
            "Stable",
            "bunkhouse",
            "",
        ] {
            assert_eq!(
                parse_building_kind_str(retired),
                None,
                "retired/foreign name {retired} must reject"
            );
        }
    }

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
            attacker: CombatTarget::Unit(UnitId(1)),
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
        // presentation permission) cannot be verified: the identity is
        // scrubbed to the uniform absent unit while hit feedback survives.
        let mut scrubbed = event;
        scrubbed.attacker = CombatTarget::Unit(UnitId(0));

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

    #[test]
    fn combat_event_payload_identity_preserves_unit_and_building_kinds() {
        for (attacker, expected_kind, expected_id) in [
            (CombatTarget::Unit(UnitId(7)), "unit", 7),
            (CombatTarget::Building(BuildingId(8)), "building", 8),
        ] {
            assert_eq!(
                combat_attacker_kind_and_id(attacker),
                (expected_kind, expected_id)
            );
        }
    }

    /// Runtime setup fog: the fixture boots with a fresh `VisibilityMap`, an
    /// initial refresh (revision >= 1), the enemy start Unexplored and the
    /// own start Visible. Restarts and the benchmark reset re-fog the same
    /// way through the shared seed path.
    #[test]
    fn runtime_setup_and_resets_manage_the_visibility_resource() {
        let mut world = World::new();
        setup_fixture(&mut world);

        let revision = world.resource::<VisibilityMap>().revision();
        assert!(
            revision >= 1,
            "initial refresh must have bumped the revision"
        );
        let own_start = MapFixture::team_starts()[0];
        let own_research_center = own_start.research_center_anchor;
        let entry_anchor = EntryZone::North.anchor();
        let states = world
            .resource::<VisibilityMap>()
            .packed_cell_states(TeamId(1), 128, 96);
        assert_eq!(
            states[(own_research_center.y * 128 + own_research_center.x) as usize],
            2
        );
        assert_eq!(
            states[(entry_anchor.y * 128 + entry_anchor.x) as usize],
            0,
            "alien entry anchors must boot Unexplored — runtime is not full-info"
        );

        // A normal restart re-inserts a fresh fogged map.
        reset_fixture_world(&mut world);
        assert!(world.get_resource::<VisibilityMap>().is_some());
        assert!(world.resource::<VisibilityMap>().revision() >= 1);

        // The benchmark reset re-inserts a fresh fogged map too: the mixed
        // workload runs under the same fog rules as the runtime skirmish.
        reset_benchmark_world(&mut world);
        assert!(world.get_resource::<VisibilityMap>().is_some());
        assert!(
            world.resource::<VisibilityMap>().revision() >= 1,
            "the benchmark fog must be refreshed, not a stale empty map"
        );
    }

    /// Runtime boot seeds the one authored human core and no passive enemy
    /// base.
    #[test]
    fn setup_fixture_seeds_only_the_authored_human_start() {
        let mut world = World::new();
        setup_fixture(&mut world);
        let start = MapFixture::team_starts()[0];
        assert_eq!(
            research_center_anchor(&world, TeamId(1)),
            start.research_center_anchor,
            "boot seeds the authored human core"
        );
        assert_eq!(world.resource::<BuildingIndex>().iter().count(), 1);
        assert!(world.resource::<TeamEconomy>().0.contains_key(&TeamId(1)));
        assert!(!world.resource::<TeamEconomy>().0.contains_key(&TeamId(2)));
    }

    fn research_center_anchor(world: &World, team: TeamId) -> GridPos {
        let entity = world
            .resource::<BuildingIndex>()
            .iter()
            .find_map(|(_, entity)| {
                world
                    .get::<Building>(*entity)
                    .is_some_and(|building| {
                        building.team == team && building.kind == BuildingKind::ResearchCenter
                    })
                    .then_some(*entity)
            })
            .expect("a seeded Research Center per team");
        world.get::<Footprint>(entity).unwrap().anchor
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

        let event_from = |attacker: CombatTarget| CombatEvent {
            attacker,
            target: CombatTarget::Unit(UnitId(1)),
            damage: 2,
            position: Vec2::new(11.5, 45.5),
            ranged: true,
            killed: false,
        };
        let enemy_at_home = UnitId(5);
        spawn_unit(
            &mut world,
            enemy_at_home,
            TeamId(2),
            map.cell_center(EntryZone::East.anchor()),
            UnitKind::Swarmer,
            unit_spec(UnitKind::Swarmer).speed,
        );
        let seen_cell = Footprint::new(MapFixture::team_starts()[0].research_center_anchor, 4, 4)
            .perimeter_cells()[0];
        let home_anchor = MapFixture::team_starts()[0].research_center_anchor;
        let visible_anchor = GridPos::new(home_anchor.x + 1, home_anchor.y - 2);
        let hidden_anchor =
            GridPos::new(EntryZone::East.anchor().x - 4, EntryZone::East.anchor().y);
        // Enemy towers are seeded directly like the sim's completed-building
        // helper: Team-2 issuers cannot place Gate/GunTurret through the
        // command path at all.
        let seed_attacker_tower = |world: &mut World, map: &mut GridMap, anchor: GridPos| {
            let spec = building_spec(BuildingKind::GunTurret);
            let footprint = Footprint::new(anchor, spec.width, spec.height);
            for cell in footprint.cells() {
                map.set_blocked(cell, true);
            }
            let id = world.resource_mut::<IdAllocator>().allocate_building();
            let entity = world
                .spawn((
                    Building {
                        id,
                        team: TeamId(2),
                        kind: BuildingKind::GunTurret,
                        construction: ConstructionState {
                            progress_seconds: 0.0,
                            complete: true,
                            active_builder: None,
                        },
                    },
                    footprint,
                    Health {
                        current: spec.max_health,
                        max: spec.max_health,
                    },
                ))
                .id();
            world.entity_mut(entity).insert(AttackCooldown::default());
            world.resource_mut::<BuildingIndex>().insert(id, entity);
            id
        };
        let hidden_building = seed_attacker_tower(&mut world, &mut map, hidden_anchor);
        let visible_building = seed_attacker_tower(&mut world, &mut map, visible_anchor);
        refresh_visibility(&mut world, &map);
        world.insert_resource(map.clone());
        let missing = UnitId(999);
        world.insert_resource(CombatEvents(vec![
            event_from(CombatTarget::Unit(UnitId(1))),
            event_from(CombatTarget::Unit(enemy_at_home)),
            event_from(CombatTarget::Unit(missing)),
            event_from(CombatTarget::Building(BuildingId(1))),
            event_from(CombatTarget::Building(hidden_building)),
            event_from(CombatTarget::Building(BuildingId(92))),
            event_from(CombatTarget::Building(visible_building)),
        ]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(
            drained[0].attacker,
            CombatTarget::Unit(UnitId(1)),
            "friendly attacker stays"
        );
        assert_eq!(
            drained[1].attacker,
            CombatTarget::Unit(UnitId(0)),
            "hidden enemy attacker must not expose its stable id"
        );
        assert_eq!(
            drained[1].position,
            event_from(CombatTarget::Unit(enemy_at_home)).position
        );
        assert_eq!(
            drained[2].attacker,
            CombatTarget::Unit(UnitId(0)),
            "missing attacker is scrubbed"
        );
        assert_eq!(
            drained[3].attacker,
            CombatTarget::Building(BuildingId(1)),
            "friendly building attacker stays"
        );
        assert_eq!(
            drained[4].attacker,
            CombatTarget::Unit(UnitId(0)),
            "hidden building ID and kind are both scrubbed"
        );
        assert_eq!(
            drained[5].attacker,
            CombatTarget::Unit(UnitId(0)),
            "missing building attacker is scrubbed"
        );
        assert_eq!(
            drained[6].attacker,
            CombatTarget::Building(visible_building),
            "visible building attacker may present"
        );
        assert_eq!(
            combat_attacker_kind_and_id(drained[4].attacker),
            ("unit", 0),
            "a scrubbed hidden building serializes only the absent unit identity"
        );
        world.remove_resource::<GridMap>();
        world.insert_resource(CombatEvents(vec![event_from(CombatTarget::Building(
            hidden_building,
        ))]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(
            drained[0].attacker,
            CombatTarget::Unit(UnitId(0)),
            "building visibility is checked from its footprint without a GridMap resource"
        );
        world.insert_resource(map.clone());
        let dead_building = world
            .resource::<BuildingIndex>()
            .entity(visible_building)
            .unwrap();
        world.get_mut::<Health>(dead_building).unwrap().current = 0;
        world.insert_resource(CombatEvents(vec![event_from(CombatTarget::Building(
            visible_building,
        ))]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(
            drained[0].attacker,
            CombatTarget::Unit(UnitId(0)),
            "dead building attacker IDs and kinds are scrubbed"
        );
        assert_eq!(
            combat_attacker_kind_and_id(drained[0].attacker),
            ("unit", 0)
        );

        // March the enemy unit into Team-1 vision — a cell on the immediate
        // perimeter of the authored central Research Center — and its id
        // becomes presentable.
        let enemy_entity = world.resource::<UnitIndex>().entity(enemy_at_home).unwrap();
        world
            .entity_mut(enemy_entity)
            .insert(SimPosition::new(map.cell_center(seen_cell)));
        refresh_visibility(&mut world, &map);
        world.insert_resource(CombatEvents(vec![event_from(CombatTarget::Unit(
            enemy_at_home,
        ))]));
        let drained = take_presentable_events(&mut world);
        assert_eq!(
            drained[0].attacker,
            CombatTarget::Unit(enemy_at_home),
            "a visible enemy attacker may present"
        );
    }

    #[test]
    fn live_aliens_use_fog_and_the_seed_has_no_enemy_building() {
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
        assert!(!building_hidden_from_local_player(&world, own));
        assert_eq!(world.resource::<BuildingIndex>().iter().count(), 1);

        // A live alien at the east entry is fog-hidden until a scout reaches it.
        let alien_cell = EntryZone::East.anchor();
        let alien = spawn_unit(
            &mut world,
            UnitId(5),
            TeamId(2),
            map.cell_center(alien_cell),
            UnitKind::Swarmer,
            unit_spec(UnitKind::Swarmer).speed,
        );
        assert!(!visible_to(&world, TeamId(1), alien_cell));

        // A team-1 scout beside the alien clears its cell for presentation.
        let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        world
            .entity_mut(scout)
            .insert(SimPosition::new(map.cell_center(alien_cell)));
        refresh_visibility(&mut world, &map);
        assert!(visible_to(&world, TeamId(1), alien_cell));
        assert!(world.get::<Unit>(alien).is_some());
    }

    /// The enemy branch of the building fog gate: with the Team-2 base gone,
    /// only this test keeps a hostile building exercising
    /// `building_hidden_from_local_player` — hidden under fog, revealed when
    /// a team-1 scout reaches its footprint.
    #[test]
    fn enemy_building_is_fog_gated_until_scouted() {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);
        world.insert_resource(VisibilityMap::default());
        refresh_visibility(&mut world, &map);

        // A completed enemy Depot at the east entry — far outside team-1
        // vision. It needs no BuildingIndex entry: the gate reads only the
        // entity's components.
        let spec = building_spec(BuildingKind::Depot);
        let enemy = world
            .spawn((
                Building {
                    id: BuildingId(50),
                    team: TeamId(2),
                    kind: BuildingKind::Depot,
                    construction: ConstructionState {
                        progress_seconds: 0.0,
                        complete: true,
                        active_builder: None,
                    },
                },
                Footprint::new(EntryZone::East.anchor(), spec.width, spec.height),
                Health {
                    current: spec.max_health,
                    max: spec.max_health,
                },
            ))
            .id();
        assert!(building_hidden_from_local_player(&world, enemy));

        // A team-1 scout beside it reveals the footprint: the gate opens.
        let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        world
            .entity_mut(scout)
            .insert(SimPosition::new(map.cell_center(EntryZone::East.anchor())));
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
            UnitKind::Worker,
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
            UnitKind::Worker,
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
            UnitKind::Worker,
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
            UnitKind::Guard,
            6.0,
        );

        assert_eq!(idle_worker_ids(&world, TeamId(1)), vec![UnitId(1)]);
    }

    #[test]
    fn building_view_transform_centers_and_scales_to_the_footprint() {
        // A 4×4 Research Center at (12, 46) renders around (14, 48) — not the
        // anchor's cell center — and the unit box scales to the footprint.
        let research_center = building_view_transform(&Footprint::new(GridPos::new(12, 46), 4, 4));
        assert_eq!(research_center.translation, Vec3::new(14.0, 0.0, 48.0));
        assert_eq!(research_center.scale, Vec3::new(4.0, 1.0, 4.0));

        let bunkhouse = building_view_transform(&Footprint::new(GridPos::new(13, 10), 2, 2));
        assert_eq!(bunkhouse.translation, Vec3::new(14.0, 0.0, 11.0));
        assert_eq!(bunkhouse.scale, Vec3::new(2.0, 1.0, 2.0));
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
                    (UnitId(1), RejectReason::Crowded),
                    (UnitId(2), RejectReason::Crowded),
                ],
                ..CommandResult::default()
            }),
            "Command: 0 accepted, 0 unreachable, 0 not owned, 0 missing, 2 rejected (Crowded)"
        );
    }

    #[test]
    fn placement_result_stays_attributed_when_later_feedback_overwrites_latest() {
        let mut world = World::new();
        setup_fixture(&mut world);
        start_match(&mut world);
        world.insert_resource(PendingCommands::default());
        world.insert_resource(PlacementCommandResults::default());
        world.insert_resource(CommandFeedback::default());
        world.resource_mut::<PendingCommands>().0.extend([
            PendingCommand {
                command: PlayerCommand::PlaceBuilding {
                    issuer: TeamId(1),
                    builder: UnitId(1),
                    kind: BuildingKind::Wall,
                    anchor: GridPos::new(-1, -1),
                },
                placement_request_id: Some(71),
            },
            PendingCommand {
                command: PlayerCommand::Units(UnitCommand {
                    issuer: TeamId(1),
                    units: vec![UnitId(1)],
                    kind: UnitCommandKind::Stop,
                }),
                placement_request_id: None,
            },
        ]);

        apply_pending_commands(&mut world);

        let placement = world
            .resource_mut::<PlacementCommandResults>()
            .0
            .remove(&71)
            .expect("tracked placement result is retained");
        assert!(!placement.accepted);
        assert_eq!(placement.feedback, "Command rejected (OutOfBounds)");
        assert_eq!(
            world.resource::<CommandFeedback>().text,
            "Command accepted for 1 unit(s)",
            "the unrelated later result remains the shared latest feedback"
        );
    }

    /// The full survival-fixture contract after setup/reset: seeded ids,
    /// indexes, stockpiles, a Start session, and no combat transients.
    fn assert_fixture_restored(world: &World) {
        let mut unit_ids: Vec<u32> = world
            .resource::<UnitIndex>()
            .iter()
            .map(|(id, _)| id.0)
            .collect();
        unit_ids.sort_unstable();
        assert_eq!(unit_ids, (1..=4).collect::<Vec<u32>>());

        let mut building_ids: Vec<u32> = world
            .resource::<BuildingIndex>()
            .iter()
            .map(|(id, _)| id.0)
            .collect();
        building_ids.sort_unstable();
        assert_eq!(building_ids, vec![1]);

        assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);
        assert_eq!(world.resource::<TeamEconomy>().0.len(), 1);
        let stockpile = &world.resource::<TeamEconomy>().0[&TeamId(1)].stockpile;
        assert_eq!(
            (
                stockpile.food,
                stockpile.scrap,
                stockpile.coal,
                stockpile.crystals
            ),
            (200, 300, 150, 0)
        );
        assert_eq!(*world.resource::<WaveState>(), WaveState::default());
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
            attacker: CombatTarget::Unit(UnitId(1)),
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
    /// and explored fog.
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
            .push(PendingCommand {
                command: PlayerCommand::Units(UnitCommand {
                    issuer: TeamId(1),
                    units: vec![UnitId(1)],
                    kind: UnitCommandKind::Move {
                        target: Vec2::new(60.5, 60.5),
                    },
                }),
                placement_request_id: None,
            });

        // Use the fog: a scout reveals a live alien at an authored entry.
        let fixture = MapFixture::battlefield();
        let map = fixture.map;
        let alien_cell = EntryZone::East.anchor();
        spawn_unit(
            &mut world,
            UnitId(5),
            TeamId(2),
            map.cell_center(alien_cell),
            UnitKind::Swarmer,
            unit_spec(UnitKind::Swarmer).speed,
        );
        let scout = world.resource::<UnitIndex>().entity(UnitId(1)).unwrap();
        world
            .entity_mut(scout)
            .insert(SimPosition::new(map.cell_center(alien_cell)));
        refresh_visibility(&mut world, &map);
        assert!(explored_by(&world, TeamId(1), alien_cell));

        reset_fixture_world(&mut world);

        assert!(
            world.resource::<PendingCommands>().0.is_empty(),
            "restart must clear pending human commands"
        );
        assert!(matches!(
            world.resource::<MatchSession>().phase,
            MatchPhase::Start
        ));
        let states = world
            .resource::<VisibilityMap>()
            .packed_cell_states(TeamId(1), 128, 96);
        let alien_entry = EntryZone::East.anchor();
        assert_eq!(
            states[(alien_entry.y * 128 + alien_entry.x) as usize],
            0,
            "restart must return used fog to the entry's boot Unexplored state"
        );
    }

    /// The benchmark contract: the shared normal seed (fogged human core)
    /// extended with the director workload, and no
    /// `MatchSession` — the missing session reads as Playing forever, so a
    /// settled stale session cannot leak in and the load can never freeze.
    #[test]
    fn benchmark_reset_is_session_free_survival_workload() {
        let mut world = World::new();
        // A stale normal-skirmish session must not leak into the benchmark:
        // reset removes it, and the missing session reads as Playing.
        world.insert_resource(MatchSession {
            phase: MatchPhase::Result(MatchResult(TeamId(1))),
        });
        reset_benchmark_world(&mut world);
        assert!(world.get_resource::<MatchSession>().is_none());
        assert_eq!(
            active_phase(&world),
            MatchPhase::Playing,
            "the missing session reads as Playing"
        );

        // The one-base survival roster: four workers, 46 defenders, and
        // 150 actual aliens.
        let count =
            |team: TeamId, kind: UnitKind| sorted_unit_ids(&world, team, |u| u.kind == kind).len();
        assert_eq!(count(TeamId(1), UnitKind::Worker), 4);
        assert_eq!(count(TeamId(1), UnitKind::Guard), 23);
        assert_eq!(count(TeamId(1), UnitKind::Rifleman), 23);
        let aliens = [
            UnitKind::Swarmer,
            UnitKind::Spitter,
            UnitKind::Brute,
            UnitKind::Behemoth,
        ]
        .into_iter()
        .map(|kind| count(TeamId(2), kind))
        .sum::<usize>();
        assert_eq!(aliens, 150);
        assert_eq!(world.resource::<UnitIndex>().iter().count(), 200);
        assert_eq!(world.resource::<WaveState>().next_wave, 101);

        // One Research Center and all 18 authored sources survive from the
        // shared seed.
        let research_centers = world
            .resource::<BuildingIndex>()
            .iter()
            .filter(|(_, entity)| {
                world.get::<Building>(**entity).is_some_and(|building| {
                    building.kind == BuildingKind::ResearchCenter && building.construction.complete
                })
            })
            .count();
        assert_eq!(research_centers, 1);
        assert_eq!(world.resource::<ResourceIndex>().iter().count(), 18);
    }

    /// The bridge drain is Team-1-only: a tick whose sim slot holds both a
    /// Team-1 and a Team-2 route reject surfaces only Team 1's code as human
    /// `CommandFeedback`, leaves Team 2's entry in the sim resource, and a
    /// second drain with no pending Team-1 reject publishes nothing new.
    #[test]
    fn route_reject_drain_surfaces_team_one_only() {
        let mut world = World::new();
        world.insert_resource(LastRouteReject(std::collections::HashMap::from([
            (TeamId(1), RejectReason::Unreachable),
            (TeamId(2), RejectReason::Crowded),
        ])));
        world.insert_resource(CommandFeedback::default());

        drain_route_reject_feedback(&mut world);

        let feedback = world.resource::<CommandFeedback>();
        assert_eq!(feedback.revision, 1);
        assert_eq!(feedback.text, "Worker route impossible (Unreachable)");
        assert_eq!(feedback.last_reject_code, Some(RejectReason::Unreachable));
        assert_eq!(
            world.resource::<LastRouteReject>().0.get(&TeamId(2)),
            Some(&RejectReason::Crowded),
            "the Team-2 entry stays in the sim resource"
        );
        assert!(
            !world
                .resource::<LastRouteReject>()
                .0
                .contains_key(&TeamId(1)),
            "the drained Team-1 entry is consumed"
        );

        // Nothing pending for Team 1: the feedback channel is untouched.
        let revision = world.resource::<CommandFeedback>().revision;
        drain_route_reject_feedback(&mut world);
        assert_eq!(world.resource::<CommandFeedback>().revision, revision);
        assert_eq!(
            world.resource::<LastRouteReject>().0.get(&TeamId(2)),
            Some(&RejectReason::Crowded),
            "a Team-2-only slot never publishes human feedback"
        );
    }

    /// The heat fields behind `economy_snapshot`: neutral 0/0 at full
    /// efficiency until the first scheduled heat step, then the seeded
    /// Research Center's 15/10 live totals.
    #[test]
    fn economy_snapshot_heat_fields_read_the_seeded_fixture() {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);

        assert_eq!(
            heat_fields(&world),
            (0, 0, 1.0),
            "the default seed state reads neutral, never a fake deficit"
        );
        step_heat(&mut world, SIM_STEP_SECONDS);
        assert_eq!(
            heat_fields(&world),
            (15, 10, 1.0),
            "the first heat step publishes the warm Research Center totals"
        );
    }

    /// A world without `HeatState` (and an unstepped default) report 0/0 at
    /// the full fractional efficiency; a deficit reports its exact fraction
    /// and a severe one floors at `MIN_SURFACE_EFFICIENCY`.
    #[test]
    fn economy_snapshot_heat_fields_default_neutral_and_floor_deficits() {
        let mut world = World::new();
        assert_eq!(heat_fields(&world), (0, 0, 1.0));
        world.insert_resource(HeatState::default());
        assert_eq!(heat_fields(&world), (0, 0, 1.0));

        world.insert_resource(HeatState {
            supply: 15,
            demand: 16,
            burn_remainder: 0.0,
        });
        let (supply, demand, efficiency) = heat_fields(&world);
        assert_eq!((supply, demand), (15, 16));
        assert!(
            (efficiency - 0.9375).abs() < 1e-6,
            "15/16 must read as the exact fractional efficiency, got {efficiency}"
        );

        world.insert_resource(HeatState {
            supply: 1,
            demand: 16,
            burn_remainder: 0.0,
        });
        let (_, _, efficiency) = heat_fields(&world);
        assert!(
            (efficiency - grus_sim::MIN_SURFACE_EFFICIENCY).abs() < 1e-6,
            "a severe deficit floors at MIN_SURFACE_EFFICIENCY, got {efficiency}"
        );
    }

    /// The cold-state selection: only completed friendly buildings with
    /// catalogue heat demand tint while the live efficiency is below 1 —
    /// construction sites and zero-demand kinds (Wall, Furnace) and enemy
    /// structures never do.
    #[test]
    fn cold_state_selects_only_completed_friendly_heat_demanding_buildings() {
        let fixture = MapFixture::battlefield();
        let mut map = fixture.map.clone();
        let mut world = World::new();
        seed_skirmish(&mut world, &mut map, &fixture);

        let seed_building =
            |world: &mut World, id: u32, team: TeamId, kind: BuildingKind, complete: bool| {
                let spec = building_spec(kind);
                world
                    .spawn((
                        Building {
                            id: BuildingId(id),
                            team,
                            kind,
                            construction: ConstructionState {
                                progress_seconds: 0.0,
                                complete,
                                active_builder: None,
                            },
                        },
                        Footprint::new(GridPos::new(60, 60), spec.width, spec.height),
                        Health {
                            current: spec.max_health,
                            max: spec.max_health,
                        },
                    ))
                    .id()
            };
        seed_building(&mut world, 51, TeamId(1), BuildingKind::Bunkhouse, false);
        seed_building(&mut world, 52, TeamId(1), BuildingKind::Wall, true);
        seed_building(&mut world, 53, TeamId(1), BuildingKind::CoalFurnace, true);
        seed_building(&mut world, 54, TeamId(2), BuildingKind::Depot, true);

        let rc_entity = world
            .resource::<BuildingIndex>()
            .entity(BuildingId(1))
            .unwrap();
        let (rc_team, rc_kind, rc_complete) = {
            let rc = world.get::<Building>(rc_entity).unwrap();
            (rc.team, rc.kind, rc.construction.complete)
        };
        assert_eq!(rc_team, TeamId(1));
        assert!(rc_complete);
        assert_eq!(building_spec(rc_kind).heat_demand, 10);

        // A warm world tints nothing, the Research Center included.
        assert!(!building_is_cold(&world, rc_team, rc_kind, rc_complete));

        // A live deficit tints only the completed friendly heat consumers.
        world.insert_resource(HeatState {
            supply: 15,
            demand: 16,
            burn_remainder: 0.0,
        });
        assert!(building_is_cold(&world, rc_team, rc_kind, rc_complete));
        assert!(
            !building_is_cold(&world, TeamId(1), BuildingKind::Bunkhouse, false),
            "construction sites never tint"
        );
        assert!(
            !building_is_cold(&world, TeamId(1), BuildingKind::Wall, true),
            "zero-demand Walls never tint"
        );
        assert!(
            !building_is_cold(&world, TeamId(1), BuildingKind::CoalFurnace, true),
            "zero-demand Furnaces never tint"
        );
        assert!(
            !building_is_cold(&world, TeamId(2), BuildingKind::Depot, true),
            "enemy buildings never tint"
        );
    }

    /// HPA-181: the appended CoalFurnace kind must round-trip the wire
    /// name — outbound serializer and inbound parser share `kind_name_str`.
    #[test]
    fn coal_furnace_catalogue_name_round_trips_the_parser() {
        let name = kind_name_str(BuildingKind::CoalFurnace);
        assert_eq!(name, "CoalFurnace");
        assert_eq!(
            parse_building_kind_str(&name),
            Some(BuildingKind::CoalFurnace)
        );
    }
}
