# Winter-city art — HPA-290

Six self-contained glTF 2.0 binary assets with one shared material per prop — per-piece colors ride `COLOR_0` vertex colors, so every model renders as a single surface — plus the snow albedo used by `battlefield.tscn`. No external model downloads, plug-ins or runtime generation.

| Asset | Triangles | Use |
| --- | ---: | --- |
| `collapsed_building.glb` | 396 | Broken concrete facade, empty windows, collapsed floor, snow and rebar |
| `wrecked_vehicle.glb` | 276 | Snowbound transport wreck with broken windows and rusted hood |
| `frozen_debris.glb` | 208 | Frozen masonry and fallen steel girder |
| `ration_cache.glb` | 120 | Food: stacked orange ration cases with ivory straps |
| `scrap_ruin.glb` | 84 | Scrap: rusted metal salvage frame and loose girders |
| `coal_seam.glb` | 112 | Coal: angular charcoal outcrop with a snow crest |

Obstacle bases fill X/Z −0.5..0.5 edge-to-edge — the Blender source asserts it, since `ruins.gd` scales each prop's footprint to the authoritative blocked rectangle — with Y 0..2. `ruins.gd` derives centers and horizontal scale from `GrusBridge.obstacle_rects()`; variants cycle in bridge order. Every resource fits one cell with a ground-level origin; gathering picks against the projected art bounds, so the whole silhouette stays clickable at any window size or zoom.

Node name/type suffixes and LOD generation are disabled in the `.glb.import` files (`-col`, `-rigid`, `-navmesh` style names can never add physics, navigation or occluders to static art), and `winter_glb_import.gd` enables vertex-color albedo on the imported material — Godot imports `COLOR_0` but does not honor it without that opt-in. The gameplay smoke still sweeps every imported prop for gameplay nodes. Resource roots retain the bridge's visibility, picking and depletion behavior.

Models were authored for this repository on 2026-10-28 using [the Blender source](../../../scripts/build_winter_assets.py), exported with Blender 5.2.1 LTS. To regenerate from the repository root:

```sh
blender --background --factory-startup --python-exit-code 1 --python scripts/build_winter_assets.py
```

`--python-exit-code 1` makes a failed export assert fail the whole command instead of leaving a mixed stale/fresh GLB set under a success status. The exporter asserts bounds and triangle budgets. Commit the `.glb.import` and PNG `.import` settings; Godot recreates the ignored `.godot/` cache.

`snow_albedo.png` is an Imagegen-generated texture, created 2026-10-28 at 1254² and shipped at 1024² VRAM-compressed. Prompt: seamless top-down compacted off-white snow, subtle pale blue-grey windswept ice patches, sparse shallow fissures, low-contrast broad polygonal painterly shapes, neutral diffuse lighting; no objects, directional shadows, landmarks or text. The ground material uses one planar sample at one tile per eight world units (`uv1_scale` 16×12 over the 128×96 ground face) — a flat plane needs no triplanar triple-sample. No hand-painted road or terrain features imply additional gameplay geometry.
