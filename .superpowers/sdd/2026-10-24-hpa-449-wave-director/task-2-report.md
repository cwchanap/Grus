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
