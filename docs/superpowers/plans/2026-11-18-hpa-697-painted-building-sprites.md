# HPA-697 — Painted building sprites: implementation plan

**Status:** Revised after [first review](https://github.com/cwchanap/Grus/pull/30#pullrequestreview-5968017762) and [second review](https://github.com/cwchanap/Grus/pull/30#pullrequestreview-5968625388); planning only, **no rendering implementation yet**.
**Design:** [2026-11-18-hpa-697-painted-building-sprites-design.md](../specs/2026-11-18-hpa-697-painted-building-sprites-design.md)
**Linear + mockup:** https://linear.app/cwchanap/issue/HPA-697/painted-building-sprites-in-play
**Separate art ticket:** [HPA-1146](https://linear.app/cwchanap/issue/HPA-1146/grus-art-regenerate-painted-crystal-lance-sprite) owns regeneration of the one defective Crystal Lance PNG and its **own art PR**. HPA-697 is the **one implementation PR** (this existing draft), not a planning PR followed by a second one.

## Scope / working rules

Presentation-only Godot change: eight finished painted Sprite3D buildings, shared construction-site sprite for **every unfinished** building, and completed Wall/Gate as existing GLBs. Retain Rust-owned footprint scale, `set_state(complete, cold)` projection, all gameplay and input semantics, and fixed -54° orthographic camera. No Rust/sim/bridge edits, art/SFX generation, UI redesign, collision, animation, new rendering framework, generalized picking layer or eighth smoke.

**Do not wait for HPA-1146 to write the renderer.** Import the current eight + site sources on the draft branch to avoid a temporary Bunkhouse-only/fallback pipeline; note that `crystal_lance.png` is a **provisional art input**. Before HPA-697 goes ready/merges, incorporate the approved HPA-1146 replacement and repeat the art/screenshot checks. Never redraw it in the implementation task.

## Step 1 — Import, render and sprite-aware picking together (all seven smokes green)

**Files:** `godot/assets/structures/painted/*.png` and `.import`, `godot/scripts/{building_view,battlefield_controller,smoke_helpers,smoke_test,economy_smoke_test,defenses_smoke_test,reset_test}.gd`; relevant scene only if necessary.

1. Import all nine current source PNGs with `mipmaps/generate=true`, `compress/mode=0` (lossless), `detect_3d/compress_to=0`. Audit imported textures for mipmaps, alpha, CPU-readable image and correct source identity. No registry/asset pipeline.
2. Extend `building_view.gd` with eight finished-kind preloaded textures, one shared site texture, and existing completed Wall/Gate GLBs. The `Model` wrapper compensates Rust root scale `(1/w,1,1/h)`; Sprite3D width `w`, `pixel_size=w/texture_width`; tilt −54° with billboard off; bottom at the camera-facing (+z) footprint edge and ground +0.01. Set `sprite.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF` (painted PNGs include baked contact shadow; finished Wall/Gate retain 3D shadow). Keep the existing state caching, retry, idempotent swap and unknown-kind fallback.
3. Use depth-tested, unshaded `Sprite3D` with lossless/mipmapped import, start with `ALPHA_CUT_OPAQUE_PREPASS` and check against discard on real default/zoom-out captures. Rust's cold flag changes only `modulate` on sprite kinds; warm returns to `Color.WHITE`. For completed Wall/Gate GLBs, do not add a fake sprite or material frost effect.
4. Create **one small shared per-texture alpha cache** reachable from `building_view.gd` and `battlefield_controller.gd` (e.g. static dictionary/helper on the existing building-view script; **not** a new service). Build a thresholded `BitMap.create_from_image_alpha(image, 0.1)`; share its mask for picking and its first opaque top row / horizontal center for the health bar. Avoid `Image.get_used_rect()`, which includes thousands of near-transparent halo pixels. Convert the top-center sprite-local point into **BuildingView-local** space using `global_transform.affine_inverse() * sprite.global_transform * local_top`, then add +0.15 local Y; keep the GLB mesh-bounds path for Wall/Gate.
5. **Do not defer silhouette picking to Step 2:** widen `_view_art_bounds` / `SmokeHelpers.view_art_error` and the health-bar bounds scanners to `GeometryInstance3D` for valid `MeshInstance3D` / `SpriteBase3D`. Extend the **existing** `_model_ray_hit_depth` using `SpriteBase3D.generate_triangle_mesh().intersect_ray` and the shared cached BitMap mask. Map sprite-local hit Y to top-down texture row with the appropriate Y flip, checking the same alpha threshold. Preserve the mesh triangle branch for GLBs, team/fog filtering and the existing 20px near-miss behavior.
6. Update **all representation-dependent assertions immediately**, not in a later checkpoint: `smoke_test.gd::_roof_pick_point` and `_radius_steal_pick_point` must sample opaque sprite points instead of GLB upward-facing mesh surfaces for painted kinds; retain mesh behavior for Wall/Gate. Replace the old default-scale/edge-on horizontal-ray and height-check `site→Depot` probe (currently at `smoke_test.gd:2291-2307`) with a site/finished **texture and Model identity swap + real camera-ray alpha** assertion using the actual footprint transform. The existing transparent-corner test must keep passing. In `economy_smoke_test.gd::_overlay_is`, distinguish painted Sprite3D `modulate` from the retained GLB Wall/Gate **always-warm** checks. Update old `.glb` assumptions in `reset_test`, `defenses_smoke_test` and all other affected smoke paths **without dropping gameplay assertions**.
7. Retain obsolete GLBs temporarily until Step 3 for a tidy deletion diff, but ensure the live renderer and import assertions verify **sprites** already.

**Checkpoint (CLI + MCP observation):** Fresh Godot 4.7.1 import; **all seven existing real-input Godot headless smokes run through their standard CLI/CI commands**, with success markers and exit codes. MCP inspects the observed end-to-end behavior and 1280×720 starting/site screenshots against the HPA-697 mockup on one native platform. No duplicate MCP scene-batch runner. Provisional Crystal Lance art is not final approval.

## Step 2 — Unit-versus-building overlap only (all seven smokes green)

**Files:** `godot/scripts/battlefield_controller.gd` and existing `smoke_test.gd` assertions.

1. Keep sprite alpha-aware silhouette picking from Step 1. Expose its nearest eligible building hit using a small `_building_hit_at(screen_position, team_filter) -> Dictionary` returning `{view, depth}`; `depth = INF` for a **20px fallback-only** selection, while `_building_at` stays a thin wrapper returning the view. Do not let fallback-only hits outrank an eligible unit.
2. In `_select_at` (and `_nearest_enemy_view` for consistent right-click targeting), compare depth **only for overlapping unit + opaque building hit**. Calculate unit depth from a small ground-level body reference, not the full height or exact ground origin: `unit_depth = (unit.global_position + Vector3(0, 0.15, 0) - ray_origin).dot(ray_direction)`. A building beats the unit **only** if `building_depth < INF` and `building_depth < unit_depth`. The ~0.15 Y anchor protects clicks on a Worker inside a finished Gate's shallow plinth; a tall Behemoth behind a painted Bunkhouse must not steal the roof click. Keep the existing unit-only box selection, camera controls, team/fog filtering and nonoverlapping command behaviors. No generalized picker or arbitrary unit-mesh raycasts.
3. Extend **existing** real mouse-event smoke checks for: near unit in front wins, far unit behind opaque roof loses, Worker crossing completed Gate remains selectable, transparent-corner / fallback-only building never steals a unit click, enemy/fog/right-click paths remain correct.

**Checkpoint (CLI + MCP observation):** All seven original headless smokes pass again. MCP validates a real screenshot of foreground/background unit occlusion and the input results, including a gate traversal and default/zoom-out framing where applicable.

## Step 3 — Delete unused GLBs, generator output and overlay

**Files:** `godot/assets/structures`, `scripts/build_winter_assets.py`, `godot/assets/structures/README.md` and existing tests.

1. Once eight completed kinds and the common site are confirmed to use PNGs, delete exactly **nine orphaned GLBs** (the eight finished sprite kinds + `construction_site.glb`) and their `.import` files. Keep `wall.glb`, `gate.glb` and their import/corridor checks.
2. Remove the corresponding obsolete structure exports/authoring blocks from the shared Blender generator, but retain helpers, the Wall/Gate generation, the neutral `structure_albedo.png` they still depend on, and unrelated winter prop outputs. Drop `ice_overlay.tres`, old `ICE_OVERLAY` script usage and mesh-cold checks after confirming no consumers remain. Search before deletion; update structure README and generator instructions.
3. Narrow `smoke_test.gd::_assert_structure_import` to the two retained GLBs. **Preserve** the existing exact `SMOKE_STRUCTURE_AUDIT source_bytes=editor` marker: `.github/workflows/ci.yml` greps for it. Extend the same audit for **nine runtime PNGs**, verifying readable alpha, expected texture identity, mipmaps and no compression/foreign gameplay nodes where applicable. The smoke should fail if a missing PNG is replaced by a fallback or an orphan GLB is accidentally used.

**Checkpoint (CLI/CI):** All seven existing Godot smokes, clean editor reimport, Linux exported-build E2E and hosted CI pass; assert no runtime reference to any deleted file, preserved `SMOKE_STRUCTURE_AUDIT source_bytes=editor`, and no stale GLB import audit over dead assets. MCP reviews the recorded E2E evidence and native-screen captures; no gameplay changes.

## Step 4 — Replayable real viewport evidence and release gate

**Files:** existing smoke capture points, `docs/hpa-697-painted-sprite-evidence.md`, same PR body/checklists.

1. Reuse `SmokeHelpers.capture_frame` under `GRUS_CAPTURE_DIR` (not a new custom or throwaway journey), adding a small number of captures to the **existing** real-input tests at: starting painted city; active construction-site; cold deficit; heat recovery/warm; unit crossing behind/in front at default/zoom-out; selected/damaged building. Current `smoke-start`, `framing-default` and `framing-zoomout` hooks already exist. Capture in a **real renderer**, not `--headless` dummy video.
2. Execute the **existing CLI smoke and real-renderer capture commands** on one native platform, recording their **actual** exit codes/markers. For **MCP end-to-end acceptance**, have MCP verify the completed real-input journey results and inspect the real 1280×720 PNG captures against the **right side of the HPA-697 Linear mockup** (correct sprites, UI, health bar alignment, cold state, no flat shadows, depth order); document its visual observations and any mismatches. Do not create or require an MCP-specific batch runner, extra journey or synthetic pixel-diff oracle; CI and smoke assertions are the deterministic gates. No human sign-off.
3. Merge the separate HPA-1146 **art PR**, incorporate the new Crystal Lance PNG under the existing runtime path, re-run its alpha/import and screenshot checks; no generator or gameplay change. Do **not** merge HPA-697 with the provisional cabin-like sprite.
4. Use the **hosted CI** checks as Rust fmt/Clippy/workspace tests/sim coverage/balance/export/survival benchmark gate; skip redundant manual local Rust coverage and release-balance reruns because this diff changes no Rust. Confirm CI conclusion for the **final head** and record links. Run local Godot tests/capture as needed for representation changes.
5. Include the CLI replay commands, CI evidence, MCP-inspected E2E captures and any gaps in **this same PR**. It remains draft until the objective gates below pass.

## End-to-end acceptance criteria (CLI/CI execution + MCP visual/E2E verification; no human sign-off)

- [ ] **Approved art:** HPA-1146 merged; all nine approved PNGs present under `painted/` with texture-readable alpha, mipmaps, lossless imports and no remaining usages of the replaced nine GLBs. No provisional Crystal Lance art remains.
- [ ] **Correct inventory:** All **eight** finished kinds show their proper texture; any **incomplete** kind (including Wall/Gate) shows the one site sprite; completed Wall/Gate use their GLBs. Startup, completion, cold changes and restart do not leave the old `Model` or flash unfinished art.
- [ ] **Geometry and shadows:** Existing CLI-run smoke assertions verify Rust catalogue footprint/root scale, sprite front-edge grounding and non-stretched image under the −54° camera; `view_art_error` passes. Painted `cast_shadow` is explicitly OFF (Wall/Gate GLBs retain normal shadows). A shared thresholded alpha mask drives both opaque picking and health-bar top-row placement; bar points are transformed back to BuildingView-local space and appear above the visible roofs in real captures. MCP visually verifies no cardboard shadows or floating/misaligned bars.
- [ ] **Cold + gameplay:** Real Godot economy/survival smoke passes construction progress, cold tint → warmth after furnace, research/production/placement, turret attack/HP, Wall/Gate passage, fog/pause/restart and unchanged gameplay snapshot assertions. `_overlay_is` verifies **both** painted Sprite3D cold/warm and completed GLB Wall/Gate never-cold behavior.
- [ ] **Picking + depth:** Real-input smoke selects opaque roof/pylon outside center radius and never selects empty PNG corners; migrated `_roof_pick_point` / `_radius_steal_pick_point` and site→Depot swap probes assert **actual** texture/alpha hits. A fallback-only building hit (depth `INF`) never steals a nearby unit; unit in front wins, unit behind loses to opaque roof, and a Worker crossing a finished Gate remains clickable. Unit-only box selection, Wall/Gate picking, fog-hidden enemy exclusions and right-click commands remain correct.
- [ ] **Mockup visual check:** MCP verifies the CLI-replayed end-to-end journey and inspects recorded 1280×720 real in-game frames against the ticket mockup (painted city layout, readable silhouettes, relative sizes, frost, ground contact, shadows, HP, depth, no HUD regression); reproducible `GRUS_CAPTURE_DIR` commands and screenshot artifacts are linked. Any material visual deviations are corrected and recorded.
- [ ] **Reproducible gates:** Fresh Godot import, **all seven existing Godot smoke scenes via CLI/CI**, native exported-build E2E and hosted CI including Rust checks/coverage/balance/200-unit benchmark pass on final head; preserve the `SMOKE_STRUCTURE_AUDIT source_bytes=editor` CI marker, link actual logs/results, and have MCP review the E2E/capture evidence. No separate duplicate E2E journey, new runner, or unreported failure.
- [ ] **One PR:** This same HPA-697 draft becomes the sole implementation PR; art generation stays in HPA-1146's own PR. No human acceptance criterion.

## Existing verification commands

From repo root, using Godot 4.7.1 and a **freshly built/copied** `grus-godot` library (see `CLAUDE.md` for macOS re-signing):

```sh
godot --headless --path godot --editor --quit-after 120
for scene in smoke_test reset_test economy_smoke_test combat_lifecycle_smoke_test wave_smoke_test defenses_smoke_test underground_smoke_test; do
  godot --headless --path godot "res://scenes/${scene}.tscn" || exit 1
done

# Replay with a real display/renderer and opt-in captured images:
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/smoke_test.tscn
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/economy_smoke_test.tscn
GRUS_CAPTURE_DIR=/tmp/grus-hpa-697 godot --path godot res://scenes/defenses_smoke_test.tscn
```

Use the existing exported-build `GRUS_E2E_BINARY` path and hosted GitHub Actions checks. MCP performs the final E2E/screenshots check over the outputs of these existing commands. Do not claim any tests or captures were run when only the plan was edited.
