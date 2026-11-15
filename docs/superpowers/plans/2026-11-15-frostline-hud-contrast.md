# Frostline HUD Contrast Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the underground room readouts, Pause Resume action and active queue portrait easier to see at 1280 × 720.

**Architecture:** Adjust the existing Godot controls and theme overrides, retaining snapshot-driven content, existing node identities and input handlers. Check real rendered frames as well as focused layout and state assertions.

**Tech Stack:** Godot 4.7.1, GDScript, existing Frostline theme, art and smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-15-frostline-hud-visual-polish-design.md`.

## Global Constraints

- Reuse existing art, fonts, theme and Godot controls; do not add assets or fabricate gameplay data.
- Do not change Rust authority, commands, rules, world models or snapshots.
- At 1280 × 720 preserve three complete visible underground rows, scrolling, clipping, accessibility, focus and usable commands.
- Preserve Start/Pause/Result transitions, selection, input, ordered queue, timer, progress and overflow.
- Record failures and retries; label presentation-only Victory and synthetic deeper-level probes. Keep PR #24 draft; Linux checks are waived.

## Review Focus

- A room's name, benefit and construction percentage remain legible without overlap, including the synthetic long-name and L4 benefit probes.
- Locked, occupied and actionable slots remain distinguishable by shape and text, not color alone.
- Resume remains accessible by its existing input paths and the compact Pause card follows viewport resizing and restores other session phases.
- Active portraits are distinguishable without obscuring the head timer or progress bar, and queued/empty sockets and the six-job overflow remain contained.
- Accessible queue names, image alternatives and selection-clearing behavior are not lost; no screen-reader result is claimed without a screen-reader run.

### Task 1: Improve three existing visual treatments

**Files:** Modify `godot/scripts/underground_drawer.gd`, `godot/scripts/frostline_hud.gd` and `godot/scenes/main.tscn` only as needed. Extend existing tests in `godot/scripts/underground_smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, and `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Keep `refresh_session(phase: String, winner_team: int)`, `_render_queue(building: Dictionary)`, existing room/queue nodes and all controller connections unchanged. Continue to use the existing live snapshots and catalogue.

- [ ] Add focused assertions for the chosen readout contrast treatment, prominent Resume styling and active portrait visibility while retaining every existing geometry, content, accessibility and input assertion. Include a populated-queue selection-loss check for the prior deferred gap if it can be exercised through the existing real-input journey.
- [ ] Run the relevant headless smokes and record expected failures that are assertions, not startup errors.
- [ ] Make the smallest existing-style adjustments: increase room-name and benefit legibility without shrinking/clipping content, make Resume a clear primary action, and make the head portrait more visible while retaining readable timer/progress. Do not alter Start, Result or the world.
- [ ] Run the affected headless smokes and fresh windowed captures of underground, real Pause and FIFO/overflow. Compare 1280 × 720 frames with the previous visual review and mockups 2–3. Explicitly label synthetic L4/long-name probes and document every failure, timeout and retry.
- [ ] Commit the implementation and tests. Write a report with exact RED/GREEN commands, screenshot provenance, visual comparison, limitations and commit.

### Task 2: Final verification and evidence

**Files:** Add fresh captured evidence and a concise comparison under `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/`; update its index as needed.

**Interfaces:** Use existing `SmokeHelpers.capture_frame` journeys and record the implementation head for every capture. Never label an image from a failed journey as a successful run.

- [ ] Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, all seven headless Godot smoke scenes listed in `CLAUDE.md`, and `git diff --check` for this pass.
- [ ] Capture and inspect final-head 1280 × 720 frames for underground, Pause and FIFO/overflow. Preserve earlier frames, failures, retries, screenshot timestamps, hashes and probe labels. If a journey times out, record that and do not claim it passed.
- [ ] Commit evidence; obtain independent whole-branch review and address critical/important findings.
- [ ] Update existing draft PR #24 with verified results and unresolved issues; do not mark it ready or run Linux checks.
