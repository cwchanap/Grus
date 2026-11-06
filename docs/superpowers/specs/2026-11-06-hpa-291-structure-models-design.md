# HPA-291 — Winter Survival structure art design

**Status:** Planning draft, revised after [GitHub design review](https://github.com/cwchanap/Grus/pull/20#issuecomment-6690066100); assets, integration, and testing have not yet been implemented.
**Linear:** https://linear.app/cwchanap/issue/HPA-291/grus-survival-art-structure-models
**Parent:** [Winter Survival gameplay contract](2026-10-21-winter-survival-design.md)
**Execution:** This draft becomes the **one asset-and-presentation implementation PR** for HPA-291. Extend the same PR with GLBs, integration, tests, evidence, and review fixes.

## Goal / acceptance boundary

Replace all ten placeholder surface-building primitive meshes with coherent, readable, low-poly frozen-city assets. Show a clearly unfinished construction site until the authoritative construction state becomes complete. Extend the existing Rust-to-Godot view projection to `set_state(complete, cold)` so the model changes on actual construction completion, and signal heat-deficit frost without erasing imported palettes. Keep wall and gate tiles contiguous in both axes.

HPA-291 already owns 3D art generation **and the presentation-side hookup**. The only permitted Rust change is a small view-state projection in `crates/grus-godot/src/lib.rs`; **no `crates/grus-sim` changes**, gameplay numbers, new buildings, animation, physics, SFX, unit models, unrelated UI changes, new renderer, or generic asset framework. HPA-82 owns unit art; HPA-269 owns SFX. HPA-44 already delivered the separate UI icon family.

## Audited seams on `main` (2026-11-06)

- `crates/grus-sim/src/catalog.rs::building_spec` is authoritative for **all ten** kinds and their `width`/`height`; do not duplicate a new gameplay-size table.
- `crates/grus-godot/src/lib.rs::building_view_transform` centers each building at its footprint midpoint and **scales the view root X/Z to width/height, Y stays 1**. `attach_missing_gameplay_views` instantiates `res://scenes/building_view.tscn`. `initialize_view_metadata` stamps `building_id`, `building_kind`, and `team_id` after attachment.
- `godot/scenes/building_view.tscn` currently has a primitive `Body` and `HealthBar`. `godot/scripts/building_view.gd` styles walls, gates, and turrets with primitives, applies team colors via a mutable material, and exposes `set_cold(bool)` and `set_health_ratio(float)` to Rust.
- Rust's `update_cold_tints` already has `building.construction.complete` and calls `set_view_cold` on every view, which currently invokes `set_cold`. Extend this one existing view projection to call `set_state(complete, cold)` instead; retain the Rust-owned cold rule: Team 1, **complete**, heat-demanding structure, and surface efficiency below 1. Wall, Gate, and Coal Furnace never turn cold. Do not recompute the rule in Godot.
- `GrusBridge.building_snapshot(id)` already exposes `complete` and `construction_progress` for the **selected-building HUD**; keep that controller read. **Do not add per-view snapshot polling.** The bridge's existing per-building push pass already has the completion flag and supplies it with `cold` to the view. GDScript never needs a new `GrusBridge` dependency.
- `scripts/build_winter_assets.py` already contains `prop_material`, `tint`, `box`, `rock`, palette constants and an `export()` helper asserting bounds/grounding/triangle count. Extend **this script** for the new models instead of forking it. `godot/assets/winter/` supplies the existing `.glb.import` recipe and `winter_glb_import.gd` for vertex-color albedo; `godot/assets/ui/icons/buildings/` supplies matching 2D art. Reuse all three.
- `godot/scripts/economy_smoke_test.gd::_tint_is` currently assumes `Body is MeshInstance3D` and checks its flat albedo; integration must revise that **without weakening** warm/cold/recovery assertions. `smoke_test.gd` already has `_art_bounds` and `_foreign_nodes` helpers for imported geometry.

## Asset inventory and footprint contract

**Exported** GLBs remain centered in Godot-local X/Z **-0.5..+0.5** with a grounded Y=0 origin. **Author** each model at its actual world-space footprint in Blender: e.g. a 4×4 Research Center spans Blender X/Y approximately -2..+2, and a round chimney remains round while editing. Extend the existing Blender `export()` with a square `footprint` parameter and divide Blender's horizontal X/Y coordinates by that footprint **before** the existing normalized-bound assertions/export. Blender Z (Godot Y) is **not scaled**. The bridge's existing root `(width, 1, height)` scale reverses that horizontal normalization at runtime. All ten current footprints are square; do not add aspect-ratio machinery. **No exported geometry overhangs authoritative blocked cells**; a thin plinth may meet tile edges, while chimneys/rooftops stay inside. Check actual in-game silhouettes at 1280×720, since export-normalized previews are not the same as the final root-scaled view.

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
- **Authoring source is required.** Extend `scripts/build_winter_assets.py` with the structure section and small `export(name, footprint=..., out_dir=..., max_height=...)` parameters. Keep existing HPA-290 outputs/behavior unchanged. For structures: author at world size, normalize Blender horizontal coordinates on export, assert ±0.5 bounds, grounded Y, height allowance and <4k triangles, and export all eleven GLBs. Reuse one documented command `blender --background --factory-startup --python-exit-code 1 --python scripts/build_winter_assets.py`; record provenance, shared diffuse texture, authoring versions and regeneration steps in `godot/assets/structures/README.md`. Do not duplicate a second Blender helper module.
- Compare in-game appearance with the existing building PNG icons, but do not modify HPA-44 artwork as part of this task.

## Presentation integration: one BuildingView and a narrow bridge projection

1. Keep `BuildingView` root, group/metadata, Rust-owned transform, `HealthBar`, and `set_health_ratio`. Follow the already-working `godot/scripts/resource_view.gd` pattern: a `Model` child and visible warning placeholder for an unmapped kind. Compute the asset path as `res://assets/structures/%s.glb` using `kind.to_snake_case()`; ordinary Godot `load()` already uses `ResourceLoader` caching and this path is selected once, so **no added resource-cache dictionary or computed `preload()`**.
2. Change the bridge's existing cold-view projection from `set_view_cold(handle, cold)` / `set_cold(cold)` to `set_view_state(handle, complete, cold)` / Godot `set_state(complete: bool, cold: bool)`. The pass already has both values and sends them each Update; **GDScript caches and value-gates** the last completion/cold values. On metadata arrival, a tiny `_process` leg styles the initial `Model` then calls `set_process(false)` as in `resource_view.gd`. Before completion, display the normalized `construction_site.glb`; on the first `complete=true` push, replace it with the matching final GLB **exactly once**, preserving root identity/footprint and overlay state. Never poll `GrusBridge` from a building view or invent simulation timers. The selected controller HUD still reads real `building_snapshot` progress.
3. Drop the now-unused solid Team-1/Team-2 building-color override; the survival fixture does not spawn Team-2 buildings. Imported mesh materials, vertex colors and the shared diffuse texture show their authored colors unmodified in warm mode. For cold mode, assign **one shared translucent, pale ice `StandardMaterial3D`** to each active model `MeshInstance3D.material_overlay`; for warm/recovery assign `null`. The ice material can be a simple `.tres` resource reused across views. A translucent whitening overlay avoids the darkening-only limitation of multiplying vertex-colored albedos; **never duplicate or mutate imported model materials**. Cache cold state until a model is attached, apply it on swap, and do not overlay the `HealthBar`.
4. Retain `set_health_ratio` and health-bar visibility/scale. Position the bar above the active `Model` AABB (not a hand-coded height per kind). Health, identity, fog, damage, destruction, restart, and commands remain Rust-owned. Imported assets add no collision. Picking remains presentation-only.
5. **Implement building silhouette picking in this ticket**, not conditionally: existing `_nearest_view` uses a fixed 20px radius around a building's ground center, making a 4×4 roof unclickable at high zoom. Reuse `battlefield_controller.gd::_projected_art_rect` and `_view_art_bounds` for buildings, selecting the closest eligible center on overlap and retaining the current team/fog filtering. Scope the bounds to the **active `Model` child**, so `HealthBar` does not inflate the target rectangle. Keep the existing center-radius fallback for newly attached/unstyled views and do not change unit/resource picking semantics.

## Verification / sign-off

- **Inventory/import:** all 11 GLBs load from a clean Godot import, texture dependencies resolve, no foreign gameplay nodes, geometry is grounded and within the normalized footprint, nonempty meshes, correct names. Spot-check import from a fresh `.godot/` cache.
- **Live footprint:** sampled view AABBs in world coordinates fit the catalogue-derived X/Z bounds (4×4, 3×3, 2×2, 1×1). A roof overhang never implies a walkable cell is blocked or vice versa. Straight horizontal and vertical Wall–Gate–Wall previews have no obvious seams.
- **State:** new Bunkhouse and wall/gate/turret show scaffolding before completion and the correct GLB after completion from Rust's pushed state. Early state pushes survive delayed metadata and model swaps. Cold-deficit overlays appear on completed heat-demanding structures and clear on furnace recovery; zero-demand buildings stay uniced. Multiple instances share **only the immutable ice material resource**, not mutated model materials.
- **Gameplay:** preserve unit/building selection, construction resume, damage bars, fog, paused/restarted sessions, turret combat and input. Replace old `Body` tint assumptions with assertions that every **active-model** mesh has the shared ice `material_overlay` in cold mode and `null` in warm/recovery; keep the existing authoritative heat/phase assertions. Verify clicking the Research Center roof and other visible building silhouettes at maximum zoom, while ignoring health-bar-only pixels.
- Extend **existing** `smoke_test.gd`, `economy_smoke_test.gd` and `defenses_smoke_test.gd`; no eighth smoke runner. **Keep existing smokes green at each checkpoint:** at the pilot stage assert only Bunkhouse + construction-site imports, while preserving placeholder fallbacks for other kinds. Once all eleven assets land, add a catalogue-derived loop over `catalogue_snapshot().buildings` plus the construction-site path, checking import/normalization/foreign nodes for every kind. Avoid an early all-eleven hard failure that masks other smoke regressions during the pilot.
- Run fresh editor import, seven headless Godot smokes, and existing CI (Rust fmt/Clippy/tests/coverage, Linux export/E2E and wave benchmark). Do not invent passed evidence. Inspect real 1280×720 gameplay screenshots of a normal city, cold deficit, construction in progress, and wall/gate runs; record actual evidence in `docs/` and update `README.md`.

## Deliberate exclusions

No unit-model work (HPA-82), sound effects (HPA-269), new underground 3D rooms, changing the HPA-44 icon family, simulation or catalogue edits, procedural generation at runtime, LOD/streaming system, weather particles, shader-framework rewrite, new camera mode, or animations.
