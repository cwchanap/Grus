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
- A selected building’s `building_snapshot` currently exposes only `queue_label` and head `queue_progress`, not the full FIFO queue. `GrusBridge.analyze(building_id)` is already capable of queuing Analyze when given a Research Center ID, so the controller will resolve that ID without requiring selection; it is **not** a bridge gap.
- `unit_view.gd` and `building_view.gd` expose `health_ratio()` on live views. Max HP, combat stats, heat contributions and carry limit exist in Rust’s catalogue but are not yet projected; `Carry` exists on worker entities but is absent from `unit_task_snapshot`. Project these read-only fields rather than hardcoding a duplicate balance table.
- HPA-44’s 33 existing 128 × 128 PNG icons live under `godot/assets/ui/icons/`. HPA-695’s raw art is under `art-src/2026-11-09/{icons,screens,ui,sprites}/`: integrate its 15 new action/status icons into the **same** icon tree at 128 × 128, while placing screen art and UI-kit panels in `godot/assets/ui/frostline/`. Do not import its building sprites.


## Layout and interaction contract (1280 × 720)

| Region | Visible content | Interaction / presentation |
| --- | --- | --- |
| Top bar | Food, Scrap, Coal, Crystals, population used/cap, heat supply/demand | Painted icon + live number; labels in tooltips. Heat supply/demand bar and cold chip shown only below full surface efficiency. The idle-worker jump stays available. |
| Upper left | Wave number/current prep, live aliens, next-wave countdown ring, incoming side | Ring progress uses Rust `seconds_remaining` and `timer_total_seconds`. Its amber warning is exactly `incoming_zones` nonempty (the sim's existing 30-second window). No duplicate `warning_seconds` projection. |
| Upper middle | Research tube, points/target, ETA, Flame Tower/Crystal Lance/wave-growth markers, Analyze | Marker thresholds come from the catalogue. Analyze works during Playing without selecting the Research Center; the existing `GrusBridge.analyze(id)` remains the command entrypoint. |
| Bottom left | Existing interactive minimap | Preserve fog, camera rectangle, wave pings, click navigation and enemy visibility; presentational reframing only. |
| Bottom centre | Selection portrait, current/max HP, worker task and carried load, training queue | View `health_ratio()` × catalogue `max_health` yields displayed current/max HP. Worker load is live `carry_amount/carry_limit` and `carry_kind`. Producer queue shows up to five FIFO sockets, head remaining time/progress, and `+N` overflow; no synthetic queues. |
| Bottom right | Contextual action card: build/train and Move/Stop/Attack-move/Gather/Rally/Cancel | Real actions only. The static 9 build and 3 train Buttons retain their IDs; visible train tiles are filtered to selected producer via catalogue `producer` (Barracks: Guard/Rifleman only). Icon + costs and normal/armed/locked/unaffordable states. |
| Tab drawer | Six room kinds, excavate row, level/slot list | Preserves 1-based level/0-based slot wire contract, row reuse, scroll, real Button clicks, disabled locks and focus/Tab/Esc behavior. |
| Session overlays | Start key art/emblem, Pause, Victory/Defeat banners | Existing session phase/winner and original Start/Resume/Restart/Quit commands drive the visuals. |

### Snapshot-driven detail and tooltip contract

- Project only the HUD-visible Rust rules (see bridge section) rather than copying balance constants into Godot. Building tooltip fields include catalogue cost/time, max HP, heat supply/demand, damage, range, and counter target when applicable. Unit/tooltips similarly show max HP, damage/range/counter where applicable; guard/rifleman tiles may use existing alien icons to illustrate their counters. `heat_supply` is a catalogue nominal value, not a promise of active furnace output; the top bar uses live `economy_snapshot`.
- A selection's **current** HP is `clampi(roundi(view.health_ratio() * max_health), 0, max_health)`; show numeric health only for a single unit/building, never invent an aggregate for a mixed selection. A missing live view/snapshot must not display stale health. Worker's empty `Carry` reports zero and no resource kind. Display carry as current amount / `carry_limit` with the kind icon when holding resources.
- The FIFO queue array is a real projection, not a second UI-side queue. Show the first five jobs in sockets, head remaining seconds derived from catalogue `units[head].seconds` and existing `queue_progress`, and a `+N` overflow chip for the rest; retain blocked reason and head status. Avoid per-frame recreation of controls.
- Research buttons remain locked until catalogue `unlock_points`. An unaffordable tile uses a different Theme variation **but stays clickable** to surface Rust's `InsufficientResources`; button disabled is reserved for real locks/non-playing. Never silently swallow Rust rejects. Short costs and tooltip wording must come from catalogue; prices/thresholds are not GDScript constants.

### Armed orders and input

Keep **one** controller-owned `_armed_order: String` rather than separate booleans: `""`, `"attack_move"`, `"move"`, `"gather"`, `"rally"`. This replaces `_attack_move_armed` (update existing smoke checks). Build placement stays the existing separate `_placement_kind` because it has its own preview/queued-placement lifecycle; don't fold it into the order state.

- Clicking a command tile arms its order; the next **right-click target** executes through the existing bridge operations. Attack-move/Move require a ground point, Gather requires a valid resource and selected workers, Rally requires a friendly producer and ground point. One armed order takes precedence over the generic contextual right-click (no accidental fallback on an invalid target) and clears on dispatch, selection/context loss, cancel, pause/Result and restart. Give the player target feedback on rejection.
- Stop (`S`) executes immediately via `_issue_stop`. Cancel (`Esc`) clears placement or the armed order **before** it can pause the game; it must not enqueue a gameplay command. Right-click with no armed order retains existing contextual Gather/Attack/ResumeConstruction/Rally/Move priorities.
- HUD derives the highlighted armed tile from controller state passed to rendering; it never owns the modal world-click state. Use the existing keyboard `A`/`S` behavior and update affected smokes; maintain Ctrl-number groups, drag-selection, Tab/Esc focus and mouse world-input gating.

### Underground ownership

The drawer owns `_ug_rooms`, `_ug_slots_per_level`, `_ug_armed_index`, `_ug_kind_buttons`, toggle callback, row construction, dynamic slot/header rendering, and its own `reset()`. It resolves the chosen room kind before emitting only two intents: `excavate_requested()` and `room_requested(level, slot, kind)`. The controller keeps `_input` Tab/Esc interception, session/playing gate, `GrusBridge.excavate/build_room` dispatch and `command_status`; restart calls `drawer.reset()`. No two owners of underground arming.

### Layout and art details

- One Godot Theme registered through `godot/project.godot` `gui/theme/custom`; **do not wrap the HUD in a new root Control** just to apply the theme, because existing `HUD/...` paths are a smoke/test contract.
- The old hardcoded `InputShield` rectangle becomes redundant when the new full-width STOP panels cover the top bar. Remove it **with the re-layout and its obsolete smoke assertion**, and verify panel clicks and world blocking instead. Every decorative full-screen effect uses mouse IGNORE.
- Keep Barlow and Barlow Condensed fonts (OFL) **bundled with their license**, and set the theme default; don't make typography optional or depend on system fonts.
- Use the 15 new icons under `godot/assets/ui/icons/{actions,status}/` at 128 × 128: downsample existing raw PNGs during import (not new art generation), or enforce `process/size_limit=128` on the Godot import if retaining full-size source. Reuse the existing `_icon_for_kind(group, kind)` cache and update `icons/README.md`. No parallel icon tree or new image generation.
- Keep the UI-kit panels/screens separately under `godot/assets/ui/frostline/{ui,screens}/`, maintaining the original raw source in `art-src/`. Set `StyleBoxTexture.region_rect` and `texture_margin_*` to exclude source transparent padding; use `axis_stretch_horizontal = TILE_FIT` for top bar repeat regions so icicles are not smeared. Action tiles use Godot Button normal/pressed/disabled plus one `theme_type_variation` for unaffordable; no custom tile state machine.
- One small static tile scene/helper is enough, with the already-shipped PNG vocabulary (plate, tile, socket, round button, glass bar). Tiny arrow, plus, cancel, restart and quit glyphs may be drawn with simple Godot shapes as explicitly allowed by HPA-698. No separate art task unless genuinely new imagery is required.
- Screen overlays use the source frost image only under cold deficit, an unobtrusive always-on vignette, and amber side glow only when the current `incoming_zones` contains N/E/S/W. All overlays fade away without obscuring readability or intercepting input.

## Minimal architecture

1. `Main` / `battlefield_controller.gd` remains the **one command and input owner**: camera, drag selection, target picking, `_armed_order`, `_placement_kind`, Ctrl-number groups, command feedback, session handling, click/keyboard paths, Tab/Esc interception and combat-effect forwarding. It already connects static build/train/session buttons directly; keep those connections, don't forward them through a long signal relay chain.
2. `godot/scripts/frostline_hud.gd` attaches directly to the current `HUD` CanvasLayer. It takes the existing controller selection/arming context plus read-only, preferably once-per-frame snapshots and renders economy, wave, research, selection and session UI. No second sim cache, autoload, event bus or authoritative validator.
3. `godot/scripts/underground_drawer.gd` attaches to the current UndergroundPanel. Its single visual/armed-state owner handles runtime-created room Buttons and emits the **two** action-intent signals above. The controller dispatches the bridge commands and keeps error/status feedback.
4. Use the existing `Main/HUD` node identities, a global Godot Theme plus one tiny reusable action tile; built-in Controls and StyleBoxTextures provide the rest. Keep the minimap's existing script/data, fog and visual interaction. No broader GUI framework.

### Small read-only Rust bridge projections (and one no-Rust Analyze change)

**Production queue:** In `crates/grus-godot/src/lib.rs::building_snapshot`, add `training_queue` with the full ordered job kind labels from `ProductionQueue.jobs`. Preserve current `queue_label`, `queue_progress`, `blocked_reason` and missing/fog-hidden behavior; an existing producer with no queue returns an empty array.

**Catalogue stat rows:** Add `max_health` and optional combat `damage`, `range`, `counter_target` to the existing `catalogue_snapshot_dict().units` loop; to `.buildings` add `max_health`, `heat_supply`, `heat_demand` and optional combat `damage`, `range`, `counter_target`. No redundant hardcoded balance. Add top-level `carry_limit` from `CARRY_LIMIT`.

**Worker carry:** Add `carry_amount` and `carry_kind` to `unit_task_snapshot` from the current worker `Carry` component (empty reports 0 + empty name). Existing task/source/building keys and hidden/unknown unit behavior remain.

**Wave/research geometry:** Add `timer_total_seconds` to `wave_snapshot`, matching the Rust initial preparation interval vs subsequent regular interval. Add `wave_growth_points` to the **static** research catalogue using `unlock_points(RESEARCH_WAVE_BOOST_PERCENT)`; two tower milestones already come from existing per-building `unlock_points`. Do **not** project `warning_seconds`: amber warning derives exactly from the already-projected `incoming_zones` list.

**Analyze without selection:** **No new bridge method.** Controller finds the live friendly Research Center view using existing `building_views` metadata (kind, team, building ID), then calls unchanged `GrusBridge.analyze(building_id)`. No hardcoded building ID or additional Rust world query. Rust still validates session, core, ownership, completion and Crystals. Missing core disables Analyze or returns a visible error; it must not pretend to succeed.

These are the complete intended **presentation-only** bridge additions. The simulation source, commands and tick order stay unchanged.

## Acceptance / test contract

- Use **one PR** for design, implementation and review revisions. All **seven existing Godot smoke journeys** must pass at **every meaningful checkpoint**, not only at the end. A pure-extraction checkpoint must pass all seven **without altering test selectors or UI appearance**; re-layout commits update selectors/assertions alongside the changed nodes. Keep existing behavior assertions, rather than deleting tests as the UI changes.
- Add bridge/GUI smoke assertions for FIFO queue (two queued jobs, head progress, remaining/overflow presentation), projected health/stats/carry, correct attack-counter icon, wave timer geometry/amber warning, the one armed-order state, context-sensitive train choices, and selection-independent Analyze including the existing zero-Crystal Rust reject. Reuse `economy_smoke_test.gd` Guard→Rifleman journey for the FIFO check, taking care to observe the queue before its head finishes at 20× simulation speed.
- Verify current/maximum HP display, paid prices and no stale state after selection/restart. Preserve fog and minimap assertions, research progress and tower unlocks, action/placement/cancel, underground Tab/Esc/focus/slot pricing, and Start/Pause/Result lifecycle. No artificial direct state changes in place of real user input.
- At default **1280 × 720**, inspect actual rendered frames for Start, warm play, cold/wave warning, selected Barracks with queue, multi-unit selection, L3 underground drawer and Result, checked against approved mockups. A headless smoke is not a substitute for these visual checks.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`, `cargo test --workspace`, a fresh Godot import and all seven smoke runs, plus existing CI/e2e/coverage gates. Only claim checks that actually ran.
- Out of scope: HPA-697 building sprites / Crystal Lance regeneration, any new image art/SFX, balance and rules, localization, responsive layout, backward compatibility scaffolding, a new GUI framework or new gameplay command variants.
