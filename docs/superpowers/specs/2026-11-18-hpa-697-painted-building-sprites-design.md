# HPA-697 — Painted buildings in play: design specification

**Status:** Proposed, design-only draft. This PR is the **single implementation PR for HPA-697**; implementation, tests, visual evidence and review revisions go onto this branch.
**Linear:** https://linear.app/cwchanap/issue/HPA-697/painted-building-sprites-in-play
**Art prerequisite:** [HPA-1146 — regenerate Crystal Lance PNG](https://linear.app/cwchanap/issue/HPA-1146/grus-art-regenerate-painted-crystal-lance-sprite) (separate art task/PR), following completed HPA-695.
**Baseline audited:** `main` at `0f614a3305e282860a30a3714d41eaf6fa9b3edb` (2026-11-18).
**Reference:** HPA-697's attached “Mockup: buildings today against the painted sprites” (left current low-poly city, right target painted city); the existing Frostline HUD from HPA-698 stays unchanged.

## Outcome and scope

Make the actual Godot match resemble the **right half of the Linear mockup** while retaining Rust-owned gameplay and existing user interaction. Import nine final PNGs from `art-src/2026-11-09/sprites/` (eight kinds + one generic site), using `godot/assets/structures/painted/` as the runtime location. Finished buildings use these assets:

| Rust kind | PNG | Sim footprint |
| --- | --- | --- |
| ResearchCenter | research_center.png | 4×4 |
| Barracks | barracks.png | 3×3 |
| Bunkhouse | bunkhouse.png | 2×2 |
| Depot | depot.png | 2×2 |
| CoalFurnace | coal_furnace.png | 2×2 |
| GunTurret | gun_turret.png | 2×2 |
| FlameTower | flame_tower.png | 2×2 |
| CrystalLance | crystal_lance.png | 2×2 |
| Incomplete **any** kind | construction_site.png | parent building footprint |

**Wall and Gate:** retain `wall.glb` and `gate.glb` **when complete**. The shared construction-site sprite shows before completion, including for wall/gate projects. Existing unit, ruin, terrain and resource 3D views remain untouched. The fixed orthographic camera in `godot/scenes/main.tscn` is pitched **-54°**; no camera work, rotation or new renderer is needed.

The Crystal Lance PNG from HPA-695 is known to look like a cabin with a pylon. **Do not import that defective image as the final version**; merge HPA-1146's replacement first. This implementation ticket does not generate, redraw or revise art.

## Existing seams to preserve

- `crates/grus-godot/src/lib.rs::building_view_transform` centers each building at its Rust catalogue footprint midpoint and scales the view root **(width, 1, height)**. Do not touch this or any `grus-sim` code; do not duplicate a footprint table in GDScript.
- `update_building_view_states` already pushes value-gated `set_state(complete, cold)`, including first sight, construction completion and heat transitions. No new bridge calls, per-frame snapshots or presentation state machine.
- `godot/scripts/building_view.gd` owns the `Model` child, early metadata/state buffering, retry on failed model attachment, `HealthBar`, `set_health_ratio` and building-view group. Preserve their observable contracts.
- `godot/scripts/battlefield_controller.gd::_building_at` currently ray-tests GLB triangle meshes and excludes the health bar; its helpers assume `MeshInstance3D`. Extend that seam for sprites rather than replacing selection with a 20px center hit or a separate selector.
- `_select_box` currently box-selects **units only**. Preserve that behavior; do not introduce a new building multi-selection mechanic. Click selection of buildings must still work, including mixed unit/building visual overlaps.

## Rendering proposal (small and presentation-only)

1. Keep one `BuildingView` node and an active child named `Model`. Completed wall/gate instantiate their existing GLB PackedScene. Other kinds and all incomplete sites instantiate a lightweight `Node3D` containing one `Sprite3D` with a preloaded texture. A missing mapping still shows a visible, diagnosable placeholder instead of vanishing. Use a single explicit eight-kind texture map plus one site texture, not an asset registry.
2. Reuse Rust's authoritative view-root scale. For sprite quads, compensate for the root's nonuniform X/Z scaling **inside the visual wrapper**, then compute visible width from `view.scale.x` and the PNG's dimensions so the painted building's base fits its actual footprint. Align the sprite facing the fixed -54° camera and bottom-anchor its visible base at ground level. One common placement rule should handle all nine sources; add a documented, minimal per-asset offset only if automated 1280×720 screenshot comparisons demonstrate a mismatch. Never move Rust footprint centers to correct art.
3. Use `Sprite3D` with **depth test enabled**, `fixed_size=false`, non-emissive/unshaded painterly presentation, and alpha cutting (start with `ALPHA_CUT_DISCARD`) so pixels write/test depth and nearby 3D units can appear in front while farther units fall behind opaque sprite pixels. Transparent PNG corners remain holes. Do **not** use “always on top”, `no_depth_test`, a CanvasLayer, global `render_priority` overrides, or a new custom shader by default. Test both orderings before considering a narrowly scoped alternative for poor alpha edges.
4. The existing cold condition stays Rust-owned. For painted sprites, `set_state` swaps the image **only** when completion changes and tints the same `Sprite3D.modulate` a subtle icy blue when cold; warm/recovered returns to `Color.WHITE`. Incomplete sites and zero-heat-demand Wall/Gate stay warm. Do not mutate shared PNG textures or use the old translucent ice mesh/GLB overlay on painted sprites. If `ice_overlay.tres` becomes unused, remove it and its stale documentation references.
5. Seat the existing `HealthBar` just above the sprite's actual world-space rendered top, or above the GLB mesh AABB for Wall/Gate. Keep its visibility, `health_ratio()`, hide-on-full-health, damage and destruction behavior. No collider, physics body, navigation or occupancy change.

## Picking and ordering

- Preserve the existing team/fog filtering, closest-visible-hit depth selection, 20px **near-miss fallback** and unit/resource selection paths. For a Sprite3D `Model`, calculate its projected quad from the live transform; ray-plane intersect the active sprite, reject hits outside its textured rectangle and reject transparent texels by sampling the existing PNG alpha (load/cache once per texture, not per pixel in every frame). Return a comparable world-ray depth to the existing mesh-triangle path. Health bars must never be pickable.
- For completed Wall/Gate, keep the existing triangle test. For newly placed or completed painted buildings, click the **visible opaque art** instead of empty transparent margins. A near unit should remain individually selectable and visually in front; an occluded farther unit must not appear on top of a sprite or hijack click selection. Fix overlap priority only inside the existing selection/picking path if the automated overlap tests prove it necessary.
- Construction and completion swaps, heat tint changes, fog hide/show, restart, and zoom/pan must not leave stale pick geometry or orphan `Model` nodes.

## Test seams and risk

Current Godot smokes hard-code `.glb` `scene_file_path`, `MeshInstance3D` bounds, `ICE_OVERLAY` and mesh cold-state checks. Rewrite those *representation* assertions for the eight sprites and generic site without weakening gameplay assertions. Keep GLB triangle/footprint/seam checks for Wall/Gate. Primary updates are in `godot/scripts/{smoke_test,economy_smoke_test,defenses_smoke_test,smoke_helpers}.gd`; check other smoke call sites before declaring completion.

Risk is **not** Rust simulation; it is visual geometry: normalized scaling under a nonuniform parent, transparency depth ordering, sprite art silhouette picking, and a health bar that floats or overlaps the art. Validate via real Godot viewport captures at 1280×720 (normal, cold, under construction, wall/gate, unit behind/in front, zoom) against the attached ticket mockup, not screenshot mocks or offscreen texture checks alone.

## Exclusions and review boundary

No new images/SFX, no HUD redesign, no unit/wall/gate finished sprites, no animated buildings, no 2D renderer rewrite, no per-kind physics collision, no gameplay/bridge/catalogue changes, no new Godot smoke runner, no compatibility migration layer, and no new engine or reusable rendering framework. Keep the branch draft until the separate HPA-1146 art PR is merged and real E2E/render evidence is attached.

**Useful engine references:** [SpriteBase3D alpha/depth properties](https://docs.godotengine.org/en/stable/classes/class_spritebase3d.html), [3D transparency limitations](https://docs.godotengine.org/en/stable/tutorials/3d/3d_rendering_limitations.html).
