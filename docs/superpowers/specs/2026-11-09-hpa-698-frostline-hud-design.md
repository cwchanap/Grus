# HPA-698 — Frostline HUD redesign: design specification

**Status:** Draft for review (planning only; no gameplay or UI implementation in this commit).
**Linear:** [HPA-698](https://linear.app/cwchanap/issue/HPA-698/hud-redesign-frostline-look-with-the-generated-art)
**Art dependency:** HPA-695, merged as [PR #23](https://github.com/cwchanap/Grus/pull/23).
**Visual reference:** Four owner-approved mockups attached to HPA-698 (in-match placement, selected Barracks/cold/wave, underground drawer, start/style/result screens), plus `art-src/2026-11-09/BRIEF.md`.
**Execution:** This documentation draft is the beginning of the **single implementation PR for HPA-698**. All implementation, tests, screenshots and review fixes land in this PR; do not open a separate implementation PR.

## Goal and scope

Replace the current text-heavy overlay with an icon-first, 1280 × 720 “Frostline” HUD: forged dark steel and frost, painted icons, concise numeric values, hover explanations, and clearly differentiated actionable/locked/unaffordable/armed states. The complete match must remain playable from Start to Result, including the Tab underground drawer.

This is a **presentation and minimal bridge-projection change**, not a new gameplay feature. Godot may derive display-only colors, percentages and affordability hints from Rust snapshots/catalogue. Rust remains the only authority for spending, unlocks, orders, research, construction and victory. Do not change sim balance, production order, session state, wave behavior or tick order.

Excluded: HPA-697 painted building sprites, new art generation or regeneration, new SFX, new economic rules, a broader HUD framework, localization, responsive redesign for other aspect ratios, and new build/train actions. In particular, the Crystal Lance sprite correction belongs to the separately tracked art work for HPA-697, not this PR.

## Verified implementation baseline

- `godot/scenes/main.tscn` owns `Main/HUD` (CanvasLayer) and the existing names: `CommandStatus`, `WaveLabel`, `ResearchPanel/{ResearchBar,ResearchLabel,ResearchIcon,AnalyzeButton}`, `EconomyPanel`, `SelectionPanel`, `CommandPanel/{BuildGrid,TrainGrid}`, `UndergroundPanel`, `SessionPanel`, `Minimap`, `PauseButton`. Existing smoke scenes navigate these paths; preserve important action/control paths where practical.
- `godot/scripts/battlefield_controller.gd` (~1,330 lines) currently combines camera/selection/placement/key input, all HUD rendering, Tab drawer state, and bridge command dispatch. Extract visual responsibilities without moving authoritative selection/placement/command state out of this controller.
- Static rules and prices come from `GrusBridge.catalogue_snapshot()`. Live `economy_snapshot()`, `wave_snapshot()`, `research_snapshot()`, `building_snapshot(id)`, `unit_task_snapshot(id)`, `underground_snapshot()`, and `session_snapshot()` are already available. The minimap consumes the **same** wave snapshot as the HUD.
- The selected building’s `building_snapshot` currently exposes only `queue_label` and head `queue_progress`, not the full queue. `GrusBridge.analyze(building_id)` currently requires a building id. These are the two intentional bridge behavior gaps called out by HPA-698.
- `unit_view.gd` and `building_view.gd` already expose `health_ratio()` on the live view nodes. Worker task details are available from `unit_task_snapshot`; do not invent raw HP, combat orders or state unavailable through these surfaces.
- HPA-44’s 33 existing runtime PNG icons are already under `godot/assets/ui/icons/`. HPA-695’s raw art is under `art-src/2026-11-09/{icons,screens,ui,sprites}/`; this PR imports only the first three groups. Do not import its building sprites into gameplay.

## Layout and interaction contract (1280 × 720)

| Region | Visible content | Interaction / presentation |
| --- | --- | --- |
| Top bar | Food, Scrap, Coal, Crystals, population used/cap, heat supply/demand | Painted icon + live value, no constantly repeated resource names; names and explanations in tooltips. Keep idle-worker jump accessible. A heat meter shows supply relative to demand and a cold chip appears only when surface efficiency is below 1.0. |
| Upper left | Wave number/current prep, live aliens, next-wave countdown ring, warning-side chip | The ring uses Rust-sourced total interval and seconds remaining; amber treatment in the final Rust-defined warning window. Incoming sides use only `incoming_zones`; no hidden-alien or guessed wave data. |
| Upper middle | Research progress tube, current/target points, ETA, milestones and Analyze | Mark Flame Tower and Crystal Lance from catalogue `buildings.*.unlock_points` and the late-wave growth threshold from Rust catalogue data. Show nonzero-progress without rounding up to 100%; ETA is an estimate. Analyze is actionable during Playing without selecting Research Center, with Rust handling affordability. |
| Bottom left | Existing interactive minimap | Preserve fog, camera rectangle, click navigation, enemy visibility and wave pings; reframe visually without changing its data source or parent path. |
| Bottom centre | Selection portrait, health bar, current worker task or producer queue | One selected unit/building uses the existing kind icon and `health_ratio()`; worker task comes from `unit_task_snapshot`. A selected producer shows the full FIFO queue with head progress and blocked state. Multi-selection shows type/count and does not pretend there is one shared HP/task. |
| Bottom right | Command card, nine existing build actions and three train actions | Use icon tiles with small catalogue-sourced costs; locked, unaffordable, selected/armed and normal states are distinct. Context-sensitive presentation may hide irrelevant train choices, but must keep all existing accessible commands and real press paths. |
| Tab drawer | Six underground room choices, excavate row, level/slot rows and progress | Keep 1-based levels, 0-based wire slots, true catalogue cost/time, current arm state, disabled shallow/occupied slots, one-click command dispatch and focus/Tab/Esc behavior. |
| Lifecycle overlay | Start key art + emblem; paused controls; victory/defeat banner | Respect `session_snapshot().phase` and winner; use the existing Start/Resume/Restart/Quit controls, no extra game state. |

**Action card decisions:** Reuse the already-shipped build/train buttons, key shortcuts and contextual right-click movement/gather/rally rather than adding a second order implementation. Stop and attack-move can be presented as explicit action tiles only when wired to their existing controller command/arm paths. A visible tile may not be decorative or trigger a different gameplay action. Keep status/reject feedback readable outside hover-only text.

**Affordability:** Compare a tile’s catalogue cost to the one live economy snapshot for styling; a cosmetic “can't afford” state is not authoritative validation. Preserve Rust command rejection and the visible feedback channel for stale resource changes. Research-locked actions remain non-interactive until their catalogue threshold is reached. Armed placement/attack-move is visually distinct and Esc cancellation is unchanged.

**Underground focus:** The controller continues to intercept Tab and Esc in `_input` before Godot’s focus navigation; drawer buttons keep mouse STOP, and clicking empty drawer space cannot issue world commands. Opening, closing and reset must clear stale visual armed states consistently with the controller’s command state.

## Minimal architecture

1. Keep `Main` / `battlefield_controller.gd` as the **input and command owner**: camera, drag selection, control groups, placement preview, keyboard, context actions, bridge command calls, status feedback and combat-event forwarding. Pass already-fetched snapshot values/selected IDs to the HUD; do not introduce a second cache of gameplay state or another autoload.
2. Attach a focused `godot/scripts/frostline_hud.gd` to the existing `HUD` CanvasLayer. It owns the theme, static catalogue-bound tile setup, snapshot-to-control rendering, selection layout, wave/research/economy indicators, and SessionPanel presentation. It emits signals for actionable UI presses, handled by the controller; the HUD does not enqueue gameplay commands itself.
3. Extract the underground **visual construction and updates only** into `godot/scripts/underground_drawer.gd` on the existing UndergroundPanel. It keeps its current row reuse/per-tick refresh, index rules, tooltips and buttons. It emits excavate / build-room intents; the controller owns dispatch and visible feedback. Do not introduce a generic UI store or event bus.
4. Build one `godot/assets/ui/frostline_theme.tres` (or equivalent Godot Theme) from the new UI-kit textures. Reuse Godot Panel/StyleBoxTexture, TextureRect, Button, ProgressBar and a small command-tile helper or scene only where repeated behavior justifies it. The reusable visual vocabulary is **panel, action tile, socket, round button and progress tube**; not five new independent widget systems. Prefer nine-slice edges and capped icons rather than stretching a whole PNG into every aspect ratio.
5. Copy only the needed HPA-695 icons, screens and UI-kit PNGs into a clearly named `godot/assets/ui/frostline/` subtree; retain HPA-44 icons for resources/buildings/units/underground. Use Barlow/Barlow Condensed with the OFL license included if distributing font files; do not rely on a developer’s installed fonts. No runtime asset generation, downloaded dependencies or image editing inside this code ticket.
6. One full-screen non-interactive overlay handles soft vignette, optional cold-edge frost from `ui/frost_overlay.png`, and amber incoming-side glow. Its mouse filter must IGNORE input; edges should fade without obscuring wave, research, buttons or status. Use existing wave zones/efficiency only; simple ColorRect/shader tint is enough.

### Small, read-only bridge additions

**Full producer queue:** In `crates/grus-godot/src/lib.rs::building_snapshot`, serialize every `ProductionQueue.jobs` entry in FIFO order into `training_queue` (array of kind names/labels from existing Rust production kind helpers), in addition to the current head progress and blocked reason. Empty/no-queue buildings return an empty array; missing/fog-hidden buildings still return an empty snapshot. The frontend never reconstructs a queue from the head.

**Global Analyze button:** Expose a no-argument `GrusBridge.analyze()` that finds the unique owned Research Center **on the Rust bridge side** and queues the existing `PlayerCommand::Analyze { issuer: TeamId(1), building: ... }`. The existing simulation validator still checks ownership, completion, match phase and Crystals. No selected building ID, new PlayerCommand variant or resource-spending code in Godot. A missing core should not produce a fabricated successful queue result.

**Exact timer/milestone geometry:** Expose the current wave timer’s applicable total duration and warning threshold as read-only fields on `wave_snapshot` (derived from the existing Rust preparation/interval/warning constants), and the 75% wave-growth milestone point threshold in the static research catalogue. This avoids hardcoding gameplay duration/threshold numbers in GDScript just to draw the ring and marker. No gameplay or tick changes.

## Acceptance / test contract

- In the default **1280 × 720** desktop viewport, all seven Godot smoke journeys still complete using actual button/input paths. Verify full match Start → Playing → Paused/Resume → Result → Restart, including the new screen art. Take actual screenshots of in-match warm, cold/wave warning, producer selected, underground L3/scroll, Start and Result.
- Surface indicators display exact snapshot amounts; cold and amber overlays appear/disappear on real snapshot transitions. The wave ring and ETA do not falsely freeze or reset. Locked/unaffordable/armed tile styles change correctly as resources, selection and research progress change.
- Training queue is FIFO, includes all queued jobs, and retains correct head progress/blocked feedback. No selected Research Center is required for Analyze; a zero-Crystal click still yields Rust’s `InsufficientResources` feedback with no resource or research gain. Pause/Result disallow commands.
- Preserve existing smoke assertions for resource semantics, research milestones, minimap fog, building/training press behavior, underground Tab/Esc/focus/price/room slot logic, and session transitions. Update tests to the new appearance/structure deliberately; do **not** delete or bypass journey assertions because a node moved.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`, `cargo test --workspace`, a fresh Godot import and all seven headless smokes. Keep the existing repo CI/e2e/coverage gate.
- Do not claim visual approval, game playthrough, or passing tests until they are actually run. The draft remains open for implementation/review rather than being treated as a completed feature.
