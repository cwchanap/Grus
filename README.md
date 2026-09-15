# Grus

2.5D real-time strategy game built with Godot and Bevy ECS via godot-bevy.

## HPA-470 battlefield baseline

The first retained MVP slice runs a fixed-angle 3D Godot battlefield over a flat Bevy ECS simulation plane. Bevy owns stable unit identity, commands, occupancy/pathing, and authoritative movement. Godot owns presentation and input; gameplay transforms flow one way from Bevy into Godot.

The simulation runs on an explicitly configured **20 Hz Bevy fixed clock**. Godot presentation interpolates between the previous and current authoritative positions every visual update; the runtime smoke measured a maximum visual step of **0.095 world units** for a speed-12 unit, versus a 0.6-unit fixed-tick step without interpolation.

Occupancy changes use one shared `GridMap::set_blocked` seam. The map revision changes only when walkability actually changes, and active move orders replan once when they observe a newer revision instead of pathfinding every tick. This is the construction-facing occupancy contract consumed by later MVP work.

The first recorded desktop target is **Linux x86_64** with **Godot 4.6.2** and the repository-pinned **Rust 1.89.0** toolchain.

## HPA-471 economy skirmish

The runtime fixture is now the authored skirmish seed: 8 villagers (4 per team), 2 completed Town Centers, 18 finite resource sources (berries/trees/gold, plus southwest/northeast expansion nodes), and starting stockpiles of 200 Food / 300 Wood / 100 Gold at Age 1.

### Economy architecture

Bevy remains fully authoritative for gameplay. A typed catalogue (`crates/grus-sim/src/catalog.rs`) is the single source of costs, build/train seconds, footprints, unlocks, population values, speeds, and gather rates. Gameplay lives in four modules:

- **commands** — player command application, ownership validation, `RejectReason` codes, unit spawning
- **economy** — worker tasks, gather/carry/deposit, resource sources, drop-off routing
- **buildings** — placement validation, construction, completion grants (Dropoff, queues, Farm sources)
- **production** — FIFO queues, population caps, spawn clearance, rally points, one-time Age 2

Systems tick in a fixed order (movement → economy → construction → production) so completions land deterministically. Godot drives gameplay only through `GrusBridge` command calls (move/stop/gather/place/resume/enqueue/rally) and reads state through typed snapshots: `economy_snapshot` (stockpiles, age, population, idle workers, last reject code), `building_snapshot` (completion, construction/queue progress, blocked reason, rally), and the read-only `placement_preview`. Numeric reject/blocked codes follow `RejectReason` declaration order; presentation flows one way from Bevy into Godot views.

### Build and launch

Install Godot 4.6.2 with export templates, then from the repository root:

```bash
cargo build -p grus-godot
mkdir -p godot/bin
# Linux/CI loads the .so, macOS loads the dylib — copy the one for your platform:
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
cp target/debug/libgrus_godot.dylib godot/bin/libgrus_godot.dylib  # macOS
godot --path godot
```

On a clean clone the GDExtension is not registered until the editor has scanned the project once (`godot --headless --path godot --editor --quit-after 120`), which writes `godot/.godot/extension_list.cfg`. The library in `godot/bin/` must be re-copied after every rebuild — a stale binary fails at startup (for example a false duplicate-`UnitId` panic) rather than silently.

### Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

The main Godot smoke exercises real input-derived click/box/additive selection, control groups, move, stop, HUD input shielding, zoom, pan, the 20 Hz cadence, and interpolated presentation against the ECS-backed skirmish views. The reset smoke despawns/reseeds the skirmish fixture and requires it to settle back to exactly 8 unit views / 2 building views / 18 resource views with unique stable ids and the starting 200/300/100 stockpile. The economy smoke runs the full HPA-471 loop through the real UI paths — gathering until every spend is solvent, House (cap 10 → 20), Storehouse delivery proof, Farm with the `FarmOccupied` reject, Barracks/Archery Range/Stable training Spearman/Archer/Cavalry, Age 2 unlocking the Stable, a Town Center rally point followed by a trained unit, and idle-worker navigation — at 20× virtual time, asserting construction/queue progress and numeric reject/blocked codes from snapshots throughout.

### Controls

- Left click: select a friendly unit or building (units take priority within the click radius)
- Shift + left click: add to selection
- Left drag: box select
- Right click: contextual — gather on a resource, resume construction on an incomplete friendly building, set a rally point when a producer is selected, otherwise move
- `S`: stop selected units
- `Ctrl+1` through `Ctrl+9`: assign a control group
- `1` through `9`: recall a control group
- Mouse wheel: zoom
- Middle drag: pan the camera
- Build buttons (bottom panel): arm placement for House/Storehouse/Farm/Barracks/Archery Range/Stable; the ground preview renders green when valid and red when blocked; left-click places, right-click or `Esc` cancels
- Train buttons: queue Villager/Spearman (Barracks), Archer (Archery Range), Cavalry (Stable) on the selected building
- Advance Age: research Age 2 on the selected Town Center (300 Food, 200 Gold); gather rates rise to 2.2/s and the Stable unlocks
- Idle button (top right): selects and navigates to the next idle worker

HUD controls consume their mouse events so UI interaction does not leak into world commands.

### Reset modes

- **Fixture reset** (`reset_fixture`, used by `reset_test.tscn`): despawns gameplay and reseeds the 8-unit/2-building/18-resource skirmish with starting stockpiles.
- **Benchmark reset** (`reset_benchmark_fixture`, used by `benchmark_200.tscn`): reseeds the benchmark-only 200-villager fixture for the 1080p performance baseline; it is never used by the runtime skirmish or its smokes.

### Linux x86_64 export

```bash
mkdir -p build
godot --headless --path godot --export-debug "Linux x86_64" ../build/Grus.x86_64
./build/Grus.x86_64
```

CI also boots the exported executable headlessly and verifies that the Rust GDExtension initializes successfully.

## Testing / CI

CI runs three jobs. `rust-build-lint` gates formatting, Clippy, and workspace compilation. `unit-tests` enforces the 90% `grus-sim` line-coverage gate and runs the `grus-godot` library unit tests. `e2e` runs the Godot import/smoke/export validations, the bevy-e2e boot test against the exported build, and the 200-unit benchmark.

Rust build/lint:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
```

Unit coverage:

```bash
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
cargo test -p grus-godot --lib
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
```

E2E:

```bash
cargo build -p grus-godot --features e2e
# stage extension and export Godot build as CI does
GRUS_E2E_BINARY=<path-to-exported-grus> \
  xvfb-run -a cargo test \
  -p grus-godot \
  --features e2e \
  --test e2e_boot \
  -- --test-threads=1
```

`GRUS_E2E_BINARY` must point at the exported Grus executable (stage the extension, then `godot --headless --path godot --export-debug "Linux x86_64"` as in the export section above); without it the e2e test skips loudly instead of running.

## 200-unit 1080p baseline

The 200-unit fixture exists only for this benchmark (`benchmark_200.tscn` after an explicit benchmark reset); the runtime game and its smokes run the skirmish fixture above.

CI run **95** records the post-20-Hz/interpolation 1920x1080 baseline while **all 200 units are moving** through the retained command/path/movement systems.

| Item | Baseline |
| --- | --- |
| Host OS | Ubuntu 24.04, Linux x86_64 |
| Reference CPU | 4 vCPU, AMD EPYC 7763 64-Core Processor |
| Godot | 4.6.2 |
| Build mode | Rust dev/debug GDExtension in the Godot editor runtime; debug export verified separately |
| Display | X11 via Xvfb, 1920x1080 |
| Renderer | `gl_compatibility`, OpenGL 4.5 |
| Adapter | Mesa `llvmpipe` (LLVM 20.1.2, 256-bit) software renderer |
| Sample | 8 warm-up frames + 40 measured frames |
| Mean frame time | 192.607 ms |
| p50 | 198.735 ms |
| p95 | 208.873 ms |
| p99 | 210.047 ms |
| Max | 210.047 ms |
| Mean FPS | 5.2 |

This is intentionally a **software-rendered CI reference baseline**, not a representative discrete/integrated-GPU performance result. The project target remains 60 FPS; a hardware-rendered workstation measurement is still required before treating that target as achieved or missed.
