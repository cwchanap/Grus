# Frostline HUD Visual Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clarify the drawer and Pause presentation, then refine the other reviewed HUD surfaces without gameplay changes.

**Architecture:** Adjust existing Godot theme overrides and layout using the existing snapshot-driven controls. Exercise actual input and inspect real 1280 × 720 captures; synthetic evidence is explicitly labeled.

**Tech Stack:** Godot 4.7.1, GDScript, existing Frostline theme and smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-15-frostline-hud-visual-polish-design.md`.

## Global Constraints

- Reuse existing art, fonts, theme and Godot controls; no fabricated statistics, city, or gameplay Victory.
- Do not change Rust authority, commands, rules, world models or snapshots.
- At 1280 × 720 preserve three complete visible underground rows, scrolling, clipping, accessibility, focus and usable commands.
- Preserve Start/Pause/Result transitions, selection and real input.
- Record failures and retries; label presentation-only Victory and synthetic deeper-level probes. Keep PR #24 draft; Linux checks are waived.

## Review Focus

- A locked or occupied slot remains identifiable without depending on color alone: check labels and disabled states in the drawer smoke.
- Opening the drawer while paused still permits inspection but no purchase: retain the paid-journey freeze and disabled-click checks.
- Keyboard focus and Esc still close the drawer or resume as appropriate: retain existing real-input checks and exercise Resume.
- Session geometry resets between Start, Pause and Result: verify transitions through existing session smokes.
- A populated six-job queue still projects five sockets and overflow: retain the live snapshot-bound overflow checks.

### Task 1: Drawer states and Pause composition

**Files:** Modify `godot/scripts/underground_drawer.gd`, `godot/scripts/frostline_hud.gd`, `godot/scenes/main.tscn` as needed. Test `godot/scripts/underground_smoke_test.gd`, `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd` as appropriate.

**Interfaces:** Keep named existing nodes, controller signal connections and `refresh_session(phase: String, winner_team: int)` unchanged. Drawer continues to consume the existing Rust snapshots.

- [ ] Add focused failing assertions for compact, centered Pause geometry and usable Resume, and visually distinct locked, occupied and actionable drawer treatments without suppressing state text; retain the existing geometry, focus, paused inspection and paid journey assertions.
- [ ] Run affected headless smoke(s) and record the expected assertion failures, not startup errors.
- [ ] Implement the smallest theme/layout changes: quiet repeated slot frame detail, distinguish the three states, and center a compact Pause composition. Restore Start/Result geometry and behavior on transitions; do not alter control names or commands.
- [ ] Run the three affected headless smokes and a real-render underground smoke with 1280 × 720 captures. Inspect drawer states and real Pause against prior captures and approved mockup 3. Record failures/retries and exact provenance.
- [ ] Commit changes and provide report with commands, RED/GREEN evidence, captures, limitations and commit.

### Task 2: Queue, Start and Result visual refinement

**Files:** Modify `godot/scripts/frostline_hud.gd` and `godot/scenes/main.tscn` as needed. Test `godot/scripts/economy_smoke_test.gd`, `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Preserve Task 1's session layout transitions, existing queue node names, snapshots, five sockets, FIFO, overflow and the capture helper. No new gameplay data.

- [ ] Add focused failing checks for the chosen queue separation or Start/Result layout improvement while retaining existing FIFO, overflow, containment, input and session assertions.
- [ ] Run affected headless smoke(s) to verify the intended RED assertion.
- [ ] Make the smallest useful existing-art refinements to active queue portrait separation, Start cues and Result composition. Avoid unrelated redesign or new assets; if a surface cannot be improved without violating constraints, document that instead of fabricating data.
- [ ] Run economy, general and combat headless smokes and capture real-render FIFO/overflow, Start, real Defeat and presentation-only Victory at 1280 × 720. Compare against mockups 2 and 4 and record all failures/retries and limitations.
- [ ] Commit changes and provide report with commands, RED/GREEN evidence, captures, limitations and commit.

### Task 3: Final verification and evidence

**Files:** Add fresh capture evidence under `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/`; update its README as needed.

**Interfaces:** Use the existing opt-in `SmokeHelpers.capture_frame` and smoke journeys. Screenshots must correspond to the final implementation head.

- [ ] Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, all seven Godot headless smoke scenes listed in `CLAUDE.md`, and `git diff --check` for this pass; retain exact results and diagnostics.
- [ ] Capture and inspect fresh 1280 × 720 frames for drawer, Pause, queue, Start and Result, comparing with approved mockups and prior frames; label synthetic probes and preserve failed attempts/retries.
- [ ] Commit evidence with provenance, honest limitations and remaining gaps; obtain a final independent whole-branch review and address critical/important findings.
- [ ] Push the reviewed branch and update PR #24 while keeping it draft; disclose skipped Linux and unresolved timing/teardown observations.
