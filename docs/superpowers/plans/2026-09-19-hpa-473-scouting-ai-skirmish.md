# HPA-473 Scouting-Aware Economic AI Skirmish Implementation Plan

**Goal:** Ship the first complete Grus human-versus-economic-AI match with authoritative fog/visibility, minimap, fair scripted AI, scouting/defense/attack behavior, and restart-safe integration.

**Architecture:** Bevy ECS stays authoritative. Add only `visibility.rs` and `ai.rs`; extend the existing command/combat/placement/reset seams and the Godot presentation bridge. Visibility is the single information gate shared by combat, placement, resource knowledge, rendering, minimap and AI. AI uses ordinary `PlayerCommand` application and never mutates economy/resources/spawns directly. This PR is the implementation PR for HPA-473.

**Tech Stack:** Rust 1.95.0, Bevy 0.19.1, godot-bevy 0.12.0, godot-rust 0.5.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-19-hpa-473-scouting-ai-design.md`

## Global constraints

- Exactly one Linear ticket and one GitHub PR; implementation continues on this branch.
- Preserve the Bevy-authoritative / Godot-presentation boundary.
- Preserve the >90% `grus-sim` production-code line coverage gate.
- Keep each implementation commit buildable/testable; do not land a multi-commit period where the canonical FixedUpdate chain cannot compile.
- Canonical final order: human commands -> combat -> movement -> economy -> construction -> production -> visibility -> AI -> route feedback.
- Add only `visibility.rs` and `ai.rs` as new gameplay modules. No generic AI, knowledge, fog, minimap or service framework.
- Missing `VisibilityMap` means full information for focused pure-sim tests/benchmark only. **Keep `seed_skirmish()` unchanged**; fog-aware Rust tests opt in explicitly, and runtime visibility is enabled later in the same commit that retargets the Godot smokes.
- One visibility geometry contract for every consumer: unit current cell; completed buildings reveal from every footprint cell; Euclidean radius; units are visible by current cell; buildings/resources are visible/known when any footprint cell qualifies.
- Exactly two public knowledge predicates own the fallback: `visible_to(...)` and `explored_by(...)`. They return true when visibility is absent; no consumer checks `VisibilityMap` directly.
- Keep `target_eligible()` as the only combat eligibility seam.
- Keep `validate_placement()` as the only placement authority.
- AI may retain only the last genuinely observed enemy Town Center cell. No live hidden enemy/resource reads.
- AI commands use `apply_player_command()`; no direct stockpile edits, free units, free buildings or spawn helpers.
- Runtime human remains Team 1 and runtime AI Team 2. Rust AI tests parameterize the same policy for both authored starts; do not add side-selection UI.
- Primitive fog/minimap presentation only. No generated art or new SFX is needed, so no separate asset task is required.
- Every AI/entity enumeration that affects a decision is sorted by stable ID before choice.
- The authored 18-resource map is not a planned change. Fix policy/scouting first; map edits require evidence that the AI discovered/reached the expansion and still stalled.
- Final tuning/performance optimization belongs to HPA-474 unless a measured regression blocks this ticket.

---

## Risks / mitigations

- Shared `GridMap` pathing can indirectly reveal that an unexplored blocker exists. Per-team path maps are deliberately out of scope; direct render/select/target/placement-preview leaks are still prohibited.
- `ai.rs` is subject to the >90% production-line coverage gate. Keep each policy step a pure decision function and `step_ai` a thin cadence/apply wrapper.
- Runtime fog breaks prior full-information Godot choreography. Do not enable runtime visibility in a commit before the economy/combat smokes are retargeted.
- Missing visibility intentionally means full info in focused sim tests. Runtime setup/reset coverage must prove normal gameplay always inserts it.

## Task 1: Add authoritative visibility contracts and deterministic reveal tests

**Files:**
- Create: `crates/grus-sim/src/visibility.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-sim/src/map.rs`
- Modify: `crates/grus-sim/src/catalog.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Add: `crates/grus-sim/src/visibility/tests.rs` or inline focused tests

- [ ] Add `CellVisibility::{Unexplored, Explored, Visible}`, `VisibilityMap`, per-team explored/current sets and a monotonic `revision`.
- [ ] Add `VISION_RADIUS_CELLS = 10` beside `ATTACK_MOVE_RADIUS` in `catalog.rs`; do not add per-unit vision tables in this ticket.
- [ ] Add a catalogue regression asserting `VISION_RADIUS_CELLS as f32 >= ATTACK_MOVE_RADIUS` so later tuning cannot create targets inside acquisition range but outside vision.
- [ ] Expose only the small GridMap geometry helpers visibility needs (dimensions / in-bounds); do not expose blocked internals.
- [ ] Lock reveal geometry: unit origin = `world_to_cell(current)`; completed building origins = every `Footprint::cells()`; circle metric = `dx² + dy² <= radius²`.
- [ ] Lock entity predicates used by every consumer: unit visible/known from current cell; building/resource visible/known when **any** footprint cell qualifies. Do not let combat/rendering/AI choose their own center-vs-edge rule.
- [ ] Implement deterministic circle reveal over map cells.
- [ ] Implement exclusive world function `refresh_visibility(world, map)`: living units + completed buildings reveal; incomplete sites do not.
- [ ] Current visibility is recomputed; explored visibility is retained.
- [ ] Revision changes only when a team's effective visibility/exploration changes.
- [ ] Expose exactly two public knowledge predicates, `visible_to(...)` and `explored_by(...)`, over the shared cell/footprint subject shape. Both return true when `VisibilityMap` is absent; later consumers call them unconditionally and never inspect the resource themselves.
- [ ] Keep any geometry-resolution helpers private to `visibility.rs`.
- [ ] Tests: initial reveal, retained exploration after moving away, loss of current vision, map-edge clipping, incomplete-site no-vision, completed-building vision, stable revision when nothing changes.
- [ ] Add an authored-start contract: every `starting_resources()` cell is Explored after initial reveal for its team, while `expansion_resources()` remain Unexplored. Radius 10 is accepted only while that test holds.
- [ ] Derive `PartialEq` on `UnitCommandKind`, `UnitCommand` and `PlayerCommand` so later fairness tests can compare `Vec<PlayerCommand>` directly.
- [ ] Do not auto-create `VisibilityMap` from commands or `seed_skirmish()`.

**Focused verification:**

```bash
cargo test -p grus-sim visibility
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Task 2: Enforce visibility at combat, placement, gathering and fixed-step boundaries

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/tests/system_order.rs`
- Add/modify: fog-aware Rust test setup helpers only

- [ ] Append `RejectReason::Unexplored`; do not reorder prior variants.
- [ ] Make `target_eligible()` call `visible_to(...)` unconditionally; missing visibility keeps full-info behavior through that predicate only.
- [ ] Add direct-attack regression: visible target accepts; once vision is lost the direct order clears and movement/pursuit stops.
- [ ] Add AttackMove regression: hidden target is ignored, ground destination continues, later visible target is reacquired.
- [ ] Do not create a second acquisition filter or enemy query path.
- [ ] Insert the `Unexplored` placement check **immediately after footprint bounds and before occupancy/unit_cells is computed**. Exact order: ownership -> unlock -> bounds -> explored -> occupancy -> affordability -> reachable slot.
- [ ] Add a reject-code regression where an unexplored footprint overlaps a hidden enemy unit/current goal: result must be `Unexplored`, never `Occupied`.
- [ ] `placement_preview()` continues calling the same validator for Team 1; preview/final result stay aligned once runtime fog is enabled in Task 3.
- [ ] Make Gather call `explored_by(...)` unconditionally: unexplored standalone source rejects; explored standalone source remains known after current vision is lost; own completed Farm is valid; an enemy Farm is never selected as an own economic source.
- [ ] In `apply_attack`, when runtime visibility exists collapse nonexistent and live-but-hidden targets to the same `InvalidTarget` rejection. Preserve `TargetMissing` only for the missing-visibility/full-info seam. Add an ID-scanning oracle regression.
- [ ] Extend the executable system-order/focused tests to prove construction completion grants vision only after construction and a newly placed incomplete site grants none.
- [ ] **Do not insert `VisibilityMap` into normal Godot setup/reset in this task.** Rust fog tests opt in explicitly, so this commit leaves existing runtime smokes green.

**Focused verification:**

```bash
cargo test -p grus-sim combat
cargo test -p grus-sim buildings
cargo test -p grus-sim economy
cargo test -p grus-sim --test system_order
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Task 3: Project the sim visibility into enemy views, fog and minimap

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/battlefield_controller.gd`
- Modify: `godot/scenes/unit_view.tscn`
- Modify: `godot/scenes/building_view.tscn`
- Modify: `godot/scenes/resource_view.tscn`
- Create: `godot/scripts/fog_of_war.gd`
- Create: `godot/scripts/minimap.gd`
- Create: `godot/scripts/scouting_ai_smoke_test.gd`
- Create: `godot/scenes/scouting_ai_smoke_test.tscn`
- Modify: `godot/scenes/battlefield.tscn`
- Modify: `godot/scripts/economy_smoke_test.gd`
- Modify: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Modify: existing Godot unit/bridge tests as needed

- [ ] In this same commit, insert fresh `VisibilityMap` in `setup_fixture()` / `reset_fixture_world()`, run initial refresh, remove it in `clear_gameplay_world()`, and add the FixedUpdate refresh after production. Keep `seed_skirmish()` and benchmark reset visibility-free.
- [ ] Add runtime setup + restart assertions that normal gameplay actually has visibility state; the absent-resource fallback must not silently turn a runtime reset into full-info mode.
- [ ] Make unit/building/resource scene roots default-hidden. In `initialize_view_metadata`, stamp the authoritative initial visibility in the **same GodotNodeHandle pass** as identity metadata; `sync_view_visibility` then maintains it beside the existing metadata/health Update systems. A newly instantiated enemy view must never spend a frame visible by default.
- [ ] Friendly units/buildings remain visible. Enemy units/buildings follow current Team-1 visibility. Standalone finite resources become visible after exploration and remain visible. Enemy Farms follow enemy-building current visibility.
- [ ] Make GDScript picking helpers skip `not is_visible_in_tree()` explicitly; do not rely on rendering alone to prevent selection/targeting.
- [ ] Clear a selected enemy building as soon as its view becomes hidden.
- [ ] Make `building_snapshot()` return empty for hidden enemy buildings so stale IDs cannot inspect current health/queue/construction.
- [ ] Filter attacker presentation in `take_presentable_events()`: a hidden enemy attacker cannot provide a tracer origin/stable id to GDScript; still preserve hit/death feedback at a visible friendly target.
- [ ] Add cheap `visibility_revision()` for per-frame polling plus `visibility_snapshot()` with width, height, packed 0/1/2 states **and revision**. Fog/minimap fetch the packed payload only after the cheap revision changes.
- [ ] Implement `fog_of_war.gd` with two primitive MultiMeshInstance3D overlays (Unexplored / Explored). Rebuild only when revision changes. No custom shader.
- [ ] Implement `minimap.gd`: 128x96 ImageTexture from cell states; overlay friendly markers, currently visible enemy markers, explored resource markers and camera viewport.
- [ ] Minimap click recenters the existing camera only; it must not issue gameplay commands.
- [ ] Start the **single growing HPA-473 smoke** here: initial hidden enemies, reveal, hide, stale selection clearing, no hidden minimap marker and minimap recenter. Later tasks extend this same scene/script; do not create a second HPA-473 smoke.
- [ ] Retarget the existing HPA-471 economy smoke under real fog in this PR: its far Storehouse/expansion-resource path must first explore the required cells rather than relying on full information.
- [ ] Retarget the existing HPA-472 combat smoke under real fog in this PR: march/scout into vision before attacking the far enemy villager/Town Center. A queued bridge call is not proof the sim accepted a hidden target.
- [ ] Keep HPA-470/HPA-471/HPA-472 contracts green without copying their full journeys into the new smoke.

**Runtime verification:**

```bash
cargo build -p grus-godot --locked
# stage the rebuilt extension first
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

---

## Task 4: Add the feature-local AI map plan and ordinary-command economy policy

**Files:**
- Create: `crates/grus-sim/src/ai.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/session/tests.rs`
- Modify: `crates/grus-sim/tests/system_order.rs`
- Modify: `godot/scripts/smoke_test.gd`
- Modify: `godot/scripts/economy_smoke_test.gd`
- Modify: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Modify: `godot/scripts/scouting_ai_smoke_test.gd`
- Add: `crates/grus-sim/src/ai/tests.rs` or inline focused tests

- [ ] Add `AiMapPlan` beside `MapFixture` with mirrored authored House/Farm/Storehouse/production slots and one scout/attack route for each team.
- [ ] Keep the plan coordinate-only: no live UnitId/BuildingId/ResourceId references.
- [ ] Make `MapFixture::team_plan(team)` the one authored **Rust** base-coordinate table. Migrate duplicate session/system-order Rust anchors to it and recalculate any exact route/tick assertion that changes. Leave GDScript smoke build constants as human-UI test choreography; do **not** add an `AiMapPlan` bridge snapshot.
- [ ] Add `AiController` with TeamId, 1.0 s decision accumulator, scout-route index, optional genuinely observed enemy-Town-Center cell and a tiny round-robin army-kind cursor.
- [ ] Implement `step_ai(world, map, seconds)` in `grus-sim`. It returns immediately on `!gameplay_active(world)` **before** accumulating time.
- [ ] No persistent squad/goal/planner graph. Derive live military groups from `UnitIndex` each decision.
- [ ] Implement each ordered policy step as a pure decision function over `&World`, controller/decision state and `&AiMapPlan`, returning zero/one `PlayerCommand`. `decide_ai_commands()` is their ordered composition; `step_ai()` is only cadence + apply.
- [ ] Sort every unit/building/resource candidate enumeration by stable ID before selecting; never rely on HashMap iteration order.
- [ ] Lift the existing bridge idle-worker definition into `grus-sim` as one reusable helper: Villager + `WorkerTask::Idle` + no `MoveOrder`. Bridge HUD and AI both consume it.
- [ ] Make AI commands idempotent at 1 Hz: gather/build only truly idle workers; skip already-existing/incomplete authored buildings; enqueue only against an empty/test-bounded queue; never reissue the same scout/AttackMove while the intended MoveOrder/CombatOrder is already active.
- [ ] Gather candidates are explored standalone resources or own completed Farms only. Do not iterate `ResourceIndex` into a visible enemy Farm.
- [ ] Put **Scout before idle-worker allocation/growth/age-up/army training once a surplus worker exists**. Use the lowest stable-ID idle military unit if one exists; otherwise an idle surplus Villager uses Move along the route. Never pull an active economic worker off-task just to scout.
- [ ] Economic behaviors then cover idle-worker allocation, House/Barracks/Archery Range/Farm/Storehouse growth, Age-2 attempt, Stable and round-robin military production. Expansion Storehouse logic may fire only after scouting actually discovers the expansion.
- [ ] Use catalogue costs/ages/producer compatibility; do not duplicate price tables in AI.
- [ ] Apply AI commands with `apply_player_command()` and keep results private from human `CommandFeedback`.
- [ ] Make `LastRouteReject` team-aware inside the same sim feedback resource (latest reject per TeamId). Route failure captures the worker's team before cancellation; the bridge drains Team 1 only, so Team 2 cannot surface/overwrite human route feedback. Do not add a second feedback channel.
- [ ] Insert Team-2 controller in normal setup/reset. Keep benchmark AI-free.
- [ ] Add one narrow `disable_ai_for_test()` bridge helper that succeeds only while MatchSession is Start and removes only `AiController`. HPA-470/471/472 scripted gameplay smokes call it before `start_match()`; the HPA-473 smoke does not. This explicitly chooses AI-free legacy choreography instead of relying on an assumed attack-threshold horizon.
- [ ] Add a bounded headless economy scenario proving gather -> deposit -> build -> train -> Age 2 happens with no stockpile/resource grants.
- [ ] Add worker-loss replacement and House/population-stall recovery scenarios.
- [ ] Add a real raid regression: kill AI workers through combat and prove its later income/production reflects the loss rather than being replenished for free.

**Focused + runtime verification:**

```bash
cargo test -p grus-sim ai
cargo test -p grus-sim economy
cargo test -p grus-sim production
cargo test -p grus-sim combat
cargo build -p grus-godot --locked

# stage rebuilt extension, then run the full runtime smoke set because AI
# is now present in normal setup:
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

The legacy gameplay smokes must assert/call the Start-only AI-disable helper successfully before starting. The HPA-473 smoke must assert AI remains present and observe ordinary AI progress.

---

## Task 5: Add fair scouting, memory, defense and attack/regroup behavior

**Files:**
- Modify: `crates/grus-sim/src/ai.rs`
- Modify: `crates/grus-sim/src/fixture.rs` only if route coordinates need correction
- Modify: `crates/grus-sim/src/visibility.rs`
- Add/modify: AI + visibility focused tests

- [ ] Before every strategic decision, update `remembered_enemy_town_center` only from a currently visible enemy Town Center. Never refresh it from hidden live ECS state.
- [ ] Continue the early-scout contract from Task 4: preserve an already-active route order; on the next leg prefer the lowest stable-ID idle combatant, otherwise an idle surplus Villager. No scout-specific unit/state machine or active-worker retask.
- [ ] Visible enemy threats inside the base-defense radius can trigger a direct Attack from currently available military units.
- [ ] Hidden threats cannot change the defense decision.
- [ ] At six live military units, issue grouped AttackMove. If the enemy Town Center has been observed, use the remembered **ground cell**. Otherwise advance along the authored route.
- [ ] Direct Attack is used only for currently visible enemy entities; remembered locations are AttackMove ground targets, never hidden entity targets.
- [ ] After military losses, the live count drops below threshold; normal production rebuilds and a later decision regroups. No persistent squad repair subsystem.
- [ ] Required invariance regression: seed two independent worlds with identical own/observed AI state, move hidden enemies to different unseen cells, and assert the next AI command list is identical. Do not add a Bevy World-cloning mechanism.
- [ ] Make one hidden threat visible and assert the decision may change to defense.
- [ ] Verify observed Town Center memory persists after vision is lost but contains only the observed cell, not live health/position.
- [ ] Parameterize policy tests for Team 1 and Team 2 using mirrored map plans.

**Focused verification:**

```bash
cargo test -p grus-sim ai
cargo test -p grus-sim visibility
cargo test -p grus-sim combat
```

---

## Task 6: Make lifecycle/restart airtight and prove a complete match from both starts

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `crates/grus-sim/src/ai.rs`
- Modify: `crates/grus-sim/src/visibility.rs`
- Modify: `godot/scripts/scouting_ai_smoke_test.gd`
- Modify: `godot/scenes/scouting_ai_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`

- [ ] Gate AI decisions with the existing Playing check. Start/Paused/Result must produce no AI commands and must not advance the AI decision accumulator.
- [ ] `clear_gameplay_world()` removes `VisibilityMap` and `AiController`, then normal reset reseeds both fresh.
- [ ] Restart regression asserts Unexplored/explored state, remembered Town Center, scout-route cursor and AI state return to their initial values; human pending commands remain cleared by the existing seam.
- [ ] Add a bounded pure-Rust full-match journey with ordinary systems/commands: economy -> scouting -> army -> combat -> Result. Follow the existing `run_army_journey` shape: state/tick capped, no exact tick arithmetic or wall-clock dependency.
- [ ] Run the same AI policy from both authored sides in separate headless scenarios. This is a test parameter, not a new runtime mode.
- [x] Extend the **same** HPA-473 Godot smoke from Task 3 with ordinary AI economy/build/train/scout/age progress and Restart freshness. Do not require a full Victory/Defeat match in Godot CI; Rust owns that proof.
  > Evidence note (Task 7): the smoke's **age beat is Rust-owned** — the sessioned AI full match settles the Result before Age 2, so no bounded, state-predicated Godot age assertion exists to make; this is documented in `godot/scripts/scouting_ai_smoke_test.gd`. Economy/build/train/scout/restart coverage stays in the smoke as planned.
- [ ] Keep the runtime smoke bounded by state/event predicates; avoid brittle “after exactly N ticks” assertions.
- [ ] Assert the bounded journey actually discovers/reaches an authored expansion before any late-game rebuild expectation. The existing 18-resource table is not a planned edit.
- [ ] If the AI reaches the expansion and still stalls because of genuine authored scarcity, stop and revise the plan with that evidence before changing map content. Do not add procedural resource spawning, debug grants or a second fixture.
- [ ] Add the one new smoke to the existing `e2e` CI job. Do not create a fourth CI job just for HPA-473.

**Required runtime gate:**

```bash
cargo test --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
cargo build -p grus-godot --features e2e --locked

# stage extension, then:
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

---

## Task 7: Documentation and final regression/export gate

**Files:**
- Modify: `README.md`
- Modify: HPA-473 spec/plan only if implementation evidence changes a contract
- Modify: CI only for proven smoke/export needs

- [ ] Document fog states, minimap controls, AI behavior boundaries and the fact that AI uses the same economy/command rules.
- [ ] Document runtime Team 1 vs Team 2 and clarify that both-start coverage is automated, not a user-selectable mode.
- [ ] Record any evidence-driven authored map adjustment made in Task 6.
- [ ] Run formatting, Clippy, workspace build/tests and >90% coverage.
- [ ] Run all Godot smokes, Linux export boot, Bevy E2E boot and the retained 200-unit regression benchmark.
- [ ] Confirm no hidden enemy appears through rendering, picking, building inspection, minimap, combat tracer or AI decision traces.
- [ ] Confirm no AI-only stockpile/spawn/build mutation exists.
- [ ] Confirm restart returns a fresh Start state with initial fog and fresh AI memory.
- [ ] Keep the PR draft until implementation + full gate are complete; then move HPA-473 to review with this same PR.

## Definition of done

- Human Team 1 can start a normal match against Team-2 economic AI, gather/build/train, scout through real fog, fight, reach Victory/Defeat and restart without editor/debug controls.
- Unexplored/explored/current visibility is simulation-owned and used consistently by combat, placement, resources, rendering, selection/inspection, minimap and AI.
- AI gathering/building/training/age-up consumes the shared catalogue and ordinary commands; worker/building losses have real economic consequences.
- AI behavior is invariant to hidden enemy positions and responds only when information becomes visible or was genuinely remembered.
- AI scouts, defends visible threats, assembles an attack force and rebuilds after losses.
- Rust tests exercise both authored starting sides.
- All existing regression gates plus the new HPA-473 runtime smoke pass.
- `grus-sim` production-line coverage remains >90%.
- No new gameplay content, generic AI framework, fog shader subsystem, extra map/mode, generated art or final HPA-474 tuning work sneaks into this PR.
