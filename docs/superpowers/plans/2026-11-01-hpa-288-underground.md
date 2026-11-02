# HPA-288 — Underground levels and rooms: implementation plan

Status: **Proposed; implementation not started** · 2026-11-01  
Issue: [HPA-288](https://linear.app/cwchanap/issue/HPA-288/grus-survival-78-underground-levels-and-rooms)  
Design: [HPA-288 underground spec](../specs/2026-11-01-hpa-288-underground-design.md)  
Parent: [Winter Survival §8](../specs/2026-10-21-winter-survival-design.md)  
Starting baseline: `main` after merged [PR #15 (HPA-188)](https://github.com/cwchanap/Grus/pull/15)

## Delivery contract

**One ticket → one PR.** This planning draft is the eventual implementation PR for HPA-288. Add the implementation, tests, docs, CI evidence and review fixes to this branch/PR rather than opening a new implementation PR. HPA-289 (wave benchmark/balance) remains blocked until this feature is complete. Leave artwork and SFX work to their existing separate tickets.

Implement the smallest model that fits the current authoritative Rust/Bevy ECS simulation; there is no need for underground entities, a second game scene, saved-state abstraction, asynchronous workers, or new generic command/production framework.

## Checkpoint 1 — Typed catalogue, state and deterministic excavation (Rust)

**Touch**: `crates/grus-sim/src/catalog.rs`, `catalog/tests.rs`, add `src/underground.rs` + `src/underground/tests.rs`, `src/lib.rs`.

- [ ] Add six `RoomKind` variants and one `RoomSpec` table containing kind minimum level, cost, build duration, and resource/population/heat/research effect. Commit provisional prices/timers from the design spec as clearly editable catalogue values; no copied numeric configuration in systems/Godot.
- [ ] Add excavation constants (initial 100 Scrap, x1.6 depth multiplier, 50% Coal after integer Scrap rounding, 30 s base +15 s/level, four slots) and one `excavation_cost_and_seconds(level)` helper. Test L1–L5 exact table and checked high-level/unaffordable behavior.
- [ ] Add `Underground: Resource`, paid `Excavation` state, `UndergroundLevel` and `Room`. Model four optional slots per level, elapsed construction and per-room fractional remainder; derive depth from `levels.len()`. Default is empty.
- [ ] Add `step_underground(world, seconds)`: gate by existing session phase and living core; advance one dig and independent room timers; complete levels/rooms once; credit fractional resource outputs only for rooms already completed at tick entry. Assert no output in Start/Paused/Result, invalid `seconds`, or before construction finishes.
- [ ] Re-export only useful `underground` types/functions through `lib.rs`—no broad generic abstraction.

**Proof**: small pure Rust tests for cost rounding, paid progress, L5+ availability, no impossible free excavation, no repeated completion, fractional output at 0.05 s and larger test deltas, and freeze/resume.

## Checkpoint 2 — Route and validate real player commands (Rust)

**Touch**: `crates/grus-sim/src/commands.rs`, `research.rs` only if reusing its living-core helper, `underground.rs`, commands/underground tests.

- [ ] Add `PlayerCommand::Excavate { issuer, building }` and `BuildRoom { issuer, level, slot, kind }`, dispatched through `apply_player_command`. Preserve the global session gate.
- [ ] Validate Team 1 and a completed living Research Center; use the named BuildingId validation chain for Excavate. No need to select the Research Center to use the Tab panel; the bridge snapshot will provide the core ID.
- [ ] Excavate: reject while another dig is in progress; compute next depth and checked price/duration; `try_spend` atomically **once**; start the timer without immediately exposing a level.
- [ ] BuildRoom: reject non-existent/incomplete levels, invalid slot, occupied slot, kind below its `min_level` and insufficient Scrap; `try_spend` once before writing the room state. Allow separate room constructions in parallel and room building during a deeper excavation.
- [ ] Append `ExcavationInProgress`, `UndergroundLevelMissing` and `RoomSlotOccupied` **after** existing reject variants; reuse current generic rejects otherwise. Update bridge rejection-label mapping if it has an exhaustive match. Preserve all existing numeric codes.

**Proof**: commands submitted twice in the same tick do not double-spend, wrong team/core/missing/unfinished levels return the expected reason without state change, slot 0–3 valid and slot 4 rejected, L3 kinds locked at L2, a full L1 cannot accept room five.

## Checkpoint 3 — Integrate population, heat, research, seed and tick (Rust)

**Touch**: `crates/grus-sim/src/{tick.rs,production.rs,heat.rs,research.rs,fixture.rs,lib.rs}`, adjacent feature tests, `crates/grus-sim/tests/system_order.rs`.

- [ ] Insert `Underground::default()` in `seed_skirmish`; prove bridge restart/reset/benchmark reseeding already clears it via the shared fixture path.
- [ ] Add completed Shelter capacity to Team-1 `population_cap`; sum with completed surface capacity and clamp at `MAX_POPULATION`. Tests cover under-construction, two Shelters, a saturated cap, foreign team.
- [ ] Sum completed Geothermal Tap supply into `step_heat` with L3=15, L4=20, L5=25. Never burn Coal for taps or add underground heat demand. Tests cover unfinished, surface furnace fuel state and heat-deficit improvement.
- [ ] Add completed Labs to `research_rate(world)` **outside** the existing surface-efficiency product, preserving the single research/ETA source and Victory logic. Test +1/+2 with full heat and 25% floor cold; incomplete Labs add zero.
- [ ] Change only the canonical `step_tick` order to **waves → underground → heat → combat → movement → economy → construction → production → research → visibility**. Extend existing system-order tests to prove a completed Tap affects same-tick heat and completed Lab affects same-tick research, while lethal Research Center combat still wins over same-tick research Victory.

**Proof**: complete simulated journey through L3 with paid excavation, paid Tap+Lab, then authoritative heat/research snapshots/rates; Restart returns to default. No manually copied FixedUpdate schedule.

## Checkpoint 4 — Thin Godot bridge and functional placeholder panel

**Touch**: `crates/grus-godot/src/lib.rs`, `godot/scenes/main.tscn`, `godot/scripts/battlefield_controller.gd`, applicable bridge parser/e2e tests.

- [ ] Add `GrusBridge.excavate(research_center_id)`, `build_room(level, slot, kind)` through `queue_command` and the existing revision/status feedback. Unknown kind and invalid negative arguments fail safely; all stockpile/slot decisions stay in Rust.
- [ ] Add static `catalogue_snapshot().underground`: slot count and room catalogue (`min_level`, typed costs, seconds and benefits). Do not reimplement excavation formulas or room-effects tables in GDScript.
- [ ] Add `underground_snapshot()` dynamic read: living core ID; completed depth; current dig target/progress/remaining; Rust-computed next-level price/time; levels and four slots with kind/build progress/completion. Keep it read-only, with empty/default output before fixture setup.
- [ ] Add simple scrollable underground panel to `main.tscn`. Tab toggles visibility; Esc closes the panel before the existing pause shortcut. One row per level with four slot buttons; empty slots open available-kind menu, in-progress slots show countdown/progress, completed ones show the benefit. Always show next dig price/time and an Excavate action.
- [ ] Read static catalogue once in `_ready`; fetch dynamic underground snapshot only while the panel is visible, and build/rebuild rows only when depth/slot contents change. Disable mutations outside Playing and while no core exists. Allow display during pause. Ensure input is consumed by panel instead of leaking to ground selection, placement or unit commands.
- [ ] Show the queued/rejected action in the existing HUD `CommandStatus`. No new standalone scene, new renderer, icon art or signal-bus framework.

**Proof**: bridge input parsing/serializer cases, Godot 1280×720 visual/readability inspection, Tab focus/close behavior, and correct UI lock text sourced from catalogue.

## Checkpoint 5 — End-to-end smoke, documentation and CI

**Touch**: focused `godot/scenes/underground_smoke_test.tscn` + `godot/scripts/underground_smoke_test.gd` (reuse smoke helpers), `.github/workflows/ci.yml` only if needed to register the focused smoke, `README.md`, `CLAUDE.md`.

- [ ] Add a bounded real Godot smoke that opens Tab, queues a paid L1 dig, observes a frozen timer while paused, resumes, then funds L2/L3 through real worker Gather commands (initial stockpile alone is insufficient). Construct a Tap and Lab via actual bridge/UI commands and assert **+15 heat supply** for L3 and **+1 research/s** beyond the same heat-scaled surface rate. Assert populated level/slot snapshots and normal feedback. Finally Restart and verify zero depth.
- [ ] Prefer a standalone small smoke over growing the already-large main/economy smoke, but reuse their helper utilities and 20× simulation speed. No test-only stockpile mutations, skipping construction, timing wall-clock sleeps or direct assignment to the sim resource. Keep runtime within CI's existing smoke budget.
- [ ] Keep the existing smokes/e2e/benchmark passing. Do **not** replace or rebalance the benchmark—HPA-289 does that.
- [ ] Update README: Tab controls, excavation formula/table, room choices, resource/heat/research effects, pause/restart and no worker/cold requirements. Update `CLAUDE.md` module list, bridge methods and canonical tick order. Check UI help text and smoke entry list.
- [ ] Add test evidence and any measured performance impact to **this same PR**. Don't mark unchecked gates complete before running them.

## Validation matrix

| Scenario | Rust tests | Godot |
| --- | --- | --- |
| L1–L5 exact prices, durations, overflow, atomic spend | Required | L1 real spend |
| Slot bounds, duplicate purchase, min-level unlocks, room timers | Required | L3 room build |
| 0.5/0.4 unit/s fractional stockpile output, freeze | Required | Representative snapshot |
| Shelter population up to MAX_POPULATION | Required | Snapshot when relevant |
| Geothermal Tap depth bonus, heat efficiency | Required | L3 +15 heat |
| Lab additive research independent of cold | Required | L3 +1 research/s |
| Session lifecycle, core loss, Restart, tick ordering | Required | Pause + Restart |
| Panel input and visibility without battlefield leakage | — | Required |

### CI / local acceptance (collect actual results, do not pre-check)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
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
4. Does the global Tab UI work without selecting the Research Center, while all commands still require a living Team-1 core?
5. Does the Godot smoke exercise real controls/resources with deterministic short waits rather than test-only bypasses?

**Completion bar:** Gameplay is genuinely playable through L3; heat and research hooks are observable; the complete automated checks run; there is no follow-on "finish HPA-288" PR.
