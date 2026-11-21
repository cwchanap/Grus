# HPA-697 — painted building sprites: real-renderer evidence record (Step 4)

**Date:** 2026-11-21 (fix round 1 re-verified) · **Platform:** Oracle Linux ARM64 (aarch64), pinned Godot `4.7.1.stable.official.a13da4feb` at `/home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64`, software GL (llvmpipe) under Xvfb, `godot/bin/libgrus_godot.so` built `--locked` (no Rust diff on this branch; last `crates/` commit predates the branch base).

**Tested head:** every run in §2/§3 and every log under `logs/` was executed on the working tree at commit **`1df85c5`** (`test(hpa-697): real-render capture hooks …` `b3498a9` → `b2b4181` placement-verdict classification → `1df85c5` defenses march fix) with no other diffs than the uncommitted platform `.import` flips. This record plus the two economy stills are committed on top of that head.

This record documents the Step-4 work actually executed on this tree: opt-in capture hooks added to the existing real-input smokes, the seven headless CLI gates, real-renderer (non-headless) replay runs with `GRUS_CAPTURE_DIR`, and the objective gaps that remain open (approved art, MCP-mediated inspection, hosted CI, exported E2E). Nothing here is synthetic: every run below records its actual exit code and marker, every image is a real 1280×720 frame written by `SmokeHelpers.capture_frame` (`GRUS_CAPTURE … 1280x720` lines in the journey logs), and every run's complete stdout/stderr is committed under `logs/`.

## 1. Changes made (presentation/test only; no Rust, no new runner)

- `godot/scripts/smoke_test.gd` — three new opt-in (`SmokeHelpers.capture_enabled()`) captures at existing real-input legs: `overlap-front` (selected Worker keeps its click in front of the painted Research Center facade), `overlap-behind` (the opaque painted roof wins the click over the occluded Worker behind it), `gate-crossing` (Worker stopped inside a completed Gate's corridor, clickable).
- `godot/scripts/combat_lifecycle_smoke_test.gd` — opt-in `damaged-building` capture that waits for the real final assault to drop the Research Center below full health (the world health bar renders only below full health); fog-spawn sampling pinned to 0.25× sim (the `smoke_test.gd` gate-corridor precedent) and the fog-scout staging point moved to `anchor+22` so every wave-1 spawn slot sits ≥2.5 cells outside the 10-cell vision radius before the sample.
- `godot/scripts/economy_smoke_test.gd` — windowed-pacing hardening of the existing journey: the gated stationary-click selector also accepts the `Idle` task and confirms its click across rendered frames; food/coal gatherers park immediately when the banked thresholds are met; `_place_building` captures the placement identity baseline **before** the armed click and retains it across every retry, and each watch sample is classified (accepted-settled / resolved typed reject / pending verdict / pending view / pending disarm) instead of treating a fixed poll count as rejection — silence never re-arms onto the anchor, and the synchronous focused regression for the classifier prints `GRUS_ECONOMY_PLACEMENT_WATCH_PROBE_OK` inside the journey. The windowed march rides 1× (budgets stay exact in match seconds); buffered inputs, render-paced waits and captures ride 1× and restore the caller's march.
- `godot/scripts/defenses_smoke_test.gd` — the journey keeps the **20× march on every host** because its construction is wave-raced by design (one builder must finish the ring inside the 300-s preparation the smoke asserts via the wave clock); what burned match time per awaited frame at 20× under the real renderer was the armed-placement polling (`_place_armed_defense` request drain, census, view sync — ~1 sim-s per catch-up physics step), which now rides 1×, and the ring-completion wait is sized for the builder-paced tail (40 → 220 sim-s: 29 defenses × 5–8 s build + slot walks).
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

Full sweep re-run on the exact `1df85c5` content (2026-11-21, logs committed under `logs/<scene>-headless.log`); all EXIT=0 with zero `SMOKE_FAIL`/`SCRIPT ERROR`, the economy gate carries the new `GRUS_ECONOMY_PLACEMENT_WATCH_PROBE_OK` classifier regression, and the `SMOKE_STRUCTURE_AUDIT source_bytes=editor` CI marker is intact in `smoke_test`. Rust fmt/clippy/tests/coverage/balance are unchanged-scope (this branch carries no Rust diff) and are delegated to hosted CI per the Step-4 brief.

## 3. Real-renderer replay runs (Xvfb `:99`, 1920×1080, llvmpipe ≈4–5 FPS, `gl_compatibility`)

```
DISPLAY=:99 GRUS_CAPTURE_DIR=<dir> /home/opc/opt/godot/Godot_v4.7.1-stable_linux.arm64 --path godot res://scenes/<scene>.tscn
```

| Journey | Exit | Captures | Marker |
| --- | --- | --- | --- |
| smoke_test.tscn | **0** | 15 × 1280×720 | `GRUS_GODOT_SMOKE_OK …` (earlier round, scripts unchanged since) |
| combat_lifecycle_smoke_test.tscn | **0** | 8 × 1280×720 incl. `damaged-building.png` | `GRUS_COMBAT_LIFECYCLE_SMOKE_OK …` (earlier round, scripts unchanged since) |
| economy_smoke_test.tscn | **0** | 5 × 1280×720 incl. **`economy-cold.png` / `economy-warm.png`** | `GRUS_ECONOMY_SMOKE_OK bunkhouse_model=site_to_sprite overlays=cold_warm_recovery …` (`logs/economy_smoke_test-windowed.log`, run at `1df85c5`) |
| defenses_smoke_test.tscn | **0** | — (journey has no capture hooks; none required by the brief) | `GRUS_DEFENSE_SMOKE_OK wall_ring=28 … gate_human_traversal=true …` (`logs/defenses_smoke_test-windowed.log`, run at `1df85c5`) |

The earlier-round windowed failures documented below in §5 (economy exit 1, defenses exit 1) are **fixed** as of `1df85c5`: both journeys now complete green under the real renderer with the wave-clock and gameplay assertions unchanged.

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
| `economy-cold.png` | cold deficit: painted sprites under the icy tint at 15/16 · surface 94% (windowed economy run at `1df85c5`) |
| `economy-warm.png` | heat recovery: the same views back to warm modulate at 27/17 · surface 100% after the Coal Furnace |
| `radius-steal-framing.png` | opaque-roof pick beside a styled Wall (radius-steal probe framing) |
| `mcp-inspection-sheet.png` | three-panel inspection input: (A) HPA-697 Linear mockup, (B) journey start frame, (C) construction-site frame |

`GRUS_CAPTURE` log lines for every artifact above record `1280x720`; all frames come from the real Compatibility renderer (never `--headless`). The smoke/combat stills come from the earlier green windowed runs of the then-committed scripts (unchanged since); the economy cold/warm stills come from the `1df85c5` windowed run whose complete log is committed under `logs/`.

### Committed run logs (`docs/superpowers/evidence/hpa-697-step4/logs/`)

Complete stdout/stderr of every gate in §2 and the two windowed replays in §3, all executed on `1df85c5`: `import.log` (fresh editor import, EXIT=0), one `<scene>-headless.log` per §2 row (each ends in its `GRUS_*_SMOKE_OK` marker), and `economy_smoke_test-windowed.log` / `defenses_smoke_test-windowed.log` (renderer banner `OpenGL API 4.6 … Mesa - llvmpipe`, `GRUS_CAPTURE … 1280x720` lines, journey OK markers, EXIT=0).

## 4. Objective gates — exact status

- **Approved art (HPA-1146): OPEN.** HPA-1146 is Todo/unstarted with no art PR found; no approved replacement exists. The provisional Crystal Lance PNG remains intentionally in place. **No art was generated, no other PR merged, and no approval is claimed.**
- **MCP end-to-end acceptance: OPEN (parked).** This session has no MCP client (shell/file tools only; `pi mcp list` → none). The inspection *input* is committed (`mcp-inspection-sheet.png`) and the working recipe from the Step-1 round-5 probe stands: chrome-devtools MCP with `--headless --isolated --executablePath /home/opc/.cache/ms-playwright/chromium-1254/chrome-linux-arm64/chrome` (controller's stored config needs those flags; Chrome stable is absent on this VM); the godot MCP is structurally excluded (project mutation + its own runner). No MCP inspection was performed and none is claimed.
- **Hosted CI: OPEN — requires push.** The branch is 13 commits ahead of `origin/hpa-697-painted-building-sprites`; this task stays inside the worktree and does not push. fmt/clippy/workspace tests/sim coverage/balance/export/benchmark conclusions for the final head must be taken from the resulting GitHub Actions run after push.
- **Exported-build E2E: NOT RUN — not feasible on this platform.** `~/.local/share/godot/export_templates/` is empty and the only export preset is `Linux x86_64` (`godot/export_presets.cfg`), which cannot execute natively on this ARM64 host. The gate stays with hosted CI (x86_64 runner) after push.
- **Windowed cold-deficit / warm-recovery stills: PRODUCED** (`economy-cold.png` / `economy-warm.png`, committed; windowed economy run exit 0 at `1df85c5`). The cold/warm *assertions* remain deterministically green in the headless economy gate (`overlays=cold_warm_recovery`).

## 5. Known gaps and their exact root causes (honest record)

1. **FIXED as of `1df85c5` — windowed economy (exit 1 → exit 0).** Earlier runs died in leg 1: at 20× under the ~4.5 FPS renderer every buffered-input await burned ≈4 sim-s of the 300-s wave preparation, and the click-heavy legs overran it (the Research Center, untrained at the swarm's arrival, fell to the assault ~t≈320). Two earlier failure modes were also found and fixed in the inherited work (a Gathering-only gated selector that never matched the Idle park subjects; a placement watch that re-armed onto an anchor whose placement had actually succeeded). The fix that holds: the windowed economy march rides 1× (every authored budget stays exact in match seconds), inputs/view-sync waits/captures ride 1×, and `_place_building` classifies each watch sample (typed reject vs pending verdict vs pending view/disarm) with the identity baseline captured before the armed click. Headless behavior unchanged (20× march, all assertions green).
2. **FIXED as of `1df85c5` — windowed defenses (exit 1 → exit 0).** Earlier runs failed nondeterministically at 20× (`Attack command queued` on a fixed-screen-point click racing wave-1 alien positions; `simulation accepted Wall placement at (67,45) but its view never appeared` — the census poll outpaced by the view sync while each awaited physics step burned ~1 sim-s). Root cause of the fragility was pacing, not presentation: the journey is wave-raced by design (one builder must finish 29 defenses inside the 300-s preparation), so a whole-journey 1× march cannot fit its own wave-clock guard. Fix: keep the 20× march on every host, ride the armed-placement polls (and every buffered click) at 1×, and size the ring-completion wait for the builder-paced tail (40 → 220 sim-s). Headless and windowed now both pass with every assertion — including the `wave == 0 / next_wave == 1` guard — unchanged.
3. Items in §4 as marked: approved art (HPA-1146), MCP-mediated inspection, hosted CI on the final head, and the exported-build E2E all remain OPEN and are recorded without projection. No failure is hidden, no test was skipped, and no result above is projected rather than observed.

## 6. Reproduction

Fresh import, then the seven headless gates (§2 commands), then the real-renderer replays (§3 commands) with `GRUS_CAPTURE_DIR` pointing at a writable directory. The committed PNGs correspond byte-for-byte to the `GRUS_CAPTURE` lines in `logs/economy_smoke_test-windowed.log` (cold/warm stills, `1df85c5`) and in the earlier green smoke/combat windowed runs (scripts unchanged since those runs). The committed `logs/` files are the complete, unedited stdout/stderr of the runs cited in §2/§3 on the exact tested head.
