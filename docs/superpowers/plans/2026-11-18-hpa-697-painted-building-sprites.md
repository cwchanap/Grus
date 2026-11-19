# HPA-697 — Painted building sprites: implementation plan

**Status:** Revised after [PR #30 design review](https://github.com/cwchanap/Grus/pull/30#pullrequestreview-5968017762); planning only, **no rendering implementation yet**.
**Design:** [2026-11-18-hpa-697-painted-building-sprites-design.md](../specs/2026-11-18-hpa-697-painted-building-sprites-design.md)
**Linear + mockup:** https://linear.app/cwchanap/issue/HPA-697/painted-building-sprites-in-play
**Separate art ticket:** [HPA-1146](https://linear.app/cwchanap/issue/HPA-1146/grus-art-regenerate-painted-crystal-lance-sprite) owns regeneration of the one defective Crystal Lance PNG and its **own art PR**. HPA-697 is the **one implementation PR** (this existing draft), not a planning PR followed by a second one.

## Scope / working rules

Presentation-only Godot change: eight finished painted Sprite3D buildings, shared construction-site sprite for **every unfinished** building, and completed Wall/Gate as existing GLBs. Retain Rust-owned footprint scale, `set_state(complete, cold)` projection, all gameplay and input semantics, and fixed -54° orthographic camera. No Rust/sim/bridge edits, art/SFX generation, UI redesign, collision, animation, new rendering framework, generalized picking layer or eighth smoke.

**Do not wait for HPA-1146 to write the renderer.** Import the current eight + site sources on the draft branch to avoid a temporary Bunkhouse-only/fallback pipeline; note that `crystal_lance.png` is a **provisional art input**. Before HPA-697 goes ready/merges, incorporate the approved HPA-1146 replacement and repeat the art/screenshot checks. Never redraw it in the implementation task.

## Step 1 — Import and render all kinds, with smokes kept green

**Files:** `godot/assets/structures/painted/*.png` + `.import`, `godot/scripts/building_view.gd`, `godot/scripts/{smoke_test,economy_smoke_test,defenses_smoke_test,reset_test,smoke_helpers}.gd`, and relevant scene only if necessary.

1. Import all nine current source PNGs; set and commit `mipmaps/generate=true`, `compress/mode=0` (Lossless), `detect_3d/compress_to=0`. Assert runtime imported texture has mips, alpha and CPU-readable image. No registry, automatic discovery or extra asset-generation pipeline.
2. Extend `building_view.gd`: eight-kind typed texture preload dict plus one site texture, keeping finished Wall/Gate as GLB. `Model` wrapper compensates Rust root scale with `(1/w,1,1/h)`; sprite width `w`, `pixel_size=w/texture_width`; tilt **−54°** with billboard disabled; position its **bottom edge** at the building footprint's front/camera-side **+z** boundary and y≈0.01. No new footprint constants. Retain early-state/retry/idempotent-completion contracts and fallback for *unknown* kinds.
3. Use depth-enabled Sprite3D, `texture_filter=LINEAR_WITH_MIPMAPS`, `fixed_size=false` and unshaded painterly colors. Prefer `ALPHA_CUT_OPAQUE_PREPASS` for soft contact shadows, with screenshot comparison against discard if needed. Rust-pushed cold = reversible subtle `modulate`; reset white when warm. HealthBar seats above top *visible alpha* row, not the quad center; preserve mesh AABB seating for Wall/Gate.
4. Widen appropriate existing bounds/health/test helper child scans from only `MeshInstance3D` to `GeometryInstance3D`, safely accepting SpriteBase3D and meshes. Retain existing `SmokeHelpers.view_art_error` root scale, ground and XY/XZ footprint checks; use the live visual AABB. Update **all currently broken representation assertions in the same change**: `smoke_test` site/finish/import/overlay tests, `economy_smoke_test`, `defenses_smoke_test` and importantly `reset_test` startup/restart checks. Preserve all original gameplay and cold/heat assertions; do not substitute direct bridge calls for input legs.
5. Keep provisional GLBs **until step 3** so this step changes no unrelated generator behavior. Limit their remaining audits so a stale GLB cannot be mistaken for the new rendered sprite.

**Checkpoint (MCP + CI):** Build/stage the pinned Godot extension, fresh Godot 4.7.1 texture import, execute **all seven existing Godot smokes** through an MCP-controlled runner and assert their success markers/exit codes. A real 1280×720 startup/site screenshot shows painted base size, ground contact and no UI regression against the ticket mockup. Report the provisional Crystal Lance image honestly, not as approved art.

## Step 2 — Reuse silhouette picking with sprite alpha

**Files:** `godot/scripts/battlefield_controller.gd` plus existing smoke assertions.

1. Extend `_view_art_bounds` and `_projected_art_rect` for Sprite3D via `GeometryInstance3D.get_aabb()`, retaining `Model`-scoped bounds (no HealthBar or stale construction bounds after swap).
2. In existing `_model_ray_hit_depth`, add `SpriteBase3D.generate_triangle_mesh().intersect_ray`, convert the hit to sprite-local pixel in `get_item_rect()` / `pixel_size`, reject transparent texels with cached `BitMap.create_from_image_alpha(texture.get_image())` + `get_bitv`. Cache once per texture, not once per mouse click; keep old mesh triangle path for Wall/Gate. No hand-written ray-plane subsystem. Preserve fog/team filtering and a small 20px near-miss fallback for actually unhit/unstyled views.
3. Fix the **known overlap edge case** only: when a candidate unit within its existing click radius overlaps a building's visible hit, compare their camera-ray depth before deciding which to click or right-click target. This prevents an occluded far unit from stealing a click on an opaque painted roof. Do not ray-test arbitrary unit meshes or change non-overlapping command behavior. Keep `_select_box` units-only.
4. Use actual mouse event inputs in `smoke_test.gd` (or a shared existing smoke) to assert an opaque roof hit away from the 20px center, transparent PNG corner miss outside that radius, visible unit in front, unit behind unable to steal the roof click, proper right-click/fog and Wall/Gate picking. No eighth script.

**Checkpoint (MCP + CI):** Seven existing Godot smokes pass again; capture one real overlap frame at default and zoom-out sizes, including a tall alien if available. Check depth via visual pixels and click results, not just shadow-model centers.

## Step 3 — Delete unused GLBs, generator output and overlay

**Files:** `godot/assets/structures`, `scripts/build_winter_assets.py`, `godot/assets/structures/README.md` and existing tests.

1. Once eight completed kinds and the common site are confirmed to use PNGs, delete exactly **nine orphaned GLBs** (the eight finished sprite kinds + `construction_site.glb`) and their `.import` files. Keep `wall.glb`, `gate.glb` and their import/corridor checks.
2. Remove the corresponding obsolete structure exports/authoring blocks from the shared Blender generator, but retain helpers, the Wall/Gate generation, the neutral `structure_albedo.png` they still depend on, and unrelated winter prop outputs. Drop `ice_overlay.tres`, old `ICE_OVERLAY` script usage and mesh-cold checks after confirming no consumers remain. Search before deletion; update structure README and generator instructions.
3. Narrow `smoke_test.gd::_assert_structure_import` to the two retained GLBs. Extend the same audit for **nine runtime PNGs**, verifying readable alpha, the intended imported texture, mipmaps and no compression/foreign gameplay nodes where applicable. The smoke should fail if a missing PNG is replaced by a fallback or an orphan GLB is accidentally used.

**Checkpoint (MCP + CI):** All seven existing Godot smokes, clean editor reimport, Linux exported-build E2E and hosted CI pass; assert no runtime reference to any deleted file and no stale GLB import audit over dead assets. No gameplay changes.

## Step 4 — Replayable real viewport evidence and release gate

**Files:** existing smoke capture points, `docs/hpa-697-painted-sprite-evidence.md`, same PR body/checklists.

1. Reuse `SmokeHelpers.capture_frame` under `GRUS_CAPTURE_DIR` (not a new custom or throwaway journey), adding a small number of captures to the **existing** real-input tests at: starting painted city; active construction-site; cold deficit; heat recovery/warm; unit crossing behind/in front at default/zoom-out; selected/damaged building. Current `smoke-start`, `framing-default` and `framing-zoomout` hooks already exist. Capture in a **real renderer**, not `--headless` dummy video.
2. Have MCP **launch, replay and inspect** those same smoke journeys on **one native platform**, verify status/marker/exit code and inspect the real captured 1280×720 files against the **right side of the HPA-697 Linear mockup**. Evidence documents exact replay commands, screenshot paths, objective geometry/texture/cold/interaction checks and any visible mismatches; no subjective human approval or invented automatic pixel-diff oracle. Capture review is a **visual check by MCP**; the deterministic gates are test assertions and CI.
3. Merge the separate HPA-1146 **art PR**, incorporate the new Crystal Lance PNG under the existing runtime path, re-run its alpha/import and screenshot checks; no generator or gameplay change. Do **not** merge HPA-697 with the provisional cabin-like sprite.
4. Use the **hosted CI** checks as Rust fmt/Clippy/workspace tests/sim coverage/balance/export/survival benchmark gate; skip redundant manual local Rust coverage and release-balance reruns because this diff changes no Rust. Confirm CI conclusion for the **final head** and record links. Run local Godot tests/capture as needed for representation changes.
5. Include CI evidence, MCP E2E replay commands/captures and any gaps in **this same PR**. It remains draft until the objective gates below pass.

## End-to-end acceptance criteria (MCP-driven, no human sign-off)

- [ ] **Approved art:** HPA-1146 merged; all nine approved PNGs present under `painted/` with texture-readable alpha, mipmaps, lossless imports and no remaining usages of the replaced nine GLBs. No provisional Crystal Lance art remains.
- [ ] **Correct inventory:** All **eight** finished kinds show their proper texture; any **incomplete** kind (including Wall/Gate) shows the one site sprite; completed Wall/Gate use their GLBs. Startup, completion, cold changes and restart do not leave the old `Model` or flash unfinished art.
- [ ] **Geometry:** The MCP smoke verifies the Rust catalogue footprint/root scale, sprite bottom/front-edge grounding and proportional non-stretched image under the −54° camera; `view_art_error` passes. Buildings stay inside footprint and their health bars sit above visible tops.
- [ ] **Cold + gameplay:** Real Godot economy/survival smoke passes construction progress, cold tint → warmth after furnace, research/production/placement, turret attack/HP, Wall/Gate passage, fog/pause/restart and unchanged gameplay snapshot assertions.
- [ ] **Picking + depth:** Real-input smoke can select opaque roof/pylon outside center radius but not transparent corner, preserves unit-only box selection and Wall/Gate picking, and distinguishes units in front/behind without hidden-unit click hijack. Fog-hidden enemy isn't selectable.
- [ ] **Mockup visual check:** MCP inspects and records 1280×720 real in-game frames against the ticket mockup (painted city layout, identifiable sprites, relative sizes, frost, ground contact, depth, no HUD regression); reproducible `GRUS_CAPTURE_DIR` command and screenshot artifacts are linked. Any deviations recorded and material failures corrected.
- [ ] **Reproducible gates:** Fresh Godot import, **all seven existing Godot smoke scenes**, native exported-build E2E and hosted CI including Rust checks/coverage/balance/200-unit benchmark pass on final head; link actual logs/results. No separate duplicate E2E journey, new smoke runner, or unreported failure.
- [ ] **One PR:** This same HPA-697 draft becomes the sole implementation PR; art generation stays in HPA-1146's own PR. No human acceptance criterion.

## Existing verification commands

From repo root, using Godot 4.7.1 and a **freshly built/copied** `grus-godot` library (see `CLAUDE.md` for macOS re-signing):

```sh
godot --headless --path godot --editor --quit-after 120
for scene in smoke_test reset_test economy_smoke_test combat_lifecycle_smoke_test wave_smoke_test defenses_smoke_test underground_smoke_test; do
  godot --headless --path godot "res://scenes/\${scene}.tscn" || exit 1
done

# Replay with a real display/renderer and opt-in captured images:
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/smoke_test.tscn
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/economy_smoke_test.tscn
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/defenses_smoke_test.tscn
```

Use the existing exported-build `GRUS_E2E_BINARY` path and hosted GitHub Actions checks; do not claim these commands were run when only the plan was edited.
