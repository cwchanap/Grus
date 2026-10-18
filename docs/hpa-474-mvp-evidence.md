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

(empty — to be filled)

## Restart + lifecycle outcomes

(empty — to be filled)

## Tuning log

(empty — to be filled)

## Early release performance

**BLOCKED — no reference host.** The early 60-second release sample (`GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh`, then `GRUS_BENCHMARK_SECONDS=60` on the exported executable at 1920×1080) requires the hardware-rendered Linux x86_64 reference host that does not exist in this session — see the blocking acceptance dependency under **Reference host / revision** above. No sample, marker output, or hardware figure exists; none will be recorded until that host is provisioned.

What could be verified without the host: the packaging script itself ships in this task, and its `GRUS_MAIN_SCENE` trap path was exercised on the development machine (script run with the variable set fails at the Linux-only extension-staging step, and `git diff -- godot/project.godot` is empty afterwards — the main-scene substitution was live mid-run and fully restored on failure; a run without the variable never touches `project.godot`). The export itself was not run.

**Performance gate (Task 3 Step 5): no performance change.** With the sample blocked there is no measurement and therefore no measured owner — the gate's narrowest-owning-change step has nothing measured to act on, so no performance-motivated code change was made in this task. Profiling and any fix are deferred until the reference host exists.

## Final release performance

(empty — to be filled; requires the Linux x86_64 reference host)

## Package verification

(empty — to be filled)

## Fresh-player usability

(empty — to be filled)

## Asset attribution

(empty — to be filled)

## Final CI

(empty — to be filled)

## Remaining limitations

- No hardware-rendered Linux x86_64 reference host is available; all hardware acceptance evidence (performance, GPU/adapter identification) is blocked until one is provisioned.
