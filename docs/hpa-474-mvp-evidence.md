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

(empty — to be filled; requires the Linux x86_64 reference host)

## Package verification

**BLOCKED — no Linux x86_64 host (same Task-0 dependency).** Steps 5–7 of Task 5 (run `./scripts/package_linux_x86_64.sh` without `GRUS_MAIN_SCENE` on a fresh checkout, extract elsewhere, launch outside the editor, and complete the packaged gameplay journey: Start → pause/resume → select + gather → place/build → train → scout/fog/minimap → fight → Victory or Defeat → Restart-clean → start again → Quit) **did not run and no result is recorded or invented**. The packaging script and its `GRUS_MAIN_SCENE` trap were delivered in Task 3 (see Early release performance above), but the script's export/archive steps require the Linux x86_64 host that this session does not have — the same blocking acceptance dependency recorded under **Reference host / revision**. No tarball, extraction, launch, journey observation, or `sha256sum dist/grus-linux-x86_64.tar.gz` exists; the package hash is explicitly **BLOCKED** until that host exists.

## Fresh-player usability

**NOT RUN — no first-time player available in this agent session.** Step 7 requires a person who has never played Grus to attempt the extracted build using only the in-game controls label, with no developer walkthrough, and records whether they can start, gather/build/train, scout, fight, interpret the result, restart, and quit. No such person and no extracted build exist in this session, so **no fresh-player observation is recorded or substituted** (per Ledger Ruling 1 the author's own run would not be acceptable evidence anyway). This missing acceptance evidence is listed under Remaining limitations.

## Asset attribution

Asset attribution: none.

Verified by inspection on 2026-10-16: the repository contains no third-party game assets — no image, audio, font, or 3D-model files anywhere under `godot/` or `crates/`, no Godot `.import` artifacts, and no third-party license/attribution files. All presentation is generated in code (procedural meshes, the runtime `ImageTexture` minimap, and the generated audio blip described in README).

## Final CI

(empty — to be filled)

## Remaining limitations

- No hardware-rendered Linux x86_64 reference host is available; all hardware acceptance evidence (performance, GPU/adapter identification) is blocked until one is provisioned.
- The packaged-build acceptance chain (Task 5 Steps 5–8) is blocked by the same missing host: no package build/extract/launch, no packaged gameplay journey, no `sha256sum` of `dist/grus-linux-x86_64.tar.gz` (see Package verification above).
- The first-time-player usability evidence (Task 5 Step 7) is missing: no fresh player attempted the extracted build with only the in-game controls label in this session; running it needs a human with the extracted package on the reference host (see Fresh-player usability above).
- The four interactive playtests (Playtest matrix), their restart-cleanliness and Victory/Defeat lifecycle records (Restart + lifecycle outcomes), and consequently any balance tuning (Tuning log) are **not run**: this agent session has no human player and no playable interactive session. All four cells carry an explicit not-run status above; running them only needs a human with `godot --path godot` (and `GRUS_MIRRORED_START=1` for the mirrored pair) on the delivered build.
