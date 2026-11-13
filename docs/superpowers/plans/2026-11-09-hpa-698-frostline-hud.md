# HPA-698 — Frostline HUD implementation plan

**Linear:** [HPA-698](https://linear.app/cwchanap/issue/HPA-698/hud-redesign-frostline-look-with-the-generated-art)
**Design spec:** [Frostline HUD design](../specs/2026-11-09-hpa-698-frostline-hud-design.md)
**Art baseline:** `art-src/2026-11-09/` (HPA-695, PR #23, merged).
**Branch:** `hpa-698-frostline-hud`
**PR policy:** **This planning draft is the one and only PR for HPA-698**. All implementation, test changes, screenshots, review fixes and polish land on this branch.
**State:** Design and plan only. The following checkpoints are not yet implemented or verified.

## Review decisions (PR #24 design review)

Eight findings accepted: (1) project existing Rust HUD stats instead of leaving placeholder numbers; (2) use one armed-order state for the actual mockup command card; (3) keep every smoke green at every checkpoint; (4) let the drawer own its runtime room-arm state; (5) reuse the 128 px icon tree and configure tiled 9-slice panels correctly; (6) keep `GrusBridge.analyze(id)` unchanged and select the actual Research Center view internally; (7) bundle required fonts, filter producer units, bound queue display, set project-wide theme, remove obsolete InputShield and permit drawn tiny glyphs; (8) use existing `incoming_zones` instead of a redundant warning-seconds field.

**Second review — three pre-flight guards (all accepted):** keep `catalogue_snapshot().units` limited to `UnitKind::HUMAN` so alien entries with an empty `producer` cannot poison `_producer_kinds` and cause right-click Move to call `set_rally(-1, ...)`; extend the exact wave-snapshot shape assertion to six keys at the same checkpoint as the bridge change; and invert the two existing "deselect disables Analyze" smoke assertions when the top-bar button becomes selection-independent. These are surgical test/plan changes, not new architecture or gameplay features.

Keep all HUD commands and gameplay authoritative in Rust; no new sim command, balance change, interface framework, art generation or other PR. Godot uses the existing Theme, Control, Button, TextureRect and ProgressBar types.

## Checkpoint 1 — Project Rust-owned HUD details + assert them in real Godot

**Files:** `crates/grus-godot/src/lib.rs`; existing `godot/scripts/economy_smoke_test.gd`, `smoke_test.gd` and other affected smoke assertions. No UI layout moves.

- [ ] In `building_snapshot(id)`, add ordered `training_queue` from `ProductionQueue.jobs`; preserve existing `queue_label`, head `queue_progress`, `blocked_reason`, Rally and fog/invalid-id conventions. An existing producer without a queue gets an empty array; a hidden/missing building remains an empty snapshot.
- [ ] In `catalogue_snapshot_dict`, keep the existing `UnitKind::HUMAN` loop and project `max_health` for the **trainable human units only**. Do **not** switch it to `UnitKind::ALL`: alien `producer` is `""` and the current `_producer_kinds` collection would then treat an empty selected-producer kind as valid, making ordinary ground right-click issue `set_rally(-1, ...)` instead of Move. Alien counters can use the already-shipped alien icons and `counter_target` names without adding alien catalogue rows. For human units and buildings, project optional combat `damage`, `range` (from `attack_range`), `counter_target` and `counter_bonus` (from `CombatSpec`) to support accurate tooltips. Project buildings' `max_health`, `heat_supply`, `heat_demand`; add top-level `carry_limit` from Rust `CARRY_LIMIT`. All prices, timers, unlocks and producer names stay source-of-truth catalogue data.
- [ ] In `unit_task_snapshot` add `carry_amount` and `carry_kind` from the existing `Carry` component. Empty Carry is `0`/`""`, and no/hidden worker task returns the existing empty snapshot. Avoid additional queues/components.
- [ ] Add `timer_total_seconds` to `wave_snapshot_dict` based on Rust preparation-vs-interval constants and the current `next_wave`; do **not** add `warning_seconds`. **In the same commit, update `godot/scripts/combat_lifecycle_smoke_test.gd::_snapshot_has_safe_surface` (currently lines 460–471)**: expect **six** keys, include `timer_total_seconds` in the expected key list, and assert its numeric value is positive. `wave_smoke_test.gd` and `defenses_smoke_test.gd` inherit this assertion; leaving it at five keys would fail three of seven smoke journeys. Add `wave_growth_points` to `catalogue.research` using `unlock_points(RESEARCH_WAVE_BOOST_PERCENT)`.
- [ ] Extend the **existing** smoke journeys to assert the new bridge projections. In `economy_smoke_test.gd` immediately after the two real Guard/Rifleman enqueue clicks and their charge verdict (currently around lines 916–935), check `training_queue == ["Guard", "Rifleman"]` before the first job finishes at 20×; later assert transition to Rifleman head and empty queue. Use bounded polling rather than assuming both enqueue commands settle in the same frame. Assert worker carry fields and catalogue stats through actual fixture state. In `smoke_test.gd` assert wave timer total and milestone projection using the real fixture/catalogue.
- [ ] Check no hidden IDs leak private data and no existing key/label changes break smoke contracts. This checkpoint does **not** modify `GrusBridge.analyze(id)` or the session/gameplay logic.

**Gate:** Rust fmt, workspace clippy/tests, a fresh staged GDExtension and **all seven headless Godot smokes passing with success markers**. A Rust unit test by itself cannot validate Godot dictionary serialization.

## Checkpoint 2 — Pure presentation extraction, unchanged scene geometry

**Files:** `godot/scripts/battlefield_controller.gd`, new `godot/scripts/frostline_hud.gd` and `godot/scripts/underground_drawer.gd`, minimal script attachment in `godot/scenes/main.tscn`.

- [ ] First relocate current HUD **rendering only** (`_refresh_hud`, `_refresh_research_controls`, `_refresh_selection`, `_refresh_session_overlay` and static icon/cost setup) to a script attached **directly** to the existing `HUD` CanvasLayer. The controller still owns snapshots/selection, command state/feedback and every static Button's pressed connection; avoid introducing a duplicate snapshot cache or a hub of relay signals.
- [ ] Leave `Main/HUD` and the named button/control paths and entire layout intact; no theme, fonts, style, icon sizes or new visible controls in this step. Existing smoke selectors should remain unchanged, demonstrating that the extraction is behavior-preserving.
- [ ] Move the underground catalogue rows and entire visual arming state (`_ug_rooms`, `_ug_slots_per_level`, `_ug_kind_buttons`, `_ug_armed_index`, `_on_kind_toggled`, row updates) into the drawer script on the **same** UndergroundPanel. It owns state and emits only `excavate_requested()` and `room_requested(level, slot, kind)`. The controller keeps Tab/Esc interception in `_input`, Playing gate, bridge command dispatch and status. Restart calls `drawer.reset()`.
- [ ] Preserve the existing controller command/input paths (`_attack_move_armed` is deliberately not replaced until the armed-tile behavior checkpoint), input shielding, focus and test-visible commands without UI behavior changes. Do not restructure Rust sim, scene tree or selection.
- [ ] Use the existing underground per-tick refresh behavior while the drawer is visible, only rebuilding rows on depth changes; while paused, inspect but never issue commands.

**Gate:** **All seven Godot smoke journeys pass without modifying their selectors or behavior assertions.** If anything fails, fix extraction in the same checkpoint before re-skinning. Capture the exact commit as the known-green baseline.

## Checkpoint 3 — Re-skin, import assets, relocate HUD and adapt selectors together

**Files:** `godot/project.godot`, `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`, `godot/assets/ui/frostline/{ui,screens}/`, `godot/assets/ui/icons/{actions,status}/`, `godot/assets/ui/icons/README.md`, one small `action_tile.tscn`, Theme resource and font/OFL files; affected existing smoke tests.

- [ ] Import HPA-695's **15 new icons** to the **existing** icon directory at 128 × 128; downsample the already-created source images (asset import preparation, **not new art generation**) or set deterministic `process/size_limit=128` in Godot import settings. Extend the existing icon inventory, reuse `_icon_for_kind(group, kind)` and the original HPA-44 painted resource/building/unit/underground PNGs; do **not** create another icon tree.
- [ ] Import only HPA-695 `ui/` plates/tiles/socket/round button/progress frame/frost and `screens/` start/emblem/victory/defeat into `godot/assets/ui/frostline/`; leave `sprites/` for HPA-697. Bundle **Barlow and Barlow Condensed TTFs and their OFL text** from a permissible source as required by the ticket, and use them as the theme fonts (no system font assumption). No new illustrations, regenerated images or SFX.
- [ ] Create one Theme using built-in Button pressed/disabled states and one `theme_type_variation` for unaffordable tiles. Use `StyleBoxTexture.region_rect` and `texture_margin_*` to exclude PNG transparent padding; the long top bar repeats snow edges with `axis_stretch_horizontal = TILE_FIT`. Add one tiny static action tile scene/helper rather than a widget framework. Draw simple small glyphs for arrow/plus/cancel/restart/quit if needed.
- [ ] Set `gui/theme/custom` in `project.godot` instead of nesting HUD in a new Control. Re-layout 1280 × 720 HUD regions to match the mockups; preserve key `HUD/...` names and their actual button/press targets. The new full-width STOP panels replace the old hardcoded `InputShield`: delete it **in this same re-layout commit**, update its obsolete smoke assertion, and explicitly test that clicking the top bar never selects a world object.
- [ ] Move resources/heat, ring/research progress, minimap, selection, command tiles and Tab drawer into the approved zones. Keep existing Rust numeric text/meaning, tooltip content and temporary rendering for features scheduled in checkpoint 4. Drawn overlay effects must always use mouse IGNORE.
- [ ] Update affected existing scene-path, type, placement and bounds assertions **in the same commits as the UI changes**. Do not delete real-click assertions, especially `economy_smoke_test.gd` and underground Tab/Esc focus.

**Gate:** Fresh Godot texture/font import and **all seven smokes green**, plus a visually inspected 1280 × 720 in-match screenshot for clipping/missing texture, not just a static editor preview. Avoid moving to the next checkpoint with known red journeys.

## Checkpoint 4 — Complete the mockup behavior with per-feature assertions

**Files:** `godot/scripts/battlefield_controller.gd`, `godot/scripts/frostline_hud.gd`, `godot/scripts/underground_drawer.gd`, current HUD scenes and affected existing smokes.

- [ ] Render full catalogue-backed selected HP: `clampi(roundi(view.health_ratio() * max_health), 0, max_health)` for one live unit/building; worker task and `carry_amount/carry_limit` + `carry_kind`; unit/building tooltips with price, time, max HP, heat, damage, range and counter target where applicable. Use already-present alien icons for counter art. Never display made-up mixed-selection HP or stale state.
- [ ] Render a **five-socket** FIFO producer queue from `training_queue` (head progress and remaining seconds derived from existing catalogue train seconds, `+N` chip when more than five jobs) and blocked state. Show only producer-matching train tiles using the existing catalogue `units[kind].producer`; the Barracks shows Guard/Rifleman, not Worker. Keep click/charge/queue sequence and the existing `queue_label`.
- [ ] Change the controller's one `_attack_move_armed: bool` to one `_armed_order: String` with `""`, `"attack_move"`, `"move"`, `"gather"`, `"rally"`. Reuse the current Move/Gather/Rally/AttackMove bridge paths on the **next right click**, consuming the order once, with no unintended generic context fallback. Stop (`S`) runs immediately; Cancel/Esc clears placement or order before pause. Clear order on lost selection/invalid context, reset, phase change; render the active tile from controller state. Update existing attack-move smoke assertions at the same time, and add real-click tests of all modal orders.
- [ ] Use `seconds_remaining / timer_total_seconds` for the wave countdown ring; amber state and directional edge glow trigger **only** from nonempty `wave.incoming_zones`. Keep one wave snapshot shared with minimap and avoid independent countdown/zone logic.
- [ ] Research markers come from static building unlock points and `wave_growth_points`; display target/progress/ETA without rounding up early. For an Analyze press, find the **live, Team-1 Research Center view ID** using existing `building_views` metadata, call unchanged `GrusBridge.analyze(id)`, and keep Rust session/Crystals validation. It must work regardless of current selection, and remain disabled in Start/Paused/Result or when the core is absent. Test zero-Crystal rejection via the existing command feedback path without manufacturing resource state. **Update the two existing deselection assertions in the same behavior commit**: `godot/scripts/smoke_test.gd` around lines 1978–1992 and `godot/scripts/combat_lifecycle_smoke_test.gd` around lines 385–392 currently require `analyze_button.disabled` after clicking empty ground. They must instead assert deselection with **Analyze still enabled while Playing**, then drive an Analyze click without Research Center selection and check authoritative command feedback / correct zero-Crystal rejection. Remove the old disabled-button no-dispatch check only because the expected behavior is intentionally reversed; retain pause/Result disable checks.
- [ ] Complete Tab drawer art/readability, room type tooltips, arm/disabled/occupied state, real slot/Excavate press routing, pause inspection and restart reset. The drawer owns its room kind state and emits resolved intent; the controller alone dispatches. Preserve all underground row reuse, level/slot indexing and real input focus.
- [ ] Apply Start key art/emblem and Victory/Defeat banners, a subtle always-on vignette, cold-only frost and an incoming-side glow. Only session/heat/wave snapshots drive transitions; no additional lifecycle state. Noninteractive effects have mouse IGNORE.
- [ ] **Add or adjust the relevant existing smoke assertion with each feature commit**, then rerun its targeted smoke and the seven-smoke suite before the checkpoint is finished. Preserve every existing gameplay assertion.

**Gate:** All seven smokes green with real UI input, and a manual 1280 × 720 review of warm, cold+warning, queued Barracks, underground L3, Start and Result frames.

## Checkpoint 5 — Final QA, evidence and readiness gate

**Files:** existing smoke tests only for actual uncovered regressions; PR description and screenshot/evidence references.

- [ ] Verify Start → Playing → Pause/Resume → Result → Restart as a real-input journey, plus unit move/stop/attack-move/gather, producer Rally, locked/unaffordable costs, Analyze feedback, underground L3/Tap/Lab, fog/minimap and lifecycle overlays. Check queue FIFO and visibility of the "+N" overflow chip via a bounded real-input test; do not introduce a test-only simulation path.
- [ ] Compare actual rendered screenshots with the four approved mockups at **1280 × 720**: warm match, cold with incoming wave, selected Barracks and its five sockets, underground Lab/scroll/dig, Start emblem/key art and victory/defeat banners. Record limitations and screenshots; headless smoke alone does not prove matching art direction.
- [ ] Confirm no changed source under `crates/grus-sim/`, no HPA-697 sprites/new image generation/SFX, and no parallel UI state store. All gameplay balance and command semantics remain as before.
- [ ] Run full Rust + Godot checks with the repo-pinned toolchain and freshly staged GDExtension (per `CLAUDE.md`), plus existing CI export/e2e/coverage. Verify exit codes **and** explicit success markers. Mark ready only after genuine passing evidence.

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

**No implementation or tests have been run for this planning-only draft.** Checkpoints describe work to be performed in the **same** PR, not a sequence of PRs. If new imagery is genuinely needed, scope it to a separate art task instead of adding generation work to HPA-698.
