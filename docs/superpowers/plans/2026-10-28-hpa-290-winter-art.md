# HPA-290 Winter terrain and ruined-city art implementation plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task by task.

**Goal:** Replace ground, obstacle boxes and resource cylinders with readable winter-city art.

**Architecture:** Keep the existing ground mesh and bridge-derived ruin placements. Import six low-poly GLBs; scale three normalized obstacle variants only along the ground plane. Resource roots, metadata, fog and picking remain owned by the existing presentation path.

**Tech stack:** Godot 4.7.1, Blender 5.2.1 glTF exporter, generated snow albedo PNG.

**Spec:** [HPA-290](https://linear.app/cwchanap/issue/HPA-290/grus-survival-art-winter-terrain-and-ruined-city-map) and [winter-survival design](../specs/2026-10-21-winter-survival-design.md).

## Global constraints

- Presentation only; no Rust or simulation changes.
- Preserve the 128×96 battlefield, bridge-derived obstacle rectangles and existing resource IDs.
- Commit Godot-importable glTF assets and textures under `godot/`.
- Fixed-angle camera readability: off-white snow, blue-grey concrete, rust steel, orange ration cases and charcoal coal.
- No additional collision, navigation, gameplay entities or dependencies.

## Review focus

- Imported geometry must not overhang navigable cells or obscure adjacent targets.
- Resource roots remain the fog/picking boundary; depletion must remove their art.
- Shared GLBs must not acquire per-instance material mutations.
- Fresh Godot import must resolve every committed dependency.
- Existing UI-driven gathering, reset, combat and wave smoke paths must remain green.

## Task 1: Assets and integration

**Files:** `godot/assets/winter/`, `scripts/build_winter_assets.py`, `godot/scenes/battlefield.tscn`, `godot/scenes/resource_view.tscn`, `godot/scripts/{ruins,resource_view,smoke_test}.gd`, `README.md`.

**Interfaces:** obstacle GLBs span at most X/Z ±0.5 and Y 0..2; resource GLBs span X/Z ±0.5 and are grounded at Y=0. Root transforms and resource metadata remain unchanged.

- [x] Extend the existing smoke: imported art, bridge-derived ruin centers and geometry bounds, matching Food/Scrap/Coal models, textured ground. Observe its failure on placeholders.
- [x] Export collapsed building, wrecked vehicle, frozen debris, ration cache, scrap ruin and coal seam models. Include generation sources and provenance.
- [x] Swap in the models and snow material through the existing presentation scripts/scenes.
- [x] Import from a clean Godot cache; rerun all five gameplay smokes and the workspace tests.
- [x] Inspect the imported art and a live battlefield render; record evidence and limits.

## Execution notes

- Reuse the existing clean Orca worktree on `cwchanap/hpa-290-winter-terrain` at `ba5429b`.
- Use self-contained flat-shaded materials for model readability, rather than a new texture atlas or runtime material customization.
- Snow texture generated with Imagegen; models authored locally with Blender. Runtime uses committed assets; regeneration is optional.
- Independent review found resource silhouettes beyond the click radius at maximum zoom. Export resources at 75% size; the new projected-vertex picking regression failed before and passed after the correction.
- Results and review limits: [verification evidence](../../hpa-290-winter-art-evidence.md).
