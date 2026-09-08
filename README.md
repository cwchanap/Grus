# Grus

2.5D real-time strategy game built with Godot and Bevy ECS via godot-bevy.

## HPA-470 battlefield baseline

The first retained MVP slice runs a fixed-angle 3D Godot battlefield over a flat Bevy ECS simulation plane. Bevy owns unit identity, commands, pathing, movement, and the authoritative 20 Hz simulation. Godot owns presentation and input; gameplay transforms flow one way from Bevy into Godot.

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
```

The Godot smoke exercises real input-derived click/box/additive selection, control groups, move, stop, HUD input shielding, zoom, and pan against the 200-unit ECS fixture.

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

CI run 70 records the first reproducible 1920x1080 baseline while **all 200 units are moving** through the retained command/path/movement systems.

| Item | Baseline |
| --- | --- |
| Host OS | Ubuntu 24.04.4, Linux x86_64 |
| CPU | 4 vCPU, AMD EPYC 7763 |
| Godot | 4.6.2 |
| Build mode | Rust dev/debug GDExtension in the Godot editor runtime; debug export verified separately |
| Display | X11 via Xvfb, 1920x1080 |
| Renderer | `gl_compatibility`, OpenGL 4.5 |
| Adapter | Mesa `llvmpipe` software renderer |
| Sample | 60 warm-up frames + 240 measured frames |
| Mean frame time | 161.506 ms |
| p50 | 161.223 ms |
| p95 | 166.938 ms |
| p99 | 171.352 ms |
| Max | 176.211 ms |
| Mean FPS | 6.2 |

This is intentionally a **software-rendered CI baseline**, not a representative GPU performance result. The project target remains 60 FPS; a hardware-rendered workstation measurement is required before treating that target as achieved or missed.
