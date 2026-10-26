# HPA-449 verification evidence

Recorded 2026-10-26 on the existing `hpa-449-wave-director` PR branch.

## Local acceptance

Host: Apple M1 Pro, macOS 26.6.2 arm64. Rust 1.95.0 and Godot
4.7.1 (`a13da4feb`), plain non-e2e **debug** GDExtension. The staged
library was rebuilt and copied before runtime checks; its SHA-256 is
`8969bdd5a769d02c5e37cdfbefbdf574f4cc6eaaffd33e3617f2febd71acc00e`.

| Gate | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 241 passed |
| Serialized e2e-feature library tests | 18 passed |
| `grus-sim` line coverage, excluding `tests.rs` | 96.11%, above the 90% gate |
| Plain extension build and editor import | Exit 0 |
| Strict-exit feature split | Plain dependency graph excludes Bevy async-io; e2e graph is the positive control |
| Linux packaging stale-archive guard test | Passed with its stubbed tools; this is not Linux runtime proof |

Coverage command:

```sh
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
```

The actual exported-game E2E test is a separate Linux CI gate, recorded below.
A workspace test run without `GRUS_E2E_BINARY` does not establish that result.

## Real Godot journeys

All five final scenes used the rebuilt plain extension, `--headless`,
`--rendering-method gl_compatibility`, and a writable `--log-file` path.
Every process exited 0 and printed its required marker; no script-error
output was accepted.

| Scene | Wall time | Required marker |
| --- | ---: | --- |
| `smoke_test.tscn` | 7.402 s | `GRUS_GODOT_SMOKE_OK` |
| `reset_test.tscn` | 0.471 s | `GRUS_RESET_SMOKE_OK` |
| `economy_smoke_test.tscn` | 17.565 s | `GRUS_ECONOMY_SMOKE_OK` |
| `combat_lifecycle_smoke_test.tscn` | 26.027 s | `GRUS_COMBAT_LIFECYCLE_SMOKE_OK` |
| `wave_smoke_test.tscn` | 26.602 s | `GRUS_WAVE_SMOKE_OK` |

These cover Start/Playing/Pause, the warning and first launch, meaningful
alien movement, accepted controller A/attack-move and visible-alien
right-click input, a real Crystal kill, fog-gated markers, alien-caused
Defeat, Result locking and fresh Restart. Ordinary preparation scenarios
assert no wave launched before completion or reset. The dedicated wave
scene also verifies authoritative HUD/ping anchors, ping expiry, clock
freeze, and explicitly labeled primitive-role probes.

A separate **rendered visual-only preview** inspected all four role shapes
and damaged health bars at camera size 72. Its temporary metadata-backed
views were not evidence of gameplay, fog or simulation spawning. No preview
script or artificial views were added to the playable game.

## Benchmark workload and regression

Initial census: one Research Center, 18 real sources, 4 gathering Workers,
46 human combatants and 150 actual aliens. Alien creation uses the ordinary
wave-100 planner, live cap and overflow health rule. Setup assignments are
stable-ID ordered.

The original one-off orders naturally exhausted: the first native 60-second
attempt exited 1 after 28.32 wall seconds. A pure simulation regression
reproduced inactivity at simulated second 28 with 0 humans, 149 live aliens
and 0 buildings. That failure was retained, not counted as a measurement.

The existing benchmark runner now replays ordinary AttackMove inputs every
five measured seconds for surviving combatants toward opposing, alternating
North/South fixture anchors. The helper rejects normal match sessions.
Workers retain their gather tasks; IDs, real deaths, health, cooldowns and
every existing liveness verdict remain unchanged. A regression verifies
activity through 60 simulated seconds and separately checks preservation
and normal-session rejection. This script adds real command/pathfinding
work; the following results supersede the one-off-order measurements.

## Fresh measurements

Both runs used 1920×1080, Godot 4.7.1 and the debug library identified above.
No builds or test matrix ran concurrently with either sample. This was a
shared host, not an isolated performance laboratory.

| Field | Local headless | Native rendered Apple M1 Pro |
| --- | ---: | ---: |
| Requested duration | 10 s | 60 s |
| Measured duration | 10.00 s | 60.01 s |
| Rendered frames | 742 | 4407 |
| Mean frame time | 13.483 ms | 13.616 ms |
| p50 / p95 / p99 | 6.930 / 38.531 / 54.077 ms | 6.822 / 34.051 / 52.784 ms |
| Maximum frame time | 253.382 ms | 288.669 ms |
| Mean FPS | 74.2 | 73.4 |
| Live start / min / mean / end | 200 / 165 / 184.6 / 165 | 200 / 150 / 155.5 / 150 |
| Visible-in-tree min / mean / end | 71 / 95.9 / 164 | 0 / 20.7 / 0 |
| Visibility revision | 239 | 1087 |
| Process exit / complete marker | 0 / present | 0 / present |

The native renderer reported OpenGL 4.1 Metal 90.5 compatibility, Apple
M1 Pro, vendor Apple, display macOS. Host 1/5/15-minute load averages were
10.808 / 10.765 / 15.175 before and 9.288 / 10.396 / 14.715 after the native
run. Fog hides survivors after the defenders die: **this is not a sustained
200-visible-unit render load**. The scripted survivor movement remains real
and all liveness windows passed.

The native run is a rendered **macOS debug project-runner** measurement.
It does not satisfy the separate reference Linux x86_64 release-export
hardware-acceptance gate. Historical PvP/HPA-448 measurements are not
comparable to this workload.

## Platform and shutdown limits

- Local Linux export/boot and exported-game E2E were not run: Docker's
  daemon was unavailable. Hosted CI subsequently passed those real gates,
  as recorded below; local macOS results were not substituted for them.
- Restricted macOS invocations produced a CA-certificate lookup error or
  could not initialize the app/default user-log path. Final runtime checks
  used the required sandbox escalation and writable log paths; no host
  certificate settings were changed.
- An earlier debug benchmark intermittently reported `AudioStreamWAV` and
  `AudioStreamPlaybackWAV` at process shutdown despite stop/stream-release
  cleanup. The fresh final samples did not emit that warning. This does not
  prove the intermittent shutdown issue is universally fixed; no growing
  in-game leak was established.

Full local command output, smoke-result JSON, native metadata JSON and the
labeled visual preview are retained under `/private/tmp/hpa449-task6/` in
the implementation workspace's host environment. Hosted checks are tracked
through the same [implementation PR](https://github.com/cwchanap/Grus/pull/10).

## Hosted Linux acceptance

[CI run 40366902630](https://github.com/cwchanap/Grus/actions/runs/40366902630)
(run 211) completed successfully on reviewed implementation commit `4b81cde`.
All three jobs passed: `rust-build-lint`, `unit-tests` and `e2e`. This final
documentation update records that result without changing implementation.

The unit job ran 219 simulation unit tests plus the economy/occupancy/system-
order integration tests under the coverage gate, 15 bridge library tests and
18 serialized e2e-feature tests. The runtime job passed import, all five real
Godot smokes, strict plain/e2e dependency split, Linux smoke export and its
full packaged smoke contract. The real-main exported-game E2E test ran with
`GRUS_E2E_BINARY` and passed 1 test in 16.62 seconds.

The fresh Linux software-GL benchmark exited 0 with its complete marker.
The runner reported x86_64 / AMD EPYC 7763, Mesa 25.2.8, llvmpipe
(LLVM 20.1.2, 256 bits), X11, 1920×1080:

| Field | Hosted Linux software GL |
| --- | ---: |
| Requested / measured duration | 10 / 10.16 s |
| Frames / mean FPS | 22 / 2.2 |
| Mean / p50 frame time | 461.838 / 459.175 ms |
| p95 / p99 / maximum | 527.300 / 672.910 / 672.910 ms |
| Live start / min / mean / end | 200 / 168 / 170.7 / 168 |
| Visible-in-tree min / mean / end | 71 / 74.6 / 110 |
| Visibility revision | 222 |

These software-renderer results establish the CI timing/liveness contract;
they are not hardware performance acceptance. The reference Linux release-
host measurement remains unmeasured.
