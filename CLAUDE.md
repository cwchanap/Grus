# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Grus is a 2.5D real-time strategy game: a Rust/Bevy ECS simulation driving a Godot 4 presentation layer via godot-bevy GDExtension. `README.md` is the authoritative feature/baseline record (fixture contents, controls, benchmark numbers); this file covers how to work in the repo.

## Toolchain pins

Everything is pinned exactly and CI builds `--locked`: Rust **1.95.0** (`rust-toolchain.toml`), Godot **4.7.1**, `bevy =0.19.1`, `godot =0.5.5`, `godot-bevy =0.12.0`, `pathfinding =4.16.0`. Edition 2024. Bump versions only deliberately — `bevy`/`godot`/`godot-bevy` are `=`-pinned because the bridge depends on version-specific behavior (see "Interpolation" below).

## Build, run, verify

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.dylib godot/bin/libgrus_godot.dylib   # macOS
cp target/debug/libgrus_godot.so    godot/bin/libgrus_godot.so      # Linux/CI
godot --path godot
```

**Re-copy the library into `godot/bin/` after every rebuild.** A stale binary fails at startup with a misleading error (e.g. a false duplicate-`UnitId` panic), not silently. On a clean clone the GDExtension is not registered until the editor scans once: `godot --headless --path godot --editor --quit-after 120` (writes `godot/.godot/extension_list.cfg`). macOS gotcha: overwriting the staged dylib can invalidate its ad-hoc code signature (`cs_mtime != mtime`) — the kernel SIGKILLs Godot instantly with no output; re-sign after copying with `codesign --force --sign - godot/bin/libgrus_godot.dylib`.

Lint and tests (mirror CI, which uses `--locked` everywhere):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings
cargo test --workspace

# single test / module
cargo test -p grus-sim barracks_trains            # by name substring
cargo test -p grus-sim --test system_order          # one integration target
cargo test -p grus-sim economy::tests               # one inline module

# coverage gate (90% lines on grus-sim gameplay code; `tests.rs` files excluded)
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'

cargo test -p grus-sim --release --locked --test survival_balance -- --ignored --nocapture

# e2e-feature unit tests mutate process-global env vars — must be serialized
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
```

Godot headless smokes (each is a real end-to-end contract, not a stub):

```bash
godot --headless --path godot res://scenes/smoke_test.tscn           # input/selection/move/HUD/cadence + fog/minimap scout leg + research HUD
godot --headless --path godot res://scenes/reset_test.tscn           # fixture reset → 4 units / 1 building / 18 resources
godot --headless --path godot res://scenes/economy_smoke_test.tscn   # full HPA-471 economy loop at 20x sim speed
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn   # HPA-472 combat/session lifecycle at 20x + HPA-188 research boundary leg
godot --headless --path godot res://scenes/wave_smoke_test.tscn      # HPA-449 wave presentation; extends the combat lifecycle journey
godot --headless --path godot res://scenes/defenses_smoke_test.tscn  # HPA-450 wall/gate/turret defense journey
godot --headless --path godot res://scenes/underground_smoke_test.tscn # HPA-288 paid L3/Gather, focused Tab/Esc, pause/restart, slot locks, cold Lab + Tap
```

The `bevy_e2e` out-of-process test needs an exported build: `godot --headless --path godot --export-debug "Linux x86_64" ../build/Grus.x86_64`, then `GRUS_E2E_BINARY=<path> cargo test -p grus-godot --features e2e --test e2e_boot --locked -- --test-threads=1`. Without `GRUS_E2E_BINARY` it skips loudly instead of running. The e2e selector surface is inert unless `BEVY_E2E=1`.

## Architecture

**`crates/grus-sim`** — authoritative gameplay, no Godot and no Bevy plugins/schedules. It exposes plain exclusive functions over a bare `World`:

```rust
apply_player_command(&mut world, &mut map, cmd) -> CommandResult
step_tick(&mut world, &mut map, seconds)  // canonical tick: waves → underground → heat → combat → movement → economy → construction → production → research → visibility
step_waves(&mut world, &mut map, seconds)
step_underground(&mut world, seconds)
step_combat(&mut world, &mut map, seconds)
step_movement(&mut world, &map, seconds)
step_economy(&mut world, &mut map, seconds)
step_construction(&mut world, &map, seconds)
step_production(&mut world, &mut map, seconds)
step_heat(&mut world, seconds)
step_research(&mut world, seconds)
refresh_visibility(&mut world, &map)
```

This is deliberate: sim tests build `World::new()` + `seed_skirmish` and call the steps directly, with no app, no schedule, and no engine. Keep new gameplay in this shape. `step_tick` (`src/tick.rs`) is the single source of runtime tick order — the bridge runs nothing else; the individual `step_*` functions exist for tests that deliberately stop mid-tick.

Modules: `catalog` (the single source of costs, build/train seconds, footprints, unlocks, population, speeds, gather rates — change balance here, nowhere else), `commands` (ownership validation, `RejectReason`, spawning), `economy` (worker tasks, gather/carry/deposit, drop-off routing), `buildings` (placement validation, construction, completion grants), `production` (FIFO queues, surface + Shelter pop cap, spawn clearance, rally), `underground` (one `Underground` resource: paid excavation, four slots/level, independent room timers and fractional resource credit), `movement` (A* + separation), `map` (`GridMap`, `GridPos`, `Footprint`), `heat` (per-tick surface supply/demand from completed Team-1 buildings plus depth-scaled Geothermal Taps, pooled Coal-furnace burn, the `surface_efficiency` accessor), `research` (Team-1's monotonic point total toward Victory — the heat-scaled surface + cold-independent Lab `research_rate`, `step_research`, `add_research` and the `building_unlocked`/`unlock_points` threshold helpers), `tick` (the canonical `step_tick` order), `fixture` (authored skirmish + benchmark seeds — the `TeamStart` table and `RUINS` table are the only layout coordinate sources), `ids`.

**`crates/grus-godot`** — the GDExtension. `build_app` (`#[bevy_app]`) registers the godot-bevy plugins, sets `Time::<Fixed>` to `SIM_STEP_SECONDS` (0.05 → 20 Hz), and chains `FixedUpdate` in a fixed order:

```
apply_pending_commands → advance_sim → drain_route_reject_feedback → stage_combat_events
```

Order is a contract, not an accident. `advance_sim` reads `Time<Fixed>` and runs grus-sim's `step_tick` — waves → underground → heat → combat → movement → economy → construction → production → research → visibility — inside one scoped `GridMap` resource borrow: waves launch after commands and before combat/movement so a new alien can acquire a target in the launch tick; underground completion precedes heat, so a just-completed Tap supplies this tick and a Lab contributes later in this tick; heat computed at the top of the tick scales that same tick's completed-turret cooldowns, production progress and research gain; arrival is visible to economy/construction in the same tick; construction completion (population cap, producer queues) feeds production; production runs after movement so a completed unit's rally route is planned against post-movement occupancy and marches next tick; and research runs after combat so a lethal hit on the Research Center settles Defeat before the same tick's passive gain could reach the target. The closing visibility refresh is what presentation and the next tick's command validation consume. `crates/grus-sim/tests/system_order.rs` calls `step_tick` itself, so reordering the canonical tick fails those tests.

**`godot/`** — presentation and input only. Two autoloads: `BevyAppSingleton` (godot-bevy's app host) and `GrusBridge` (`GrusBridgeNode`, all `#[func]`s). GDScript never touches gameplay state directly; it issues commands (`move_units`, `stop_units`, `gather`, `attack_units`, `attack_move_units`, `place_building`, `resume_construction`, `enqueue_unit`, `set_rally`, `analyze`, `excavate`, `build_room`, `start_match`, `set_paused`, `restart_match`, `reset_fixture`, `reset_benchmark_fixture`, `set_sim_speed`) and reads typed snapshots (`economy_snapshot`, `building_snapshot`, `unit_task_snapshot`, `placement_preview`, `catalogue_snapshot`, `wave_snapshot`, `research_snapshot`, `underground_snapshot`, `session_snapshot`, `visibility_snapshot`, `obstacle_rects`, `command_feedback*`, `last_reject_code`). Gameplay data flows one way, Bevy → Godot.

### Underground controls and data

Tab toggles the panel in `_input`, including with a focused Button; Esc in that same handler closes an open panel and marks the event handled (no pause). Closed-panel Esc retains cancel/pause/resume. The panel uses mouse STOP, no separate world-input guard. Excavate is global, with no selected Research Center ID. Room kinds arm via ordinary Buttons, then eligible empty slot Buttons queue `build_room`; no selected-level state or popup. Lab armed at depth 3 disables L1/L2 with `Requires L3`; Shelter re-enables L1. Purchases are disabled outside Playing, inspection is allowed during pause, and Rust remains the final validator.

`catalogue_snapshot().underground` is static room tuning (`slots`, `room_kinds`, `rooms`); `underground_snapshot()` is read only while visible and carries depth, dig target/progress/remaining, Rust-priced next cost/time and level rows. Levels are **1-based**, slot array/command indices **0-based 0..3**. Six catalogue choices: Shelter (+10 cap), Greenhouse (+0.5 Food/s), Coal Vein (+0.4 Coal/s, L2), Tap (+15 heat at L3, +5/deeper level, no burn), Lab (+1 research/s, L3), Deep Mine (+0.4 Scrap/s, L4). L1–L5 dig prices (Scrap/Coal/time): 100/50/30, 160/80/45, 256/128/60, 410/205/75, 655/328/90. `catalog::excavation_cost_and_seconds` owns rounded `100 × 1.6^(n−1)` Scrap, ceil-half Coal and `30 + 15 × (n−1)` seconds with saturation. Never copy this formula into GDScript. Room prices/timers and the full table are in `README.md`/`catalog.rs`.

Underground needs no workers or cold/fuel modifier. Credit only resource rooms complete at tick entry, then advance paid dig/room timers; nonperiodic benefits apply later the same canonical tick. Start/Paused/Result or a missing living core freeze progress/output. Restart/fixture/benchmark teardown explicitly removes and reseeds `Underground`; no stale depth, slots or remainders survive. The independent Rust `underground_journey` integration test proves Gather-funded L3 Tap+Lab at 179.65 simulated seconds (120.35 s prep margin); the real-input Godot smoke adds a paid cold-base Lab check and keeps the shared 300-s prep guard.

### Non-obvious invariants

- **Numeric reject/blocked codes are `RejectReason` declaration order** (`last_reject_code`, snapshot fields; `-1` = none). GDScript smokes assert those integers, so reordering or inserting a variant mid-enum silently changes the wire contract — append instead, and update the smokes when you don't.
- **Interpolation is hand-rolled.** godot-bevy 0.12 pins `Time<Fixed>::overstep_fraction()` to 0 and syncs transforms once per tick, so `sync_interpolated_unit_transforms` writes each unit view's `Node3D` transform directly every `Update` using `Engine::get_physics_interpolation_fraction()`. Don't route unit positions through `GodotTransformSyncPlugin` expecting smoothing.
- **Coordinates:** sim is a flat 2D plane (`Vec2 { x, y }`); Godot is 3D with sim `y` → world `z`, `y = 0`.
- **Occupancy** mutates only through `GridMap::set_blocked`. The map revision changes only when walkability actually changes; active `MoveOrder`s replan once on a newer revision rather than pathing every tick, and `last_failed_replan` suppresses re-running A* toward a known-unreachable goal until the map changes again.
- **Layout coordinates are single-sourced:** `MapFixture::team_starts()` returns the `TeamStart` table (Research Center anchor, 4 worker cells, Barracks/Depot anchors, `safe_sources: [ResourceId; 6]` per team) and `fixture::RUINS` is the only static-blocker table — `battlefield()` applies it through `set_blocked_rect`, `GrusBridge.obstacle_rects()` exposes the same inclusive rectangles, and `ruins.gd` instantiates one imported ruin prop scaled to each rect. Never copy ruin or start coordinates into tests, `.tscn`, or GDScript — read the table or the live views. Team 1's start is the central city (RC center = map midpoint); team membership and safe-source ids come from the table, never from an `x < 64` style side test.
- **Two reset modes, do not mix:** `reset_fixture` reseeds the 4-unit/1-building/18-resource survival fixture (runtime + smokes) and resets `WaveState`; `reset_benchmark_fixture` reseeds 50 human defenders plus 150 actual aliens through the shared wave spawn/AttackMove path, keeps one Research Center and inserts **no `MatchSession`** (a missing session reads as Playing), and is used only by `benchmark_200.tscn`. Both share the single human-start `seed_skirmish` path — the mirrored seed / `GRUS_MIRRORED_START` launch mode was removed in HPA-445.
- **Sim speed** is `Engine.time_scale` (godot-bevy's fixed driver derives its delta from `_physics_process`), not `Time<Virtual>` alone — `set_sim_speed` writes both; the economy smoke runs at 20×.
- `setup_fixture` is guarded on `UnitIndex` already existing, so a bridge reset before the first fixed tick does not double-seed.

## Tests

Inline `#[cfg(test)] mod tests;` lives in sibling `src/<module>/tests.rs` files (excluded from the coverage gate); cross-module contracts live in `crates/grus-sim/tests/` (`system_order.rs`, `economy_flow.rs`, `occupancy_replan.rs`). Behavior that crosses the Godot boundary belongs in a GDScript smoke scene under `godot/scenes/` + `godot/scripts/`, driven through real UI/input paths rather than direct state pokes.

## Docs

Design specs and implementation plans live in `docs/superpowers/specs/` and `docs/superpowers/plans/`, named `YYYY-MM-DD-<ticket>-<slug>.md`. Read the relevant spec before changing an area it covers — they record locked contracts (command results, approach slots, reset/cutover ordering, tick order).
