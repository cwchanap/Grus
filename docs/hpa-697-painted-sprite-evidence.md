# HPA-697 — painted building sprites: real-renderer evidence record (Step 4)

**Date:** 2026-11-21 · **Platform:** Oracle Linux ARM64 (aarch64), pinned Godot `4.7.1.stable.official.a13da4feb` at `/home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64`, software GL (llvmpipe) under Xvfb, `godot/bin/libgrus_godot.so` built `--locked` (no Rust diff on this branch; last `crates/` commit predates the branch base).

This record documents the Step-4 work actually executed on this tree: opt-in capture hooks added to the existing real-input smokes, the seven headless CLI gates, real-renderer (non-headless) replay runs with `GRUS_CAPTURE_DIR`, and the objective gaps that remain open (approved art, MCP-mediated inspection, hosted CI, exported E2E). Nothing here is synthetic: every run below records its actual exit code and marker, and every image is a real 1280×720 frame written by `SmokeHelpers.capture_frame` (`GRUS_CAPTURE … 1280x720` lines in the journey logs).

## 1. Changes made (presentation/test only; no Rust, no new runner)

- `godot/scripts/smoke_test.gd` — three new opt-in (`SmokeHelpers.capture_enabled()`) captures at existing real-input legs: `overlap-front` (selected Worker keeps its click in front of the painted Research Center facade), `overlap-behind` (the opaque painted roof wins the click over the occluded Worker behind it), `gate-crossing` (Worker stopped inside a completed Gate's corridor, clickable).
- `godot/scripts/combat_lifecycle_smoke_test.gd` — opt-in `damaged-building` capture that waits for the real final assault to drop the Research Center below full health (the world health bar renders only below full health); fog-spawn sampling pinned to 0.25× sim (the `smoke_test.gd` gate-corridor precedent) and the fog-scout staging point moved to `anchor+22` so every wave-1 spawn slot sits ≥2.5 cells outside the 10-cell vision radius before the sample.
- `godot/scripts/economy_smoke_test.gd` — windowed-pacing hardening of the existing journey: the gated stationary-click selector (`_select_unoccluded_unit`) also accepts the `Idle` task (sources deplete exactly when the parks run; a Gathering-only gate never matches and at 20× each polled frame is one sim-second) and confirms its click across rendered frames; food/coal gatherers park immediately when the banked thresholds are met (out of the southeast source ring before wave 1); the builder parks before the rally leg; `_place_building` returns on the kind's new building view **and**, for non-Wall/Gate kinds, the armed placement being cancelled again (a request is a one-tick transient a slow renderer can outpace, and later real clicks must not land while a placement is armed — Wall/Gate stay armed by design and their callers cancel explicitly).
- `.import` flips for `structure_albedo.png` / `snow_albedo.png` (s3tc → etc2) are this machine's platform reimport; left uncommitted per standing policy.

## 2. Deterministic CLI gates — pinned Godot 4.7.1, fresh editor import, final tree

```
/home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64 --headless --path godot --editor --quit-after 120   # EXIT=0
/home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64 --headless --path godot res://scenes/<scene>.tscn
```

| Scene | Exit | Marker (abridged) | `SMOKE_FAIL`/`SCRIPT ERROR` |
| --- | --- | --- | --- |
| smoke_test | 0 | `GRUS_GODOT_SMOKE_OK … structures=painted_catalogue_hookup silhouette_pick=opaque_sprite_rc radius_steal=rect_scoped overlap_pick=depth_scoped_right_click dispatch_target=tuple_readback` + `SMOKE_STRUCTURE_AUDIT source_bytes=editor` | 0 |
| reset_test | 0 | `GRUS_RESET_SMOKE_OK units=4 buildings=1 resources=18 …` | 0 |
| economy_smoke_test | 0 | `GRUS_ECONOMY_SMOKE_OK bunkhouse_model=site_to_sprite overlays=cold_warm_recovery zero_demand=wall_gate_furnace bounds=catalogue_grounded …` | 0 |
| combat_lifecycle_smoke_test | 0 | `GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh … fog=hidden_then_revealed markers=visibility_gated controller_attack_move=accepted …` | 0 |
| wave_smoke_test | 0 | `GRUS_WAVE_SMOKE_OK …` | 0 |
| defenses_smoke_test | 0 | `GRUS_DEFENSE_SMOKE_OK wall_ring=28 … gate_human_traversal=true …` | 0 |
| underground_smoke_test | 0 | `GRUS_UNDERGROUND_SMOKE_OK gathered_scrap=730 gathered_coal=110 paid_scrap=1026 …` | 0 |

Full sweep re-run on the exact final content (2026-11-21 00:35–00:38 UTC); the `SMOKE_STRUCTURE_AUDIT source_bytes=editor` CI marker is intact in `smoke_test`. Rust fmt/clippy/tests/coverage/balance are unchanged-scope (this branch carries no Rust diff) and are delegated to hosted CI per the Step-4 brief.

## 3. Real-renderer replay runs (Xvfb `:99`, 1920×1080, llvmpipe ≈4–5 FPS, `gl_compatibility`)

```
DISPLAY=:99 GRUS_CAPTURE_DIR=<dir> /home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64 --path godot res://scenes/<scene>.tscn
```

| Journey | Exit | Captures | Marker |
| --- | --- | --- | --- |
| smoke_test.tscn | **0** | 15 × 1280×720 | `GRUS_GODOT_SMOKE_OK …` |
| combat_lifecycle_smoke_test.tscn | **0** | 8 × 1280×720 incl. `damaged-building.png` | `GRUS_COMBAT_LIFECYCLE_SMOKE_OK …` |
| economy_smoke_test.tscn | 1 | — (died in leg 1; see §5) | — |
| defenses_smoke_test.tscn | 1 | — (journey has no capture hooks; see §5) | — |

### Committed capture artifacts (`docs/superpowers/evidence/hpa-697-step4/`)

From the green windowed `smoke_test` / `combat_lifecycle` runs above:

| File | Brief capture point |
| --- | --- |
| `smoke-start.png` | starting painted city (opening framing, 4 workers, HUD) |
| `construction-site.png` | active construction site (shared site sprite mid-build) |
| `framing-default.png` / `framing-zoomout.png` | city at the default 54-size and wheel-zoomed 72-size framing |
| `overlap-front.png` | unit in front of the painted facade keeps the click (selected) |
| `overlap-behind.png` | unit behind the opaque painted roof loses the click (RC selected) |
| `gate-crossing.png` | Worker stopped inside a completed Gate's corridor |
| `selected-building.png` | selected painted Research Center with HUD HP/heat detail |
| `damaged-building.png` | damaged painted Research Center under real assault, world health bar visible |
| `radius-steal-framing.png` | opaque-roof pick beside a styled Wall (radius-steal probe framing) |
| `mcp-inspection-sheet.png` | three-panel inspection input: (A) HPA-697 Linear mockup, (B) journey start frame, (C) construction-site frame |

`GRUS_CAPTURE` log lines for every artifact above record `1280x720`; all frames come from the real Compatibility renderer (never `--headless`).

## 4. Objective gates — exact status

- **Approved art (HPA-1146): OPEN.** HPA-1146 is Todo/unstarted with no art PR found; no approved replacement exists. The provisional Crystal Lance PNG remains intentionally in place. **No art was generated, no other PR merged, and no approval is claimed.**
- **MCP end-to-end acceptance: OPEN (parked).** This session has no MCP client (shell/file tools only; `pi mcp list` → none). The inspection *input* is committed (`mcp-inspection-sheet.png`) and the working recipe from the Step-1 round-5 probe stands: chrome-devtools MCP with `--headless --isolated --executablePath /home/opc/.cache/ms-playwright/chromium-1254/chrome-linux-arm64/chrome` (controller's stored config needs those flags; Chrome stable is absent on this VM); the godot MCP is structurally excluded (project mutation + its own runner). No MCP inspection was performed and none is claimed.
- **Hosted CI: OPEN — requires push.** The branch is 10 commits ahead of `origin/hpa-697-painted-building-sprites`; this task stays inside the worktree and does not push. fmt/clippy/workspace tests/sim coverage/balance/export/benchmark conclusions for the final head must be taken from the resulting GitHub Actions run after push.
- **Exported-build E2E: NOT RUN — not feasible on this platform.** `~/.local/share/godot/export_templates/` is empty and the only export preset is `Linux x86_64` (`godot/export_presets.cfg`), which cannot execute natively on this ARM64 host. The gate stays with hosted CI (x86_64 runner) after push.
- **Windowed cold-deficit / warm-recovery stills: NOT PRODUCED — see §5.** The cold/warm *assertions* remain deterministically green in the headless economy gate (`overlays=cold_warm_recovery`).

## 5. Known gaps and their exact root causes (honest record)

1. **`economy_smoke_test.tscn` under the real renderer (exit 1).** The first leg is a ~300 s-of-match-time race against wave 1: the click-heavy legs between the stockpile bank and the rallied-Worker spawn consume ≈4 sim-seconds per buffered-input await at 20× under the ~4.5 FPS renderer (≈100 extra match-seconds vs headless), so the enqueue point — and with the Research Center untrained at the swarm's arrival (~310–315 s), the Center itself — falls past wave 1's assault. The hardening in §1 removed the earlier failure modes (gated selects never matching Idle sources, placement-armed clicks, the wandering builder stealing the rally click); what remains is a journey-design margin, not a presentation defect: the deterministic headless gate is green, and the fix (training the first workers before the expansion legs, or reSTRUCTURING the first leg's funding) is an economy-journey redesign that deserves its own review round. `economy-cold`/`economy-warm` stills are therefore not produced this round.
2. **`defenses_smoke_test.tscn` under the real renderer (exit 1, nondeterministic leg).** Two different pacing failures observed on two runs (`controller did not issue the ordinary move through the Gate: Attack command queued` — a fixed-screen-point click racing wave-1 alien positions; `simulation accepted Wall placement at (67,45) but its view never appeared` — view-sync lag). The journey has no capture hooks (none are required by the brief's capture list) and its deterministic gate is the green headless run.
3. Items in §4 as marked. No failure is hidden, no test was skipped, and no result above is projected rather than observed.

## 6. Reproduction

Fresh import, then the seven headless gates (§2 commands), then the real-renderer replays (§3 commands) with `GRUS_CAPTURE_DIR` pointing at a writable directory. The committed PNGs correspond byte-for-byte to the `GRUS_CAPTURE` lines in the journey logs of the green windowed runs.
