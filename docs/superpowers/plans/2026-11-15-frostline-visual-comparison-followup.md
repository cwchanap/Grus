# Frostline Visual Comparison Follow-up Implementation Plan

> **For agentic workers:** Use subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Bring the in-game HUD closer to the approved mockups without sacrificing the accessible opening view or working controls.

**Architecture:** Refine the existing Godot controls, theme and drawer rather than adding art or changing the game. Evaluate the lower panels, underground choices and wave card together because their geometry and click regions interact.

**Tech Stack:** Godot 4.7.1, GDScript, existing Frostline assets and smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-09-hpa-698-frostline-hud-design.md`, `docs/superpowers/specs/2026-11-15-frostline-hud-visual-polish-design.md` and `docs/superpowers/specs/2026-11-15-frostline-access-shutdown-design.md`.

## Global Constraints

- Reuse existing art, fonts, theme and controls; do not change Rust authority, rules, commands, snapshots, fixture, world models or fog.
- At 1280 × 720 preserve three complete underground rows, scrolling, clipping, usable buttons, locks, clickable unaffordable actions, accessibility, focus, tooltips and session transitions.
- Preserve camera size 54, pan/zoom/minimap, and real opening Gather, scout and Depot interactions with their existing clearance assertions.
- Keep Linux checks waived and PR #24 draft. Preserve diagnostic and capture artifacts; label synthetic probes and disclose failures and retries.

## Review Focus

- All six opening resource clicks must still acquire their authoritative source, and scout/Depot targets remain clear: existing main/economy/underground real-input smokes.
- Production queue, selection text and contextual buttons must remain within their controls when lower panels are compacted: queue and selection smokes and captured populated states.
- Underground names, prices, status and focus must remain readable without overlapping; three rows, scrolling and real actions must remain available: drawer smoke and captured armed, locked, occupied and scrolling states.
- Wave prep, active and multi-direction warnings and tooltip must remain readable and reachable without covering opening targets: wave and main smokes plus rendered warning capture.
- UI must remain usable at Start, Paused and Result; audio shutdown must retain its best-effort behavior: lifecycle and shutdown smokes.

### Task 1: Refine the current HUD composition

**Files:** `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`, existing Godot theme or HUD scripts only as needed; `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/underground_smoke_test.gd`, and wave/lifecycle tests as appropriate.

**Interfaces:** Preserve existing `HUD` node paths, room and slot commands, selection/controller ownership, static catalogue data and session APIs.

- [ ] Inspect the approved mockups and current 1280 × 720 captures. Identify the smallest changes to reduce the lower panels' visual weight, make underground room icons and labels clearer, and improve the wave card's relationship to the top bar. Do not force a geometry change that compromises starting target clearance; retain and report any tradeoff.
- [ ] Add focused assertions for the intended geometry and text containment in existing real-input smokes; run them before production edits and retain the expected failure.
- [ ] Make the minimum existing-control/theme changes that meet those assertions. Keep names and costs derived from the catalogue; do not rely on color alone or hide commands.
- [ ] Run all seven headless Godot smokes, focused shutdown regression and `cargo test --workspace --locked`; retain failures and retries. Capture 1280 × 720 real opening, selected-unit, queue, underground and wave-warning frames; compare them with the approved mockups and prior captures. Clearly label synthetic states.
- [ ] Record exact commands, exits, screenshots, remaining compromises and scope in the task report; commit the focused changes.

### Task 2: Verify and update the draft PR

- [ ] Independently review Task 1 for visual and input regressions, correct any important findings and re-review fixes.
- [ ] Run final `cargo fmt --all -- --check`, all seven headless smokes, shutdown runner contract and `git diff --check` for this pass. Record exact results and limitations.
- [ ] Capture and document final-head comparison evidence; obtain independent whole-branch review. Update existing PR #24 without marking it ready.
