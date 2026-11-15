# Frostline Battlefield Framing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the real starting base closer on screen and reduce visual competition from command-tile frames, while retaining gameplay and camera controls.

**Architecture:** Adjust the initial orthographic camera size in the existing Godot scene; make scoped theme/style changes to the existing command controls. Keep game data, world models and input ownership unchanged.

**Tech Stack:** Godot 4.7.1, GDScript, existing Frostline theme and smoke journeys.

**Spec:** `docs/superpowers/specs/2026-11-15-frostline-framing-design.md`.

## Global Constraints

- Reuse existing art, fonts, theme and Godot controls; no fabricated city, statistics or gameplay Victory.
- Do not change Rust authority, commands, rules, snapshots, world-model size or fixture content.
- Preserve pan, zoom limits, picking, minimap, placement, focus, accessibility, session transitions and every real command. Unaffordable actions stay clickable; true locks remain disabled.
- At 1280 × 720 retain three complete visible underground rows, scrolling, clipping and usable commands.
- Record failures/retries and label synthetic probes; keep PR #24 draft; Linux checks are waived.

## Review Focus

- The closer default view keeps the actual Research Center and starting workers selectable through real input, and players can zoom out to the former size and beyond.
- Camera pan/zoom and minimap recenter remain aligned with world picking and placement, including the prior maximum and minimum zoom limits.
- Normal, hover, pressed, armed, locked, unaffordable and focused command tiles remain distinguishable, with prices, labels and tooltips intact.
- An unaffordable action still reaches the existing Rust rejection; a disabled lock still cannot queue a command.
- Layout changes do not cause command/drawer overlap, pointer leakage into the world, queue clipping or loss of keyboard/accessibility semantics.

### Task 1: Closer default battlefield framing

**Files:** Modify `godot/scenes/main.tscn` and, only as needed for correct initial framing, `godot/scripts/battlefield_controller.gd`. Test `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd` and other real-input smokes if affected.

**Interfaces:** Keep the `Camera3D` node path, `MIN_CAMERA_SIZE = 24.0`, `MAX_CAMERA_SIZE = 120.0`, `ZOOM_STEP = 6.0`, and existing pan/minimap/pick handlers. Do not change fixture coordinates or model transforms.

- [ ] Add a focused real-input assertion for an initial camera size of `54.0`, visible/selectable starting core and workers, and wheel zoom-out to at least the prior `72.0` view. Preserve existing zoom, selection, placement, minimap and drag assertions.
- [ ] Run the relevant smoke and record the intended RED against the current `72.0` default.
- [ ] Set the initial orthographic size to `54.0`; adjust initial camera positioning only if needed to keep the actual starting core and workers usable above the HUD. Preserve zoom limits and user control.
- [ ] Run all seven headless Godot smokes. Capture the real starting fixture at 1280 × 720 before/after with the same camera position, orientation and UI state but different orthographic sizes, plus a real zoom-out to `72.0`; compare relative screen-space size and visibility without implying extra buildings or expanded fog-of-war.
- [ ] Commit changes and report exact RED/GREEN, captures, provenance, failures/retries and usability tradeoffs.

### Task 2: Quiet command-panel tiles

**Files:** Modify `godot/scripts/frostline_hud.gd`, `godot/scenes/main.tscn` and/or existing `godot/assets/ui/frostline/frostline_theme.tres` only as needed. Test `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/underground_smoke_test.gd` and `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Keep all existing `HUD/CommandPanel/{BuildGrid,TrainGrid,OrderGrid}` paths, button metadata, signal connections, controller state and catalogue-driven costs/locks.

- [ ] Add focused assertions for the quieter command-tile treatment and visibly distinct armed, unavailable and focus states; preserve existing price, label, geometry, focus, clipping and real-input tests.
- [ ] Run affected headless smoke(s) and record the expected assertion RED, not a startup failure.
- [ ] Reduce repeated frame competition using existing theme/art while retaining icon visibility, text/price contrast, accessible names, focus and distinct actionable/locked/unaffordable/armed states. Do not silently disable unaffordable actions or alter their reject path.
- [ ] Run all seven headless Godot smokes; capture real normal/armed/locked/unaffordable command states and an open drawer at 1280 × 720. Compare with mockups 1–3 and the prior frame, recording failures/retries and limits.
- [ ] Commit changes and report exact RED/GREEN, screenshots and provenance, failures/retries and remaining visual gaps.

### Task 3: Final verification and evidence

**Files:** Add capture evidence under `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/`; update its index.

**Interfaces:** Use the existing opt-in capture helper and real smoke journeys. Each published capture must identify its implementation head and any synthetic content.

- [ ] Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, all seven headless Godot smoke scenes listed in `CLAUDE.md`, and the pass-range `git diff --check`.
- [ ] Capture and inspect final-head 1280 × 720 default and old-zoom views, command states and drawer; compare with the approved mockups without presenting a different game state as a controlled camera comparison.
- [ ] Record exact source heads, hashes, limitations and all failures/retries; commit evidence and obtain final independent whole-branch review.
- [ ] Update existing PR #24 while keeping it draft; disclose skipped Linux checks and unresolved timeout/teardown observations.
