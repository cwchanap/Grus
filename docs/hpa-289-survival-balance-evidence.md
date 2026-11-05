# HPA-289 — Survival balance evidence

Status: **engineering evidence recorded; owner playtest and several Linux-side gates NOT RUN / BLOCKED.** This document records only what was actually measured, at the revision where it was measured, and labels everything else honestly. Some controller-run checks (coverage gate, serialized e2e-feature library tests, the seven scene smokes) were executed against the `ee6a32a` code after this document's first version; the two wave-count failures recorded there were repaired in a later GDScript-only follow-up and all seven smokes now pass on this branch — see §6–§7 for before/after detail. Remaining pending verification can update the sections marked below.

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
measurement ran at is stated explicitly; `ee6a32a` differs from some of them
but no catalogue or gameplay-impacting change landed between the latest
verification commits and `ee6a32a` — tasks 3–4 touched only
`fixture.rs`/its tests, `benchmark_200.gd` and `ci.yml`. Later commits on this
branch are docs-only apart from `6bbabe2` and a follow-up defenses-smoke
lifetime fix, which changed smoke GDScript only — no Rust or simulation
code — so checks the controller ran against the `ee6a32a` code still describe
the shipped Rust implementation; scene-smoke results were measured on the
repaired scripts.

| Measurement | Commit it ran at | Notes |
|---|---|---|
| Survival guard, pre-tune baseline | task-2 working tree before the catalogue edit (original catalogue: Wall 400, per-wave 4, quadratic 4) | release profile |
| Survival guard, post-tune | `f05dd07`; re-verified `526b8ae`, `65bfd5e` | release profile, exact CI command |
| Fortress 1,200-tick test (trajectory measured) | `a38da4e`; re-verified `40404fe`, `4844511` | debug profile |
| Godot `GRUS_BENCHMARK_200` marker runs | task-4 working tree, content committed as `ee6a32a` | debug non-e2e extension, headless/dummy driver (§5.2) |
| `smoke_test.tscn`, `reset_test.tscn` | same task-4 tree | exit 0 |
| `cargo llvm-cov` 90% line-coverage gate | `ee6a32a` code (no Rust/sim diffs since — later commits are docs or smoke GDScript only) | exit 0 — 96.77% (LH 4,710 / LF 4,867); controller-run after this doc's first version |
| Serialized e2e-feature library tests | `ee6a32a` code | exit 0 — 39/39; `cargo test -p grus-godot --lib --features e2e --locked --offline -- --test-threads=1` |
| Seven Godot scene smokes | first run on `ee6a32a` code: **5 pass / 2 fail**; after the GDScript-only wave-count fix on this branch (no sim/Rust change): **7 pass**; defenses re-verified twice after the lambda-lifetime fix — exit 0, marker, zero freed-capture errors — see §6–§7; logs under `.superpowers/sdd/…/<scene>.log` |

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

### 5.2 Headless marker runs — local scene-contract evidence only

Host/method: macOS 25.6.0 arm64, Godot 4.7.1, **debug non-e2e**
`libgrus_godot.dylib`, launched `--headless` with `--rendering-method
gl_compatibility`. `gl_compatibility` is the *configured* rendering method, not
a detected renderer: Godot's headless display driver uses the **dummy rendering
driver** — the marker itself prints empty `adapter=`/`vendor=` fields and
`display=headless` — so **no GPU rendering occurred**. These are headless/dummy
scene-contract runs: they prove the real workload simulates and every view
instantiates without error, nothing about rendered frame cost. Still not the
Linux llvmpipe CI path and not the missing hardware reference host.

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
ended at 178 live; every authored building survived the sampled window
(`end_buildings=146`). `visibility_revision=206` is recorded as an observation
only — `VisibilityMap.revision` increments only when effective visibility
changes and already accumulates during setup/readiness, so it is **not** a
simulated-time clock; the sampled simulated duration is **unmeasured** (the
runner logs no simulated-time field). Heat stayed saturated at 123/112 = 1.000;
benchmark coal ended at 350 after starting at 400 (observed value, not
derived).

### 5.3 Wall-clock versus simulated seconds — read this before comparing

`GRUS_BENCHMARK_SECONDS` is a **wall-clock** sample bound. These runs requested
10 wall-clock s but reported `duration_s ≈ 19.7` because a single ~10.1 s frame
(`max_ms ≈ 10,150`, repeatable) overshot the deadline mid-sample; the loop is a
single-frame overshoot by design and the stall's cause is **unverified** (a
headless run compiles no shaders, so "shader warmup" is not an available
explanation either). More generally, on any slow target **60 wall-clock seconds
do not equal 60 simulated seconds** — fixed 20 Hz ticks lose time under
`max_physics_steps_per_frame` — and the sampled simulated duration is
unmeasured (§5.2). Consequently:

- the **breach** evidence is owned by the Rust test (§5.1, simulated time);
  **no breach is claimed for the headless marker run** — all 146 buildings
  stood at sample end;
- the headless marker proves only that the real 200-unit workload simulates,
  instantiates all views and reaches the marker without error on this host —
  it rendered nothing and measured no simulated duration.

## 6. Gate summary — measured and contract checks

| Gate | Result | Time domain |
|---|---|---|
| Fixed-defense survives wave 10, falls before wave 40 | **PASS** — defeat wave 11 @ 1,383.75 s | simulated |
| Fortress 60-s trajectory (breach, tower fire, alien floor, heat) | **PASS** — §5.1 | simulated |
| `cargo llvm-cov` 90% gate (`ee6a32a` code) | **PASS** — 96.77% lines (LH 4,710 / LF 4,867), exit 0 | — |
| Serialized e2e-feature lib tests (`ee6a32a` code): `cargo test -p grus-godot --lib --features e2e --locked --offline -- --test-threads=1` | **PASS** — 39/39 | — |
| `GRUS_BENCHMARK_200` marker, exit 0 (local macOS headless, dummy driver) | **PASS** ×2 — §5.2 | wall clock ≈19.7 s for a requested 10 s; simulated duration unmeasured |
| Scene smokes, 7 total | **7 PASS** — first run on `ee6a32a` code was 5 PASS / 2 FAIL (combat_lifecycle + wave expected the pre-tune 12-alien first wave); after updating the stale first-wave assertions to the tuned 10-alien roster, all seven exit 0 with their markers (§7); the defenses run's earlier freed-lambda-capture ERROR lines are gone after the callback-lifetime fix | — |

## 7. Remaining gate status — NOT RUN / BLOCKED / controller-run results

- **Normal-match progression baseline and owner interactive playtest: NOT RUN /
  BLOCKED.** The spec requires at least one recorded real playtest (Victory/
  Defeat time and wave, economy/heat/Lab/Analyze decisions, pressure around
  waves 10 and 20–25) before merge; none exists. The automated guard and
  fortress benchmark are **not** a substitute — they are fixed-setup,
  no-economy scenarios, and they were not equated with playtest anywhere in
  this record. Per owner direction, PR #17 stays open and unmerged until that
  playtest happens; follow-up playtest iterations belong in separate tickets.
- **Hardware-rendered 60-s Linux x86_64 packaged benchmark: BLOCKED / not
  run** — no suitable reference host exists. Headless dummy-driver runs and
  llvmpipe numbers are not substitutes, and verified engineering is not being
  held indefinitely on this external dependency.
- **Linux software-GL CI fortress run** (`GRUS_BENCHMARK_SECONDS=10`, xvfb,
  `LIBGL_ALWAYS_SOFTWARE`, exit-0 + sentinel, 1920×1080): **NOT RUN** on this
  branch's fortress workload at the time of writing — it runs on the next CI
  execution; llvmpipe frame costs (~450 ms+ historically) make the new
  `VIEW_WAIT_FRAMES = 600` headroom the thing to watch, and its frames are not
  a performance signal.
- **Linux export / packaged bevy-E2E on this branch: NOT RUN** — the only
  e2e-surface check executed so far is the serialized in-process library run
  below.
- **Scene smokes — all 7 pass after a GDScript-only follow-up fix:**
  `smoke_test`, `reset_test`, `economy_smoke_test`, `defenses_smoke_test` and
  `underground_smoke_test` exit 0 with their markers against the `ee6a32a` code
  (logs under `.superpowers/sdd/2026-11-03-hpa-289-survival-benchmark-balance/<scene>.log`).
  `combat_lifecycle_smoke_test` and `wave_smoke_test` initially **failed** at
  `combat_lifecycle_smoke_test.gd:27` — "first wave did not launch twelve live
  aliens (waited 15s)". The smokes still expected the pre-tune wave-1 roster of
  12; the tuned `WAVE_BUDGET_*` now spawns 10 (`wave_budget(1) = 8 + 2 + 0`;
  `plan_roster` pins 10 Swarmers in `crates/grus-sim/src/waves/tests.rs`).
  **Repair (GDScript only, no sim/Rust change, same headless Godot 4.7.1 host):**
  the lifecycle smoke's first-wave gates now assert `live_aliens >= 10` and
  `_alien_views() >= 10` — still the full actual roster, not a weakened
  nonzero check; the wave smoke shares that journey and needed no separate
  edit. After the fix both exit 0 with their real markers:
  `GRUS_COMBAT_LIFECYCLE_SMOKE_OK` and `GRUS_WAVE_SMOKE_OK` (exit 0 verified
  from the process status, not the marker text alone).
  `defenses_smoke_test.gd` (extends the same lifecycle script) was probed for
  the same staleness: its `live_aliens >= 12` gate passed post-tune only by
  accumulating wave 1 + wave 2 (22 live). Tightening it to `>= 10` was tried
  and **reverted after two identical real failures** — observing the intact
  10-Swarmer pack at the wave-1 launch tick lets the horde converge on and
  destroy the lone Gun Turret before a tracer renders, while the accumulated
  horde keeps the turret firing long enough. The `>= 12` horde-size gate is
  therefore retained; only its timeout message was corrected (it no longer
  claims the North wave itself launches twelve). The same repair also
  deflaked the smoke's marker line: it now prints the `resource_id` captured
  while the Scrap-source view was provably alive instead of dereferencing the
  view at exit — the worker can legitimately deplete the 400-Scrap node
  mid-journey, which freed the view and crashed the final `print` on one
  verification run (a pre-existing latent flake, not caused by the tuning).
  `defenses_smoke_test` re-verified green: exit 0,
  `GRUS_DEFENSE_SMOKE_OK` (gather_source=3).
  That passing run still logged two `ERROR: Lambda capture at index 0 was
  freed. Passed "null" instead.` lines inside `_wait_until`
  (`combat_lifecycle_smoke_test.gd:34`; one backtrace points at
  `defenses_smoke_test.gd:282`): the post-breach, crystal-kill and tracer wait
  lambdas captured the turret view Node3D, which the horde can legitimately
  destroy mid-wait; the kill/tracer latches still gated success, so the run
  passed but its output was not clean. The lambdas now capture the stable
  `turret_id` and `_observe_turret` re-resolves the view through
  `_building_view` each poll under the same `is_instance_valid` guard — every
  assertion, timeout and the marker contract unchanged. Re-verified on the
  same headless Godot 4.7.1 host in two consecutive runs: exit 0,
  `GRUS_DEFENSE_SMOKE_OK` (setup_s 22.7/22.6, post_wave_s 10.9/11.0), zero
  `Lambda capture`/ERROR lines; before/after logs in the plan workspace.
- **Coverage gate — RUN and PASSED on the `ee6a32a` code** (controller-run
  after this doc's first version; no Rust/sim diffs since — later commits are
  docs or smoke GDScript only):
  `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --output-path .superpowers/sdd/2026-11-03-hpa-289-survival-benchmark-balance/coverage-final.lcov --locked --offline --ignore-filename-regex 'tests\.rs$'`
  → **exit 0, 96.77% lines (LH 4,710 / LF 4,867)**. The earlier version of this
  section asserted the gate "was green at each implementation commit" — no such
  per-commit runs exist in the task reports, and that claim was removed.
- **Serialized e2e-feature library tests — RUN and PASSED** on the `ee6a32a`
  code: `cargo test -p grus-godot --lib --features e2e --locked --offline --
  --test-threads=1` → **exit 0, 39/39 passed**.
- `cargo fmt`, `cargo clippy` and `cargo test --workspace` were green at each
  implementation commit per task reports 2–4; no fresh consolidated run of
  those on the docs commits is claimed here.
- Any figure above may be updated by pending verification; nothing here is a
  promise about runs that have not happened.
