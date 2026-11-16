# Frostline visual comparison follow-up

Unretouched 1280 × 720 captures from the follow-up branch head. The final
captures were taken from the working tree committed as `d74899f` (review fix
round) on top of `10be6aa` and `e27c77e`; base `efe40ad`. Godot 4.7.1, OpenGL
4.1 Metal compatibility renderer, Apple M1 Pro. Source PNG mtimes are
2026-11-15 17:55 MST (2026-11-16 00:55 UTC); the files were copied into this
directory without re-encoding. Images do not embed a commit.

Every frame comes from an existing real-input smoke journey unless the row is
labelled **synthetic**. Captures are opt-in (`GRUS_CAPTURE_DIR`), use
`SmokeHelpers.capture_frame`, and wait for `RenderingServer.frame_post_draw` on
a real renderer. No captured frame manufactures gameplay state. The four
synthetic rows are widget probes on copied snapshots (no command, spend or sim
state) and are labelled as such.

| Frame | Journey / provenance | sha256 |
| --- | --- | --- |
| `framing-default.png` | `smoke_test` real opening fixture at the authored 54.0 framing, nothing selected: compact wave card in the top rail, quiet empty-selection hint. | `882b0148e325aa4b373438d78316ac290237a10825f1819a07a7c0a0ad7dd38e` |
| `command-tiles-armed.png` | `smoke_test` real worker selection, real Bunkhouse arming; the separate Wall focus ring is set by the test. | `714a6cfbaed39f786d3b87136fa580eb2cb2ce1f106e5b54809e189a981cbf53` |
| `command-tiles-unaffordable.png` | `economy_smoke_test` real unaffordable Guard click and Rust `InsufficientResources` rejection with the selected Barracks queue row empty. | `de8e7d9190d52997998c1039b5c90ce511ef7384d228d663998ad4b27fd43996` |
| `economy-queue-barracks.png` | `economy_smoke_test` real Barracks Guard→Rifleman FIFO: pressed head socket with timer/progress, painted queued socket, recessed empties. | `1de706f6b88ae1fd5b2564d2f2b366faf0af09733e301228d4130a45975febeb` |
| `economy-queue-overflow.png` | `economy_smoke_test` Research Center after six real Worker clicks: five sockets plus the amber `+1` overflow chip. | `05784c8be53f6a3193a9a8109b3ef975355812feec3a4421901645eabab8fda5` |
| `underground-l3.png` | `underground_smoke_test` real paid L3 with completed Lab/Tap and Tap armed: bounded two-line header, 34 px kind icons over the clear name/price band, level badges, lock/occupied/armed states, three complete rows and the live North warning. | `2ad1e26fe04a7f5774e4f381eebc4c472f50e12fe38e20afcd958964b2a27a74` |
| `underground-unarmed.png` | `underground_smoke_test` after disarming the Tap: every empty slot is the flat recess, completed rooms keep their plates, names and benefit bands. | `6ed88dff969d1b861ca71c715da84919bbb3b08030d632feba92e978cf1635ac` |
| `underground-construction.png` | `underground_smoke_test` real Lab construction: percent badge top-right, room name centred. | `24dc2fd8b69a55dc410b43683bac16a822daf2e6013ad9ec55bd71a5eec60713` |
| `underground-dig-progress.png` | `underground_smoke_test` real L1 dig: progress fill and amber remaining readout outside the level viewport; the bounded header carries the live percent/time on its second line. | `46b4132484d7cd24d77956a8bad14c09f37a75716e4c154bf7604c1a13ad7150` |
| `wave-warning.png` | `wave_smoke_test` real first-wave North warning: amber ring and edge glow, and the card's own visible `Incoming: North` row with the full direction name. | `cfabd2ab8e66987cfa3a4a9c02432c27c40e9459354cf7ac08593d04088413e4` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid gameplay. | `674fe1d0a2d45e23af038928ac7e7207318b278a202c8a7e6e71f2f3e4478a0b` |
| `underground-long-name.png` | **Synthetic presentation-only L4** long-name/construction probe; the real journey never funds L4. | `9455e7740d9a620f5c6958f5d64aafa864d09862421038602c0c79a9a4b949e6` |
| `underground-header-saturated-probe.png` | **Synthetic presentation-only header** copy of the live drawer read with depth 38 / target 39 / the bridge's saturated u32 Scrap + coal / 600 s. The header stays inside the padded drawer and shows the complete price/time; the level rows behind it are the live three-level read, not a fabricated 38-level state. | `729f7467a0f1257d9153a3e585f95fdab00952c8d1471ac33e5365cbf0767f33` |
| `wave-warning-three-zone-probe.png` | **Synthetic presentation-only wave** copied snapshot consistent with the director's own wave 20 rotation (`wave=19`, `next_wave=20`, regular interval, West/North/East). Shows all three full direction names in the visible warning row; not a real wave 20. | `0ff3629670ade174cd56df2d1ba29567667125644acd0e62270546e54a5768fe` |

## Comparison with the approved mockups and prior captures

Frame names match `../access-shutdown-2026-11-15/` so the two directories can be
compared directly; those frames show the prior head (`0d522d9`).

- **Wave card (mockups 1–3, prior `framing-default.png`).** The card used to
  float 64 px below the top bar with a wide empty right half. It now sits 4 px
  under the top bar in the top rail, is 258 px wide, and carries a separate
  visible `Incoming: …` row that spells the snapshot's full direction names
  (one to three zones), with the same names and the exact wave indices in the
  readout and card accessible descriptions. The card cannot be flush with the
  top bar: the northwest scout click point (screen `≈(283,108)`) and the Depot
  placement point (`≈(310,65)`) sit in the band above the old card top, so a
  flush card at the old width would cover them. Narrowing clears the scout
  click to the card's right (measured clearance rises from 11.60 to 15.33 px);
  the Depot keeps its unchanged 9.25 px from the top bar. The retained
  tradeoff is the 4 px rail gap, not a lost input target.
- **Underground drawer (mockup 3, prior `underground-l3.png`).** The drawer now
  begins flush under the top bar and its header is a bounded two-line block
  (title/depth/Esc line plus the snapshot's own complete price/time, or the
  live dig percent/remaining). Word wrap and `clip_text` bound the block; the
  saturated probe shows the longest projected u32 price/time inside the padded
  drawer. The room-kind tiles are 72 px tall with 34 px icons, the unchanged
  32 px clear name/price band and the level badge chip; the 152 px viewport
  still shows three complete rows. Names and prices still come from the
  catalogue/snapshots.
- **Lower panels (mockups 1–3, prior `framing-default.png`).** The empty
  selection card keeps its single body-size `Nothing selected` line; the
  selection/command card geometry, the reserved contextual train strip and the
  minimap frame are unchanged. The command card cannot shrink to the mockups'
  ~190 px without hiding the always-available build commands or jumping the
  panel height with the selection, so its 400–710 px footprint stays.

## Verification and limits

At `d74899f`, `cargo fmt --all -- --check` and `cargo test --workspace
--locked` (11 suites, 432 passed, 0 failed, one ignored), all seven headless
Godot smokes, the focused shutdown scene and the shutdown-runner contract
exited 0. The economy journey printed `northwest_scout=15.33
Depot_placement=9.25`; the main smoke's tightest opening safe source remains
the southeast Scrap at 9.3 px from the command card, which was not moved. The
underground smoke prints `margin_s=35.60`.

The four windowed evidence runs were invoked literally as:

```
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix1-final-captures" godot --path godot res://scenes/smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix1-final-captures" godot --path godot res://scenes/economy_smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix1-final-captures" godot --path godot res://scenes/underground_smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix1-final-captures" godot --path godot res://scenes/wave_smoke_test.tscn
```

Each exited 0 and wrote the frames above; the retained raw captures and logs
stay under that directory's parent.

Limitations: Linux, export/e2e, screen-reader operation, keyboard-traversal
proof and a paid L4 remain out of scope. The accessible descriptions and the
visible warning row are asserted as values in the smoke tests; this pass does
not operate a screen reader. The four synthetic rows are widget probes, and
the saturated-header probe's level rows are the live three-level read. The
wave card's 4 px rail gap and the command card's height remain the disclosed
composition compromises. The focused shutdown regression passed but its
runner deletes successful scratch logs, so the retained output supports the
runner result rather than independent per-mode raw-log review.
