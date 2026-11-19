# HPA-697 — Painted building sprites in play: design specification

**Status:** Revised after [PR #30 design review](https://github.com/cwchanap/Grus/pull/30#pullrequestreview-5968017762); **design-only draft**, not implemented.
**Linear / mockup:** https://linear.app/cwchanap/issue/HPA-697/painted-building-sprites-in-play (the *right* half of the attached before/after image is the presentation target).
**Art dependency:** [HPA-1146](https://linear.app/cwchanap/issue/HPA-1146/grus-art-regenerate-painted-crystal-lance-sprite), a separate art-only PR, must land **before final acceptance/merge**, not before coding starts. Import the existing Crystal Lance PNG provisionally on this branch and replace its bytes from HPA-1146 before sign-off.
**Baseline:** `main` at `0f614a3305e282860a30a3714d41eaf6fa9b3edb`, 2026-11-18. [Winter survival / structure design](2026-11-06-hpa-291-structure-models-design.md) and completed Frostline HUD are already implemented.
**Execution:** Extend this **same HPA-697 PR** through implementation, automated tests, visual evidence and review fixes.

## Goal and content boundary

Render eight finished structures using the already-authored painted PNGs; any unfinished structure (including Wall/Gate) displays one shared construction-site sprite. Finished Wall/Gate keep their GLBs for the 1×1 seam/corridor contract. No sim/bridge/footprint/AI changes, no new art or SFX generation, no HUD rebuild, no new renderer, no eighth smoke.

| Completed kind | Runtime PNG | Rust footprint |
| --- | --- | --- |
| ResearchCenter | `painted/research_center.png` | 4×4 |
| Barracks | `painted/barracks.png` | 3×3 |
| Bunkhouse | `painted/bunkhouse.png` | 2×2 |
| Depot | `painted/depot.png` | 2×2 |
| CoalFurnace | `painted/coal_furnace.png` | 2×2 |
| GunTurret | `painted/gun_turret.png` | 2×2 |
| FlameTower | `painted/flame_tower.png` | 2×2 |
| CrystalLance | `painted/crystal_lance.png` | 2×2 |
| **Any incomplete kind** | `painted/construction_site.png` | its own authoritative footprint |
| **Wall/Gate complete** | existing `wall.glb` / `gate.glb` | 1×1 |

Runtime textures live in `res://assets/structures/painted/`, copied from `art-src/2026-11-09/sprites/`. Retain approved raw source images there. The Crystal Lance supplied in HPA-695 is known to resemble a cabin; HPA-1146 owns regeneration of **that one source image**; this PR only consumes it. The coal-furnace painted image is already acceptable.

## Architecture: reuse existing contracts

- Rust's `building_view_transform` owns position at **footprint center** and root scale **(width, 1, height)**. Godot's fixed orthographic `Camera3D` in `main.tscn` is pitched **−54°** toward north (−z), with no rotation mechanics. The Rust bridge's value-gated `set_state(complete, cold)` and metadata order remain unchanged.
- `godot/scripts/building_view.gd` retains one `Model` child, early state caching, one completion swap, attachment retries, the existing `HealthBar` and `set_health_ratio`, and the `building_views` group/metadata. Use an explicit **eight-texture preload map**, one site texture and the existing Wall/Gate GLB PackedScenes; keep the visible unknown-kind fallback.
- For a painted kind, construct a `Model` wrapper with inverse horizontal scaling `(1/width, 1, 1/height)` to cancel the Rust root's nonuniform X/Z scales; the effective sprite must stay square and **width world units wide**, with `pixel_size = width / texture_width` (all nine sources square). Do not hardcode footprint sizes per kind. For the two mesh kinds keep their normal GLB model transform.
- **Pin the view-ray depth, not merely the on-screen position:** rotate the Sprite3D **−54° about X**, with billboard disabled; place its **bottom image edge at the camera-side (+z) edge** of the authoritative footprint, with bottom y approximately ground +0.01 to avoid z-fighting. With quad world width `w`, square image and a centered sprite, the quad center in post-compensation/world-relative coordinates is approximately `y = 0.5 × w × cos(54°) + 0.01` and `z = (footprint_depth / 2) − 0.5 × w × sin(54°)`. The image plane runs toward −z as it rises. Evaluate image alpha padding/ground contact in screenshots; do not compensate by changing the Rust transform.
- The last rule is a **starting geometry contract**, not a proof that every unit/art outline is never occluded incorrectly. Verify at default and zoom-out camera sizes, including Worker and tall aliens behind/near buildings. Keep actual depth testing; no CanvasLayer, `no_depth_test`, always-on-top or shader framework.

## Texture import and shading

Commit corresponding PNG `.import` files with `mipmaps/generate=true`, `compress/mode=0` (Lossless), `detect_3d/compress_to=0` (prevent reimport flipping back to VRAM compression). Runtime audit verifies texture mipmaps, alpha and CPU-readable image, plus that the texture matches the imported source. Default `Sprite3D.texture_filter=LINEAR_WITH_MIPMAPS` and `fixed_size=false`. Use `shaded=false` to preserve painted colors.

Start with `ALPHA_CUT_OPAQUE_PREPASS` to retain translucent contact shadow/soft edges while giving opaque pixels depth; compare against `ALPHA_CUT_DISCARD` on the real Compatibility renderer if necessary. Preserve contact shadow and thin crystal/smokestack at the default 54-size camera and zoom-out. Don't claim the alpha-mode choice is verified until captured pixels demonstrate correct silhouettes, near/far occlusion, and absence of obvious hard fringes/shimmer. Godot 4.7 API refs: [SpriteBase3D](https://docs.godotengine.org/en/4.7/classes/class_spritebase3d.html), [BitMap](https://docs.godotengine.org/en/4.7/classes/class_bitmap.html), [texture imports](https://docs.godotengine.org/en/4.7/tutorials/assets_pipeline/importing_images.html).

Rust already restricts cold to complete Team-1 heat-consuming buildings; Wall, Gate and CoalFurnace never receive cold. Reuse `Sprite3D.modulate` for a subtle reversible icy tint; warm returns to `Color.WHITE`. Do not mutate PNG resources. No GLB icy overlay remains needed; delete `ice_overlay.tres` and its obsolete mesh application path after removing consumers. The `HealthBar` seats **above the top center of the alpha-visible painted artwork** (e.g. cached `Image.get_used_rect()` / alpha mask bounds), with a slight camera-facing offset, not at the quad's geometrical center. Wall/Gate keep mesh AABB-based bar placement.

## Picking and visibility without a parallel subsystem

- Reuse `battlefield_controller.gd::_building_at`, `_projected_art_rect`, `_view_art_bounds` and `_model_ray_hit_depth`. For bounds and health-bar seating, widen the current `MeshInstance3D` child filters to **`GeometryInstance3D`**, checking that children are `MeshInstance3D` or `SpriteBase3D` with valid visible geometry. Both expose `get_aabb()`. Scope to `Model`, excluding the sibling `HealthBar`; preserve the cache invalidation that naturally occurs when `Model` swaps.
- For painted sprites, reuse `SpriteBase3D.generate_triangle_mesh().intersect_ray` in the existing ray path to obtain depth comparable to the GLB triangle path; transform the hit to sprite-local pixels using `get_item_rect()` and `pixel_size`, and confirm an opaque texel via one cached `BitMap.create_from_image_alpha(texture.get_image())` mask per texture (lookup `get_bitv(Vector2i)`). Respect vertical orientation and image alpha threshold; transparent corners must not count as a hit. Do not write a separate general ray-plane engine.
- Preserve the current **20px center-radius fallback only for near misses / unstyled views** and all fog/team filtering; ensure it never converts a transparent-corner hit away from the center into an opaque-art hit. Keep unit/resource selection and unit-only drag box selection unchanged.
- Pre-existing overlap bug: `_select_at` currently picks any unit within 20px **before** buildings, and `_nearest_enemy_view` compares center distances only. For overlapping sprite/near unit, make a narrow depth-aware comparison using the existing building hit depth and a conservative existing-unit position estimate so a **visibly foreground building** is not hijacked by an occluded unit. Do not introduce per-unit mesh raycasts, generalized hit-test registries or change non-overlapping selection.

## Retire dead art and preserve Wall/Gate source

After all PNG references and tests are updated, delete the **eight superseded finished GLBs + `construction_site.glb`**, their `.import` files, the corresponding obsolete structure-export blocks in `scripts/build_winter_assets.py`, and the old ice-overlay material. Retain the Wall/Gate GLBs, generator/export support required for those two, `structure_albedo.png` if still referenced, and their wall/gate import & corridor tests. Refresh `godot/assets/structures/README.md`, inventory/audit assertions and generator instructions. Search all code/tests/docs references before removing anything. Do not remove the raw painted PNGs or touch unrelated winter terrain/props.

## Tests, captures, acceptance

The existing Godot smokes drive real input and already cover startup/reset, economy & construction, cold/furnace recovery, turret combat, Wall/Gate, waves and underground. Update representation assertions **in the same implementation checkpoint that changes representation**, including `reset_test.gd`, so all seven smokes remain green at each meaningful checkpoint. Keep state/lifecycle, cold and construction checks intact; extend source/texture/import audits for sprites instead of continuing to audit dead GLBs. Reuse `SmokeHelpers.view_art_error` with `GeometryInstance3D` bounds, verifying size/grounding for sprites; test the first/last opaque rows separately only if a screenshot/automated failure shows the generic AABB assertion is insufficient.

Use existing `GRUS_CAPTURE_DIR` and `SmokeHelpers.capture_frame` inside the smokes to capture **real 1280×720 frames** at startup, site in progress, cold and restored warm, and behind/in-front unit overlap at default/zoomed framing. Run the existing native end-to-end smoke flow **through MCP** (launch/inspect/log/assert/screenshots), no separate duplicate scripted user journey. The ticket's **right-hand mockup** is a visual comparison artifact: MCP inspects and records visible similarities/deviations; objective pass/fail remains tied to automated texture, geometry, tint, selection, depth, input and lifecycle assertions. No human acceptance criterion.

Merging requires: all nine correct (HPA-1146 updated) runtime PNGs, eight correct finished kinds, shared site even for incomplete Wall/Gate, finished Wall/Gate GLBs and corridor tests, opaque-pixel clicking, depth ordering, correct cold/HP display, all seven smoke journeys, exported-build e2e and hosted CI green, and recorded screenshot evidence. One native platform suffices. Keep this PR draft while these are outstanding.
