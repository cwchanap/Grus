# Frostline HUD Visual Fidelity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox syntax.

**Goal:** Bring the existing HUD closer to the approved mockups without changing gameplay or the game-world models.

**Architecture:** Reuse the existing Godot scene, theme, painted UI textures, input controller and snapshot-driven presentation. Improve each independent region in place; retain node paths and existing behavioral smoke coverage.

**Tech Stack:** Godot 4.7.1, GDScript, existing Rust bridge and Godot smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-12-frostline-hud-visual-fidelity-design.md`

## Global Constraints

- Reuse the existing art, fonts, theme and snapshots; do not introduce new artwork, change world models, game rules, command authority or available statistics.
- Preserve Start/Pause/Result transitions, named controls, input, focus, accessibility, queue state and drawer behavior.
- Add focused real-input and layout regressions, run the applicable non-Linux checks, and capture fresh final-head 1280 × 720 images for direct mockup comparison.
- Label presentation-only Victory and any synthetic deeper-level probe accurately. Continue on PR #24; skip the previously waived Linux checks.

## Review Focus

- Victory and Defeat use different banner artwork: each title must be centered in its visible banner, not just the `TextureRect` bounding box (Task 1).
- The Result backdrop must cover the viewport continuously without intercepting session buttons or exposing the underground drawer (Task 1).
- Start controls must retain their documented keys and real-click Start; Pause/Resume must not inherit Start-only decorations (Task 1).
- FIFO head, empty sockets, progress, queue overflow and blocked states must remain legible and unchanged with painted frames (Task 2).
- Excavation at zero/progress/completion and deep scrolling must remain snapshot-driven, with three whole rows and populated-row clicks clear of command controls (Task 3).

### Task 1: Start and Result

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`; test `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Preserve `HUD/SessionPanel/{StartButton,ResumeButton,RestartButton,QuitButton}`, `HUD/SessionBackdrop/{ResultBanner,ResultTitle}`, `HUD/ResultBar`, and `refresh_session(phase: String, winner_team: int)`. Use existing `godot/assets/ui/frostline/` and `godot/assets/ui/icons/` textures.

- [ ] Add Start layout assertions for three icon-led objectives, a horizontal controls strip with the existing key instructions, a visible bright Start action, and no overlapping or offscreen controls. Preserve real Start and Pause/Resume checks.
- [ ] Add Result assertions for continuous full-viewport backdrop coverage and title placement over the visible artwork for both winner values; retain real Defeat, real Tab gating and Restart assertions.
- [ ] Run `godot --headless --path godot res://scenes/smoke_test.tscn` and `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn`; record the expected new failures.
- [ ] Implement the two presentation layouts with existing textures and phase-specific visibility; keep inputs and snapshot statistics unchanged.
- [ ] Rerun both smokes with exit 0 and their `GRUS_GODOT_SMOKE_OK` and `GRUS_COMBAT_LIFECYCLE_SMOKE_OK` markers. Capture real-rendered 1280 × 720 Start and real Defeat, plus a labelled presentation-only Victory probe; inspect both banner title positions against `mockup-4.png`.
- [ ] Commit the task.

### Task 2: Painted production sockets

**Files:** Modify `godot/scripts/frostline_hud.gd` and, only if required by layout, `godot/scenes/main.tscn`; test `godot/scripts/economy_smoke_test.gd`.

**Interfaces:** Preserve five `HUD/SelectionPanel/QueueSockets` sockets and their `Icon`, `Time`, `Progress` children and overflow chip. Use `godot/assets/ui/frostline/ui/socket.png` or existing painted plate textures; `training_queue`, catalogue seconds and progress remain authoritative.

- [ ] Add assertions for painted socket treatment and unchanged non-overlapping bounds in the six-enqueue overflow and Guard→Rifleman FIFO journeys; retain head progress/time, empty, blocked and overflow checks.
- [ ] Run `godot --headless --path godot res://scenes/economy_smoke_test.tscn` and record the expected missing-painted-treatment failure.
- [ ] Implement painted empty, queued and highlighted-head appearances without obscuring the icon, time, progress or `+N` chip.
- [ ] Rerun the economy smoke with exit 0 and `GRUS_ECONOMY_SMOKE_OK`; capture real 1280 × 720 Barracks and overflow frames and compare with `mockup-2.png`.
- [ ] Commit the task.

### Task 3: Underground presentation and final QA

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`, `godot/scripts/underground_smoke_test.gd`, and `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/` as needed.

**Interfaces:** Preserve `HUD/UndergroundPanel/ScrollContainer/LevelsBox`, `excavate_requested`, `room_requested(level, slot, kind)`, 1-based levels and 0-based slots. Derive excavation and room display solely from `underground_snapshot()` and the catalogue.

- [ ] Add assertions for painted kind/slot states, readable labels, excavation idle/progress/completed values, three fully visible level rows, clipping and Analyze/card non-overlap. Preserve and exercise the existing real wheel and populated-row command-click probes.
- [ ] Run `godot --headless --path godot res://scenes/underground_smoke_test.tscn` and record the expected new visual-state failure.
- [ ] Implement readable painted slot and kind styling and snapshot-driven excavation progress without reducing the three-row viewport or changing purchasing behavior.
- [ ] Run all seven existing Godot smokes with exit 0 and success markers, `cargo fmt --all -- --check`, and `cargo test --workspace --locked`; record failures and retries. Do not run Linux checks.
- [ ] Capture final-head 1280 × 720 Start, real Defeat, labelled Victory presentation probe, FIFO/overflow and real L3 drawer frames. Inspect against all four approved mockups and update evidence/provenance; disclose any synthetic deeper-level probe and remaining visual differences.
- [ ] Commit the task. Obtain whole-branch review and update the existing draft PR with verification and limitations.
