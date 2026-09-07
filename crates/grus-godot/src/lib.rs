use bevy::prelude::*;
use godot::builtin::{PackedInt32Array, VarDictionary, Vector2};
use godot::classes::Node3D;
use godot::prelude::{ExtensionLibrary, ToGodot, Variant, gdextension};
use godot_bevy::plugins::event_bridge::AddGodotEventAppExt;
use godot_bevy::prelude::*;
use grus_sim::{
    GridMap, MapFixture, SIM_STEP_SECONDS, SimPosition, TeamId, Unit, UnitCommand,
    UnitCommandKind, UnitId, apply_command, spawn_unit, step_movement,
};

#[derive(Clone, Debug, Event)]
struct MoveUnitsRequest {
    units: Vec<UnitId>,
    target: Vec2,
}

#[derive(Default, Resource)]
struct PendingCommands(Vec<UnitCommand>);

#[derive(Component)]
struct UnitViewInitialized;

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotAssetsPlugin)
        .add_plugins(GodotTransformSyncPlugin::default())
        .add_plugins(GodotPackedScenePlugin)
        .init_resource::<PendingCommands>()
        .add_godot_event::<MoveUnitsRequest>("move_units", decode_move_units)
        .add_observer(queue_move_units)
        .add_systems(Startup, setup_fixture)
        .add_systems(Update, initialize_unit_views)
        .add_systems(
            FixedUpdate,
            (
                apply_pending_commands,
                advance_simulation,
                sync_unit_transforms,
            )
                .chain(),
        );
}

fn decode_move_units(payload: Variant) -> Option<MoveUnitsRequest> {
    let dict = payload.try_to::<VarDictionary>().ok()?;
    let packed_ids = dict.get("units")?.try_to::<PackedInt32Array>().ok()?;
    let target = dict.get("target")?.try_to::<Vector2>().ok()?;
    let units = packed_ids
        .as_slice()
        .iter()
        .filter_map(|id| u32::try_from(*id).ok())
        .filter(|id| *id != 0)
        .map(UnitId)
        .collect::<Vec<_>>();

    if units.is_empty() {
        return None;
    }

    Some(MoveUnitsRequest {
        units,
        target: Vec2::new(target.x, target.y),
    })
}

fn queue_move_units(event: On<MoveUnitsRequest>, mut pending: ResMut<PendingCommands>) {
    let event = event.event();
    pending.0.push(UnitCommand {
        issuer: TeamId(1),
        units: event.units.clone(),
        kind: UnitCommandKind::Move {
            target: event.target,
        },
    });
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

    world.resource_scope(|world, map: Mut<GridMap>| {
        for command in commands {
            let _outcome = apply_command(world, &map, command);
        }
    });
}

fn advance_simulation(world: &mut World) {
    world.resource_scope(|world, map: Mut<GridMap>| {
        step_movement(world, &map, SIM_STEP_SECONDS);
    });
}

fn sync_unit_transforms(mut units: Query<(&SimPosition, &mut Transform), With<Unit>>) {
    for (position, mut transform) in &mut units {
        if transform.translation.x != position.current.x
            || transform.translation.z != position.current.y
        {
            transform.translation.x = position.current.x;
            transform.translation.z = position.current.y;
        }
    }
}
