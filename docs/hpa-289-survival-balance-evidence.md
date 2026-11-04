# HPA-289 — Survival balance evidence

Status: **engineering evidence recorded; owner playtest and several Linux-side gates NOT RUN / BLOCKED.** This document records only what was actually measured, at the revision where it was measured, and labels everything else honestly. Pending verification (the controller's final gate run and future CI runs on this branch) can update the sections marked below.

Spec: [`docs/superpowers/specs/2026-11-03-hpa-289-survival-benchmark-balance-design.md`](superpowers/specs/2026-11-03-hpa-289-survival-benchmark-balance-design.md) §§4–6.

## 1. Research arithmetic (theoretical — not a feasibility or affordability result)

Current catalogue values (`crates/grus-sim/src/catalog.rs`, unchanged by this pass):
`RESEARCH_TARGET_POINTS = 9_000`, `RESEARCH_BASE_PER_SECOND = 2.0` at full
`surface_efficiency`, each completed underground **Lab** adds `research_per_second = 1.0`,
and each accepted **Analyze** converts `ANALYZE_COST` = 10 Crystals into
`ANALYZE_RESEARCH_GAIN` = +90 points.

Idealized passive-research times at full heat, **no Analyze**:

| Idealized case | Points/s | Earliest theoretical time |
|---|---:|---:|
| Zero Labs | 2 | 4,500 s / 75 min |
| One Lab active from t=0 | 3 | 3,000 s / 50 min |
| Two Labs active from t=0 | 4 | 2,250 s / 37.5 min |
| Two Labs completed at t=180 s | 2 then 4 | 2,340 s / 39 min |

The **38–45 minute** goal (2,280–2,700 s) corresponds to an *average* of
**3.33–3.95 points/s** once Analyze gains are included. This table is arithmetic
only: it assumes Labs already exist and heat stays at 1.0. Real excavation
prices, Scrap/Coal cost, cold ramp-down, Crystal drops and Analyze timing decide
whether any row is reachable — **no normal-match progression run has measured
that** (see §7).

Cross-check used by the guard: unattended passive research cannot beat the
survival window, since `9,000 / 2.0 = 4,500 s > wave_deadline(40) = 4,200 s`
(`wave_deadline(w) = 300 + 100 × (w − 1)`; `WAVE_PREPARATION_SECONDS = 300`,
`WAVE_INTERVAL_SECONDS = 100`).

## 2. Measurement provenance

Every figure below was captured on the macOS workspace host
`Darwin Chans-MacBook-Pro-3.local 25.6.0 arm64` (Apple Silicon), Godot
`4.7.1.stable.official.a13da4feb`, Rust 1.95.0 pinned toolchain. The commit each
measurement ran at is stated explicitly; current HEAD (`ee6a32a`) differs from
some of them but no catalogue or gameplay-impacting change landed between the
latest verification commits and HEAD — tasks 3–4 touched only
`fixture.rs`/its tests, `benchmark_200.gd` and `ci.yml`.

| Measurement | Commit it ran at | Notes |
|---|---|---|
| Survival guard, pre-tune baseline | task-2 working tree before the catalogue edit (original catalogue: Wall 400, per-wave 4, quadratic 4) | release profile |
| Survival guard, post-tune | `f05dd07`; re-verified `526b8ae`, `65bfd5e` | release profile, exact CI command |
| Fortress 1,200-tick test (trajectory measured) | `a38da4e`; re-verified `40404fe`, `4844511` | debug profile |
| Godot `GRUS_BENCHMARK_200` marker runs | task-4 working tree, content committed as `ee6a32a` | debug non-e2e extension |
| `smoke_test.tscn`, `reset_test.tscn` | same task-4 tree | exit 0 |

## 3. Fixed-defense survival guard (spec §4)

Test: `crates/grus-sim/tests/survival_balance.rs`, release-only
(`#[ignore]`d for the normal suite), CI step
`cargo test -p grus-sim --release --locked --test survival_balance -- --ignored --nocapture`.

Reference defense (authored once via `spawn_completed_building`, legality
asserted by a fast test): closed 1×1 Wall ring on x58..69 × y43..53 inclusive —
**41 Walls + 1 Gate at (63,53)** — around the Team-1 Research Center at (62,46);
**2 Gun Turrets** at (59,47) and (67,47) with armed `AttackCooldown`; four
starting Workers left idle; **45 buildings total**; heat supply 15 vs demand 14 →
`surface_efficiency == 1.0`; no Furnaces and no test-only Coal. No player
commands after setup, real `MatchSession`, real fog, `step_tick` at
`SIM_STEP_SECONDS` (20 Hz), up to 84,000 ticks.

Contract: live core + Playing at the **wave-11 deadline (1,300 s)**, a real
Defeat **before the wave-40 deadline (4,200 s)**, wave schedule asserted
on-track every tick (clock advancing, no skipped launch, launch not overdue in
sim time — regression-pinned against stalled/skewed wave clocks).

| Run | Result |
|---|---|
| Baseline, original catalogue (Wall 400, per-wave 4, quadratic 4) | **Defeat during wave 6 at 810.20 simulated s** (wave clock 810.20 s); wall-clock 3.50 s — premature, before wave 11 |
| Tuned catalogue (Wall 2400, per-wave 2, quadratic 2) | **Defeat during wave 11 at 1,383.75 simulated s** (wave clock 1,383.75 s); wall-clock 9.05 s at `f05dd07`, 8.99 s at `526b8ae`, 8.79 s at `65bfd5e`. Phase `Result(MatchResult(TeamId(2)))` — real Defeat, wave `next=12`; both turrets fired |

Mechanism diagnosis (temporary probe, deleted afterwards; run under Wall=3200,
mechanism independent of the value): RC damage at defeat was **melee 768 /
ranged 32** — a true melee breach; first ring death t=539.8, first RC damage
t=1,147.95 then death ~13.5 s later; the wave-9 horde fielded ~91 live aliens
(~740 dps of concentrated grind). **Horde mass was the binding constraint**,
then ring HP; per-alien damage and turret kill rate were second-order (raising
turret damage or lowering Swarmer damage changed defeat time by <2%).

Honest limit: the reference static defense has no economy and cannot reach the
38–45-minute / wave-20–25 aspiration; the guard documents the true limit rather
than fabricating a later fall.

## 4. Catalogue tuning pass — old → new

Exactly three `catalog.rs` constants changed, provisionally retained per owner
direction with playtest outstanding. Every intermediate probe value was
reverted; the committed diff contains only these:

| Constant | Old | New | Rationale (measured) |
|---|---:|---:|---|
| `Wall.max_health` | 400 | 2,400 | Ring erosion is the defeat path: 400-HP cells fell in ~1 s under ~740 dps of grind. 1,600 proved insufficient (defeat wave 9); 3,200 saturated; 2,400 is the smallest tested value clearing the checkpoint with margin |
| `WAVE_BUDGET_PER_WAVE` | 4 | 2 | Halves linear pressure growth; the mid-wave window was the measured binding constraint |
| `WAVE_BUDGET_QUADRATIC_TENTHS` | 4 | 2 | Halves quadratic acceleration; wave-9 horde ~91 → ~60 live; late-game ramp preserved (wave-40 budget 408 vs 288 under the per-wave-3/quadratic-1 alternative) |

`wave_budget(w) = 8 + 2·w + 2·w²/10` now (`WAVE_BUDGET_BASE = 8` unchanged).
**Wave-40 arithmetic correction (from the task-2 report):** the rejected
alternative (per-wave 3, quadratic 1) gives `8 + 3×40 + 1×40²/10 = 288`, not the
168 initially misstated; the shipped values give `8 + 2×40 + 2×40²/10 = 408`.

Measured tuning log (release journey after each one-variable change; full
14-row table in the task-2 report): Wall 400→800→1600→2400→3200 moved defeat
from wave 6 @ 810 s to wave 9 @ 1,161 s; Gate 600→1600, turret damage 10→15 and
Swarmer damage 6→4 probes were all reverted as irrelevant; softening both wave
growth terms to 2 produced the shipped **wave 11 @ 1,383.75 s** (+83.75 s of
margin past the wave-11 deadline).

Tradeoffs, recorded honestly:

- The 6× wall multiplier is large — but the mechanism data shows paper walls
  were the single most undertuned number, and the owner explicitly approved
  retaining it provisionally.
- Halving `WAVE_BUDGET_*` softens difficulty for **every** wave in every mode,
  not just this fixture. That is why an interactive owner playtest — not this
  automated guard — must validate the pass before further tuning.
- The guard checks survival pressure only; it does not prove the 38–45-minute
  victory pacing, which needs an active economy and real play.

## 5. Fortified 200-unit benchmark workload (spec §2)

`fixture.rs::extend_with_benchmark_workload` now authors, through
`spawn_completed_building`, a closed wall ring (52,37)–(77,60) inclusive —
**92 Walls + 4 Gates** (N `(64,37)`, S `(64,60)`, W `(52,48)`, E `(77,48)`) —
around the central Research Center, with **24 Gun Turrets, 10 Flame Towers,
6 Crystal Lances**, **9 Coal Furnaces** and benchmark-only `coal = 400`
(`BENCHMARK_COAL`). Workers keep 4 accepted `Gather`s; 46 defenders (23 Guards
+ 23 Riflemen) hold the four gate lanes on accepted `AttackMove`s; `step_waves`
spawns the real **150-alien** cap at the synthetic **`BENCHMARK_WAVE = 100`** —
a cap-stress workload only; the wave-100 overflow HP is **not** representative
of normal wave-20-to-25 balance.

Census after extend (asserted once in Rust): **146 buildings**
(1 RC + 92 walls + 4 gates + 40 towers + 9 furnaces), **200 units**
(50 humans + 150 aliens), 196 attack-movers + 4 gatherers.

### 5.1 Simulated-time trajectory — the authoritative result

`fixture::tests::benchmark_workload_keeps_real_activity_for_the_sixty_second_sample`,
1,200 ticks = **60 simulated seconds**, debug profile, measured at `a38da4e`
(re-verified green at `40404fe` and `4844511`, ~128–132 s wall-clock):

| Signal | Observed |
|---|---|
| Live aliens at t=60 s sim | **130 / 150** (floor `BENCHMARK_LIVE_ALIEN_FLOOR = 100` pinned after measuring 130 — ≥⅔ of the horde still fighting) |
| First perimeter breach | **simulated second 18** — a Wall/Gate destroyed; per-tick diff ties it to `map.revision()` **423 → 424** on the destroying tick with its cells released walkable |
| Tower fire | `CombatEvent.attacker == Building` in **every second 5–60**, including the final second; all 40 towers carry `AttackCooldown` |
| Heat | `surface_efficiency == 1.0` at t=0 and t=60; supply **123** (RC 15 + 9×12) vs demand **112** (RC 10 + 24×2 + 10×3 + 6×4) |
| Fuel | 400 benchmark Coal vs 9 furnaces × 0.5/s × 60 s = 270 needed |

### 5.2 Rendered marker runs — local contract evidence only

Host/renderer: macOS 25.6.0 arm64, Godot 4.7.1, `gl_compatibility`,
`display=headless`, **debug non-e2e** `libgrus_godot.dylib` — not the Linux
llvmpipe CI path and not the missing hardware reference host.

Command (run twice, both exit 0):

```bash
GRUS_BENCHMARK_SECONDS=10 godot --headless --path godot \
  --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

| Metric | Run 1 | Run 2 |
|---|---:|---:|
| `duration_s` (wall clock) | 19.74 | 19.75 |
| frames | 489 | 485 |
| mean_ms / mean_fps | 40.372 / 24.8 | 40.724 / 24.6 |
| p50_ms / p95_ms / p99_ms / max_ms | 7.724 / 42.124 / 83.189 / 10,144.203 | 7.671 / 42.586 / 83.789 / 10,179.708 |
| live units start / min / mean / end | 200 / 178 / 184.8 / 178 | 200 / 178 / 184.7 / 178 |
| visible-in-tree min / mean / end | 50 / 150.8 / 177 | 50 / 151.3 / 177 |
| `visibility_revision` | 206 | 206 |
| `end_buildings` | 146 | 146 |
| `building_kinds` | CoalFurnace:9, CrystalLance:6, FlameTower:10, Gate:4, GunTurret:24, ResearchCenter:1, Wall:92 | same |
| `heat_supply` / `heat_demand` / `surface_efficiency` | 123 / 112 / 1.000 | same |
| `coal` at sample end | 350 | 350 |

Population/heat/fuel framing: units started at 200 (50 human + 150 alien) and
ended at 178 live; every authored building survived the ~10-sim-second window
(`end_buildings=146`, `visibility_revision=206` ≈ 10.3 simulated s at 20 Hz);
heat stayed saturated at 123/112 = 1.000; benchmark coal ended at 350 after
starting at 400 (furnace burn net of worker deposits — observed value, not
derived).

### 5.3 Wall-clock versus simulated seconds — read this before comparing

`GRUS_BENCHMARK_SECONDS` is a **wall-clock** sample bound. These runs requested
10 wall-clock s but reported `duration_s ≈ 19.7` because a single ~10.1 s frame
(`max_ms ≈ 10,150`, repeatable — a one-time first-use cost, likely shader/asset
warmup when combat effects first spawn) overshot the deadline mid-sample; the
loop is a single-frame overshoot by design. Likewise, on a slow renderer,
**60 wall-clock seconds do not equal 60 simulated seconds** — fixed 20 Hz ticks
lose time under `max_physics_steps_per_frame`. Consequently:

- the **breach** evidence is owned by the Rust test (§5.1, simulated time);
  **no breach is claimed for the rendered run** — all 146 buildings stood;
- the fortress Rust journey asserts 60 *simulated* seconds; the rendered marker
  proves only that ~10 simulated seconds of the real workload render without
  error on this host.

## 6. Balance-guard vs rendered workload summary

| Gate | Result | Time domain |
|---|---|---|
| Fixed-defense survives wave 10, falls before wave 40 | **PASS** — defeat wave 11 @ 1,383.75 s | simulated |
| Fortress 60-s trajectory (breach, tower fire, alien floor, heat) | **PASS** — §5.1 | simulated |
| `GRUS_BENCHMARK_200` marker, exit 0 (local macOS headless) | **PASS** ×2 — §5.2 | wall clock ≈19.7 s for a requested 10 s |
| `smoke_test.tscn`, `reset_test.tscn` locally | **PASS** exit 0 | — |

## 7. NOT RUN / BLOCKED — no claim made

- **Normal-match progression baseline and owner interactive playtest: NOT RUN /
  BLOCKED.** The spec requires at least one recorded real playtest (Victory/
  Defeat time and wave, economy/heat/Lab/Analyze decisions, pressure around
  waves 10 and 20–25) before merge; none exists. The automated guard and
  fortress benchmark are **not** a substitute — they are fixed-setup,
  no-economy scenarios, and they were not equated with playtest anywhere in
  this record. Per owner direction, PR #17 stays open and unmerged until that
  playtest happens; follow-up playtest iterations belong in separate tickets.
- **Hardware-rendered 60-s Linux x86_64 packaged benchmark: BLOCKED / not
  run** — no suitable reference host exists. macOS headless and llvmpipe
  numbers are not substitutes, and verified engineering is not being held
  indefinitely on this external dependency.
- **Linux software-GL CI fortress run** (`GRUS_BENCHMARK_SECONDS=10`, xvfb,
  `LIBGL_ALWAYS_SOFTWARE`, exit-0 + sentinel, 1920×1080): **NOT RUN** on this
  branch's fortress workload at the time of writing — it runs on the next CI
  execution; llvmpipe frame costs (~450 ms+ historically) make the new
  `VIEW_WAIT_FRAMES = 600` headroom the thing to watch, and its frames are not
  a performance signal.
- **Linux export / packaged bevy-E2E on this branch: NOT RUN** locally; CI owns
  it.
- **Godot smokes beyond `smoke_test`/`reset_test`** (economy, combat lifecycle,
  wave, defenses, underground) on the fortress branch: **NOT RUN** locally on
  this tree — CI owns them; `defenses_smoke_test` was analyzed for the
  2,400-HP wall coupling (breach well inside its 45 real-second window).
- **`cargo llvm-cov` 90% gate, full `cargo test --workspace`, clippy/fmt** were
  green at each implementation commit (task reports 2–4); the **final
  consolidated run on this docs commit is the controller's** — this document
  does not claim it.
- Any figure above may be updated by pending verification; nothing here is a
  promise about runs that have not happened.
