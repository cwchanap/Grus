# Frostline HUD Visual Fidelity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox syntax.

**Goal:** Bring the existing HUD closer to the approved mockups without changing gameplay or the game-world models.

**Architecture:** Reuse the existing Godot scene, theme, painted UI textures, input controller and snapshot-driven presentation. Improve each independent region in place; retain node paths and existing behavioral smoke coverage.

**Tech Stack:** Godot 4.7.1, GDScript, existing Rust bridge and Godot smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-12-frostline-hud-visual-fidelity-design.md`

## Global Constraints

- Reuse the existing art, fonts, theme and snapshots; do not introduce new artwork, change world models, game rules, command authority or available statistics. The approved map-edge clear color affects presentation during gameplay as well as Result.
- Preserve Start/Pause/Result transitions, named controls, input, focus, accessibility, queue state and drawer behavior.
- Add focused real-input and layout regressions, run the applicable non-Linux checks, and capture fresh final-head 1280 × 720 images for direct mockup comparison.
- Label presentation-only Victory and any synthetic deeper-level probe accurately. Continue on PR #24; skip the previously waived Linux checks.

## Review Focus

- Victory and Defeat use different banner artwork: each title must be centered in its visible banner, not just the `TextureRect` bounding box (Task 1).
- The existing Result dim already covers the viewport; verify the map-edge clear color and real rendered Result frame, while keeping session buttons and drawer gating correct (Task 1).
- Start controls must retain their documented keys and real-click Start; Pause/Resume must not inherit Start-only decorations (Task 1).
- FIFO head, empty sockets, progress, queue overflow and blocked states must remain legible and unchanged with painted frames (Task 2).
- Excavation at zero/progress/completion and deep scrolling must remain snapshot-driven, with three whole rows and populated-row clicks clear of command controls (Task 3).

### Task 1: Start and Result

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`, `godot/scripts/fog_of_war.gd`, `godot/scripts/smoke_helpers.gd`; test `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Preserve `HUD/SessionPanel/{StartButton,ResumeButton,RestartButton,QuitButton,SessionLabel,ControlsLabel}`, `HUD/SessionBackdrop/{ResultBanner,ResultTitle}`, `HUD/ResultBar`, and `refresh_session(phase: String, winner_team: int)`. Use existing `godot/assets/ui/frostline/` and `godot/assets/ui/icons/` textures. `ControlsLabel` remains the text source for the documented key-word checks, either as the visible strip or via tests joining all visible strip cell text; preserve its phase visibility and SessionLabel's Paused/Result text.

- [ ] Add Start layout assertions for three icon-led objectives, a horizontal controls strip with the existing key instructions, a visible bright Start action, and no overlapping or offscreen controls. Preserve real Start and Pause/Resume checks.
- [ ] Add Result assertions that the existing backdrop still covers the viewport, the fog and default clear color agree where readable, and Victory and Defeat place the title at different heights; retain real Defeat, real Tab gating and Restart assertions. A headless dummy-renderer getter is not proof of pixels: require the real-rendered frame inspection below.
- [ ] Run `godot --headless --path godot res://scenes/smoke_test.tscn` and `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn`; record the expected new failures.
- [ ] Match the default clear color to a shared unexplored fog color in `fog_of_war.gd`; verify with the real-rendered Defeat frame and gameplay map-edge view, since identical color values alone cannot prove the seam is gone. Keep gameplay state unchanged.
- [ ] Implement the two presentation layouts with existing textures and phase-specific visibility; derive the research target and initial preparation duration from the catalogue and `wave_snapshot().timer_total_seconds`, and omit the unavailable regular-wave interval. Reuse the shared Start/Restart style; keep Resume separate and inputs unchanged.
- [ ] Use per-winner normalized title positions derived from the two banner images. The real rendered frames, not a test reading the same constants, decide whether the text is centered on each cloth.
- [ ] Add reusable opt-in `capture_frame` to `godot/scripts/smoke_helpers.gd`, called at existing real-input journey points: capture only when `GRUS_CAPTURE_DIR` is set, return before awaiting render signals in headless mode, use frame-post-draw on a real renderer, and surface directory or save errors. It must not mutate gameplay state.
- [ ] Rerun both smokes with exit 0 and their `GRUS_GODOT_SMOKE_OK` and `GRUS_COMBAT_LIFECYCLE_SMOKE_OK` markers. Capture real-rendered 1280 × 720 Start and real Defeat, plus a labelled presentation-only Victory probe; inspect both banner title positions against `mockup-4.png`.
- [ ] Commit the task.

### Task 2: Painted production sockets

**Files:** Modify `godot/scripts/frostline_hud.gd` and, only if required by layout, `godot/scenes/main.tscn`; test `godot/scripts/economy_smoke_test.gd`.

**Interfaces:** Preserve five `HUD/SelectionPanel/QueueSockets` sockets and their `Icon`, `Time`, `Progress` children and overflow chip. Reuse the theme `Tile` normal/pressed styleboxes for queued/head sockets and retain the flat recessed empty style; the round `socket.png` is not a square queue bezel. `training_queue`, catalogue seconds and progress remain authoritative.

- [ ] Add texture-path assertions for theme Tile normal and pressed styles and unchanged non-overlapping bounds in the six-enqueue overflow and Guard→Rifleman FIFO journeys; retain recessed empties, head progress/time, blocked and overflow checks.
- [ ] Run `godot --headless --path godot res://scenes/economy_smoke_test.tscn` and record the expected missing-painted-treatment failure.
- [ ] Replace the two hand-built queued/head flat styles with existing theme Tile normal/pressed styles; retain the recessed empty style without obscuring the icon, time, progress or `+N` chip.
- [ ] Rerun the economy smoke with exit 0 and `GRUS_ECONOMY_SMOKE_OK`; capture real 1280 × 720 Barracks and overflow frames and compare with `mockup-2.png`.
- [ ] Commit the task.

### Task 3: Underground presentation and final QA

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`, `godot/scripts/underground_smoke_test.gd`, and `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/` as needed.

**Interfaces:** Preserve `HUD/UndergroundPanel/ScrollContainer/LevelsBox`, `excavate_requested`, `room_requested(level, slot, kind)`, 1-based levels and 0-based slots. Derive excavation and room display solely from `underground_snapshot()` and the catalogue.

- [ ] Add assertions for kind and slot appearances: occupied/building uses normal painted tile with room icon; level-locked uses locked tile and lock icon plus an accessible L# explanation; armed eligible empty uses selected tile; unarmed empty stays recessed. Check the first three with Lab armed at L3 and the unarmed state separately after disarming. Preserve level-requirement semantics when updating existing text/icon checks, plus readable labels, three full rows, clipping, Analyze/card non-overlap, real wheel and populated-row command-click probes.
- [ ] Assert excavation idle, active zero/partial progress with snapshot-derived remaining time, and completion; retain actual purchase and pause inspection checks.
- [ ] Run `godot --headless --path godot res://scenes/underground_smoke_test.tscn` and record the expected new visual-state failure.
- [ ] Reuse Tile for kinds; explicitly set slot styles in all four branches, including disabled occupied rooms and disabled unarmed empty slots. Preserve eligibility, accessible labels/tooltips and input.
- [ ] Use the existing theme ProgressBar with snapshot `progress` and `remaining` for active digging on or below Excavate, outside the 152 px level viewport; do not introduce stripe art or reduce the three-row viewport.
- [ ] Run all seven existing Godot smokes with exit 0 and success markers, `cargo fmt --all -- --check`, and `cargo test --workspace --locked`; record failures and retries. Do not run Linux checks.
- [ ] Capture final-head 1280 × 720 Start, real Defeat, labelled Victory presentation probe, FIFO/overflow and real L3 drawer frames. Inspect against all four approved mockups and update evidence/provenance; disclose any synthetic deeper-level probe and remaining visual differences.
- [ ] Commit the task. Obtain whole-branch review and update the existing draft PR with verification and limitations.
