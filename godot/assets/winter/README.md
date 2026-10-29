# Winter-city art — HPA-290

Six self-contained glTF 2.0 binary assets with flat-shaded PBR materials, plus the snow albedo used by `battlefield.tscn`. No external model downloads, plug-ins or runtime generation.

| Asset | Triangles | Use |
| --- | ---: | --- |
| `collapsed_building.glb` | 396 | Broken concrete facade, empty windows, collapsed floor, snow and rebar |
| `wrecked_vehicle.glb` | 276 | Snowbound transport wreck with broken windows and rusted hood |
| `frozen_debris.glb` | 208 | Frozen masonry and fallen steel girder |
| `ration_cache.glb` | 120 | Food: stacked orange ration cases with ivory straps |
| `scrap_ruin.glb` | 84 | Scrap: rusted metal salvage frame and loose girders |
| `coal_seam.glb` | 112 | Coal: angular charcoal outcrop with a snow crest |

Obstacle bases occupy X/Z −0.5..0.5, with Y 0..2. `ruins.gd` derives their centers and horizontal scale from `GrusBridge.obstacle_rects()`; variants cycle in bridge order. Every resource fits one cell with a ground-level origin; resource geometry is exported at 75% of the authored size so its entire projected silhouette stays within the existing 20-pixel click target at maximum zoom in the 1280×720 game viewport. Art contains no collision, physics or navigation nodes. Resource roots retain the bridge's visibility, picking and depletion behavior.

Models were authored for this repository on 2026-10-28 using [the Blender source](../../../scripts/build_winter_assets.py), exported with Blender 5.2.1 LTS. To regenerate from the repository root:

```sh
blender --background --factory-startup --python-exit-code 1 --python scripts/build_winter_assets.py
```

The exporter asserts bounds and triangle budgets, and returns a failure status on script errors. Its wreck node is named `wrecked_transport` because Godot interprets a `_vehicle` node-name suffix as a vehicle physics import hint. Commit the `.glb.import` and PNG `.import` settings; Godot recreates the ignored `.godot/` cache.

`snow_albedo.png` is an Imagegen-generated 1254×1254 RGB texture, created 2026-10-28. Prompt: seamless top-down compacted off-white snow, subtle pale blue-grey windswept ice patches, sparse shallow fissures, low-contrast broad polygonal painterly shapes, neutral diffuse lighting; no objects, directional shadows, landmarks or text. The ground material uses world triplanar projection at one tile per eight world units. No hand-painted road or terrain features imply additional gameplay geometry.
