# Frostline Readability Follow-up Implementation Plan

> **For agentic workers:** Use subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Improve small-text legibility, reduce competing tile decoration and emphasize research progress without changing game behavior.

**Architecture:** Adjust existing Godot HUD controls and theme styles in place. Keep catalogue and snapshot data, controller-owned input, node paths and panel footprints intact; validate rendered output and real interactions together.

**Tech Stack:** Godot 4.7.1, GDScript, existing Frostline assets and smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-09-hpa-698-frostline-hud-design.md`, `docs/superpowers/specs/2026-11-15-frostline-hud-visual-polish-design.md` and `docs/superpowers/specs/2026-11-15-frostline-access-shutdown-design.md`.

## Global Constraints

- Reuse existing art, fonts, theme and controls; do not change Rust authority, rules, commands, snapshots, fixture, world models or fog.
- At 1280 × 720 preserve three complete underground rows, scrolling, clipping, usable buttons, locks, clickable unaffordable actions, accessibility, focus, tooltips and session transitions.
- Preserve camera size 54, pan/zoom/minimap, and real opening Gather, scout and Depot interactions with their existing clearance assertions.
- Retain all research values, catalogue-derived prices, full direction names and command labels; do not rely on color alone.
- Keep Linux checks waived and PR #24 draft. Preserve diagnostic and capture artifacts; label synthetic probes and disclose failures and retries.

## Review Focus

- Long research values and all milestone labels remain legible and contained without overlapping Analyze: test actual shaped text and visible bounds in the research smoke.
- Underground kind names, prices, level badges and icon areas remain distinct in armed, locked and affordable states: test containment and capture real L3.
- Normal, focused, armed, locked and unaffordable build/train tiles remain visually distinct and retain input semantics: existing real-input economy and main smokes plus focused assertions.
- Six Gather sources and scout/Depot click clearances remain accessible: existing main and economy smokes.
- Start, Pause and Result and wave warning remain correct: existing wave and lifecycle smokes.

### Task 1: Improve HUD readability and visual hierarchy

**Files:** `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`, `godot/scripts/underground_drawer.gd`, `godot/assets/ui/frostline/frostline_theme.tres`, existing tile scene only as necessary; `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/underground_smoke_test.gd` and other existing smokes as needed.

**Interfaces:** Preserve `HUD/ResearchPanel/{ResearchBar,ResearchLabel,ResearchMarkers,AnalyzeButton}`, build/train and underground paths and their snapshot/command interfaces. Existing theme variations and drawer buttons own visual states.

- [ ] Inspect current frames, existing theme/state definitions and the listed files. Choose the smallest changes that make research progress more prominent, improve the legibility of research milestones and room/command prices, and quiet competing tile decoration without weakening state cues. Keep footprints and all visible information unless a safer equivalent is demonstrated.
- [ ] Add focused regression assertions to existing smokes for the selected font sizes, actual shaped-text containment and non-overlap of research labels/Analyze and room labels/prices. Cover normal, armed, focused, locked and unaffordable tile states without removing existing real-input assertions. Run the affected smoke(s) before implementation and retain meaningful RED output.
- [ ] Apply the minimum presentation-only changes and run affected smokes until GREEN. Preserve catalogue values and all semantic, pointer and keyboard surfaces.
- [ ] Run all seven headless Godot smokes, focused shutdown regression, `cargo fmt --all -- --check` and `cargo test --workspace --locked`; retain failures and retries. Capture fresh 1280 × 720 real opening, research, selected/queued producer, armed/unaffordable command and L3 underground frames; compare them to prior captures and all four approved mockups. Label any synthetic probes.
- [ ] Record exact commands, exits, captures, findings, limitations and tradeoffs; commit focused changes.

### Task 2: Review, verify and update draft PR

- [ ] Independently review Task 1's spec compliance, visual readability and integration. Address important findings and re-review fixes.
- [ ] Independently verify relevant smokes, text bounds, capture provenance and pass-range whitespace; obtain final whole-branch review.
- [ ] Update draft PR #24 with outcomes, evidence and remaining limitations; do not mark it ready or merge.
