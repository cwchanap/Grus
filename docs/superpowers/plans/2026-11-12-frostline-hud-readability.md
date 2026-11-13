# Frostline HUD Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax.

**Goal:** Improve readability and mockup fidelity across the existing Frostline HUD, prioritizing the underground drawer.

**Architecture:** Modify the existing Godot scene and snapshot-driven HUD scripts in place, retaining node paths and real-input smoke journeys. Reuse the existing theme and assets; capture real-render frames to evaluate visual hierarchy as well as automated layout checks.

**Tech Stack:** Godot 4.7.1, GDScript, the existing Rust bridge and Godot smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-12-frostline-hud-readability-design.md`

## Global Constraints

- Reuse existing artwork, fonts, theme and controls; do not add artwork or fabricate gameplay data.
- Preserve world models, Rust authority, rules, commands, snapshots, named controls, input, focus, accessibility, queue behavior and session transitions.
- At 1280 × 720 retain three complete underground rows, scrolling, clipping and accessible command controls.
- Label presentation-only Victory and synthetic deeper-level probes accurately; keep PR #24 draft and skip Linux checks.
- Record every failed or retried verification, and compare fresh final-head 1280 × 720 captures with the approved mockups.

## Review Focus

- Long names and values in narrow room tiles: fit without clipping or losing the catalogue/snapshot value (Task 1).
- Locked, armed, constructing and completed rooms: distinct states and truthful requirements, benefits and progress (Task 1).
- Deep scroll and paused inspection: clipping, real wheel and command clicks remain safe; purchase controls stay gated (Task 1).
- Start, Pause and both Result variants: readable controls, correct visibility and real transitions without invented results; the shared dim affects Pause too (Task 2).
- FIFO head, empty queue and overflow: timer/progress match snapshot and remain visible without overlapping adjacent controls (Task 3).

### Task 1: Underground readability

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`; test `godot/scripts/underground_smoke_test.gd`.

**Interfaces:** Preserve `HUD/UndergroundPanel/ScrollContainer/LevelsBox`, `refresh(snap: Dictionary, phase: String)`, `excavate_requested`, `room_requested(level: int, slot: int, kind: String)`, catalogue wire names, 1-based levels and 0-based slots. The drawer's `_snapshot` remains the rendered readout source.

- [ ] Extend `_kind_layout_error` to require kind names ≥12 px and kind costs ≥11 px, disjoint name/cost rectangles, enough height for the name and `get_content_height() <= cost.size.y` after layout, in addition to existing width/containment checks. Extend `_slot_readout_error` to require occupied slot names ≥12 px, benefit and construction-percent text ≥11 px, and disjoint rendered name text versus benefit/percent text. Measure the Button's actual text bounds (including font metrics, alignment, icon and style margins), or use an explicitly bounded name label; do not compare the whole Button rectangle to its own children. Add a ≥12 px lock-text floor to the existing Lab-armed shallow-slot checks, and require `DigTime` ≥12 px alongside its bounds checks.
- [ ] In the existing synthetic four-row probe, first populate the L4 row with completed Shelter, Greenhouse, CoalVein and DeepMine slots. Take each `benefit` from `_catalogue.underground.rooms[kind].effect` (population/output formats match completed benefits); after the scrollbar is present and layout settles, run `_slot_readout_error` on all four. Then restore the existing synthetic Shelter 99% construction case and its capture, preserving the real wheel and command-card click checks. Restore the real snapshot as before; no gameplay state or command changes.
- [ ] Reuse existing assertions for locked accessibility/requirements, armed/locked/occupied/construction states, dig zero/partial/completion, three full rows, pause, clipping and real clicks.
- [ ] Run `godot --headless --path godot res://scenes/underground_smoke_test.tscn`; require a failure message from a new assertion, not merely a nonzero exit. The smoke stops at its first failure, so the initial kind-tile failure does not prove the later benefit checks went red. If startup fails, verify the staged GDExtension matches the current build before diagnosing the test.
- [ ] Simplify competing wide-slot plates by duplicating the existing Tile `StyleBoxTexture`s in `_ready`, adjusting their modulation and/or margins while retaining their original texture paths and passing them through `_set_slot_plate`. Preserve existing texture-path/state assertions. Adjust layout, contrast and type to pass the new checks; keep the excavation readout outside the clipped viewport and sourced from `snap`. Do not reduce the three-row viewport or alter game rules.
- [ ] Rerun the same smoke: exit 0 and `GRUS_UNDERGROUND_SMOKE_OK`. Capture real L3/dig/construction and labelled synthetic L4 frames at 1280 × 720; inspect their names, prices, locks and benefits against `mockup-3.png`.
- [ ] Commit the task; obtain the required independent task review before moving on.

### Task 2: Start and Result hierarchy

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`; test `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`, `godot/scripts/underground_smoke_test.gd`.

**Interfaces:** Preserve `HUD/SessionPanel/{StartButton,ResumeButton,RestartButton,QuitButton,SessionLabel,ControlsLabel}`, `HUD/SessionBackdrop/{Dim,ResultBanner,ResultTitle}`, `HUD/ResultBar`, and `refresh_session(phase: String, winner_team: int)`. Retain the existing documented control instructions as visible, accessible text.

- [ ] Add `ControlsLabel` font ≥13 px beside the Start strip checks in `smoke_test.gd` and verify its actual text fits its allocated region. Add `Dim.color.a <= 0.55` beside the Result backdrop check in `combat_lifecycle_smoke_test.gd`. Reuse the existing token, viewport, overlap, real Restart, phase-gating and Victory/Defeat checks rather than duplicating them.
- [ ] Run `godot --headless --path godot res://scenes/smoke_test.tscn` and `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn`; record each expected new assertion message. A nonzero startup failure does not count as a red test.
- [ ] Reflow the existing text controls strip for legibility, keeping all instructions and phase behavior. Use one shared dim alpha for Pause and Result, with no new phase-specific branch. Adjust existing banner, frost and action styling as needed, using existing theme/assets and no fabricated data.
- [ ] Add `SmokeHelpers.capture_frame(get_tree(), "pause")` at the real closed-panel Esc pause point in `underground_smoke_test.gd`, before opening the drawer. Propagate its error like the existing capture calls.
- [ ] Rerun all three affected smokes: exit 0 with `GRUS_GODOT_SMOKE_OK`, `GRUS_COMBAT_LIFECYCLE_SMOKE_OK` and `GRUS_UNDERGROUND_SMOKE_OK`. Capture Start, real Pause, real Defeat and labelled Victory presentation probe; inspect at 1280 × 720 against `mockup-4.png` and verify legibility over the shared lighter backdrop.
- [ ] Commit the task; obtain the required independent task review before moving on.

### Task 3: Queue emphasis and final verification

**Files:** Modify `godot/scripts/frostline_hud.gd` and `godot/scenes/main.tscn` only if needed; test `godot/scripts/economy_smoke_test.gd`; update `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/` with fresh captures and provenance.

**Interfaces:** Preserve five `HUD/SelectionPanel/QueueSockets` sockets, each `Icon`, `Time`, `Progress`, the `OverflowChip`, and snapshot-backed `_render_queue(building: Dictionary)`. Existing smoke capture calls are in `godot/scripts/smoke_helpers.gd` and the four journey scripts.

- [ ] Extend `_queue_layout_error` with a ≥14 px head `Time` font floor and checks that its visible text fits the label and that the label stays inside its socket. Reuse `_queue_painted_treatment_error`, existing FIFO/overflow layout checks and live timer/progress assertions; do not duplicate them.
- [ ] Run `godot --headless --path godot res://scenes/economy_smoke_test.tscn`; record the new head-timer-size assertion message, not just a nonzero exit.
- [ ] Increase timer emphasis and distinguish the active socket using existing art/styles; retain all five sockets and truthful live progress. Do not invent an arrow asset or alter production.
- [ ] Rerun economy: exit 0 and `GRUS_ECONOMY_SMOKE_OK`; capture FIFO and overflow frames at 1280 × 720 and compare with `mockup-2.png`. Record any failure and retry separately; do not attribute a timing failure to load without evidence.
- [ ] Commit the task; obtain the required independent task review.
- [ ] Run all seven Godot headless smokes (`smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`) and require exit 0 and their respective success markers. Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, and `git diff --check` for this pass. Preserve any failures and retries in the evidence notes.
- [ ] On the final implementation head, run `GRUS_CAPTURE_DIR=<absolute-output-dir> godot --path godot res://scenes/<scene>.tscn` without `--headless` for `smoke_test`, `combat_lifecycle_smoke_test`, `economy_smoke_test` and `underground_smoke_test`. Keep the window visible: an occluded window has previously stalled `frame_post_draw`. Verify 1280 × 720 output and capture markers for Start, real Pause, real Defeat, labelled Victory presentation probe, queue FIFO/overflow, and real L3/dig plus labelled synthetic L4 frames. Headless runs do not capture frames.
- [ ] Compare those frames directly with `.superpowers/sdd/2026-11-09-hpa-698-frostline-hud/mockup-{1,2,3,4}.png`; update the evidence README with capture origin, commit, hashes, remaining gaps and limitations. Do not describe widget-only probes as gameplay.
- [ ] Commit evidence, request a whole-branch review, address findings within scope, and update existing draft PR #24 with results and limitations; never merge.
