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
- Start, Pause and both Result variants: readable controls, correct visibility and real transitions without invented results (Task 2).
- FIFO head, empty queue and overflow: timer/progress match snapshot and remain visible without overlapping adjacent controls (Task 3).

### Task 1: Underground readability

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`; test `godot/scripts/underground_smoke_test.gd`.

**Interfaces:** Preserve `HUD/UndergroundPanel/ScrollContainer/LevelsBox`, `refresh(snap: Dictionary, phase: String)`, `excavate_requested`, `room_requested(level: int, slot: int, kind: String)`, catalogue wire names, 1-based levels and 0-based slots. The drawer's `_snapshot` remains the rendered readout source.

- [ ] Strengthen `_kind_layout_error` and `_slot_readout_error` to require kind names at least 12 px and kind costs and completed-room benefits at least 11 px, while retaining text-width and containment checks. Add checks that lock requirements and accessible names remain present; use existing real L3 and labelled synthetic L4 long-name probes. Keep existing assertions for armed/locked/occupied/construction states, dig zero/partial/completion, three full rows, real wheel, pause and command-card clicks.
- [ ] Run `godot --headless --path godot res://scenes/underground_smoke_test.tscn` and record the expected new font-size failure before implementation.
- [ ] Simplify competing wide-slot plates and adjust spacing, contrast and type to pass the new checks. Keep the excavation readout outside the clipped viewport and sourced from `snap`; use no new data or game rules. Do not reduce the three-row viewport to fit larger text.
- [ ] Rerun the same smoke: exit 0 and `GRUS_UNDERGROUND_SMOKE_OK`. Capture real L3/dig/construction and labelled synthetic L4 frames at 1280 × 720; inspect their names, prices, locks and benefits against `mockup-3.png`.
- [ ] Commit the task; obtain the required independent task review before moving on.

### Task 2: Start and Result hierarchy

**Files:** Modify `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`; test `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Preserve `HUD/SessionPanel/{StartButton,ResumeButton,RestartButton,QuitButton,SessionLabel,ControlsLabel}`, `HUD/SessionBackdrop/{Dim,ResultBanner,ResultTitle}`, `HUD/ResultBar`, and `refresh_session(phase: String, winner_team: int)`. Retain the existing documented control instructions as visible, accessible text.

- [ ] Add Start assertions that visible control-instruction text is at least 13 px, fits its allocated region without clipping or overlapping Start, and retains all documented instructions. Add Result assertions that the backdrop dim alpha is at most 0.55, both existing actions are visible, inside the viewport and non-overlapping, and their real Restart/Quit wiring and phase gating are retained. Retain separate Victory/Defeat title and real Defeat/Restart tests; the Victory probe stays labelled presentation-only.
- [ ] Run `godot --headless --path godot res://scenes/smoke_test.tscn` and `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn`; record the expected new assertion failures.
- [ ] Reflow the existing controls strip for legibility, keeping all instructions and phase behavior. Adjust the Result dim and existing banner, frost and action styling to strengthen their hierarchy without changing the world or displayed data. Use existing theme/assets only.
- [ ] Rerun both smokes: exit 0 with `GRUS_GODOT_SMOKE_OK` and `GRUS_COMBAT_LIFECYCLE_SMOKE_OK`. Capture Start, real Defeat and labelled Victory presentation probe; inspect at 1280 × 720 against `mockup-4.png` and verify text is readable over the lighter backdrop.
- [ ] Commit the task; obtain the required independent task review before moving on.

### Task 3: Queue emphasis and final verification

**Files:** Modify `godot/scripts/frostline_hud.gd` and `godot/scenes/main.tscn` only if needed; test `godot/scripts/economy_smoke_test.gd`; update `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/` with fresh captures and provenance.

**Interfaces:** Preserve five `HUD/SelectionPanel/QueueSockets` sockets, each `Icon`, `Time`, `Progress`, the `OverflowChip`, and snapshot-backed `_render_queue(building: Dictionary)`. Existing smoke capture calls are in `godot/scripts/smoke_helpers.gd` and the four journey scripts.

- [ ] In the real Guard→Rifleman FIFO and six-enqueue overflow journeys, require a visible head timer of at least 14 px, contained text and progress, visibly distinct head/queued/empty states, and no overlap with the overflow chip or adjacent controls. Retain checks that timer/progress use the real queue and that blocked, empty and overflow behavior remain correct.
- [ ] Run `godot --headless --path godot res://scenes/economy_smoke_test.tscn`; record the expected new head-timer-size failure.
- [ ] Increase timer emphasis and distinguish the active socket using existing art/styles; retain all five sockets and truthful live progress. Do not invent an arrow asset or alter production.
- [ ] Rerun economy: exit 0 and `GRUS_ECONOMY_SMOKE_OK`; capture FIFO and overflow frames at 1280 × 720 and compare with `mockup-2.png`. Record any failure and retry separately; do not attribute a timing failure to load without evidence.
- [ ] Commit the task; obtain the required independent task review.
- [ ] Run all seven Godot headless smokes (`smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`) and require exit 0 and their respective success markers. Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, and `git diff --check` for this pass. Preserve any failures and retries in the evidence notes.
- [ ] On the final implementation head, capture fresh real-render 1280 × 720 Start, real Defeat, labelled Victory presentation probe, queue FIFO/overflow, and real L3/dig plus labelled synthetic L4 frames. Compare directly with `.superpowers/sdd/2026-11-09-hpa-698-frostline-hud/mockup-{1,2,3,4}.png`; update the evidence README with capture origin, commit, hashes, remaining gaps and limitations. Do not describe widget-only probes as gameplay.
- [ ] Commit evidence, request a whole-branch review, address findings within scope, and update existing draft PR #24 with results and limitations; never merge.
