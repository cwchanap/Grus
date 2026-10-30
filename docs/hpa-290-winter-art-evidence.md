# HPA-290 winter-city art verification

Local verification on 2026-10-28–29, macOS / Apple M1 Pro, Godot 4.7.1 (`gl_compatibility`), Rust 1.95.0 and Blender 5.2.1 LTS. Base `ba5429b`. No simulation source changes; the third review round below added one read-only bridge snapshot (`unit_task_snapshot`).

## Delivered

- Textured, mipmapped snow ground over the existing 128×96 battlefield, planar-sampled at one tile per eight world units.
- Six self-contained low-poly GLBs: collapsed building, wrecked transport, frozen debris, ration cache, scrap ruin and coal seam. Each ships one shared material; per-piece colors ride `COLOR_0` vertex colors, so every prop is a single surface/draw call.
- Bridge-derived obstacle placements, one imported prop per rectangle whose footprint covers its blocked rect exactly; no copied layout coordinates or added collision/navigation.
- Existing resource roots still own IDs, visibility, picking and depletion. Gathering picks against the projected art bounds, so the silhouette is clickable at any window size or zoom. Materials are shared without per-instance mutation.
- [Asset provenance and regeneration](../godot/assets/winter/README.md).

## Checks

| Check | Result |
| --- | --- |
| `cargo build -p grus-godot --locked`, copied current library to `godot/bin` | Passed |
| `cargo test --workspace --locked` | 244 tests passed across 8 suites |
| Fresh Godot editor import with the generated `.godot` cache moved aside | Passed; no warnings/errors |
| Reimport after final resource sizing | Passed; no warnings/errors |
| `res://scenes/smoke_test.tscn` | `GRUS_GODOT_SMOKE_OK`, max visual step 0.090; exact-cover ruin bounds, foreign-node sweep and targeted real-input gather picks at max zoom |
| `res://scenes/reset_test.tscn` | `GRUS_RESET_SMOKE_OK`, fixture 4/1/18 and starting stockpile/fog preserved |
| `res://scenes/economy_smoke_test.tscn` | `GRUS_ECONOMY_SMOKE_OK`, real UI gather/build/train/rally flow |
| `res://scenes/combat_lifecycle_smoke_test.tscn` | `GRUS_COMBAT_LIFECYCLE_SMOKE_OK`, Crystal reward, pause, Defeat, restart |
| `res://scenes/wave_smoke_test.tscn` | `GRUS_WAVE_SMOKE_OK`, warning, anchor pings, fog and role silhouettes |
| Blender exporter self-checks | All six assets passed bounds and triangle budgets, 84–396 triangles each |
| `git diff --check` | Passed |

All five final smokes ran sequentially, exited 0, emitted their success marker and reported no errors or warnings. An earlier run overlapped the main timing-sensitive smoke with rendered capture and failed interpolation (`max_step=0.552`); isolated runs passed at 0.052 and 0.059 before the picking correction. Bridge-smoke `max_step` history: 0.126 was the final run before the review follow-ups, 0.174 the re-run on the second-round revision, and 0.090 the final run on the current tree after the third-round gather asserts — the table reports the current-tree figure. Render capture runs separately from gameplay smokes.

## Regressions caught and fixed

The new geometry check first failed on placeholder boxes. It subsequently caught Godot's `_vehicle` node-name hint turning the wreck mesh into a falling physics vehicle; the export suffix handling is now disabled entirely in the `.import` files, so the class of problem is gone — the smoke still sweeps imported props (ruins and resource models) for physics bodies, colliders, navigation regions and occluders.

One fresh independent whole-change reviewer found raised resource geometry outside the existing 20-pixel click radius at maximum zoom; art was first exported at 75% size to fit inside it.

A second review found that radius only held at the fixed 1280×720 viewport — the window resizes freely with no stretch mode, so larger windows or zoomed-in picking shrank the 20 px target below the silhouette. Gathering now picks against each resource's projected art bounds (with a projected-cell radius floor for zoomed-out forgiveness) instead of a fixed pixel radius, and the 75% shrink is reverted: resource art fills its gather cell again. The same review also drove: a grey-placeholder fallback plus `push_warning` for an unmapped `resource_kind` (previously invisible but still gatherable); `--python-exit-code 1` in the Blender regeneration docstring so a failed assert fails the run; vertex-color single-surface props (about 30 draw calls where per-color materials cost ~110); `generate_lods` off for 84–396-triangle props; a 1024² VRAM-compressed snow texture replacing a 1254² lossless one; the flat ground sampling planar instead of triplanar; `resource_view.gd` styling once then stopping `_process`; the ruin bounds check restored to exact coverage under `global_transform` (it had weakened to containment and compared a parent-relative transform against world coordinates); the picking check rewritten through real input (`push_input` right-clicks asserting "Gather command queued") instead of camera pokes and a private method call; and winter minimap colors replacing the green terrain palette. The CLAUDE.md `ruins.gd` description was stale and is corrected.

A third review found the real-input gather check could pass on a stale label: `"Gather command queued"` survives until other feedback overwrites it, so a swallowed `push_input` or a pick landing on a different resource still read green. Each click now writes a sentinel first, waits for the sim's feedback revision to advance with an accept, and asserts through the new `unit_task_snapshot` bridge read that every commanded worker's task names that resource's own `resource_id`; the leg also requires at least one targeted pick per Food/Scrap/Coal model kind, whose imported bounds differ. The same round caught the planar ground scale: with triplanar off, `uv1_scale` multiplies the `BoxMesh` 3×2 UV atlas — the top face spans one third of U and one half of V — so `Vector3(48, 24, 1)` is what actually yields the documented 16×12 tile repeats.

The benchmark was re-measured on this tree: 10.01 s / 786 frames, mean 12.735 ms (p50 6.999, p95 37.705, p99 45.184, max 54.072; 78.5 fps), live 200 → 163 (mean 184.1), visible-in-tree mean 96.4, visibility revision 240 — level with the superseded HPA-449 baseline (mean 12.980 ms). A sibling run under unrelated host load read 20.683 ms mean / 183.913 ms max; that spread is host noise, not a regression signal. README's Current baseline is updated accordingly.

## Rendered inspection

Both captures use the actual Godot-imported GLBs and ground material, with the game's fixed-angle orthographic view. The asset sheet enlarges props for inspection; the battlefield image uses the native 1280×720 game viewport after Start, with normal fog. Static ruins retain their existing visibility above the flat fog layer; they are authored map terrain, not hidden enemy entities. Capture exited 0 without engine warnings/errors.

![Imported winter art](hpa-290-asset-preview.png)

![Live battlefield after Start](hpa-290-battlefield.png)

## Limits

The checks above are local validation on macOS / Apple M1 Pro. Hosted results: the branch is pushed and CI run [#231](https://github.com/cwchanap/Grus/actions/runs/40921850258) passed all three Linux jobs — fmt/clippy/workspace build, the 90%-line coverage gate plus `grus-godot` unit and e2e-selector tests, all five headless Godot smokes (bridge `max_step` 0.067), the packaged x86_64 export running the full bridge-smoke contract (`max_step` 0.066), the 10-second software-GL benchmark and the bevy-e2e exported-game test.

Still outstanding: reference Linux hardware performance acceptance. CI's benchmark ran under llvmpipe software GL on a shared runner (mean 488 ms/frame), which gates the benchmark contract but is not a hardware measurement. Not merged.
