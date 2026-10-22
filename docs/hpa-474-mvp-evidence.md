# HPA-474 MVP Delivery Evidence

Evidence record for the HPA-474 MVP delivery plan. Created at preflight (Task 0); sections fill in as tasks complete.

## Reference host / revision

**BLOCKING ACCEPTANCE DEPENDENCY — no reference host exists yet.** The required reference host is a real, hardware-rendered Linux x86_64 machine (not software-rendered llvmpipe, not a CI runner). This delivery session runs on macOS arm64 with no Linux x86_64 GPU host reachable (no configured Linux box; docker daemon not running). HPA-470 recorded Linux x86_64 as the supported export target but only ever had a software-rendered llvmpipe CI machine; that runner is explicitly **not** acceptable as the hardware reference host.

Until such a host is provisioned, HPA-474's hardware-rendered acceptance criteria (benchmark numbers, GPU/renderer/adapter identification) are blocked. Host discovery is not postponed to Task 6 — it must happen before performance and packaging evidence can be recorded.

Hardware numbers below are **not invented**: no benchmark or hardware-rendered figures are recorded anywhere in this file until the reference host exists.

### Development machine (NOT the reference host)

Recorded for environment context only; it is a macOS arm64 machine and must **not** be treated as the reference host. All acceptance numbers must come from the future Linux x86_64 host.

```
$ uname -a
Darwin Chans-MacBook-Pro-3.local 25.6.0 Darwin Kernel Version 25.6.0: Fri Jul 31 19:18:49 PDT 2026; root:xnu-12377.161.14~5/RELEASE_ARM64_T6000 arm64

$ godot --version
4.7.1.stable.official.a13da4feb
```

Note: the `lscpu` command from the plan cannot run on macOS (no `lscpu` on Darwin); it applies to the future Linux x86_64 reference host once provisioned. GPU/renderer/adapter will be recorded once the benchmark prints it on that host.

**Final candidate revision:** the delivery's final code SHA is pinned under **Final CI** below (the exact SHA the controller pushes for the final CI run).

## HPA-445 interim post-AI benchmark re-baseline

HPA-445 removed the Team-2 economic AI and the mirrored-start launch mode; its plan requires re-measuring the 60-second benchmark from actual output instead of carrying forward AI-era means. The reference hardware-rendered Linux x86_64 release host required by the HPA-474 sections above **still does not exist** — that blocker and every HPA-474 BLOCKED status above are unchanged, and no HPA-474 hardware-acceptance figure is recorded. Per the HPA-445 controller ruling, the re-baseline below is a **local development-machine measurement**, explicitly **not** the reference host, on the **current debug** GDExtension build (not the release export).

**Host (measured 2026-10-21, post-AI tree @ HPA-445 branch):** Apple M1 Pro, macOS 26.6.2 (25G83), arm64; Godot 4.6.2.stable.mono.official.71f334935 (matches the repo pin); debug `grus-godot` GDExtension; `--rendering-method gl_compatibility`, display headless (the marker's `adapter`/`vendor` fields are empty under headless).

**Command:** `GRUS_BENCHMARK_SECONDS=60 /Applications/Godot_mono.app/Contents/MacOS/Godot --headless --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn`

**Actual marker output (verbatim):**

```
GRUS_BENCHMARK_200 start_units=200 min_live_units=87 mean_live_units=134.2 end_units=87 min_visible_in_tree_units=20 mean_visible_in_tree_units=81.5 end_visible_in_tree_units=20 duration_s=60.01 frames=2343 mean_ms=25.611 p50_ms=6.263 p95_ms=35.559 p99_ms=695.460 max_ms=1033.359 mean_fps=39.0 visibility_revision=822 renderer=gl_compatibility adapter= vendor= display=headless
```

**Reading:** over the 60.01 s sample the workload rendered 2343 frames at a mean **25.611 ms** per frame (mean 39.0 fps; p50 6.263 ms, p95 35.559 ms, p99 695.460 ms, max 1033.359 ms — the tail spikes are headless-startup/GC outliers on an unattended debug build). Live units started at the scripted 200 and fell to a **mean of 134.2** (min/end 87): the benchmark's scripted attack-move workload attrits units and nothing replaces them now that no AI rebuilds either side; the scene's liveness check (movement or combat present) still held. Visible-in-tree units (the render-load proxy) averaged **81.5** (min/end 20). Fog refresh advanced (`visibility_revision=822`).

**Label:** these are the **interim post-AI scripted 200-unit workload** numbers. The workload and its trajectory change again in **HPA-448** and **HPA-449**; the authoritative 60-second release-export measurement on the reference Linux x86_64 host **remains to be re-confirmed** once that host is provisioned. These local figures must not be reused as HPA-474 hardware-acceptance evidence.

## Playtest matrix

**NOT RUN — rows 1–2 require a human player and were not run in this agent session; rows 3–4 are obsolete.** The delivery session has no human player and no playable interactive session; per Ledger Ruling 1 the runs below were not executed, and no observation, duration or outcome is invented here. Rows 1–2 remain the blocking directional evidence for any balance tuning, exactly like the Task-0 reference-host blocker above blocks hardware evidence.

The mirrored launch switch itself **was** wired and unit-tested during this task (Task 4 Step 1–2): `GRUS_MIRRORED_START=1 godot --path godot` swapped the authored team starts (Team 1 right / Team 2 left, Team-2 AI reading the authored left plan as issuer Team 2); unset kept Team 1 left / Team 2 right.

*Superseded by HPA-445: the mirrored launch mode no longer exists — `GRUS_MIRRORED_START` has no effect and matrix rows 3–4 below are obsolete. A human playtester can run rows 1–2 only.*

| # | Launch | Opening | Duration | Result | Expansion | Raids | TC finish |
|---|--------|---------|----------|--------|-----------|-------|-----------|
| 1 | default (Team 1 left) | early pressure | not run in agent session — requires human playtest | not run | not run | not run | not run |
| 2 | default (Team 1 left) | economy-first / age-up | not run in agent session — requires human playtest | not run | not run | not run | not run |
| 3 | `GRUS_MIRRORED_START=1` (Team 1 right) | early pressure | obsolete — mirrored launch mode removed in HPA-445 | — | — | — | — |
| 4 | `GRUS_MIRRORED_START=1` (Team 1 right) | economy-first / age-up | obsolete — mirrored launch mode removed in HPA-445 | — | — | — | — |

## Restart + lifecycle outcomes

**NOT RUN — requires human playtest.** The restart-cleanliness checks (resources / entities / queues / selections / fog / AI state after each Restart) and the Victory/Defeat lifecycle outcomes (brief Steps 3–5) are part of the same interactive runs (matrix rows 1–2; rows 3–4 are obsolete) and share the same blocking dependency: no human player exists in this agent session. No restart observation or lifecycle outcome is recorded or invented. The automated equivalent that does run (`reset_fixture_world` re-seeding through the shared `seed_match_world` path, covered by `cargo test -p grus-godot --lib`) verifies re-seed correctness at the state level, but is not a substitute for the human evidence the plan requires.

## Tuning log

**BLOCKED ON MISSING PLAYTEST EVIDENCE — no tuning decision is possible.** With the runnable matrix runs (rows 1–2) not executed (see Playtest matrix above), there is no duration, result, expansion, raid or TC-finish observation to justify changing any owner. Per plan Step 6–7 a balance fix requires demonstrated evidence and a narrow regression first; none exists, so **no catalogue / AI / fixture value was touched in this task**. This is not a "no tuning change required" verdict — balance quality is simply unknown and tuning remains blocked until a human playtester produces the rows 1–2 evidence.

## Early release performance

**BLOCKED — no reference host.** The early 60-second release sample (`GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh`, then `GRUS_BENCHMARK_SECONDS=60` on the exported executable at 1920×1080) requires the hardware-rendered Linux x86_64 reference host that does not exist in this session — see the blocking acceptance dependency under **Reference host / revision** above. No sample, marker output, or hardware figure exists; none will be recorded until that host is provisioned.

What could be verified without the host: the packaging script itself ships in this task, and its `GRUS_MAIN_SCENE` trap path was exercised on the development machine (script run with the variable set fails at the Linux-only extension-staging step, and `git diff -- godot/project.godot` is empty afterwards — the main-scene substitution was live mid-run and fully restored on failure; a run without the variable never touches `project.godot`). The export itself was not run.

**Performance gate (Task 3 Step 5): no performance change.** With the sample blocked there is no measurement and therefore no measured owner — the gate's narrowest-owning-change step has nothing measured to act on, so no performance-motivated code change was made in this task. Profiling and any fix are deferred until the reference host exists.

## Final release performance

**BLOCKED — no reference host (same Task-0 dependency).** Task 6 Steps 1–2 (final export from the final candidate revision via `GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh`, then the identical 60-second `GRUS_BENCHMARK_SECONDS=60` sample at 1920×1080 on the exported executable, same host as Task 3) require the hardware-rendered Linux x86_64 reference host that still does not exist in this session — see the blocking acceptance dependency under **Reference host / revision**. No final export, no marker output, no early-vs-final comparison, and no mean-live-units figure exists; none are invented.

**Performance gate (Task 6 Step 3): no measurement → no performance change.** With both hardware samples blocked there is no measurement, therefore no measured owner to profile and no evidence to justify any performance-motivated code change. No profile-driven fix exists (none could: no measurements existed anywhere in this session), so there is no before/after evidence to record. Profiling and any fix remain deferred until the reference host is provisioned.

## Package verification

**BLOCKED — no Linux x86_64 host (same Task-0 dependency).** Steps 5–7 of Task 5 (run `./scripts/package_linux_x86_64.sh` without `GRUS_MAIN_SCENE` on a fresh checkout, extract elsewhere, launch outside the editor, and complete the packaged gameplay journey: Start → pause/resume → select + gather → place/build → train → scout/fog/minimap → fight → Victory or Defeat → Restart-clean → start again → Quit) **did not run and no result is recorded or invented**. The packaging script and its `GRUS_MAIN_SCENE` trap were delivered in Task 3 (see Early release performance above), but the script's export/archive steps require the Linux x86_64 host that this session does not have — the same blocking acceptance dependency recorded under **Reference host / revision**. No tarball, extraction, launch, journey observation, or `sha256sum dist/grus-linux-x86_64.tar.gz` exists; the package hash is explicitly **BLOCKED** until that host exists. Task 6's final rebuild of the normal package (re-run without `GRUS_MAIN_SCENE`, recheck extraction/boot, fresh `sha256sum`) is blocked by the same missing host — the blocked status above covers the Task-5 and Task-6 package steps alike; no new hash exists.

## Fresh-player usability

**NOT RUN — no first-time player available in this agent session.** Step 7 requires a person who has never played Grus to attempt the extracted build using only the in-game controls label, with no developer walkthrough, and records whether they can start, gather/build/train, scout, fight, interpret the result, restart, and quit. No such person and no extracted build exist in this session, so **no fresh-player observation is recorded or substituted** (per Ledger Ruling 1 the author's own run would not be acceptable evidence anyway). This missing acceptance evidence is listed under Remaining limitations.

## Asset attribution

Asset attribution: none.

Verified by inspection on 2026-10-16: the repository contains no third-party game assets — no image, audio, font, or 3D-model files anywhere under `godot/` or `crates/`, no Godot `.import` artifacts, and no third-party license/attribution files. All presentation is generated in code (procedural meshes, the runtime `ImageTexture` minimap, and the generated audio blip described in README).

## Final CI

Validated code SHA: `fdcd9ffe35419615f49ebe56d02539eec4ecc7ad` (PR-#7 review-fix commit: non-consuming benchmark liveness counter, complete-string `GRUS_BENCHMARK_SECONDS` validation, packaging stale-archive guard, README release-example correction, mirrored-restart contract + regression test, plan corrections). The record commit documenting this is a docs-only delta on top — it re-runs the same three jobs on the resulting pushed head, so the gate still covers the true final SHA.
Final CI run: https://github.com/cwchanap/Grus/actions/runs/39182679195 — **success** on pushed head `fdcd9ff` (pull_request): `rust-build-lint` ✅, `unit-tests` ✅, `e2e` ✅. (Prior records: run 39103831991 on `24806c7` and run 39104453875 on `cecffd1`, both success. The earlier e2e run surfaced the known post-marker teardown-139 warning, handled non-fatally by the existing workflow guard.)

Requirement (Task 6 Step 7): after the push, all three GitHub Actions jobs must be green on that exact SHA — `rust-build-lint`, `unit-tests`, `e2e`. This automated release gate is the final gate; it is not replaced by a hand-maintained local command list. (Supplementary only: on `fdcd9ff`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings`, `cargo test -p grus-sim --locked`, `cargo test -p grus-godot --lib --locked`, the e2e-feature lib tests, and the headless Godot `smoke_test`, `reset_test`, `combat_lifecycle_smoke_test` and `benchmark_200` scenes all ran green on the development machine.)

## Remaining limitations

Consolidated list; each item carries its per-section owner above.

- **No Linux x86_64 reference host (root blocker):** no hardware-rendered Linux x86_64 machine exists in this session, so all hardware acceptance evidence is blocked — no final export, no early or final 60-second benchmark sample, no GPU/adapter identification (see Reference host / revision, Early release performance, Final release performance).
- **Directional playtests not run:** playtest-matrix rows 1–2 are explicit not-runs and rows 3–4 are obsolete (mirrored launch removed in HPA-445); this agent session has no human player and no playable interactive session. Running rows 1–2 needs only a human with `godot --path godot` (see Playtest matrix).
- **Both lifecycle outcomes + restart cleanliness not evidenced:** the Victory/Defeat outcome records and repeated-restart cleanliness checks share the playtest blocker and were not run (see Restart + lifecycle outcomes); balance tuning stays blocked with them (see Tuning log).
- **Extracted-package journey not run + no final tarball hash:** no package build/extract/launch, no packaged gameplay journey, and no `sha256sum` of `dist/grus-linux-x86_64.tar.gz` — blocked by the missing host, identically for the Task-5 and Task-6 package steps (see Package verification).
- **Fresh-player usability not run:** no first-time player attempted the extracted build using only the in-game controls label; this needs a human with the extracted package on the reference host (see Fresh-player usability).
- **Teardown-139 still applies; handled in CI:** the post-contract-marker exit 139 (Godot's teardown path dlclose()s the extension while bevy_asset's parked async-io driver thread holds module TLS in the `.so`; upstream godot-bevy 0.12 / bevy_asset shutdown ordering, both `=`-pinned) still exists. `.github/workflows/ci.yml` gates every smoke/benchmark/export step on the printed contract marker and demotes a post-marker 139 to a `::warning` (`TEARDOWN_WARNED`) instead of a failure, so it neither masks a broken contract nor fails the run; the defect itself remains open upstream.
