use bevy::prelude::*;
use godot::prelude::{ExtensionLibrary, gdextension};
use godot_bevy::prelude::*;

#[bevy_app]
fn build_app(app: &mut App) {
    app.add_plugins(GodotAssetsPlugin)
        .add_plugins(GodotTransformSyncPlugin::default())
        .add_plugins(GodotPackedScenePlugin);
}
