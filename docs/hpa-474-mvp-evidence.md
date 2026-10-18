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

## Playtest matrix

**NOT RUN — all four interactive playtests require a human player and were not run in this agent session.** The delivery session has no human player and no playable interactive session; per Ledger Ruling 1 the four runs below were not executed, and no observation, duration or outcome is invented here. They remain the blocking directional evidence for any balance tuning, exactly like the Task-0 reference-host blocker above blocks hardware evidence.

The mirrored launch switch itself **is** wired and unit-tested in this task (Task 4 Step 1–2): `GRUS_MIRRORED_START=1 godot --path godot` swaps the authored team starts (Team 1 right / Team 2 left, Team-2 AI reading the authored left plan as issuer Team 2); unset keeps Team 1 left / Team 2 right. A human playtester can run the matrix as-is.

| # | Launch | Opening | Duration | Result | Expansion | Raids | TC finish |
|---|--------|---------|----------|--------|-----------|-------|-----------|
| 1 | default (Team 1 left) | early pressure | not run in agent session — requires human playtest | not run | not run | not run | not run |
| 2 | default (Team 1 left) | economy-first / age-up | not run in agent session — requires human playtest | not run | not run | not run | not run |
| 3 | `GRUS_MIRRORED_START=1` (Team 1 right) | early pressure | not run in agent session — requires human playtest | not run | not run | not run | not run |
| 4 | `GRUS_MIRRORED_START=1` (Team 1 right) | economy-first / age-up | not run in agent session — requires human playtest | not run | not run | not run | not run |

## Restart + lifecycle outcomes

**NOT RUN — requires human playtest.** The restart-cleanliness checks (resources / entities / queues / selections / fog / AI state after each Restart) and the Victory/Defeat lifecycle outcomes (brief Steps 3–5) are part of the same four interactive runs and share the same blocking dependency: no human player exists in this agent session. No restart observation or lifecycle outcome is recorded or invented. The automated equivalent that does run (`reset_fixture_world` re-seeding through the shared `seed_match_world` path, covered by `cargo test -p grus-godot --lib`) verifies re-seed correctness at the state level, but is not a substitute for the human evidence the plan requires.

## Tuning log

**BLOCKED ON MISSING PLAYTEST EVIDENCE — no tuning decision is possible.** With all four matrix runs not executed (see Playtest matrix above), there is no duration, result, expansion, raid or TC-finish observation to justify changing any owner. Per plan Step 6–7 a balance fix requires demonstrated evidence and a narrow regression first; none exists, so **no catalogue / AI / fixture value was touched in this task**. This is not a "no tuning change required" verdict — balance quality is simply unknown and tuning remains blocked until a human playtester produces the four-run evidence.

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

Validated code SHA: PENDING — CI URL pending — **controller pushes this SHA; CI URL to be recorded here**.

Requirement (Task 6 Step 5): after the push, all three GitHub Actions jobs must be green on that exact SHA — `rust-build-lint`, `unit-tests`, `e2e`. This automated release gate is the final gate; it is not replaced by a hand-maintained local command list. (Supplementary only: before these commits, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, and the five headless Godot smokes all ran green on this tree on the development machine.)

## Remaining limitations

Consolidated list; each item carries its per-section owner above.

- **No Linux x86_64 reference host (root blocker):** no hardware-rendered Linux x86_64 machine exists in this session, so all hardware acceptance evidence is blocked — no final export, no early or final 60-second benchmark sample, no GPU/adapter identification (see Reference host / revision, Early release performance, Final release performance).
- **Four directional playtests not run:** every playtest-matrix cell is an explicit not-run; this agent session has no human player and no playable interactive session. Running them needs only a human with `godot --path godot` (and `GRUS_MIRRORED_START=1` for the mirrored pair) (see Playtest matrix).
- **Both lifecycle outcomes + restart cleanliness not evidenced:** the Victory/Defeat outcome records and repeated-restart cleanliness checks share the playtest blocker and were not run (see Restart + lifecycle outcomes); balance tuning stays blocked with them (see Tuning log).
- **Extracted-package journey not run + no final tarball hash:** no package build/extract/launch, no packaged gameplay journey, and no `sha256sum` of `dist/grus-linux-x86_64.tar.gz` — blocked by the missing host, identically for the Task-5 and Task-6 package steps (see Package verification).
- **Fresh-player usability not run:** no first-time player attempted the extracted build using only the in-game controls label; this needs a human with the extracted package on the reference host (see Fresh-player usability).
- **Teardown-139 still applies; handled in CI:** the post-contract-marker exit 139 (Godot's teardown path dlclose()s the extension while bevy_asset's parked async-io driver thread holds module TLS in the `.so`; upstream godot-bevy 0.12 / bevy_asset shutdown ordering, both `=`-pinned) still exists. `.github/workflows/ci.yml` gates every smoke/benchmark/export step on the printed contract marker and demotes a post-marker 139 to a `::warning` (`TEARDOWN_WARNED`) instead of a failure, so it neither masks a broken contract nor fails the run; the defect itself remains open upstream.
