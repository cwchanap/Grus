# HPA-697 — Painted building sprites in play: implementation plan

**Status:** Planned, not implemented. This planning draft is the **single PR for HPA-697**; add all code/tests/evidence to it, not to a second implementation PR.
**Design:** [2026-11-18-hpa-697-painted-building-sprites-design.md](../specs/2026-11-18-hpa-697-painted-building-sprites-design.md)
**Ticket/mockup:** https://linear.app/cwchanap/issue/HPA-697/painted-building-sprites-in-play
**Separate art prerequisite:** [HPA-1146](https://linear.app/cwchanap/issue/HPA-1146/grus-art-regenerate-painted-crystal-lance-sprite) regenerates only `art-src/2026-11-09/sprites/crystal_lance.png` in its own art PR. HPA-695's original art batch and HPA-698's Frostline HUD are already merged.

## Delivery strategy

Keep work localized to Godot presentation and the **existing** smoke suites. Do not alter the Rust sim, bridge shape, building footprints, map, UI, camera or command semantics. Preserve one HPA-697 PR throughout planning, implementation, and code review. Until HPA-1146 merges, pilot the renderer on existing valid PNGs, but do not mark the full eight-kind import as complete.

### 1. Import/placement pilot — Bunkhouse and site

**Files:** `godot/assets/structures/painted/*.png[.import]`, `godot/scripts/building_view.gd`, possibly `godot/scenes/building_view.tscn`.

- After the separate art work lands, copy all nine approved sources into `godot/assets/structures/painted/`; retain the HPA-695 raw sources. Use straightforward Godot PNG import settings and a clean editor import. Before that prerequisite, pilot only Bunkhouse and the construction site from existing assets; keep a fallback for as-yet-unimported kinds.
- Build a `Model` wrapper with one `Sprite3D`. Derive world width from root's Rust-provided footprint scale and asset pixel dimensions; cancel the root's nonuniform scale within the wrapper before applying a camera-facing quad transform, and bottom-anchor the drawn foundation to y=0. Use the fixed camera's -54° angle. No second footprint catalogue or 3D model generation.
- Select the lowest-complexity Sprite3D alpha mode that proves depth behavior with actual 3D unit meshes; start with `ALPHA_CUT_DISCARD` / normal depth testing, and avoid a custom shader. Inspect screenshot pixels for bad hard fringes before considering alpha prepass/hash.
- Verify site → Bunkhouse completion, immediate pre-completed spawn, delayed `building_kind` metadata, retry-after-failed attachment and idempotent state pushes; preserve `Model` identity unless completion actually changes.

**Checkpoint (MCP):** Launch the Godot scene, read the visible Bunkhouse/site via their model textures, and capture a 1280×720 screenshot. Assert the foundation is aligned to the same simulated footprint across the swap and the artwork is not squashed/slanted by the root X/Z scale.

### 2. Complete the inventory and keep Wall/Gate

**Files:** `godot/scripts/building_view.gd`, imported PNGs and their `.import` files.

- Add all eight final-kind preload mappings from ticket inventory and one shared site texture. **Wall/Gate use their existing GLB PackedScenes after completion**, but their incomplete views use the shared site sprite. Keep a visible placeholder warning for missing mappings.
- For Sprite3D, handle Rust-pushed cold state using a subtle blue modulate; restore warm `Color.WHITE` on recovery without re-instantiation. Never apply the old GLB ice overlay material to the painted art. Keep Wall/Gate unchanged; remove unused `ice_overlay.tres` and obsolete README references if no longer consumed.
- Reposition existing `HealthBar` using the active sprite's true world-space height, retaining the GLB AABB path for Wall/Gate. Retain HP/selection metadata, visibility, completed-vs-site state, fog and restart.
- Add a README note for runtime sprite sources and reproducible import. No alternate pipeline or new art generation in this task.

**Checkpoint (MCP):** Instantiate every kind from `catalogue_snapshot().buildings` as complete and incomplete views. Assert eight matching final PNGs, Wall/Gate matching GLBs, and one shared site PNG; screenshot representative 4×4, 3×3, 2×2 and 1×1 buildings at ordinary and close zoom, including cold/warm and health-damaged views.

### 3. Preserve silhouette picking and depth order

**Files:** `godot/scripts/battlefield_controller.gd` and only small necessary helper/test changes.

- In `_building_at`, branch by live active `Model`: existing mesh-triangle hit for Wall/Gate; new sprite-quad ray hit (projected bounds, closest world-ray depth and visible texel alpha) for painted models. Exclude health bars and empty PNG corners. Cache per-texture alpha data rather than reading pixels every frame. Retain 20px near-miss fallback, team/fog filtering, the existing unit picker and right-click targeting.
- Keep `_select_box` **units-only** as it is today; preserve additive selection semantics and all command paths. Only adjust unit/building overlap priority if an MCP repro shows an occluded unit is wrongly selected.
- Use actual Godot 3D depth rather than z-order hacks. Verify a human/alien unit moving **in front of and behind** the Research Center/Bunkhouse at two camera zoom levels; transparent margins must not hide units. Check that selected buildings remain clickable on roof/cabin/pylon opaque art.

**Checkpoint (MCP):** Through real mouse input, select a completed roof away from its 20px center, click a transparent corner without selecting the building, drag-box select workers, right-click the correct target, hide an enemy under fog, and confirm that hidden views are unclickable.

### 4. Update affected existing smokes without weakening behavior

**Files:** `godot/scripts/smoke_helpers.gd`, `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/defenses_smoke_test.gd` (plus any other direct representation assertion found by repository search).

- Convert completed/source path, generic-site, active-model bounds and cold overlay assumptions for painted kinds to Sprite3D texture, nonzero-alpha/quad-grounding and `modulate` assertions. Keep Wall/Gate GLB geometry, seam and corridor assertions.
- Retain early state push, retry, no unnecessary model re-swaps, health, cold deficit/recovery, production, selection, turret fire/tracer, gate passage, command rejection, wave, underground and restart outcomes. Do not replace E2E input legs with mocks or remove gameplay assertions to make the suite green.
- In `SmokeHelpers.view_art_error`, verify Sprite3D alpha texture and that its **visible base** aligns to the Rust-provided footprint; retain mesh triangle footprint validation for Wall/Gate. Avoid applying GLB-world-AABB checks to upright screen-facing quads as if they were colliders.
- Extend a current smoke to test sprite alpha picking and near/far unit occlusion; **no new eighth smoke executable**. Update smoke success strings where their “GLB” claim is no longer true.

**Checkpoint:** A clean Godot import and all seven existing smokes pass. Run them via the pinned Godot 4.7.1 runtime with `--headless --path godot`.

### 5. Real E2E, screenshot comparison, and repository gates

- MCP-drive **one native desktop platform** through real start → building selection → placement → construction site → complete sprite → cold → furnace recovery → attack/damage → restart. Confirm visible result using actual app state and screenshots, not only Rust-state snapshots. Include Wall/Gate/units and camera zoom/pan in the capture set. One platform is sufficient since this change is not platform-specific.
- Capture actual **1280×720** in-game screenshots for the starting city, in-progress construction, cold and warm completed buildings, units crossing in front/behind, and selected/damaged structures. Compare composition, relative size, ground contact, silhouettes and depth layering with HPA-697's attached two-panel **Linear mockup (right half)**; publish the comparison/evidence under `docs/superpowers/evidence/` or as PR evidence. Do **not** use manual/human approval as an acceptance gate.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`, `cargo test --workspace --locked`, `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1` and the existing simulation coverage/balance checks. Ensure exported-debug Godot E2E and current CI/200-unit benchmark remain green. This is a presentation change; don't tune gameplay to resolve a visual mismatch.
- Attach runnable commands, actual CI links, screenshot evidence and known deviations to the **same** PR; do not claim tests were executed in this planning draft.

## End-to-end acceptance criteria (all automated)

- [ ] **Asset gate:** HPA-1146 art PR merged; all nine runtime PNGs from approved art sources import cleanly with alpha and there is no missing-texture error in editor or packaged build.
- [ ] **Inventory and lifecycle:** MCP/Godot E2E shows each of the eight finished kinds as the exact expected painted sprite, every unfinished kind (including Wall/Gate) as the generic site, and completed Wall/Gate as the existing GLBs. No intermediate unstyled flash on initial completed spawn; completion swaps once, repeated states do not re-swap.
- [ ] **Visual mockup:** MCP captures at 1280×720 are compared to HPA-697's attached mockup, verifying recognizable city silhouettes, visually correct per-footprint scale, base anchoring, normal/cold treatments and no HUD regression. Screenshots and comparisons are attached to the PR.
- [ ] **Occlusion and input:** MCP-driven real clicks hit opaque roof/pylon pixels at max zoom and ignore transparent margins; near/far moving 3D units render on their correct sides of the sprite, remain targetable as appropriate, box selection of workers and Wall/Gate targeting remain unchanged. Fog-hidden buildings are never selected.
- [ ] **Cold/health/session:** Real end-to-end cold deficit → blue-tinted building → heat restoration removes tint; HP bar stays above displayed art on damage; fog, pause, destruction, restart and selection still work.
- [ ] **Regression gates:** Clean Godot 4.7.1 import, **all seven** existing Godot headless smokes, Rust fmt/Clippy/tests/coverage/balance, one platform's exported-build MCP E2E, and CI/benchmark checks pass; actual exit codes and CI links are recorded. No Rust sim/bridge changes or eighth smoke script.
- [ ] **PR scope:** Exactly one HPA-697 PR containing the design, code, tests and evidence; art generated exclusively in HPA-1146's separate PR. No human acceptance step and no unimplemented claims of validation.

## Commands (after implementing)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings
cargo test --workspace --locked
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1

# Build/stage grus-godot dynamic library, then import in a clean editor session:
godot --headless --path godot --editor --quit-after 120
for scene in smoke_test reset_test economy_smoke_test combat_lifecycle_smoke_test wave_smoke_test defenses_smoke_test underground_smoke_test; do
  godot --headless --path godot "res://scenes/${scene}.tscn" || exit 1
done

cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
cargo test -p grus-sim --release --locked --test survival_balance -- --ignored --nocapture
```

The staged dynamic library must match the latest Rust build; on macOS, re-sign the copied dylib as documented in `CLAUDE.md`. The exported-build E2E additionally follows the repo's `GRUS_E2E_BINARY` process.
