# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Grus is a 2.5D real-time strategy game: a Rust/Bevy ECS simulation driving a Godot 4 presentation layer via godot-bevy GDExtension. `README.md` is the authoritative feature/baseline record (fixture contents, controls, benchmark numbers); this file covers how to work in the repo.

## Toolchain pins

Everything is pinned exactly and CI builds `--locked`: Rust **1.95.0** (`rust-toolchain.toml`), Godot **4.6.2**, `bevy =0.19.1`, `godot =0.5.5`, `godot-bevy =0.12.0`, `pathfinding =4.16.0`. Edition 2024. Bump versions only deliberately — `bevy`/`godot`/`godot-bevy` are `=`-pinned because the bridge depends on version-specific behavior (see "Interpolation" below).

## Build, run, verify

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.dylib godot/bin/libgrus_godot.dylib   # macOS
cp target/debug/libgrus_godot.so    godot/bin/libgrus_godot.so      # Linux/CI
godot --path godot
```

**Re-copy the library into `godot/bin/` after every rebuild.** A stale binary fails at startup with a misleading error (e.g. a false duplicate-`UnitId` panic), not silently. On a clean clone the GDExtension is not registered until the editor scans once: `godot --headless --path godot --editor --quit-after 120` (writes `godot/.godot/extension_list.cfg`).

Lint and tests (mirror CI, which uses `--locked` everywhere):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings
cargo test --workspace

# single test / module
cargo test -p grus-sim age_two_completes            # by name substring
cargo test -p grus-sim --test system_order          # one integration target
cargo test -p grus-sim economy::tests               # one inline module

# coverage gate (90% lines on grus-sim gameplay code; `tests.rs` files excluded)
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'

# e2e-feature unit tests mutate process-global env vars — must be serialized
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
```

Godot headless smokes (each is a real end-to-end contract, not a stub):

```bash
godot --headless --path godot res://scenes/smoke_test.tscn           # input/selection/move/HUD/cadence
godot --headless --path godot res://scenes/reset_test.tscn           # fixture reset → 8 units / 2 buildings / 18 resources
godot --headless --path godot res://scenes/economy_smoke_test.tscn   # full HPA-471 economy loop at 20x sim speed
```

The `bevy_e2e` out-of-process test needs an exported build: `godot --headless --path godot --export-debug "Linux x86_64" ../build/Grus.x86_64`, then `GRUS_E2E_BINARY=<path> cargo test -p grus-godot --features e2e --test e2e_boot --locked -- --test-threads=1`. Without `GRUS_E2E_BINARY` it skips loudly instead of running. The e2e selector surface is inert unless `BEVY_E2E=1`.

## Architecture

**`crates/grus-sim`** — authoritative gameplay, no Godot and no Bevy plugins/schedules. It exposes plain exclusive functions over a bare `World`:

```rust
apply_player_command(&mut world, &mut map, cmd) -> CommandResult
step_movement(&mut world, &map, seconds)
step_economy(&mut world, &mut map, seconds)
step_construction(&mut world, seconds)
step_production(&mut world, &mut map, seconds)
```

This is deliberate: sim tests build `World::new()` + `seed_skirmish` and call the steps directly, with no app, no schedule, and no engine. Keep new gameplay in this shape.

Modules: `catalog` (the single source of costs, build/train seconds, footprints, unlocks, population, speeds, gather rates — change balance here, nowhere else), `commands` (ownership validation, `RejectReason`, spawning), `economy` (worker tasks, gather/carry/deposit, drop-off routing), `buildings` (placement validation, construction, completion grants), `production` (FIFO queues, pop cap, spawn clearance, rally, one-time Age 2), `movement` (A* + separation), `map` (`GridMap`, `GridPos`, `Footprint`), `fixture` (authored skirmish + benchmark seeds), `ids`.

**`crates/grus-godot`** — the GDExtension. `build_app` (`#[bevy_app]`) registers the godot-bevy plugins, sets `Time::<Fixed>` to `SIM_STEP_SECONDS` (0.05 → 20 Hz), and chains `FixedUpdate` in a fixed order:

```
apply_pending_commands → advance_movement → advance_economy → advance_construction → advance_production → drain_route_reject_feedback
```

Order is a contract, not an accident: arrival is visible to economy/construction in the same tick, and production runs last so an Age 2 completion does not retroactively change that tick's gather rate. `crates/grus-sim/tests/system_order.rs` locks this — if you reorder, that test should fail.

**`godot/`** — presentation and input only. Two autoloads: `BevyAppSingleton` (godot-bevy's app host) and `GrusBridge` (`GrusBridgeNode`, all `#[func]`s). GDScript never touches gameplay state directly; it issues commands (`move_units`, `stop_units`, `gather`, `place_building`, `resume_construction`, `enqueue_unit`, `enqueue_age_up`, `set_rally`, `reset_fixture`, `reset_benchmark_fixture`, `set_sim_speed`) and reads typed snapshots (`economy_snapshot`, `building_snapshot`, `placement_preview`, `catalogue_snapshot`, `command_feedback*`, `last_reject_code`). Gameplay data flows one way, Bevy → Godot.

### Non-obvious invariants

- **Numeric reject/blocked codes are `RejectReason` declaration order** (`last_reject_code`, snapshot fields; `-1` = none). GDScript smokes assert those integers, so reordering or inserting a variant mid-enum silently changes the wire contract — append instead, and update the smokes when you don't.
- **Interpolation is hand-rolled.** godot-bevy 0.12 pins `Time<Fixed>::overstep_fraction()` to 0 and syncs transforms once per tick, so `sync_interpolated_unit_transforms` writes each unit view's `Node3D` transform directly every `Update` using `Engine::get_physics_interpolation_fraction()`. Don't route unit positions through `GodotTransformSyncPlugin` expecting smoothing.
- **Coordinates:** sim is a flat 2D plane (`Vec2 { x, y }`); Godot is 3D with sim `y` → world `z`, `y = 0`.
- **Occupancy** mutates only through `GridMap::set_blocked`. The map revision changes only when walkability actually changes; active `MoveOrder`s replan once on a newer revision rather than pathing every tick, and `last_failed_replan` suppresses re-running A* toward a known-unreachable goal until the map changes again.
- **Two reset modes, do not mix:** `reset_fixture` reseeds the 8-unit/2-building/18-resource skirmish (runtime + smokes); `reset_benchmark_fixture` reseeds the 200-villager fixture used only by `benchmark_200.tscn`.
- **Sim speed** is `Engine.time_scale` (godot-bevy's fixed driver derives its delta from `_physics_process`), not `Time<Virtual>` alone — `set_sim_speed` writes both; the economy smoke runs at 20×.
- `setup_fixture` is guarded on `UnitIndex` already existing, so a bridge reset before the first fixed tick does not double-seed.

## Tests

Inline `#[cfg(test)] mod tests;` lives in sibling `src/<module>/tests.rs` files (excluded from the coverage gate); cross-module contracts live in `crates/grus-sim/tests/` (`system_order.rs`, `economy_flow.rs`, `occupancy_replan.rs`). Behavior that crosses the Godot boundary belongs in a GDScript smoke scene under `godot/scenes/` + `godot/scripts/`, driven through real UI/input paths rather than direct state pokes.

## Docs

Design specs and implementation plans live in `docs/superpowers/specs/` and `docs/superpowers/plans/`, named `YYYY-MM-DD-<ticket>-<slug>.md`. Read the relevant spec before changing an area it covers — they record locked contracts (command results, approach slots, reset/cutover ordering, tick order).
