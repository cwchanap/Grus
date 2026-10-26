# HPA-449 Stage 4 Report

Date: 2026-10-26

Branch: `hpa-449-wave-director`

Base: `fd7d61f`

Status: implementation and local macOS gates complete.
Commit: `a4aabdeedbf59da1beaba2baa9151d26e077100f` (`HPA-449 integrate alien wave fixture cutover`).

## Changes

- Replaced the two-base seed with one human Research Center, four Workers, and the existing 18 authored resources. Updated dependent fixture, visibility, movement, combat, session, economy-flow, system-order, bridge, and e2e tests.
- Added `advance_waves` to the bridge fixed-step chain and the narrow read-only `wave_snapshot`. It reports the clock, public `ENTRY_ZONES` warning names/anchors, and live alien count, without exposing hidden unit positions.
- Migrated the benchmark to 50 human defenders and 150 real aliens through the shared wave spawn/AttackMove path in a session-free world. The 10-second census, fog, movement/combat liveness, and frame-stat checks remain active.
- Migrated reset and pre-wave smokes. The economy smoke retains a pre-reset and end guard, splits long work into two Playing legs, waits for the controller's screen-space picker, and gives queued mouse events two process-frame turns before asserting the real UI selection.
- Reworked the combat lifecycle smoke around warning, live alien scouting/combat, Crystal credit, alien Defeat, and restart. Restart assertions use captured stable typed IDs rather than Guard nodes which combat has freed.
- Added `CombatFx._exit_tree()` audio stop/release cleanup. A verbose benchmark found its generated WAV and playback still referenced at process exit; the rerun no longer reports those leaked objects.
- Updated README and CLAUDE project guidance, including a new measured HPA-449 local headless benchmark entry and clear separation from Linux CI and release-hardware acceptance.

## TDD and integration findings

- The first economy runs exposed a pre-wave queue workload that could run too long. It was split into two guarded Playing legs; no timeout was increased. The final economy scene completes before wave one.
- The Barracks-selection red trace showed `selected_building_id=-1`, the old unit selection still active, and `_left_press_position` unchanged despite the screen picker reporting the Barracks. Godot's macOS headless loop flushes buffered input after the first resumed frame. Two consecutive `process_frame` awaits in `_click_view`, `_click_world`, and `_click_button` fixed the synchronization without changing input routing or selection state. Final trace during the green run confirmed `wanted=2 selected=2` and `last_press=(595.0,357.7774)` matching the Barracks screen coordinate. Diagnostic probes are recorded in `/private/tmp/hpa449-task4/economy-smoke-click-radius-2.log`, `/private/tmp/hpa449-task4/economy-smoke-input-probe.log`, and `/private/tmp/hpa449-task4/economy-smoke-stage-trace.log`; the clean final run is `/private/tmp/hpa449-task4/economy-smoke-final.log`.
- The combat smoke first exposed stale `Node3D` references after defenders died, then a typed-array assignment error. Capturing Guard IDs before combat and rebuilding an `Array[int]` resolved both; final run is green.
- The integrated workspace test exposed one remaining old fixture assumption: the newly placed Depot is `BuildingId(2)` with a single seeded building, not ID 3. Updating that consumer restored the full suite.
- Workspace Clippy caught three unused imports in the revised session tests; removed them and reran both Clippy commands.
- The initial benchmark shutdown warning was console-only and `--verbose` identified `AudioStreamWAV` and `AudioStreamPlaybackWAV` as leaked. Stopping the `AudioStreamPlayer` and clearing its stream in `_exit_tree()` removed the warning in the final verbose run.

## Verification

Commands were run in the required checkout. Rust commands were prefixed with `rtk`; bounded Godot process output and exit codes are recorded at the listed paths.

### Rust

- `cargo fmt --all -- --check` — exit 0. Log: `/private/tmp/hpa449-task4/cargo-fmt-final.log`.
- `cargo test --workspace --locked` — exit 0, 239 passed across 8 suites. Log: `/private/tmp/hpa449-task4/cargo-test-workspace-final.log`. The earlier red run and stale-ID failure are in `/private/tmp/hpa449-task4/cargo-test-workspace.log`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — exit 0, no issues. Log: `/private/tmp/hpa449-task4/clippy-workspace-final.log`.
- `cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings` — exit 0, no issues. Log: `/private/tmp/hpa449-task4/clippy-godot-e2e.log`.
- Focused tests passed during implementation: `cargo test -p grus-sim fixture --lib --locked` (14), `cargo test -p grus-sim --test system_order --locked` (4), `cargo test -p grus-sim session --lib --locked` (9), `cargo test -p grus-godot --lib --locked` (15), and serialized `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1` (18).
- `cargo build -p grus-godot --locked` passed with no warnings; the rebuilt `target/debug/libgrus_godot.dylib` was copied into `godot/bin/` before Godot runs. Log: `/private/tmp/hpa449-task4/cargo-build-godot-final.log`.
- `git diff --check` passed.

### Godot 4.7.1 headless smokes

- One clean editor import after the extension was staged — exit 0. Log: `/private/tmp/hpa449-task4/godot-editor-import.log`.
- `godot --headless --path godot res://scenes/reset_test.tscn` — exit 0; `GRUS_RESET_SMOKE_OK units=4 buildings=1 resources=18 stockpile=200/300/150/0 population=4/10 fog=128x96 pre_wave=true`. Log: `/private/tmp/hpa449-task4/reset-smoke.log`.
- `godot --headless --path godot res://scenes/smoke_test.tscn` — exit 0; `GRUS_GODOT_SMOKE_OK units=4 controls=input-derived camera=pan-zoomed unreachable=feedback cadence=20hz interpolation=max_step_0.165 fog=scout_reveal_hide explored_persist=true markers=fog_gated recenter=command_free pre_wave=true`. Log: `/private/tmp/hpa449-task4/general-smoke.log`.
- `godot --headless --path godot res://scenes/economy_smoke_test.tscn` — exit 0 in 17.243 s; `GRUS_ECONOMY_SMOKE_OK food=40 scrap=140 coal=150 cap=10 units=7 buildings=2 unaffordable_reject=8 elapsed_ms=17243`. Final log: `/private/tmp/hpa449-task4/economy-smoke-final.log`.
- `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn` — exit 0; `GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh fixture=4/1/18 warning=North live_aliens=true fog=hidden_then_revealed markers=visibility_gated attack=accepted crystal_reward=1 pause_frozen=true result=alien_defeat result_reject=session_locked restart_wave_reset=true`. Final log: `/private/tmp/hpa449-task4/combat-lifecycle-smoke-typed-id-fix.log`.
- Nonfatal macOS headless CA-certificate lookup output (`Condition "ret != noErr"`) appears in these logs; each process exited 0 with its expected marker.

### 10-second session-free survival benchmark

Command: `GRUS_BENCHMARK_SECONDS=10 godot --verbose --headless --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn` (bounded at 45 seconds). It exited 0 with `GRUS_BENCHMARK_200 start_units=200 human_units=50 alien_units=150 min_live_units=167 mean_live_units=184.7 end_units=167 min_visible_in_tree_units=70 mean_visible_in_tree_units=93.8 end_visible_in_tree_units=166 duration_s=10.00 frames=786 mean_ms=12.726 p50_ms=6.794 p95_ms=36.838 p99_ms=46.533 max_ms=63.319 mean_fps=78.6 visibility_revision=239 renderer=gl_compatibility adapter= vendor= display=headless`. Verbose shutdown reported no ObjectDB leaks. Log: `/private/tmp/hpa449-task4/benchmark-10s-audio-cleanup.log`.

This is a macOS 26.6.2 arm64, Godot 4.7.1 debug-extension **headless** contract/liveness measurement. It is not Linux CI software-GL proof, rendered hardware performance, or the 60-second Linux release-export acceptance run.

## Remaining external gates

- The `grus-sim` 90% `cargo llvm-cov` gate is deferred to stage 6 as authorized by the coordinator.
- Linux export/boot, hosted CI, and the 60-second reference-host measurement were not run locally. Docker is installed but its daemon socket is unavailable; these remain Linux CI/reference-host gates.
- The initial 70/90-second combat diagnostic attempts are not counted as passes; only the final exit-0 run above is evidence. No smokes were skipped or marked successful from a timeout.

## Task-4 review fix round 1 (base `a4aabde`)

### Changes

- Sorted the benchmark's collected fixture units by stable `UnitId` before building either worker/source assignments or the defender order. The regression contract now pins the four initial pairs to `(UnitId(1), ResourceId(1))` through `(UnitId(4), ResourceId(4))`.
- Restored accepted combat input coverage in the lifecycle smoke through real controller input delivery: drag-select the three trained Guards, press A, right-click the fixture-announced ground target, then right-click a currently visible live alien. The test asserts the controller's selection, armed state, command status, and simulation acceptance; accepted orders no longer call the bridge directly.
- Added two `process_frame` awaits after the synthetic click, drag, and key events, matching the established buffered-input synchronization used by the economy smoke.
- Updated the README benchmark measurement after the deterministic route change. The latest 10-second run exited 0 and passed its live census, visibility, movement/combat liveness, and timing gates.

### Focused verification

- `rtk cargo test -p grus-sim --locked benchmark_workload_uses_the_shared_director_for_150_aliens_and_50_defenders` — exit 0, 1 passed (222 filtered out).
- `rtk cargo fmt --all -- --check` — exit 0.
- `rtk cargo build -p grus-godot --locked` — exit 0; copied the rebuilt `target/debug/libgrus_godot.dylib` into `godot/bin/libgrus_godot.dylib` before Godot runs. Both files had SHA-256 `f48c515dab48e66f86a0e1bb166057d4be7e989bb5251d6190d6525a0742c77e`.
- Combat lifecycle command: `godot --verbose --headless --path godot --rendering-method gl_compatibility --log-file /private/tmp/hpa449-task4/combat-lifecycle-round1-godot.log res://scenes/combat_lifecycle_smoke_test.tscn`, run under a 90-second subprocess timeout — exit 0 in 25.920 s. Marker: `GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh fixture=4/1/18 warning=North live_aliens=true fog=hidden_then_revealed markers=visibility_gated controller_attack_move=accepted controller_visible_alien_right_click=accepted crystal_reward=1 pause_frozen=true result=alien_defeat result_reject=session_locked restart_wave_reset=true`. Captured process output: `/private/tmp/hpa449-task4/combat-lifecycle-round1-typed.log`; Godot log: `/private/tmp/hpa449-task4/combat-lifecycle-round1-godot.log`.
- Benchmark command: `GRUS_BENCHMARK_SECONDS=10 godot --verbose --headless --path godot --rendering-method gl_compatibility --log-file /private/tmp/hpa449-task4/benchmark-10s-stable-assignments-engine.log res://scenes/benchmark_200.tscn`, run under a 45-second subprocess timeout — exit 0 in 12.642 s. Marker: `GRUS_BENCHMARK_200 start_units=200 human_units=50 alien_units=150 min_live_units=167 mean_live_units=184.8 end_units=167 min_visible_in_tree_units=73 mean_visible_in_tree_units=95.7 end_visible_in_tree_units=166 duration_s=10.00 frames=788 mean_ms=12.693 p50_ms=6.788 p95_ms=37.433 p99_ms=43.329 max_ms=53.920 mean_fps=78.8 visibility_revision=236 renderer=gl_compatibility adapter= vendor= display=headless`. Captured process output: `/private/tmp/hpa449-task4/benchmark-10s-stable-assignments.log`; Godot log: `/private/tmp/hpa449-task4/benchmark-10s-stable-assignments-engine.log`.
- `rtk git diff --check` — exit 0. The full workspace suite and broad Clippy runs were not repeated for this scoped fix.

### Evidence and limitations

- Initial Godot attempts without `--log-file` failed before the scene ran, with `Failed to open 'user://logs/godot....log'` followed by startup signal 11; exact captured outputs are `/private/tmp/hpa449-task4/combat-lifecycle-round1.log` and `/private/tmp/hpa449-task4/combat-lifecycle-round1-gl.log`. Subsequent Godot commands used `--log-file` under writable `/private/tmp`. The first explicit-log run reported parse errors at lines 144 and 367 (`Cannot infer the type of "screen_position"` / `"defender_screen"`); see `/private/tmp/hpa449-task4/combat-lifecycle-round1-logfile.log`. Both coordinates are now explicitly typed `Vector2`, and the final combat run parsed and passed.
- The benchmark process exited 0 with its full marker, but captured stdout at line 103 emitted `WARNING: 2 ObjectDB instances were leaked at exit`, naming one `AudioStreamWAV` and one `AudioStreamPlaybackWAV`, followed by `Orphan StringName: Master`. This warning was absent from the separate `--log-file` copy, so it was not duplicated across outputs. `combat_fx.gd` already stops its `AudioStreamPlayer` and clears the stream in `_exit_tree()`; no new audio cleanup defect was identified in this scoped fix. The immediately preceding local benchmark log had no ObjectDB warning; record this result as a shutdown warning, not a liveness or performance failure.
- Both scene checks are local macOS 26.6.2 arm64 Godot 4.7.1 headless results. They do not establish Linux CI, rendered hardware, or release-export performance.
