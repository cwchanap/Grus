# HPA-698 — Frostline HUD implementation plan

**Linear:** [HPA-698](https://linear.app/cwchanap/issue/HPA-698/hud-redesign-frostline-look-with-the-generated-art)
**Design spec:** [Frostline HUD design](../specs/2026-11-09-hpa-698-frostline-hud-design.md)
**Art baseline:** `art-src/2026-11-09/`, merged under HPA-695 / PR #23.
**Branch:** `hpa-698-frostline-hud`
**PR policy:** **One ticket → one PR.** This planning draft becomes the HPA-698 implementation PR. Add code, tests, evidence and review revisions to this branch; do not create a follow-up implementation PR.
**State at creation:** Documentation only. None of the checkpoints below has been implemented or verified.

## Strategy

Ship the approved 1280 × 720 Frostline mockups using existing painted PNGs and the existing Godot interface/control tree. Keep the Rust simulation, catalogue ownership and player commands intact. Limit bridge additions to read-only presentation details and a selection-independent wrapper for the already-existing Analyze command.

Favor built-in Godot Theme/StyleBoxTexture/Buttons and **two focused presentation scripts** over an abstract component library. The controller retains selection, camera, keyboard, placement and command dispatch, while the HUD and underground drawer own rendering. If a helper can be a few lines in a single script, do not promote it into a new service.

## Checkpoint 1 — Fill bridge projection gaps first

**Primary files:** `crates/grus-godot/src/lib.rs`, existing bridge tests or Godot smoke assertions.

- [ ] Extend `building_snapshot(building_id)` with `training_queue`: one ordered entry per `ProductionQueue.jobs` element, using the existing production kind/label conversion. Keep existing `queue_label`, `queue_progress` (head only), `blocked_reason`, and fog-hidden/unknown empty-dictionary behavior. Missing queue component means an empty array.
- [ ] Change the bridge Analyze entrypoint to `analyze()` with no `building_id`. Find Team 1's uniquely seeded Research Center through the bridge's World/Building query, then queue the **existing** `PlayerCommand::Analyze`. Do not add another command type or modify the sim's spending/validation code. A nonexistent core returns false; a visible core with insufficient Crystals still queues and receives the normal typed Rust reject.
- [ ] Add `timer_total_seconds` and `warning_seconds` to `wave_snapshot_dict` from the existing Rust constants (`WAVE_PREPARATION_SECONDS`, `WAVE_INTERVAL_SECONDS`, `WAVE_WARNING_SECONDS`), selecting the correct interval for the initial prep and subsequent waves. Do not infer duration from an arbitrary UI timer.
- [ ] Add the late-wave escalation threshold (e.g. `wave_growth_unlock_points`) to `catalogue_snapshot().research` from `RESEARCH_WAVE_BOOST_PERCENT` and `RESEARCH_TARGET_POINTS` using the same Rust unlock-point helper. Flame Tower and Crystal Lance already expose their thresholds in `catalogue.buildings`.
- [ ] Update existing tests/call sites for the no-argument Analyze entrypoint. Assert queue FIFO and empty queue; verify non-playing/missing-core handling and no regression to fog-hidden data. No broad bridge refactor.

**Gate:** Rust fmt, clippy, workspace tests; validate new keys against live Godot bridge snapshots before writing HUD presentation.

## Checkpoint 2 — Import and skin the reusable visual vocabulary

**Primary files:** `godot/assets/ui/frostline/{icons,screens,ui}/`, `godot/assets/ui/frostline_theme.tres`, `godot/scenes/main.tscn`, accompanying font license/readme.

- [ ] Copy only the needed generated icons, screen images and UI-kit PNGs from the committed `art-src/2026-11-09/`. Keep the existing HPA-44 resource/status/building/unit/underground icons; leave `art-src/.../sprites` and existing 3D building views untouched. Check transparency, import sizing and filter settings; do not regenerate or touch new image art in this code ticket.
- [ ] Include OFL Barlow/Barlow Condensed font files and their license **if needed** for the approved typography; do not use system-installed fonts as a hidden build dependency.
- [ ] Create one Godot Theme with resizable steel panel and button styles. Build the five practical motifs with built-in Controls: panel plate, icon tile (normal/armed/locked/unaffordable), icon socket, round button and glass progress frame. Prefer one small `frostline_action_tile` helper/scene for the repeated nine build + three train tiles; avoid generalized UI infrastructure.
- [ ] Re-layout the HUD in `main.tscn` for a fixed 1280 × 720 baseline: top resources/wave/research, bottom minimap/selection/commands, overlay underground and lifecycle panels. Preserve `Main/HUD` and named command/test nodes when practical; update smoke selectors only where an actual layout change requires it. No global camera or map geometry changes.
- [ ] Check at actual displayed size that icon/text/cost and button states remain legible; input STOP/IGNORE on UI/screen layers must never prevent the real panel click or leak it to the world.

**Gate:** Godot imports the copied assets from a fresh editor cache; a static frame shows the intended layout without clipped top/bottom controls or missing textures. Do not claim the visual gate until a real frame is inspected.

## Checkpoint 3 — Extract HUD rendering while retaining one command owner

**Primary files:** `godot/scripts/battlefield_controller.gd`, new `godot/scripts/frostline_hud.gd`, `godot/scenes/main.tscn`.

- [ ] Move static catalogue-bound HUD setup and methods equivalent to `_refresh_hud`, `_refresh_research_controls`, `_refresh_selection`, and `_refresh_session_overlay` into the HUD view script. Use one existing per-frame snapshot read for economy, wave and research, and one building snapshot only when selected. Pass the same wave dictionary to the minimap. Do not duplicate selection state or move command validation into the view.
- [ ] Have the HUD raise intent signals: build kind, train kind, analyze, idle-worker jump, stop/attack-move if exposed, start/pause/resume/restart/quit. Connect each signal to the controller's **existing handlers** in `_ready`. Keep `command_status` and feedback revision handling accessible under the existing HUD path.
- [ ] Draw top resources and population as picture + value, heat supply/demand meter and cold chip; use live `surface_efficiency` (<1.0 is cold), not a second calculation of heat gameplay. Keep prices, idle count and button affordability derived from one snapshot + one static catalogue, with full-name tooltips.
- [ ] Draw the wave ring using `seconds_remaining / timer_total_seconds`, and warning treatment from `warning_seconds`; render the current/next wave, live aliens and named incoming sides. Do not disclose fog-hidden units. Keep the minimap pings bound to the same wave snapshot.
- [ ] Draw research points/target/ETA and three catalogue-derived milestone markers, with Analyze available during Playing even if a worker/other building/nothing is selected. A zero-Crystal Analyze press must still flow to Rust's `InsufficientResources` feedback; don't silently disable it by UI affordability alone.
- [ ] For selection: use painted kind icons as portraits and live view `health_ratio()`, worker task snapshots when applicable, and the Rust-provided `training_queue` with head progress and blocked reason for producers. For multiple units, render count/group kinds rather than fabricated aggregate HP/task.
- [ ] Replace text-heavy build/train buttons with icon-first tiles and costs while preserving exact kind metadata, catalogue threshold locks, worker selection, arming/placement/cancel, training, status messages and command feedback. An unaffordable style may be clickable to surface the authoritative reject; a research-locked tile is disabled with an explanatory tooltip.

**Gate:** With the Godot editor, play the existing economy/research/build/train flows from real clicks. A visual/control refactor must not require changing Rust gameplay tests.

## Checkpoint 4 — Drawer, session art and screen effects

**Primary files:** new `godot/scripts/underground_drawer.gd`, `godot/scripts/battlefield_controller.gd`, `godot/scenes/main.tscn`, HUD script.

- [ ] Move underground **display-only** logic (`_sync_underground_rows`, header/kind/slot visuals, reused buttons, tooltips, progress) to `underground_drawer.gd`. Keep the controller's Tab/Esc interception in `_input` and its action handlers for `excavate`/`build_room`. Keep per-sim-tick refresh while visible, rows reused until level count changes, and arm/disabled state correct after close/reopen/restart.
- [ ] Preserve every existing underground semantic contract: six catalogue-ordered kinds, 1-based level IDs, 0-based command slot IDs, L3 Lab eligibility, paid excavation, scrollable levels, no input leak, and paused inspection without commands. Expose signals for slot purchase/dig only, not a second bridge client.
- [ ] Use `screens/start_key_art.png` and `screens/emblem.png` on Start, existing Pause controls in a skinned overlay, and `victory_banner.png`/`defeat_banner.png` on Result. The session phase and winner remain the only choice inputs; reuse Start/Resume/Restart/Quit buttons.
- [ ] Add a non-clickable effects overlay: low-opacity always-on vignette, frost image intensified only when surface cold, warm amber edge on the reported incoming zone(s). Support N/E/S/W (including multiple simultaneous zones); don't derive alien spawn positions independently. Check overlays do not hide command costs/status or intercept mouse/keyboard input.
- [ ] Reconcile action/state resets on pause, Result and Restart. Clear stale placement/attack/underground arm visuals consistently with existing controller state; do not change how the sim resets.

**Gate:** Real click journey covers Start → Playing → Tab room actions → Pause/Resume → Result → Restart. Capture an in-match warm frame, cold + incoming frame, drawer frame and lifecycle frames for visual review.

## Checkpoint 5 — Adapt existing smoke journeys, run verification, record evidence

**Files:** existing smoke GDScripts under `godot/scripts/`, scene files only if their runner setup requires an update; PR description/evidence links.

- [ ] Adapt existing node-path/type/text expectations, not the behaviors they cover. Update `smoke_test.gd` for HUD icon/theme assets, render bounds, research markers, placement/selection and real-click response.
- [ ] Update `economy_smoke_test.gd` and `reset_test.gd` for four resources, population, heat/cold and new numeric rendering. Update `wave_smoke_test.gd` for countdown and warning chip; keep fog/minimap assertions.
- [ ] Update `combat_lifecycle_smoke_test.gd` so Analyze succeeds as a **button interaction without Research Center selection** (when funded) and rejects with insufficient Crystals as before; test pause/result locks and verify full producer FIFO queue presentation. Do not add test-only currency mutations to manufacture a funded game path.
- [ ] Update `underground_smoke_test.gd` for the new drawer presentation while preserving its time-sensitive real-input funded L3/Tap/Lab journey, Tab/Esc focus, disabled slot and single-charge checks. Update the existing defenses smoke if HUD buttons move.
- [ ] Verify imported textures, mouse STOP/IGNORE, no clipping, legible 1280 × 720 typography, the four approved reference states and Start/Result banners in actual rendered frames. Record screenshots or an evidence note with what was really inspected.

**Required commands** (run with the repo-pinned Rust/Godot versions and a freshly staged GDExtension, per `CLAUDE.md`):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings
cargo test --workspace
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/wave_smoke_test.tscn
godot --headless --path godot res://scenes/defenses_smoke_test.tscn
godot --headless --path godot res://scenes/underground_smoke_test.tscn
```

Use the existing CI export/e2e, benchmark and coverage gates, and confirm both process exit status and each smoke's explicit success marker. A documentation-only planning commit does **not** count as these checks passing.

## Review decisions / boundaries

- **One PR:** keep all HPA-698 stages and review corrections in this same draft PR; mark ready only after implementing and verifying the UI.
- **No art generation mixed with code:** reuse the existing generated art and HPA-44 icons. Any additional/replacement images must be scoped to a separate art task; HPA-697's Crystal Lance correction is deliberately not taken on here.
- **Keep it small:** no scene framework, state-management layer, generic style compiler, new commands, new unit art, new audio, balance changes or backwards compatibility scaffolding.
- **Live data:** do not hardcode prices, research unlocks, wave durations, room prices, fog data or battle state in GDScript. The minimal bridge projections defined in the design spec are the only allowed Rust changes.
