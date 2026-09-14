//! First exported-game E2E test: prove the full process boundary.
//!
//! Chain under test: this test binary -> bevy-e2e client -> exported Godot
//! child process -> GDExtension -> godot-bevy embedded Bevy ECS -> seeded
//! Grus simulation. No input automation, no screenshots.
//!
//! Requires `GRUS_E2E_BINARY` to name the exported Grus Godot executable.
//! Unset or empty skips the test (CI always exports it; see the CI task).

use std::{path::PathBuf, time::Duration};

use bevy::reflect::TypePath;

/// Exported-binary path from the Grus CI contract; `None` means skip.
fn exported_binary() -> Option<PathBuf> {
    std::env::var("GRUS_E2E_BINARY")
        .ok()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| !path.as_os_str().is_empty())
}

#[test]
fn boot_ready_and_fixture_markers() {
    let Some(binary) = exported_binary() else {
        eprintln!(
            "skipping e2e_boot: GRUS_E2E_BINARY is not set; \
             export the Grus Godot executable to run this test"
        );
        return;
    };

    // Generous timeouts: Godot debug exports on slow CI runners.
    let options = bevy_e2e::E2eLaunchOptions::new(binary)
        .startup_timeout(Duration::from_secs(120))
        .operation_timeout(Duration::from_secs(30))
        .shutdown_timeout(Duration::from_secs(15));

    bevy_e2e::run(options, |game| {
        game.wait_for("grus.ready")?;
        game.find("player.town-center")?;
        game.find("enemy.town-center")?;
        game.find("player.starting-villager")?;

        // Cheap reflected-state round-trip: read the marker entity's own
        // `E2eId` component over BRP and assert the selector value survives.
        let id = game.component_json("player.town-center", bevy_e2e::E2eId::type_path())?;
        assert_eq!(id["value"], "player.town-center", "E2eId must round-trip");

        Ok(())
    })
    .expect("exported Grus must boot with the seeded fixture markers intact");
}
