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

(empty — to be filled; requires the Linux x86_64 reference host)

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
