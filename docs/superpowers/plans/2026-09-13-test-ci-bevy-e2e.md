# Grus unit coverage, Rust CI, and Bevy E2E implementation plan

**Date:** 2026-09-13

## Goal

Give Grus a small, reliable CI foundation with three independent signals:

1. Rust build and lint quality.
2. Rust unit/integration tests with **>90% line coverage for `grus-sim`**.
3. A real exported-game E2E path using `cwchanap/bevy-e2e` from PR #1.

Keep the work in **one Grus implementation PR**. Reuse the current CI, Godot export flow, smoke scenes, and benchmark rather than adding a second test framework or new build orchestration layer.

## Current state

Grus currently has one workspace-level CI job that performs all of the following serially:

- `cargo fmt`
- `cargo clippy`
- `cargo test --workspace`
- `cargo build -p grus-godot`
- stage the GDExtension `.so`
- install Godot 4.6.2
- import/load the Godot project
- run bridge/reset/economy smoke scenes
- export the Linux desktop build
- verify the exported build loads the Rust extension
- run the 200-unit benchmark under Xvfb

The workspace contains:

- `grus-sim`: authoritative gameplay simulation/domain logic
- `grus-godot`: Godot/GDExtension adapter plus the embedded `godot-bevy` app

The Rust workspace is currently pinned to Rust 1.89.0, Bevy 0.18.1 through `godot-bevy 0.11.0`, and `godot` (gdext) 0.4.5.

`bevy-e2e` main targets Bevy 0.19.1 and Rust 1.95, with a client/runtime feature split, out-of-process process management, BRP selectors, waits, inspection, input, failure artifacts, and serialized rendered CI.

## Key decision (amended 2026-09-13): upgrade Grus to Bevy 0.19 via godot-bevy 0.12

The original draft kept Grus on Bevy 0.18 and stopped at the dependency gate because `bevy-e2e` only existed against Bevy 0.19.1. The owner has since directed the upgrade, which resolves the gate directly: `godot-bevy 0.12.0` supports the Bevy 0.19 line (gdext 0.5.5) with feature parity for `api-4-5`, `godot_bevy_log`, and `experimental-threads`, so Grus can meet `bevy-e2e` on the same Bevy minor instead of maintaining a parallel backport.

The migration (Task 0) is a pure dependency/API migration: bump `bevy`/`godot-bevy`/`godot`, fix what the Bevy 0.18 -> 0.19 migration guide requires, keep feature parity and all existing tests green. It must not smuggle in gameplay changes, refactors, or presentation rewrites.

With Grus on Bevy 0.19.1, pin `bevy-e2e` to the exact main-HEAD commit SHA `13f5d331ade246d1248d6ce6ffdb80e65e5765d7` rather than following a moving branch. This remains the dependency gate: if that pin cannot build against the upgraded workspace, stop rather than adding a duplicate Grus-specific E2E framework.

## CI target shape

Keep a single `.github/workflows/ci.yml`, but split it into three jobs:

| Job | Purpose | Runs in parallel? |
| --- | --- | --- |
| `rust-build-lint` | Formatting, Clippy, workspace compile | yes |
| `unit-tests` | Rust tests and `grus-sim` coverage gate | yes |
| `e2e` | Godot integration/export + `bevy-e2e` + benchmark | after the first two |

Add workflow concurrency cancellation so stale PR pushes do not continue consuming runners.

The expensive Godot/Xvfb work should only start after the fast Rust gates pass.

---

## Task 0 — Migrate Grus to Bevy 0.19 (prerequisite, added by amendment)

In the root `Cargo.toml` workspace dependencies:

- `bevy` `=0.18.1` -> `=0.19.1`
- `godot-bevy` `=0.11.0` -> `=0.12.0` (keep `api-4-5`, `godot_bevy_log`, `experimental-threads`)
- `godot` `=0.4.5` -> `=0.5.5`

Update `Cargo.lock` accordingly (commit it; CI builds with `--locked`).

Fix all Bevy 0.18 -> 0.19 and gdext 0.4 -> 0.5 API breakage in `grus-sim` and `grus-godot`, consulting the upstream Bevy 0.19 migration guide. Rules:

- behavior-preserving changes only; no gameplay logic, structure, or naming rewrites
- keep `default-features = false` and the existing minimal feature set; do not enable new Bevy features unless required to compile
- if a system schedule/registration API changed, choose the direct equivalent, not a redesigned schedule

Verification:

```bash
cargo check --workspace --locked
cargo test --workspace
cargo build -p grus-godot --locked
```

Expected: the untouched test suite passes on Bevy 0.19 before any E2E wiring is added.

---

## Task 1 — Align the Rust toolchain with the E2E dependency

Update:

- `rust-toolchain.toml`
- root `Cargo.toml` workspace `rust-version`

Move the project to Rust 1.95.0 so the workspace and `bevy-e2e` use one compiler baseline.

Do not keep a nominal Rust 1.89 MSRV while CI and the E2E dependency require 1.95; Grus has no compatibility requirement that justifies testing two Rust baselines.

Verification:

```bash
rustc --version
cargo check --workspace --locked
```

Expected: all existing Grus crates compile with Rust 1.95 before any E2E wiring is added.

---

## Task 2 — Add the E2E dependency seam without affecting normal builds

In `crates/grus-godot/Cargo.toml`:

- add an `e2e` feature
- add `bevy_e2e` as an optional dependency pinned to `rev = "13f5d331ade246d1248d6ce6ffdb80e65e5765d7"`
- disable default features for the game/runtime dependency
- enable only the framework `runtime` feature in the game
- expose the client side only to the Rust integration test
- pin the dependency to the exact Git commit SHA

Target shape:

```toml
[features]
e2e = ["dep:bevy_e2e"]

[dependencies]
bevy_e2e = {
  git = "https://github.com/cwchanap/bevy-e2e",
  rev = "13f5d331ade246d1248d6ce6ffdb80e65e5765d7",
  optional = true,
  default-features = false,
  features = ["runtime"],
}

[dev-dependencies]
bevy_e2e = {
  git = "https://github.com/cwchanap/bevy-e2e",
  rev = "13f5d331ade246d1248d6ce6ffdb80e65e5765d7",
}
```

If Cargo feature unification makes the runtime/client split ambiguous in this package layout, prefer a tiny workspace-only E2E test crate over leaking the client HTTP dependency into the game library. Do not introduce a general-purpose testing package abstraction.

Acceptance:

- `cargo build -p grus-godot` does not activate the E2E runtime.
- `cargo build -p grus-godot --features e2e` activates the test runtime.
- release/default builds have no E2E behavior unless explicitly enabled.

---

## Task 3 — Register `BevyE2EPlugin` in the existing embedded Bevy app

Use the current `#[bevy_app] fn build_app(app: &mut App)` composition root in `grus-godot`.

Add the E2E plugin only under the feature:

```rust
#[cfg(feature = "e2e")]
app.add_plugins(bevy_e2e::BevyE2EPlugin);
```

Do not create another Bevy app or a standalone Bevy executable. The system under test must remain the real Grus process:

```text
Godot executable
  -> GDExtension / grus-godot
     -> godot-bevy embedded App
        -> grus-sim world
```

The E2E runtime should only activate when the parent harness sets `BEVY_E2E=1`, matching the framework contract.

Do not separately register `RemoteHttpPlugin`; let `BevyE2EPlugin` own the BRP HTTP transport and selected port.

---

## Task 4 — Add a minimal stable E2E selector surface

Add only the selectors required by the first critical E2E test.

Recommended IDs:

```text
grus.ready
player.town-center
enemy.town-center
player.starting-villager
```

Rules:

- selectors are semantic and stable
- selectors live in `grus-godot`, not `grus-sim`
- do not expose raw Bevy `Entity` IDs
- do not encode fixture implementation IDs such as `unit.1` unless that identity is itself part of the gameplay contract
- do not mark every entity in the world

Use a small E2E-only startup/post-startup system to attach `E2eId` to the deterministic authored fixture entities after the fixture is seeded.

If a dedicated `grus.ready` marker is cleaner than overloading a gameplay entity, spawn one E2E-only marker entity behind the feature.

Acceptance:

- every selector resolves to exactly one entity
- normal builds do not contain E2E marker setup systems

---

## Task 5 — Establish the >90% unit coverage gate

Use `cargo-llvm-cov` in CI.

Enforce the threshold on **`grus-sim`**, not the whole workspace:

```bash
cargo llvm-cov \
  -p grus-sim \
  --all-targets \
  --fail-under-lines 90
```

Rationale:

- `grus-sim` is the pure gameplay/domain layer and should be cheap to test thoroughly.
- `grus-godot` contains Godot FFI and presentation/adapter code that is better validated through integration/E2E execution.
- forcing 90% unit coverage on Godot glue would incentivize brittle tests rather than useful confidence.

Implementation sequence:

1. Run `cargo llvm-cov -p grus-sim --all-targets` and record the baseline.
2. Identify real uncovered simulation behavior.
3. Add behavior-oriented tests until coverage is **strictly greater than 90%**.
4. Keep the CI threshold at `90` so 90.0% is the minimum mechanical gate and the implemented suite should land above it with margin.

Prioritize tests around:

- command acceptance/rejection
- movement/pathing and occupancy
- placement validation
- gathering/economy transitions
- construction progression
- production queues and population limits
- age-up behavior
- fixture seeding/invariants
- deterministic reject codes

Do not:

- exclude ordinary handwritten modules just to make the number pass
- add tests that only execute lines without asserting behavior
- add Codecov or another external service in this task

Also continue to run `cargo test -p grus-godot --lib` so pure adapter helpers remain covered by normal unit tests even though they are outside the 90% metric.

---

## Task 6 — Create the `rust-build-lint` job

Extract fast Rust quality checks from the current monolithic CI job.

Steps:

1. checkout
2. install Rust 1.95 with `rustfmt` and `clippy`
3. restore `Swatinem/rust-cache`
4. run formatting
5. run Clippy
6. compile the workspace

Commands:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
```

Do not run unit tests, Godot, exports, or benchmarks in this job.

Acceptance: a compile/lint regression fails as `rust-build-lint`, not as a generic monolithic `rust` job.

---

## Task 7 — Create the `unit-tests` job

Run in parallel with `rust-build-lint`.

Steps:

1. checkout
2. install Rust 1.95
3. install/cache `cargo-llvm-cov`
4. run `grus-sim` coverage tests with the 90% gate
5. run `grus-godot` library unit tests
6. optionally emit an LCOV or text summary as a workflow artifact for debugging

Core commands:

```bash
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90
cargo test -p grus-godot --lib
```

Keep this job Linux-only. Cross-platform unit-test matrices add CI cost without useful signal for the pure simulation layer.

---

## Task 8 — Add the first critical exported-game E2E test

Add one deliberately small integration test, for example:

```text
crates/grus-godot/tests/e2e_boot.rs
```

The first E2E test should prove the complete process boundary, not automate an entire RTS match.

Scenario:

1. launch the already-exported Grus Godot executable through `bevy_e2e::run`
2. wait for `grus.ready`
3. find `player.town-center`
4. find `enemy.town-center`
5. find `player.starting-villager`
6. optionally inspect one reflected, stable piece of ECS state if it is cheap to expose
7. return success and allow the harness to shut down/reap the child cleanly

This proves:

```text
Rust #[test]
 -> bevy-e2e client
 -> exported Godot child process
 -> GDExtension
 -> godot-bevy embedded Bevy ECS
 -> seeded Grus simulation
```

Do not use keyboard/mouse/UI-click APIs in this first Grus E2E test. `bevy-e2e` input injection targets Bevy window/input events, while Grus presentation/input is currently owned by Godot. Bridging Godot input is a separate feature and is not needed to validate the process/ECS integration.

Do not require framework screenshots in the first test. Grus is rendered by Godot, not by a normal Bevy renderer, so screenshot support must not become a blocker for the ECS E2E foundation.

---

## Task 9 — Restructure the Godot/E2E CI job

Rename/rebuild the current expensive CI path as `e2e` and make it depend on both fast jobs:

```yaml
needs: [rust-build-lint, unit-tests]
```

Preserve the useful existing validations.

Target sequence:

```text
checkout
 -> Rust/cache
 -> build grus-godot --features e2e
 -> stage libgrus_godot.so
 -> install Godot 4.6.2
 -> verify Godot version
 -> import project / load extension
 -> existing bridge smoke scene
 -> existing fixture reset smoke
 -> existing economy integration smoke
 -> export Linux x86_64 debug build
 -> verify exported build loads Rust extension
 -> run bevy-e2e exported-game test under Xvfb
 -> existing 200-unit benchmark
 -> upload diagnostics on failure
```

The existing Godot smoke scenes remain valuable. `bevy-e2e` does not replace them because they exercise the Godot-side integration contracts directly.

Build the extension with the E2E feature for this job:

```bash
cargo build -p grus-godot --features e2e
```

Run E2E tests serially and under Xvfb so the launched Godot child inherits `DISPLAY`:

```bash
LIBGL_ALWAYS_SOFTWARE=1 \
xvfb-run -a cargo test \
  -p grus-godot \
  --features e2e \
  --test e2e_boot \
  -- --test-threads=1
```

If the integration test needs the exported executable path, provide it through one explicit environment variable owned by Grus, for example `GRUS_E2E_BINARY`, rather than teaching `bevy-e2e` about Godot project layout.

Start Linux-only. Do not add a Windows matrix in this task.

---

## Task 10 — Failure artifacts and process cleanup

Reuse `bevy-e2e` failure bundles rather than building a Grus-specific diagnostics framework.

Upload on E2E job failure:

```text
test_output/**
export-smoke.log
benchmark.log
```

Add `test_output/` to `.gitignore` if needed.

Preserve the framework expectation that child processes are reaped. Run E2E tests serialized to reduce compositor/GPU/process interference.

If a cheap survivor check can target the exported Grus executable name without false positives, add it as an `if: always()` cleanup assertion. Otherwise rely on the framework process guard in this first integration and defer a Grus-specific process scanner.

---

## Task 11 — Add workflow concurrency cancellation

At workflow level:

```yaml
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
```

This keeps repeated pushes to a draft PR from running obsolete Godot export/benchmark jobs.

No additional workflow files are needed.

---

## Task 12 — Document local verification

Add a concise README testing/CI section with commands equivalent to the three CI gates.

Rust build/lint:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
```

Unit coverage:

```bash
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90
cargo test -p grus-godot --lib
```

E2E:

```bash
cargo build -p grus-godot --features e2e
# stage extension and export Godot build as CI does
GRUS_E2E_BINARY=<path-to-exported-grus> \
  xvfb-run -a cargo test \
  -p grus-godot \
  --features e2e \
  --test e2e_boot \
  -- --test-threads=1
```

Do not introduce a Makefile, `just`, task runner, container image, or custom shell framework solely for these commands.

---

## Expected workflow skeleton

The final workflow should remain conceptually simple:

```yaml
jobs:
  rust-build-lint:
    # fmt + clippy + cargo build

  unit-tests:
    # cargo-llvm-cov grus-sim >= 90%
    # grus-godot lib tests

  e2e:
    needs: [rust-build-lint, unit-tests]
    # existing Godot smoke/export checks
    # exported-game bevy-e2e
    # existing benchmark
```

The exact YAML should reuse the current setup/cache/Godot actions rather than introduce reusable-workflow abstractions for only three jobs.

## Out of scope

Do not include the following in this PR:

- gameplay/behavior changes riding on the Bevy 0.19 migration (API-equivalent fixes only)
- multi-platform E2E matrix
- Godot input automation
- visual regression testing
- Bevy-renderer screenshot requirements
- Codecov or hosted coverage dashboards
- custom BRP methods unless a concrete Grus assertion cannot be expressed with the existing framework
- broad reflection of every gameplay component/resource
- standalone Bevy test game duplicating Grus gameplay
- a new CI/task-runner abstraction
- save/load, multiplayer, or gameplay feature work

## Definition of done

The implementation PR is complete when all of the following are true:

- `rust-build-lint`, `unit-tests`, and `e2e` appear as separate GitHub Actions jobs.
- `rust-build-lint` independently gates formatting, Clippy warnings, and workspace compilation.
- `grus-sim` line coverage is above 90% and CI enforces a 90% minimum.
- `grus-godot` library unit tests still run.
- the existing Godot import/smoke/export validations remain green.
- Grus E2E launches the exported Godot application through `bevy-e2e`.
- the E2E test resolves deterministic markers for ready state, both starting Town Centers, and a starting player villager.
- E2E tests run serialized under Linux/Xvfb.
- E2E failure diagnostics are uploaded.
- the existing 200-unit benchmark remains part of the expensive integration job.
- default/release Grus builds do not activate E2E runtime behavior.
- the Bevy/godot-bevy migration is limited to Task 0's behavior-preserving dependency/API changes, with no gameplay changes riding on it.

## Implementation order / hard gates

1. **Toolchain gate:** Rust 1.95 baseline (Task 1) — required by Bevy 0.19/godot-bevy 0.12 and `bevy-e2e` alike.
2. **Migration gate:** Task 0 Bevy 0.19 migration lands with the untouched test suite green.
3. **Dependency gate:** `bevy-e2e` @ `13f5d331ade246d1248d6ce6ffdb80e65e5765d7` builds against the upgraded workspace. Stop if it cannot, rather than duplicating the framework.
4. Split CI into `rust-build-lint` and `unit-tests`; establish the coverage baseline.
5. Raise `grus-sim` tests above 90% line coverage.
6. Add the feature-gated E2E runtime and selectors.
7. Add the exported-game boot/fixture E2E test.
8. Move the existing Godot/export/benchmark path into the dependent `e2e` job.
9. Add failure artifact upload, concurrency cancellation, and README commands.
10. Run all three CI jobs from a clean checkout before marking the PR ready.
