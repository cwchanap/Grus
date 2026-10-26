# HPA-449 Task 1 Report

## Implemented

- Added `Swarmer`, `Spitter`, `Brute`, and `Behemoth` to the typed unit catalogue, with their threat, first-wave, reward, health, speed, and combat values. `UnitKind::ALL` includes all seven kinds; `UnitKind::HUMAN` is the explicit training/catalogue subset. Guard and Rifleman have initial +5 Swarmer and +4 Spitter counter bonuses; alien bonuses remain zero.
- Catalogued integer wave timing, zone breakpoints, live cap, and budget coefficients. Added the widened, saturating budget function and pure roster planner. Rosters remain counts; ordered-prefix materialization is capped at 150 even when passed `usize::MAX`.
- Added fixture-owned North/East/South/West entry anchors and a fixture test proving each anchor is walkable and can path to a walkable Research Center perimeter candidate.
- Added the two-field `WaveState` resource with active-phase gated advance, due-wave ranges, derived countdown and warning zones, and pure zone rotation.
- Kept production and bridge player-facing behavior human-only. Alien enqueue commands reject as `WrongProducer` without spending; bridge parsing rejects alien names and its catalogue snapshot iterates `HUMAN`.

## TDD and verification

- Red: `rtk cargo test -p grus-sim --locked` failed before implementation with 74 expected compile errors for the missing catalogue, zone, and wave APIs.
- Green focused: `rtk cargo test -p grus-sim --locked` — 215 passed; `rtk cargo test -p grus-godot --lib --locked` — 15 passed.
- Formatting: `rtk cargo fmt --all -- --check` — passed.
- Clippy: `rtk cargo clippy -p grus-sim --all-targets --locked -- -D warnings` and `rtk cargo clippy -p grus-godot --all-targets --locked -- -D warnings` — both passed.
- Final workspace suite, run once before commit: `rtk cargo test --workspace --locked` — 231 passed across 8 suites.
- Diff hygiene: `rtk git diff --check` — passed.

## Self-review and boundary

Checked all `UnitKind::ALL` callers and retained the existing production allowlist. Roster generation does not allocate by threat budget, and prefix generation clamps at the catalogued live cap. Team 2 remains in the fixture as requested. This task does not wire wave ticking, spawning, rewards, bridge snapshots, or Godot HUD/minimap behavior; those runtime paths are still unverified and belong to later stages.

## Changed files

- `crates/grus-sim/src/catalog.rs`
- `crates/grus-sim/src/catalog/tests.rs`
- `crates/grus-sim/src/fixture.rs`
- `crates/grus-sim/src/fixture/tests.rs`
- `crates/grus-sim/src/lib.rs`
- `crates/grus-sim/src/production/tests.rs`
- `crates/grus-sim/src/waves.rs`
- `crates/grus-godot/src/lib.rs`
- `.superpowers/sdd/2026-10-24-hpa-449-wave-director/task-1-report.md`
