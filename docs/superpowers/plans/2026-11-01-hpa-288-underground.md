# HPA-288 — Underground levels and rooms: implementation plan

Status: **Revised after GitHub design review; implemented on this branch — required CI evidence pending** · 2026-11-02  
Issue: [HPA-288](https://linear.app/cwchanap/issue/HPA-288/grus-survival-78-underground-levels-and-rooms)  
Design: [HPA-288 underground spec](../specs/2026-11-01-hpa-288-underground-design.md)  
Parent: [Winter Survival §8](../specs/2026-10-21-winter-survival-design.md)  
Starting baseline: `main` after merged [PR #15 (HPA-188)](https://github.com/cwchanap/Grus/pull/15)

## Delivery contract

**One ticket → one PR.** This planning draft is the eventual implementation PR for HPA-288. Add the implementation, tests, docs, CI evidence and review fixes to this branch/PR rather than opening a new implementation PR. HPA-289 (wave benchmark/balance) remains blocked until this feature is complete. Leave artwork and SFX work to their existing separate tickets.

Implement the smallest model that fits the current authoritative Rust/Bevy ECS simulation; there is no need for underground entities, a second game scene, saved-state abstraction, asynchronous workers, or new generic command/production framework.

## Checkpoint 1 — Typed catalogue, state and deterministic excavation (Rust)

**Touch**: `crates/grus-sim/src/catalog.rs`, `catalog/tests.rs`, `src/economy.rs`, `src/research.rs` (one existing helper visibility change), add `src/underground.rs` + `src/underground/tests.rs`, `src/lib.rs`.

- [ ] Add six `RoomKind` variants and one `RoomSpec` table containing kind minimum level, cost, build duration, and resource/population/heat/research effect. Commit provisional prices/timers from the design spec as clearly editable catalogue values; no copied numeric configuration in systems/Godot.
- [ ] Add excavation constants (initial 100 Scrap, x1.6 depth multiplier, 50% Coal after integer Scrap rounding, 30 s base +15 s/level, four slots) and one `excavation_cost_and_seconds(level) -> (Cost, u32)` helper. Use positive `f64::round() as u32` (saturating conversion), `scrap.div_ceil(2)`, and saturating duration arithmetic; use a depth exponent that handles the full `u32` range. Test L1–L5 exact table and one very-deep, practically unaffordable case. No `Option`/unavailable-price branch or overflow subsystem.
- [ ] Add `Underground: Resource`, paid `Excavation` state, `UndergroundLevel` and `Room`. Model four optional slots per level; depth derives from `levels.len()` (no duplicate depth counter). Use `f32` progress with production's snap-to-required `PROGRESS_EPSILON`, and a separate `f64` per-room resource remainder with a tiny heat-style epsilon for whole-unit extraction. Default is empty.
- [ ] Extract saturating `ResourceStockpile::add(ResourceKind, amount)` from `economy.rs::deposit_carry` and reuse it for both worker deposits and room credits—one shared ResourceKind→stockpile match.
- [ ] **Before implementing or compiling `step_underground`**, change the *existing* `research::living_research_core(world)` visibility to `pub(crate)` in `research.rs` (no new helper). This keeps the first checkpoint independently buildable; Checkpoint 2 will reuse the same function for commands.
- [ ] Add `step_underground(world, seconds)`: gate by existing session phase and shared living-core helper; **credit only rooms complete at tick entry**, then advance one dig and independent room timers. This prevents credit on a room's first completion tick; the next tick produces. Assert no output in Start/Paused/Result, invalid `seconds`, or before construction finishes.
- [ ] Re-export only useful `underground` types/functions through `lib.rs`—no broad generic abstraction.

**Proof**: small pure Rust tests for cost rounding and saturation, production-style timer boundary (0.05 s ticks), paid progress, L5+ availability, no repeated completion, shared stockpile saturating add, fractional output at 0.05 s and larger test deltas, and freeze/resume.

## Checkpoint 2 — Route and validate real player commands (Rust)

**Touch**: `crates/grus-sim/src/commands.rs`, `underground.rs`, commands/underground tests; `research.rs` already exposes its existing helper from Checkpoint 1.

- [ ] Add `PlayerCommand::Excavate { issuer }` and `BuildRoom { issuer, level, slot, kind }`, dispatched through `apply_player_command`. Preserve the global session gate. **No BuildingId in Excavate**: it is a global panel action and the Research Center is already the uniquely seeded core.
- [ ] Reuse the **one already-`pub(crate)`** `research::living_research_core(world)` check from Checkpoint 1 across both commands and `step_underground`. Both commands allow only Team 1 with its living core; in-session destroyed-core action is already `SessionLocked`. No further helper visibility change, Research Center lookup/snapshot/core-ID round-trip or incomplete-core test matrix.
- [ ] Excavate: reject while another dig is in progress; compute next depth and saturating plain price/duration; `try_spend` atomically **once**; start the timer without immediately exposing a level.
- [ ] BuildRoom: the wire-level `level` is **1-based** (`0`/missing level → `OutOfBounds`); `slot` is **0-based** (`0..=3` valid, `4+` → `OutOfBounds`). Taken slot → existing `Occupied`. Kind below its `min_level` → `Locked`. `try_spend` once before writing state. Allow concurrent independent room builds and room building during deeper excavation.
- [ ] Append **only** `ExcavationInProgress` after current `RejectReason::Unexplored`; reuse `Occupied`, `OutOfBounds`, `Locked`, `InsufficientResources` elsewhere. Numeric codes remain stable. Existing bridge feedback formats rejection enum names generically—**no mapping change**.

**Proof**: two commands in the same tick cannot double-charge or occupy the same slot/dig; Team 2 and missing core reject without spend; level 0, level beyond current depth and slot 4 reject; L3 kinds locked on L2; full L1 has no fifth room; accepted requests charge exactly once.

## Checkpoint 3 — Integrate population, heat, research, seed and tick (Rust)

**Touch**: `crates/grus-sim/src/{tick.rs,production.rs,heat.rs,research.rs,fixture.rs,lib.rs}`, `crates/grus-godot/src/lib.rs` (reset cleanup), adjacent feature/bridge tests, `crates/grus-sim/tests/system_order.rs` and new `crates/grus-sim/tests/underground_journey.rs`.

- [ ] Insert `Underground::default()` in `seed_skirmish`, and explicitly `remove_resource::<Underground>()` in `crates/grus-godot/src/lib.rs::clear_gameplay_world` next to heat/research. Extend the **existing** bridge reset/restart tests; benchmark seed still follows the shared fixture path.
- [ ] Add completed Shelter capacity to Team-1 `population_cap`; sum with completed surface capacity and clamp at `MAX_POPULATION`. Tests cover under-construction, two Shelters, a saturated cap, foreign team.
- [ ] Sum completed Geothermal Tap supply into `step_heat` with L3=15, L4=20, L5=25. Never burn Coal for taps or add underground heat demand. Tests cover unfinished, surface furnace fuel state and heat-deficit improvement.
- [ ] Add completed Labs to `research_rate(world)` **outside** the existing surface-efficiency product, preserving the single research/ETA source and Victory logic. Test +1/+2 with full heat and 25% floor cold; incomplete Labs add zero.
- [ ] Change only the canonical `step_tick` order to **waves → underground → heat → combat → movement → economy → construction → production → research → visibility**. Extend existing system-order tests to prove a completed Tap affects same-tick heat and completed Lab affects same-tick research, while lethal Research Center combat still wins over same-tick research Victory.

**Feasibility gate before the Godot smoke**: make `underground_journey.rs` seed the ordinary `MapFixture::battlefield()`/ `seed_skirmish`, assign Workers via **real** `PlayerCommand::Gather` as in `tests/economy_flow.rs`, and execute the canonical `step_tick`. Initial 300 Scrap/150 Coal plus L1–L3 excavation and both L3 rooms (Tap 120 + Lab 150) require **786 Scrap / 258 Coal total**: at least **486 extra Scrap / 108 extra Coal** must be gathered. Serial digging plus the two concurrent L3 room timers imposes a **165 simulated-second floor**, with first wave launching at **300 seconds**. Assert both rooms complete strictly before wave 1 **and record a concrete positive margin in simulated seconds**. If margin is insufficient with four starting Workers, train extra Workers through the existing real `EnqueueUnit` command (200 starting Food is enough for four). Do **not** use stockpile injection or loosen the pre-wave smoke guard. Use the observed journey to choose the smoke's 45 s or 90 s CI timeout.

**Proof**: paid L3 Tap+Lab through authoritative commands, heat/research hooks observable, positive measured pre-wave margin; Restart returns to default. No manually copied FixedUpdate schedule.

## Checkpoint 4 — Thin Godot bridge and functional placeholder panel

**Touch**: `crates/grus-godot/src/lib.rs`, `godot/scenes/main.tscn`, `godot/scripts/battlefield_controller.gd`, applicable bridge parser/e2e tests.

- [ ] Add parameterless `GrusBridge.excavate()` and `build_room(level, slot, kind)` through existing `queue_command` and the revision/status feedback. No snapshot core ID; the bridge supplies Team 1. Parse room kind via `RoomKind::ALL` and existing `kind_name_str`; invalid inputs fail safely. All state mutations stay in Rust.
- [ ] Add static `catalogue_snapshot().underground`: slot count and room catalogue (`min_level`, typed costs, seconds and benefits). Do not reimplement excavation formulas or room-effects tables in GDScript.
- [ ] Add `underground_snapshot()` dynamic read: completed depth; current dig target/progress/remaining; Rust-computed next dig's plain price/time; levels with explicit **1-based `level`** and four **0-based slots**, each with kind, build progress/completion. Keep it read-only/default-safe, with **no core-ID field or unavailable-price state**.
- [ ] Add a scrollable panel to `main.tscn`. One **room-kind Button per kind** in a shared selection row, enabled once completed depth reaches its catalogue `min_level` and showing cost/effect in tooltip; clicking selects/arms it. One row per completed **1-based** level with four ordinary **0-based** slot Buttons. **Without an armed kind, all empty slots are disabled. With one armed, enable only empty slots whose row's `level >= selected_kind.min_level`, and only while Playing; disable too-shallow targets with a visible `Requires L<n>` explanation.** Clicking an eligible slot queues `build_room`; occupied/constructing slots are always non-actionable. Changing the armed kind recomputes target eligibility from existing static catalogue + dynamic snapshot; do **not** add selected-level state or let UI checks replace the Rust `Locked` gate. Display next dig price/time/progress plus a plain **Excavate** Button. **No PopupMenu, separate window or new UI control abstraction.**
- [ ] Route Tab in `battlefield_controller.gd::_input` and call `get_viewport().set_input_as_handled()`: this must toggle even when a prior button click left GUI focus. In the **same `_input` handler**, Esc closes an open panel and marks it handled; when closed, preserve existing Esc-to-cancel/pause in `_unhandled_input`. Set the panel's `mouse_filter = STOP` so clicks do not leak to the battlefield without bespoke guards.
- [ ] Read static catalogue once in `_ready`; fetch dynamic underground only while panel is visible, and rebuild rows only on depth/slot-kind changes. Disable mutation outside Playing but allow inspection during pause, without a core-ID branch; Rust is final validator. Show queue and reject messages via existing HUD `CommandStatus`.

**Proof**: bridge input parser/serializer tests, Godot 1280×720 visual/readability inspection, and catalogue-driven lock/tooltips. **The automated Tab-after-click and Esc-while-open regression assertions belong in Checkpoint 5**, not an untestable manual-only promise.

## Checkpoint 5 — End-to-end smoke, documentation and CI

**Touch**: standalone `godot/scenes/underground_smoke_test.tscn` and `godot/scripts/underground_smoke_test.gd` (reuse `smoke_helpers.gd`), `.github/workflows/ci.yml` (**required** twice), `README.md`, `CLAUDE.md`.

- [ ] **Only after the Checkpoint 3 gather-funded Rust L3 journey proves pre-wave feasibility**, add the real Godot smoke: open Tab; press/click a panel button; press **Tab again** to confirm the focused-button case toggles; reopen; press **Esc** and confirm it closes without pausing; repeat with real L1 excavation and assert pause freezes its timer. Gather the extra 486 Scrap/108 Coal (or the measured amount net of real unit spending) using actual Worker Gather commands. Excavate to L3. **With L1–L3 present, arm Lab: assert L1/L2 empty slots are disabled and clicks queue/spend nothing while an L3 empty slot accepts a Lab; arm Shelter and assert an L1 empty slot becomes enabled again** (do not add another menu/selected-level state). Build a Tap and Lab through real room-kind/slot Buttons, assert **+15 heat supply** for a L3 tap and **+1 research/s** beyond the same heat-scaled base; check snapshot row indices, queue/reject feedback, and Restart clears depth. Keep the Rust direct below-minimum-level `Locked` rejection test independent of this UI regression.
- [ ] Keep the smoke within the 300 simulated-second prep guard (`smoke_helpers.gd::no_launched_wave_error`) and use existing 20× speed. Determine a **45 s or 90 s CI timeout** from measured feasibility and Godot elapsed runtime, not a guess. Use a real UI click path via `smoke_helpers.gd::push_mouse_click`, not a fabricated popup/window interaction or test-only resource injection.
- [ ] Register the new smoke in **both** `.github/workflows/ci.yml` locations: a dedicated headless step with **exit-code AND sentinel grep** checks, and the `Capture teardown backtrace` scene list. It is not optional—CI currently enumerates smoke scenes manually.
- [ ] Preserve existing smokes, e2e and benchmark workload. **Do not** replace/rebalance the benchmark—HPA-289 owns that.
- [ ] Update README: Tab controls, excavation formula/table, room choices, resource/heat/research effects, pause/restart and no worker/cold requirements. Update `CLAUDE.md` module list, bridge methods, smoke commands and canonical tick order. Check UI help text.
- [ ] Record measured test/CI evidence and any real performance impact in **this same PR**. Keep unchecked gates open until actually verified.

## Validation matrix

| Scenario | Rust tests | Godot |
| --- | --- | --- |
| L1–L5 exact prices, durations, saturation, atomic spend | Required | L1 real spend |
| 1-based level/0-based slot bounds, duplicate purchase, min-level unlocks, timers | Required | L3 room build |
| 0.5/0.4 unit/s fractional stockpile output, freeze | Required | Representative snapshot |
| Shelter population up to MAX_POPULATION | Required | Snapshot when relevant |
| Geothermal Tap depth bonus, heat efficiency | Required | L3 +15 heat |
| Lab additive research independent of cold | Required | L3 +1 research/s |
| Session lifecycle, core loss, Restart, tick ordering | Required | Pause + Restart |
| Tab after focused click, Esc while open, mouse shielding | — | Required |
| Lab armed at L3 disables L1/L2 targets; Shelter re-enables L1 | Direct `Locked` validation | Required (no queue/spend on disabled slots) |
| L3 Tap+Lab with real gathering before 300-s wave prep | Required feasibility gate | Required after Rust passes |

### CI / local acceptance (collect actual results, do not pre-check)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1`
- [ ] `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'`
- [ ] Pinned Godot 4.7.1 import; all existing headless smoke scenes plus new underground smoke (exit 0 and required sentinel)
- [ ] Linux x86_64 GDExtension export, packaged smoke and exported-game `bevy-e2e`
- [ ] Existing `GRUS_BENCHMARK_SECONDS=10` CI benchmark workload stays green
- [ ] Manual check at 1280×720: panel usable at multiple depths, mouse input captured, Tab/Esc behavior, costs/feedback correct, pause and Restart intuitive
- [ ] `README.md`, `CLAUDE.md`, and PR body updated with **observed** verification (not assumed success)

## Review focus

1. Is a one-resource/four-slots-per-level model sufficient, without underground entities or per-room queues?
2. Does `step_underground` before `step_heat` provide intuitive same-tick Tap/Lab effects without changing existing heat/combat correctness?
3. Are room costs/build durations sensible as **provisional** values, with final tuning deliberately deferred to HPA-289?
4. Does the global Tab UI handle Tab after a focused button and Esc while open, and disable only per-kind ineligible target slots without a selected-level state or Research Center ID round-trip?
5. Does the **Rust** gather-funded L3 journey prove a positive margin before wave 1, and does the **Godot** smoke exercise the same real control/resource path within its measured CI timeout?

**Completion bar:** Gameplay is genuinely playable through L3; heat and research hooks are observable; the complete automated checks run; there is no follow-on "finish HPA-288" PR.
