# Battlefield Access and Shutdown Implementation Plan

> **For agentic workers:** Use subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Improve opening-view target accessibility and identify and address the observed audio shutdown warnings without changing gameplay.

**Architecture:** Adjust existing presentation geometry and test actual input against the authored fixture. Diagnose Godot audio teardown independently and only implement a minimal supported fix once reproduced and understood.

**Tech Stack:** Godot 4.7.1, GDScript, existing Rust simulation and smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-15-frostline-access-shutdown-design.md`.

## Global Constraints

- Preserve Rust authority, commands, models, fixture, fog and rules; no new artwork.
- Preserve pan, zoom limits, minimap, selection, placement, HUD tooltips, focus, accessibility and session behavior.
- Preserve three complete underground rows, clipping, scrolling and commands at 1280 × 720.
- Retain logs and captures, label synthetic probes and disclose failures and retries.
- Linux checks are waived; keep PR #24 draft.

## Review Focus

- Starting core, workers and safe sources remain visible and usable, not just their centers at a fractional pixel.
- The existing northwest scout and Depot placement are reachable with practical clearance and real input.
- Wave readouts, warnings, tooltips, session states and command/drawer controls do not clip or intercept the world unexpectedly.
- Shutdown changes do not suppress diagnostics or compromise audible cues, pause/resume, restart or ordinary quit.
- Repeated and verbose checks distinguish actual audio teardown from unrelated leaks; no fixed delay is treated as proof by itself.

### Task 1: Opening target accessibility

**Files:** `godot/scenes/main.tscn`; existing HUD/controller scripts only if necessary; `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/underground_smoke_test.gd` as needed.

**Interfaces:** Keep `Camera3D`, existing HUD node paths, authored fixture, 54.0 camera size, zoom limits and player input handlers.

- [ ] Add real-input assertions that the starting core/workers and all live safe-source targets are visible and selectable/commandable at the default view; verify the existing scout and Depot scenario with at least 8 px clearance from HUD edges, without test-only zoom. Derive fixture targets from live views or existing fixture APIs, not new coordinate tables.
- [ ] Run an affected smoke and capture the expected RED for the actual occlusion/clearance.
- [ ] Find the smallest camera-position and/or existing HUD geometry adjustment that passes without hiding other starting targets or clipping wave text, warnings and tooltips. Preserve pan/zoom/minimap and real placement/gather paths; do not make HUD click-through.
- [ ] Run all seven headless smokes and fresh 1280 × 720 rendered comparisons of the opening view and relevant command states; record actual margins, changes and remaining tradeoffs. Commit and report all failures/retries.

### Task 2: Audio shutdown

**Files:** `godot/scripts/combat_fx.gd`, `godot/scripts/battlefield_controller.gd`, existing smoke helpers/scenes only as justified by diagnosis.

**Interfaces:** Preserve `play_cue`, `set_wind_phase`, `clear_all` and existing session transitions.

- [ ] Trace the normal quit, smoke quit and engine shutdown paths; reproduce leaked instance types with verbose output and test one hypothesis at a time. Baseline diagnostic logs are under `.superpowers/diagnostics/2026-11-15-shutdown/`.
- [ ] Add a failing real shutdown regression that checks actual process output and preserves failure evidence; distinguish absent diagnostics from missing tests or disabled audio.
- [ ] Implement the smallest root-cause fix supported by reproducible evidence; preserve cue playback and user-visible quit behavior. If no reliable fix can be established, document the investigation rather than hiding output or inventing certainty.
- [ ] Run repeated relevant verbose smokes, combat lifecycle and shutdown checks; record every exit, diagnostic and retry. Commit any verified fix separately.

### Task 3: Final verification

**Files:** Update `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/` as appropriate.

- [ ] Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, all seven headless smokes, and `git diff --check` for this pass.
- [ ] Capture and inspect final-head 1280 × 720 views and disclose any synthetic state. Preserve earlier evidence.
- [ ] Obtain independent task and whole-branch reviews; fix important findings, then update the existing PR without marking it ready.
