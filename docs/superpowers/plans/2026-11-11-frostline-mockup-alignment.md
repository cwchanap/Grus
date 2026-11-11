# Frostline Mockup Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring the four identified HUD regions closer to the approved mockups without changing gameplay.

**Architecture:** Adjust the existing Godot scene and HUD presentation script, using the current theme, assets, snapshots, and controller-owned commands. Preserve named controls and the drawer's existing scrolling and input ownership.

**Tech Stack:** Godot 4.7.1, GDScript, existing Rust bridge and headless Godot smoke scenes.

**Spec:** `docs/superpowers/specs/2026-11-11-frostline-mockup-alignment-design.md`

## Global Constraints

- Preserve Rust gameplay authority, button identities, real-input behavior, FIFO and overflow, drawer scrolling and focus, and Start/Pause/Result transitions.
- Do not fabricate victory or statistics, add art or gameplay rules, or rerun the user-waived Linux checks.
- Compare fresh real-rendered 1280 × 720 frames with the approved mockups and keep existing smoke assertions.

## Review Focus

- Paused and Result overlays must not expose or intercept the wrong controls; exercise real Pause/Resume and Restart.
- Victory presentation must use the settled winner; retain the presentation-only disclosure when gameplay Victory is not achieved.
- Queue overflow, blocked state, FIFO order, head timer and progress must survive enlarged sockets; retain the six-enqueue and Guard/Rifleman journeys.
- The drawer must keep all levels reachable and command controls usable with rows populated; exercise real scrolling and drawer-open clicks.
- Long labels and viewport edges must not clip or obscure other controls; assert bounds and inspect 1280 × 720 frames.

### Task 1: Start and Result presentation

**Files:** `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`, `godot/scripts/smoke_test.gd`, `godot/scripts/combat_lifecycle_smoke_test.gd`.

**Interfaces:** Preserve `HUD/SessionPanel/{StartButton,ResumeButton,RestartButton,QuitButton}` and `refresh_session(phase: String, winner_team: int)`; use only currently available snapshot data for any Result statistics.

- [ ] Add assertions for a larger visible Start title/emblem and a bottom Result bar with distinct, reachable Restart and Quit controls; retain actual Defeat and both-winner presentation checks.
- [ ] Run the affected Godot smokes and observe the expected failures before changing presentation.
- [ ] Re-layout the Start title/emblem and the Result bar using existing theme/art; show only existing snapshot-derived stats and retain phase-specific behavior and mouse routing.
- [ ] Run `smoke_test.tscn` and `combat_lifecycle_smoke_test.tscn` with exit 0 and their `GRUS_*_SMOKE_OK` markers; capture and inspect real-rendered Start and Defeat frames.
- [ ] Commit the task.

### Task 2: Production queue presentation

**Files:** `godot/scenes/main.tscn`, `godot/scripts/frostline_hud.gd`, `godot/scripts/economy_smoke_test.gd`.

**Interfaces:** Keep five reusable `HUD/SelectionPanel/QueueSockets` FIFO sockets, head progress/remaining time, and the existing `+N` overflow chip driven by `training_queue` and catalogue data.

- [ ] Add assertions that socket/icon sizes are visibly larger, fit within the selection panel without overlap, and preserve all six real enqueues, FIFO, head and overflow checks.
- [ ] Run `economy_smoke_test.tscn` and observe the size assertion fail.
- [ ] Re-layout and style the sockets using the existing assets, preserving empty, blocked, head and overflow states.
- [ ] Run `economy_smoke_test.tscn` with exit 0 and `GRUS_ECONOMY_SMOKE_OK`; capture and inspect real-rendered Barracks and overflow frames.
- [ ] Commit the task.

### Task 3: Underground viewport and final QA

**Files:** `godot/scenes/main.tscn`, `godot/scripts/underground_drawer.gd`, `godot/scripts/underground_smoke_test.gd`, `docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/`.

**Interfaces:** Keep `HUD/UndergroundPanel/ScrollContainer/LevelsBox`, drawer signals, 1-based levels, 0-based slots, and controller-owned commands unchanged.

- [ ] Add assertions for at least three complete visible level rows at 1280 × 720, a clipped viewport clear of command controls, real scroll to deeper levels, and a real command click with populated rows and the drawer open.
- [ ] Run `underground_smoke_test.tscn` and observe the viewport assertion fail.
- [ ] Reflow the drawer's existing controls and rows to meet those assertions without obscuring Analyze or the command card; retain access to every level and slot.
- [ ] Run all seven existing Godot smoke scenes with exit 0 and explicit success markers; run `cargo fmt --all -- --check` and `cargo test --workspace --locked`. Do not run Linux checks.
- [ ] Capture and inspect final real-rendered 1280 × 720 Start, queue, underground and Result frames; update the evidence README and screenshots, clearly labelling any Victory presentation-only probe.
- [ ] Commit the task, review the full branch, and push the existing PR branch with accurate verification notes.
