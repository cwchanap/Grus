# HPA-291 — Winter Survival structure art design

**Status:** Planning draft only; assets, integration, and testing have not yet been implemented.
**Linear:** https://linear.app/cwchanap/issue/HPA-291/grus-survival-art-structure-models
**Parent:** [Winter Survival gameplay contract](2026-10-21-winter-survival-design.md)
**Execution:** This draft becomes the **one asset-and-presentation implementation PR** for HPA-291. Extend the same PR with GLBs, integration, tests, evidence, and review fixes.

## Goal / acceptance boundary

Replace all ten placeholder surface-building primitive meshes with coherent, readable, low-poly frozen-city assets. Show a clearly unfinished construction site until the authoritative construction state becomes complete. Use existing Rust-pushed `set_cold` to signal heat-deficit frost without erasing the imported models' distinct palettes. Keep wall and gate tiles contiguous in both axes.

HPA-291 already owns 3D art generation **and the presentation-side hookup**. No Rust simulation or GDExtension changes, new buildings, gameplay numbers, animation, physics, SFX, unit models, unrelated UI changes, new renderer, or generic asset framework. HPA-82 owns unit art; HPA-269 owns SFX. HPA-44 already delivered the separate UI icon family.

## Audited seams on `main` (2026-11-06)

- `crates/grus-sim/src/catalog.rs::building_spec` is authoritative for **all ten** kinds and their `width`/`height`; do not duplicate a new gameplay-size table.
- `crates/grus-godot/src/lib.rs::building_view_transform` centers each building at its footprint midpoint and **scales the view root X/Z to width/height, Y stays 1**. `attach_missing_gameplay_views` instantiates `res://scenes/building_view.tscn`. `initialize_view_metadata` stamps `building_id`, `building_kind`, and `team_id` after attachment.
- `godot/scenes/building_view.tscn` currently has a primitive `Body` and `HealthBar`. `godot/scripts/building_view.gd` styles walls, gates, and turrets with primitives, applies team colors via a mutable material, and exposes `set_cold(bool)` and `set_health_ratio(float)` to Rust.
- Rust's `update_cold_tints` already calls `set_cold` on every view (with value-gated writes). The Rust-owned rule is Team 1, **complete**, heat-demanding structure, and surface efficiency below 1; Wall, Gate, and Coal Furnace do not turn cold. Do not recompute this rule in Godot.
- `GrusBridge.building_snapshot(id)` already exposes `complete` and `construction_progress`, but Rust does not push construction-state calls to the view. Existing `battlefield_controller.gd` uses that snapshot for selected-building HUD text. A presentation-only view can read it **while unfinished**, then stop polling upon completion; no new bridge API.
- `godot/assets/winter/` already contains imported, Blender-authored GLBs with an established `.glb.import` recipe and `winter_glb_import.gd` for vertex-color albedo. `godot/assets/ui/icons/buildings/` contains the recently completed matching 2D artwork. Reuse the winter palette and import seam; no new asset pipeline.
- `godot/scripts/economy_smoke_test.gd::_tint_is` currently assumes `Body is MeshInstance3D` and checks its flat albedo; integration must revise that **without weakening** warm/cold/recovery assertions. `smoke_test.gd` already has `_art_bounds` and `_foreign_nodes` helpers for imported geometry.

## Asset inventory and footprint contract

Every imported model is a centered scene in local X/Z **-0.5..+0.5** with a grounded Y=0 origin. **No model geometry overhangs the authoritative blocked footprint when the existing Rust root X/Z scale is applied.** A thin plinth/base may reach the exact tile edges; chimneys and rooftop features stay inside them. Keep models recognizably different at 1280×720 RTS camera zoom.

| Rust `BuildingKind` | Asset stem (`.glb`) | Footprint | Key silhouette / material |
| --- | --- | --- | --- |
| `ResearchCenter` | `research_center` | 4×4 | Largest central laboratory, cyan antenna/research mast, snowbound roof |
| `Bunkhouse` | `bunkhouse` | 2×2 | Low insulated cabins, warm small windows |
| `Depot` | `depot` | 2×2 | Cargo/salvage racks, low crates and canopy |
| `Barracks` | `barracks` | 3×3 | Broad fortified hall, defensive parapet |
| `Wall` | `wall` | 1×1 | Repeating icy concrete/metal barricade with edge-to-edge contact |
| `Gate` | `gate` | 1×1 | Reinforced open passage with wall-matching edge treatment |
| `GunTurret` | `gun_turret` | 2×2 | Squat fixed gun barrel and shield |
| `CoalFurnace` | `coal_furnace` | 2×2 | Chimney and orange coal-heat vent |
| `FlameTower` | `flame_tower` | 2×2 | Fuel drums and short warm-colored nozzle |
| `CrystalLance` | `crystal_lance` | 2×2 | Tall slender cyan emitter, different from gun/flame |
| `(visual-only)` | `construction_site` | normalized 1×1 | Generic fenced foundation, visible scaffold and unfinished framing |

**Wall / Gate choice:** make these tiles symmetric / four-side compatible; their base and connecting edges meet at ±0.5 in either X or Z. They should read as connected horizontal and vertical runs, including Wall–Gate–Wall, without introducing rotation, neighbor mask, or a separate wall-network renderer. They are still one-cell authoritative objects, not a long combined mesh.

## Visual and source-file contract

- Place ten finished `.glb` files, one `construction_site.glb`, and the diffuse texture PNG(s) they reference under `godot/assets/structures/`. A small shared winter-surface albedo texture is sufficient; use restrained vertex-color material accents and an icy-blue / concrete / rust / furnace-orange / cyan palette. Keep texture paths self-contained/committed. Do not add normal/metallic atlas infrastructure by default.
- Each GLB should be a low-poly, fixed-angle readable mesh, preferably one mesh surface per model, targeting under ~4k triangles per GLB and no expensive animation/particle rigs. Static barrel/emitter silhouettes are enough; no articulated firing animation in this ticket.
- Use the existing Godot 4.7.1 import recipe: `.glb.import` files disable name/type-suffix scene-node generation and unnecessary LODs, and reuse `winter_glb_import.gd` if vertex-colored materials need the importer opt-in. Import **must not create physics bodies, colliders, navigation regions, or occluders**. Never change sim occupancy.
- Keep an optional short reproducible Blender authoring script, e.g. `scripts/build_structure_assets.py`, separate from the six existing winter-resource exports; use `--python-exit-code 1` and verify local bounding boxes before writing files. Record provenance, texture inventory, authoring versions, and generation command in `godot/assets/structures/README.md`.
- Compare in-game appearance with the existing building PNG icons, but do not modify HPA-44 artwork as part of this task.

## Presentation integration: one BuildingView, no sim changes

1. Keep the existing `BuildingView` root and its group/metadata, authoritative root transform, `HealthBar`, and Rust-callable `set_cold` / `set_health_ratio` methods. Replace `Body` primitive generation with a lightweight scene holder that loads one committed `PackedScene` by `building_kind`; use the existing snake_case naming convention rather than an added registry. Preload/cache imported resources so they are not repeatedly read each render frame.
2. Create the model only after the Rust metadata has arrived. While its authoritative `building_snapshot(id).complete` is false, show the same footprint-scaled `construction_site.glb` for any kind. Read `building_snapshot` while the view is unfinished (and once on metadata initialization), **not forever**; when complete, swap to the finished kind GLB and stop polling. Do not add timers/progress state to Rust or infer gameplay completion from visual progress. The selected HUD continues showing real construction % from the snapshot.
3. A completed model's imported surface materials must be **duplicated per view instance** before tinting. Keep textured/vertex-colored base albedo visible: normal/warm uses a neutral multiplier; `set_cold(true)` applies a visually unambiguous ice/desaturation multiplier, and `set_cold(false)` restores the original look. Cache early cold calls until the asset is attached. **Never mutate a shared imported mesh/material across identical buildings.** Existing team identity may use subtle painted trim rather than painting every building solid blue; preserve distinct Team 2 presentation if it appears in a test.
4. Retain `set_health_ratio`, health-bar visibility and scale; adjust its local height only where the new model requires clearance. Health, entity identity, selection, fog, damage, destruction, restart and building commands remain Rust-owned. No collision shapes are added to artwork; building picking remains the existing Godot screen-view path.
5. Check that the taller models remain practical to select at default and high zoom, especially the Research Center and construction sites. If visual silhouettes expose an actual pick regression, fix it minimally in existing Godot view/controller presentation and test it, rather than rewriting world picking or gameplay occupancy.

## Verification / sign-off

- **Inventory/import:** all 11 GLBs load from a clean Godot import, texture dependencies resolve, no foreign gameplay nodes, geometry is grounded and within the normalized footprint, nonempty meshes, correct names. Spot-check import from a fresh `.godot/` cache.
- **Live footprint:** sampled view AABBs in world coordinates fit the catalogue-derived X/Z bounds (4×4, 3×3, 2×2, 1×1). A roof overhang never implies a walkable cell is blocked or vice versa. Straight horizontal and vertical Wall–Gate–Wall previews have no obvious seams.
- **State:** new Bunkhouse and wall/gate/turret show scaffolding before completion and the correct GLB after completion. Unfinished buildings do not fake a finished tint. A cold-deficit sequence tints completed heat-demanding structures, and furnace recovery restores warm base appearance; zero-demand buildings stay uniced. Multiple instances of the same kind do not share mutable tint.
- **Gameplay:** preserve unit/building selection, construction resume, damage bars, fog, paused/restarted sessions, turret combat and input. Update old `Body` test assumptions to inspect active imported meshes and their instance-local materials; keep the old state/heat assertions.
- Extend **existing** `smoke_test.gd`, `economy_smoke_test.gd` and `defenses_smoke_test.gd`; use the existing suite instead of adding an eighth smoke runner. Cover every catalogue kind at import/normalization level, even if research-gated kinds are not constructed in the short E2E flow.
- Run fresh editor import, seven headless Godot smokes, and existing CI (Rust fmt/Clippy/tests/coverage, Linux export/E2E and wave benchmark). Do not invent passed evidence. Inspect real 1280×720 gameplay screenshots of a normal city, cold deficit, construction in progress, and wall/gate runs; record actual evidence in `docs/` and update `README.md`.

## Deliberate exclusions

No unit-model work (HPA-82), sound effects (HPA-269), new underground 3D rooms, changing the HPA-44 icon family, simulation or catalogue edits, procedural generation at runtime, LOD/streaming system, weather particles, shader-framework rewrite, new camera mode, or animations.
