# HPA-44 — HUD and underground icons implementation plan

**Linear:** [HPA-44](https://linear.app/cwchanap/issue/HPA-44/grus-survival-art-hud-and-underground-room-icons)
**Spec:** [HPA-44 design](../specs/2026-11-05-hpa-44-hud-underground-icons-design.md)
**Branch / PR:** `hpa-44-survival-ui-icons`; **one draft PR becomes the eventual implementation PR**. Do not open a second PR for implementation or review fixes.
**State:** Planning only. Do not claim icon assets, visual QA or runtime tests have passed before implementation.

## Current codebase findings and hazards

- `godot/scenes/main.tscn`: `BuildGrid` has nine structure Buttons; `TrainGrid` has Worker/Guard/Rifleman; all keep `metadata/kind`. `EconomyPanel/EconomyLabel`, `WaveLabel`, `ResearchPanel`, `UndergroundPanel/ExcavateButton` and `KindsBox` are the existing hooks.
- `godot/scripts/battlefield_controller.gd`: `_ready()` binds button handlers from catalogue; `_refresh_hud()` owns snapshot-derived text; `_refresh_underground_panel()` creates/reuses row Buttons and respects stable six-kind catalogue ordering. Attach icon textures to those Buttons instead of duplicating the palette.
- `economy_smoke_test.gd` explicitly requires `EconomyLabel as Label`, its **three-line** text layout, bounded panel geometry and currency/heat formatting. `wave_smoke_test.gd` requires existing `WaveLabel as Label` text; `underground_smoke_test.gd` depends on `KindsBox` child indexes and button focus/lock semantics. Keep those checks green.
- All gameplay tickets, including HPA-288/HPA-289, are Done; this ticket is unblocked. The current code has no dedicated icon assets.

## Checkpoint 1 — Declare icon inventory and add failing presentation checks

**Files:** `godot/scripts/{smoke_test,economy_smoke_test,wave_smoke_test,underground_smoke_test}.gd`.

- [ ] Establish the 33-icon source inventory in the spec and a small README checklist under `godot/assets/ui/icons/`. No Rust enums or catalogue data are changed.
- [ ] Extend the **existing** smoke paths: assert texture exists on all nine build and three train Buttons, six dynamic kind Buttons and Excavate Button; assert HUD icon nodes/textures and incoming visibility state. Keep checks focused on required nodes, displayed state, and viewport bounds rather than pixel-comparison snapshots.
- [ ] Prove initial icon assertions fail on the current placeholders and keep existing economy/wave/underground behavior assertions intact; do not start an eighth CI smoke scene.

## Checkpoint 2 — Author/import the separate art assets

**Files:** `godot/assets/ui/icons/{resources,status,underground,buildings,units}/*.svg`, `godot/assets/ui/icons/README.md`, optional necessary Godot `.svg.import` settings.

- [ ] Create the 33 **original** self-contained SVGs with 24 × 24 logical canvas, limited winter palette and silhouettes legible at 16–20 px. Distinguish Food/Scrap/Coal/Crystal/Heat, four HUD states, all six rooms/Excavate, ten structures and seven units by shape (not color alone).
- [ ] Keep runtime input as source SVG; no icon atlas, generated assets checked in without their sources, downloaded licensed art, or plugin dependency. Record art provenance and a one-page icon inventory in README.
- [ ] Run a clean Godot 4.7.1 editor import (`godot --headless --path godot --editor --quit-after 120`) and resolve any SVG import failures before wiring. Visually inspect the actual imported images at target size on light/dark surfaces.

## Checkpoint 3 — Wire existing Buttons, with no gameplay changes

**Files:** `godot/scripts/battlefield_controller.gd`, `godot/scenes/main.tscn`.

- [ ] Use an explicit, small immutable mapping keyed by existing `metadata/kind` / `room_kinds` names and resource paths. Load once at scene setup; assign `Button.icon` to nine BuildGrid and three TrainGrid Buttons, preserving all metadata, tooltips, names and callbacks.
- [ ] Assign icons to catalogue-ordered `KindsBox` Buttons and the existing Excavate Button. Keep their text, selection/armed, min-depth/disabled and row-reuse behavior; adjust icon/label spacing in the narrow 420 px panel only as required.
- [ ] Commit all ten building/seven unit assets, **but do not add actions** for Research Center or four non-trainable alien units.

## Checkpoint 4 — Surface resource, wave, research and cold icons

**Files:** `godot/scenes/main.tscn`, `godot/scripts/battlefield_controller.gd`, focused existing smoke assertions.

- [ ] Add bounded decorative `TextureRect` icon nodes/containers for Food, Scrap, Coal, Crystals, Heat and surface-cold in `EconomyPanel`. **Preserve `EconomyLabel` as a Label and its three-line format**; leave `economy_snapshot()` as the single value source. Adjust only cosmetic spacing/bounds to avoid overlap with the Idle Button and Selection Panel.
- [ ] Add the wave countdown and incoming-zone icon near `WaveLabel`, and research-progress icon alongside the existing research panel. Preserve `WaveLabel.text` strings; toggle only the incoming icon based on the already-read `wave_snapshot().incoming_zones`. Keep ResearchBar, Analyze, ETA and current UI/input shielding unchanged.
- [ ] Keep the icons static after import (except visibility/tint from the current snapshot). No new timers, GDExtension methods, tick-driven asset loading, or second HUD state model.

## Checkpoint 5 — Verify and document on **this PR**

**Files:** Existing four smoke scripts above, `README.md` if icon integration changes controls/appearance, icon README; no simulation modules.

- [ ] At 1280 × 720, manually inspect live screenshot(s) in Playing, Paused, cold deficit, incoming warning and underground open: readability at normal scale, no panel or battlefield occlusion, light/dark contrast and 16–20 px distinguishability. At 1920 × 1080, confirm no clipped controls. Attach/describe **actual** evidence in the PR, not inferred visuals.
- [ ] Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, and the existing 90% simulation coverage gate as unchanged safety gates.
- [ ] Stage a freshly built GDExtension; import Godot from a clean cache; run all seven existing headless scenes: `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`. Check exit 0 plus each scene's existing success marker.
- [ ] Let existing CI verify Linux Godot export, exported-game E2E and 200-unit benchmark; do not add new performance gates or rewrite CI just for artwork.
- [ ] Confirm the diff contains no changes under `crates/`. Review that button input, text contracts, room-kind order, unlocks and tooltips are identical, and that none of the 33 files is missing.
- [ ] Commit implementation, review fixes and evidence to this **same** branch/PR; mark ready only when assets and checks are finished.

## Acceptance checklist for the eventual PR

- [ ] 33 Godot-importable SVG icons with source/provenance.
- [ ] Existing nine build / three train / six room-kind / Excavate buttons iconified without behavior change.
- [ ] Five economy/heat and four status icons visible in the relevant HUD state; live text retains the existing semantics.
- [ ] Icon-focused assertions in existing smoke suites and full CI green.
- [ ] Real visual inspection documented. No Rust/sim/bridge changes.
