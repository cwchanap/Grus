# Grus

2.5D real-time strategy game built with Godot and Bevy ECS via godot-bevy.

## HPA-470 battlefield baseline

The first retained MVP slice runs a fixed-angle 3D Godot battlefield over a flat Bevy ECS simulation plane. Bevy owns stable unit identity, commands, occupancy/pathing, and authoritative movement. Godot owns presentation and input; gameplay transforms flow one way from Bevy into Godot.

The simulation runs on an explicitly configured **20 Hz Bevy fixed clock**. Godot presentation interpolates between the previous and current authoritative positions every visual update; the runtime smoke measured a maximum visual step of **0.095 world units** for a speed-12 unit, versus a 0.6-unit fixed-tick step without interpolation.

Occupancy changes use one shared `GridMap::set_blocked` seam. The map revision changes only when walkability actually changes, and active move orders replan once when they observe a newer revision instead of pathfinding every tick. This is the construction-facing occupancy contract consumed by later MVP work.

The first recorded desktop target is **Linux x86_64** with **Godot 4.6.2** and the repository-pinned **Rust 1.89.0** toolchain.

### Build and launch

Install Godot 4.6.2 with export templates, then from the repository root:

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --path godot
```

### Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim

godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
```

The main Godot smoke exercises real input-derived click/box/additive selection, control groups, move, stop, HUD input shielding, zoom, pan, the 20 Hz cadence, and interpolated presentation against the 200-unit ECS fixture. The reset smoke despawns/reseeds the retained fixture and requires it to settle back to exactly **200 scene nodes with 200 unique stable UnitIds**.

Rust tests cover ownership/live-entity checks, successful and unreachable routes, replacement orders, stop, obstacle avoidance, 100-unit traversal, occupancy mutation semantics, and active-route replanning after a new obstacle intersects the path.

### Controls

- Left click: select a friendly unit
- Shift + left click: add to selection
- Left drag: box select
- Right click: move selected units
- `S`: stop selected units
- `Ctrl+1` through `Ctrl+9`: assign a control group
- `1` through `9`: recall a control group
- Mouse wheel: zoom
- Middle drag: pan the camera

HUD controls consume their mouse events so UI interaction does not leak into world commands.

### Linux x86_64 export

```bash
mkdir -p build
godot --headless --path godot --export-debug "Linux x86_64" ../build/Grus.x86_64
./build/Grus.x86_64
```

CI also boots the exported executable headlessly and verifies that the Rust GDExtension initializes successfully.

## 200-unit 1080p baseline

CI run **87** records the post-20-Hz/interpolation 1920x1080 baseline while **all 200 units are moving** through the retained command/path/movement systems.

| Item | Baseline |
| --- | --- |
| Host OS | Ubuntu 24.04.4, Linux x86_64 |
| Reference CPU | 4 vCPU, AMD EPYC 9V74 80-Core Processor |
| Godot | 4.6.2 |
| Build mode | Rust dev/debug GDExtension in the Godot editor runtime; debug export verified separately |
| Display | X11 via Xvfb, 1920x1080 |
| Renderer | `gl_compatibility`, OpenGL 4.5 |
| Adapter | Mesa `llvmpipe` (LLVM 20.1.2, 256-bit) software renderer |
| Sample | 60 warm-up frames + 240 measured frames |
| Mean frame time | 160.166 ms |
| p50 | 155.723 ms |
| p95 | 170.445 ms |
| p99 | 210.995 ms |
| Max | 216.646 ms |
| Mean FPS | 6.2 |

This is intentionally a **software-rendered CI reference baseline**, not a representative discrete/integrated-GPU performance result. The project target remains 60 FPS; a hardware-rendered workstation measurement is still required before treating that target as achieved or missed.
