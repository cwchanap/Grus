//! E2E-only selector surface.
//!
//! Behind the `e2e` feature, attaches stable [`E2eId`] names to the
//! deterministic authored skirmish entities after `setup_fixture` seeds the
//! world, and spawns the `grus.ready` readiness marker. The out-of-process
//! harness addresses these over BRP; raw `Entity` ids never cross the wire.
//! Registration happens in `build_app` and only takes effect when the parent
//! harness sets `BEVY_E2E=1`, matching `BevyE2EPlugin`'s activation contract.

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseMotion};
use bevy::prelude::*;
use bevy::window::{CursorMoved, WindowEvent};
use bevy_e2e::E2eId;
use grus_sim::{Building, BuildingKind, TeamId, Unit, UnitKind};

/// The authored fixture's player team.
const PLAYER_TEAM: TeamId = TeamId(1);
/// The authored fixture's enemy team.
const ENEMY_TEAM: TeamId = TeamId(2);

/// In-game wiring for the `e2e` feature: prepares the app for
/// `BevyE2EPlugin`'s BRP stack and attaches the selector surface. Like
/// [`bevy_e2e::BevyE2EPlugin`], it registers nothing unless `BEVY_E2E=1`.
///
/// `bevy_brp_extras` (pulled in by `BevyE2EPlugin`) assumes the host game
/// runs bevy's input stack: its keyboard/mouse systems write input messages
/// this app otherwise never initializes, and would panic at runtime. The
/// buffers are inert until the harness drives them over BRP.
pub struct GrusE2ePlugin;

impl Plugin for GrusE2ePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("BEVY_E2E").as_deref() != Ok("1") {
            return;
        }
        app.add_message::<CursorMoved>()
            .add_message::<WindowEvent>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseMotion>()
            .add_systems(Startup, attach_selectors.after(crate::setup_fixture));
    }
}

/// Attaches `E2eId` selectors to the deterministic authored fixture entities
/// after `setup_fixture` seeds the world, then spawns the `grus.ready`
/// marker. Inert unless the harness sets `BEVY_E2E=1`.
pub fn attach_selectors(world: &mut World) {
    if std::env::var("BEVY_E2E").as_deref() != Ok("1") {
        return;
    }

    let mut buildings = world.query::<(Entity, &Building)>();
    let town_centers: Vec<(Entity, TeamId)> = buildings
        .iter(world)
        .filter(|(_, building)| building.kind == BuildingKind::TownCenter)
        .map(|(entity, building)| (entity, building.team))
        .collect();
    for (entity, team) in town_centers {
        let selector = if team == PLAYER_TEAM {
            "player.town-center"
        } else if team == ENEMY_TEAM {
            "enemy.town-center"
        } else {
            continue;
        };
        world.entity_mut(entity).insert(E2eId::new(selector));
    }

    // The fixture authors four equivalent starting villagers per team; the
    // selector names the first-authored one (smallest authored `UnitId`).
    let mut units = world.query::<(Entity, &Unit)>();
    let first_villager = units
        .iter(world)
        .filter(|(_, unit)| unit.team == PLAYER_TEAM && unit.kind == UnitKind::Villager)
        .min_by_key(|(_, unit)| unit.id)
        .map(|(entity, _)| entity);
    if let Some(entity) = first_villager {
        world
            .entity_mut(entity)
            .insert(E2eId::new("player.starting-villager"));
    }

    world.spawn(E2eId::new("grus.ready"));
}

#[cfg(all(test, feature = "e2e"))]
mod tests {
    use super::*;
    use crate::build_app;
    use grus_sim::UnitId;
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serializes the activation tests: they flip process-global env vars
    /// (`BEVY_E2E`, `BRP_EXTRAS_PORT`) and cargo runs tests in parallel.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    /// Builds the real app, adding only the host capability the BRP HTTP
    /// transport needs at `Startup`: task pools (bevy 0.19's bare `App`
    /// installs none; the real game's host plugin set provides them).
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default());
        build_app(&mut app);
        app
    }

    /// Steps the startup half of the game's first frame: `PreStartup` (the
    /// BRP mailbox is set up there) then `Startup` (fixture seeding, selector
    /// attachment, BRP server spawn).
    fn run_first_frame(app: &mut App) {
        // `PreStartup` only exists once the BRP stack registers its mailbox
        // setup there; it is absent when the runtime is dormant.
        let _ = app.world_mut().try_run_schedule(PreStartup);
        app.world_mut().run_schedule(Startup);
    }

    #[test]
    fn stays_dormant_without_the_bevy_e2e_flag() {
        let _guard = env_lock();
        // SAFETY: serialized by env_lock; no other test touches these vars.
        unsafe { std::env::remove_var("BEVY_E2E") };

        let mut app = test_app();
        assert!(!app.is_plugin_added::<bevy::remote::http::RemoteHttpPlugin>());

        run_first_frame(&mut app);

        let world = app.world_mut();
        let count = world.query::<&E2eId>().iter(world).count();
        assert_eq!(count, 0, "no selector may spawn while BEVY_E2E is unset");
    }

    #[test]
    fn attaches_each_selector_to_exactly_one_entity() {
        let _guard = env_lock();
        // SAFETY: serialized by env_lock; no other test touches these vars.
        unsafe {
            std::env::set_var("BEVY_E2E", "1");
            // Port 0 = ephemeral listener; real runs let the harness pick
            // the port, tests only need the transport to come up harmlessly.
            std::env::set_var("BRP_EXTRAS_PORT", "0");
        }

        let mut app = test_app();
        assert!(app.is_plugin_added::<bevy::remote::http::RemoteHttpPlugin>());

        // Only the startup half of the frame is stepped: the Godot-view
        // systems in `Update`/`PostUpdate` require a live Godot runtime and
        // are out of scope for a headless test.
        run_first_frame(&mut app);

        let world = app.world_mut();
        let mut query = world.query::<(Entity, &E2eId)>();
        let mut by_id: HashMap<&str, Vec<Entity>> = HashMap::new();
        for (entity, id) in query.iter(world) {
            by_id.entry(id.value.as_str()).or_default().push(entity);
        }

        for selector in [
            "grus.ready",
            "player.town-center",
            "enemy.town-center",
            "player.starting-villager",
        ] {
            assert_eq!(
                by_id.get(selector).map(Vec::len),
                Some(1),
                "{selector} must resolve to exactly one entity"
            );
        }
        assert_eq!(
            by_id.len(),
            4,
            "no entities may carry selectors beyond the contract"
        );

        // Selectors sit on the right gameplay entities, not just any entity.
        let player = world
            .entity(by_id["player.town-center"][0])
            .get::<Building>()
            .unwrap();
        assert_eq!(player.team, PLAYER_TEAM);
        let enemy = world
            .entity(by_id["enemy.town-center"][0])
            .get::<Building>()
            .unwrap();
        assert_eq!(enemy.team, ENEMY_TEAM);
        let villager = world
            .entity(by_id["player.starting-villager"][0])
            .get::<Unit>()
            .unwrap();
        assert_eq!(villager.team, PLAYER_TEAM);
        assert_eq!(villager.kind, UnitKind::Villager);
        assert_eq!(villager.id, UnitId(1), "first authored player villager");
    }
}
