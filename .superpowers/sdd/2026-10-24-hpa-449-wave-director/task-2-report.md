# HPA-449 task 2: Crystal kill credit

## Changes

- `combat.rs::strike` now reads the victim's `Unit` and optional alien catalogue metadata at the unit kill boundary. A Team 1 attacker killing a Team 2 alien adds `crystal_reward` to Team 1's existing `TeamEconomy` stockpile with saturating addition, before the victim is despawned. Human kills, Team 2 attacks, and victims outside Team 2 receive no credit. The award does not depend on `CombatEvents`.
- `combat/tests.rs` covers no credit before death, one-time credit at death, reward accumulation from Swarmer and Spitter metadata, human and team mismatch exclusions, and the Guard/Swarmer and Rifleman/Spitter counter damage matchups plus nonmatching cases. Existing counter comments were corrected to match the alien matchups.

## TDD and verification

- Red before the implementation: `rtk proxy cargo test -p grus-sim crystal_rewards -- --nocapture` failed in `team_one_crystal_rewards_arrive_once_on_alien_kills_and_accumulate`: stockpile stayed at 7 instead of reaching 8 after the Swarmer kill. The eligibility exclusion test passed.
- Counter baseline: `rtk proxy cargo test -p grus-sim human_counter_damage_only_applies_to_catalogued_alien_matchups -- --nocapture` passed, as did `rtk proxy cargo test -p grus-sim human_counters_target_their_alien_matchups_and_aliens_have_no_counters -- --nocapture`; stage 1 already had the catalogue and strike behavior.
- Green after implementation: `rtk proxy cargo test -p grus-sim crystal_rewards -- --nocapture` passed (2 tests); `rtk proxy cargo test -p grus-sim human_counter_damage_only_applies_to_catalogued_alien_matchups -- --nocapture` passed (1 test).
- Workspace: `rtk cargo test --workspace --locked` passed (234 tests across 8 suites).
- Formatting and whitespace: `rtk cargo fmt --all -- --check` and `rtk git diff --check` passed.

## Concerns

No known concerns. Godot and device smokes were not run because this stage changes only the simulation kill boundary and does not add presentation behavior.

## Review round 1: seeded fixture coverage

Added `seeded_skirmish_credits_crystal_reward_to_existing_team_one_stockpile` in `combat/tests.rs`. The regression calls `seed_skirmish`, allocates fresh unit IDs from its `IdAllocator`, spawns a Team 1 Guard and Team 2 Brute through `spawn_unit`, and verifies the existing Team 1 stockpile gains exactly the Brute's catalogued 5 crystals while its other fields stay unchanged. The isolated exact-once, accumulation, and eligibility tests remain.

Command: `rtk proxy cargo test -p grus-sim crystal_reward -- --nocapture`

Output (exit code 0):

```text
   Compiling grus-sim v0.1.0 (/Users/chanwaichan/orca/workspaces/Grus/hpa-449-draft-endless-alien-wave-director-design-2/crates/grus-sim)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.12s
     Running unittests src/lib.rs (target/debug/deps/grus_sim-186ffbac82252344)

running 3 tests
test combat::tests::seeded_skirmish_credits_crystal_reward_to_existing_team_one_stockpile ... ok
test combat::tests::crystal_rewards_require_a_team_one_killer_and_team_two_alien_victim ... ok
test combat::tests::team_one_crystal_rewards_arrive_once_on_alien_kills_and_accumulate ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 211 filtered out; finished in 0.01s

     Running tests/economy_flow.rs (target/debug/deps/economy_flow-f691a4b7d1f51784)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests/occupancy_replan.rs (target/debug/deps/occupancy_replan-748081ef9654a5a3)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests/system_order.rs (target/debug/deps/system_order-b28b681a0807ab1e)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
```
