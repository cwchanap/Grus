use bevy::prelude::*;
use godot::builtin::{GString, PackedInt32Array, Vector2};
use godot::classes::{Engine, INode, Node, Node3D, SceneTree};
use godot::prelude::*;
use godot_bevy::BevyApp;
use godot_bevy::prelude::*;
use grus_sim::{
    CommandOutcome, CommandRejectReason, GridMap, MapFixture, SIM_STEP_SECONDS, SimPosition,
    TeamId, Unit, UnitCommand, UnitCommandKind, UnitId, apply_command, spawn_unit, step_movement,
};

#[derive(Default, Resource)]
struct PendingCommands(Vec<UnitCommand>);

#[derive(Resource)]
struct CommandFeedback {
    revision: u64,
    text: String,
}

impl Default for CommandFeedback {
    fn default() -> Self {
        Self {
            revision: 0,
            text: "Ready".to_string(),
        }
    }
}

#[derive(Component)]
struct UnitViewInitialized;

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

        queue_command(UnitCommand {
            issuer: TeamId(1),
            units,
            kind: UnitCommandKind::Move {
                target: Vec2::new(target.x, target.y),
            },
        })
    }

    #[func]
    fn stop_units(&self, packed_ids: PackedInt32Array) -> bool {
        let units = decode_unit_ids(&packed_ids);
        if units.is_empty() {
            return false;
        }

        queue_command(UnitCommand {
            issuer: TeamId(1),
            units,
            kind: UnitCommandKind::Stop,
        })
    }

    #[func]
    fn benchmark_move_all(&self) -> bool {
        let fixture = MapFixture::battlefield();
        let player_queued = queue_command(UnitCommand {
            issuer: TeamId(1),
            units: (1_u32..=100).map(UnitId).collect(),
            kind: UnitCommandKind::Move {
                target: fixture.right_spawn,
            },
        });
        let enemy_queued = queue_command(UnitCommand {
            issuer: TeamId(2),
            units: (101_u32..=200).map(UnitId).collect(),
            kind: UnitCommandKind::Move {
                target: fixture.left_spawn,
            },
        });

        player_queued && enemy_queued
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
}

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotAssetsPlugin)
        .add_plugins(GodotTransformSyncPlugin::default())
        .add_plugins(GodotPackedScenePlugin)
        .insert_resource(Time::<Fixed>::from_seconds(f64::from(SIM_STEP_SECONDS)))
        .init_resource::<PendingCommands>()
        .init_resource::<CommandFeedback>()
        .add_systems(Startup, setup_fixture)
        .add_systems(
            Update,
            (initialize_unit_views, sync_interpolated_unit_transforms),
        )
        .add_systems(
            FixedUpdate,
            (apply_pending_commands, advance_simulation).chain(),
        );
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

fn bevy_app_singleton() -> Option<Gd<BevyApp>> {
    Engine::singleton()
        .get_main_loop()?
        .try_cast::<SceneTree>()
        .ok()?
        .get_root()?
        .try_get_node_as::<BevyApp>("BevyAppSingleton")
}

fn queue_command(command: UnitCommand) -> bool {
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
    let fixture = MapFixture::battlefield();
    let spawns = fixture.units_200();
    world.insert_resource(fixture.map);

    for spawn in spawns {
        let entity = spawn_unit(world, spawn.id, spawn.team, spawn.position, 12.0);
        world.entity_mut(entity).insert((
            Transform::from_xyz(spawn.position.x, 0.0, spawn.position.y),
            TransformSyncMetadata::default(),
            Node3DMarker,
            GodotScene::from_path("res://scenes/unit_view.tscn"),
        ));
    }
}

fn initialize_unit_views(
    mut commands: Commands,
    units: Query<(Entity, &Unit, &GodotNodeHandle), Without<UnitViewInitialized>>,
    mut godot: GodotAccess,
) {
    for (entity, unit, handle) in &units {
        let Some(mut node) = godot.try_get::<Node3D>(*handle) else {
            continue;
        };
        node.set_meta("unit_id", &i64::from(unit.id.0).to_variant());
        node.set_meta("team_id", &i64::from(unit.team.0).to_variant());
        commands.entity(entity).insert(UnitViewInitialized);
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

    let mut latest_feedback = None;
    world.resource_scope(|world, map: Mut<GridMap>| {
        for command in commands {
            let outcome = apply_command(world, &map, command);
            latest_feedback = Some(format_command_outcome(&outcome));
        }
    });

    if let Some(text) = latest_feedback {
        let mut feedback = world.resource_mut::<CommandFeedback>();
        feedback.revision = feedback.revision.wrapping_add(1);
        feedback.text = text;
    }
}

fn format_command_outcome(outcome: &CommandOutcome) -> String {
    let unreachable = outcome
        .rejected
        .iter()
        .filter(|(_, reason)| *reason == CommandRejectReason::Unreachable)
        .count();
    let not_owned = outcome
        .rejected
        .iter()
        .filter(|(_, reason)| *reason == CommandRejectReason::NotOwned)
        .count();
    let unknown = outcome
        .rejected
        .iter()
        .filter(|(_, reason)| *reason == CommandRejectReason::UnknownUnit)
        .count();

    if outcome.rejected.is_empty() {
        format!("Command accepted for {} unit(s)", outcome.accepted.len())
    } else if outcome.accepted.is_empty() && unreachable > 0 {
        format!("Destination unreachable for {unreachable} unit(s)")
    } else {
        format!(
            "Command: {} accepted, {} unreachable, {} not owned, {} missing",
            outcome.accepted.len(),
            unreachable,
            not_owned,
            unknown
        )
    }
}

fn advance_simulation(world: &mut World) {
    world.resource_scope(|world, map: Mut<GridMap>| {
        step_movement(world, &map, SIM_STEP_SECONDS);
    });
}

fn sync_interpolated_unit_transforms(
    fixed_time: Res<Time<Fixed>>,
    mut units: Query<(&SimPosition, &mut Transform), With<Unit>>,
) {
    let alpha = fixed_time.overstep_fraction();
    for (position, mut transform) in &mut units {
        let rendered = position.previous.lerp(position.current, alpha);
        if transform.translation.x != rendered.x || transform.translation.z != rendered.y {
            transform.translation.x = rendered.x;
            transform.translation.z = rendered.y;
        }
    }
}
