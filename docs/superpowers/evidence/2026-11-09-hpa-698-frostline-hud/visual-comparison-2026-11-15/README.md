# Frostline visual comparison follow-up

> Evidence retention: this checkout keeps none of these captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/visual-comparison-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 captures from the follow-up branch head. The final
captures were taken from the working tree committed as `2639dc1` (fix round 2)
after `d74899f`, `10be6aa` and `e27c77e`; base `efe40ad`. Godot 4.7.1, OpenGL
4.1 Metal compatibility renderer, Apple M1 Pro. Source PNG mtimes are
2026-11-15 21:32 MST (2026-11-16 04:32 UTC); the files were copied into this
directory without re-encoding. Images do not embed a commit.

Every frame comes from an existing real-input smoke journey unless the row is
labelled **synthetic**. Captures are opt-in (`GRUS_CAPTURE_DIR`), use
`SmokeHelpers.capture_frame`, and wait for `RenderingServer.frame_post_draw` on
a real renderer. No captured frame manufactures gameplay state. The five
synthetic rows are widget probes on copied snapshots (no command, spend or sim
state) and are labelled as such.

| Frame | Journey / provenance | sha256 |
| --- | --- | --- |
| `framing-default.png` | `smoke_test` real opening fixture at the authored 54.0 framing, nothing selected: compact wave card in the top rail, quiet empty-selection hint. | `882b0148e325aa4b373438d78316ac290237a10825f1819a07a7c0a0ad7dd38e` |
| `command-tiles-armed.png` | `smoke_test` real worker selection, real Bunkhouse arming; the separate Wall focus ring is set by the test. | `b6546ab57b174d2dc6c610d0272b8bc0746e3097c284dd85be7fac835e9e44f9` |
| `command-tiles-unaffordable.png` | `economy_smoke_test` real unaffordable Guard click and Rust `InsufficientResources` rejection with the selected Barracks queue row empty. | `1a8a9dd74ba30f00a55b08ed128df1db72d349d02daf257ca92358f59a11a691` |
| `economy-queue-barracks.png` | `economy_smoke_test` real Barracks Guard→Rifleman FIFO: pressed head socket with timer/progress, painted queued socket, recessed empties. | `8e012657120e6ebdc1a3369544415662cdff447676e404a5b482fa35bedb69aa` |
| `economy-queue-overflow.png` | `economy_smoke_test` Research Center after six real Worker clicks: five sockets plus the amber `+1` overflow chip. | `05784c8be53f6a3193a9a8109b3ef975355812feec3a4421901645eabab8fda5` |
| `underground-l3.png` | `underground_smoke_test` real paid L3 with completed Lab/Tap and Tap armed: bounded two-line header, 34 px kind icons over the clear name/price band, level badges, lock/occupied/armed states, three complete rows and the live North warning. | `29e85362a09b5acd1eff486e7dc06f38d65a5a2978e458fcdaba14faf36afaa3` |
| `underground-unarmed.png` | `underground_smoke_test` after disarming the Tap: every empty slot is the flat recess, completed rooms keep their plates, names and benefit bands. | `21ee9c84dc0e31ff179ef607cf89e81023d2cac28880b8333c71416690852900` |
| `underground-construction.png` | `underground_smoke_test` real Lab construction: percent badge top-right, room name centred. | `20be962cbfa04cf2ec842cb08e998b509755230100ffa7a5aa7b486e022f866a` |
| `underground-dig-progress.png` | `underground_smoke_test` real L1 dig: progress fill and amber remaining readout outside the level viewport; the bounded header carries the live percent/time on its second line. | `ab7a109b8851bd1afe55030fbdf8535257d607841f6c77ac3a56daf8cbd442ef` |
| `wave-warning.png` | `wave_smoke_test` real first-wave North warning: amber ring and edge glow, and the card's own visible `Incoming: North` row with the full direction name. | `4a6eb8145c23b4cbf77d58a076e314333a5fc78ee0d60531e1fcd4cacc92e662` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid gameplay. | `e6911300c6f05648a5f89d031948b6d5027f3987c90d1112809813976f46da7e` |
| `underground-long-name.png` | **Synthetic presentation-only L4** long-name/construction probe; the real journey never funds L4. | `75c8ae378bfbacee0714fea14d8a3eba8868f20f362005c5a78cf3ba4efd4c9e` |
| `underground-header-saturated-probe.png` | **Synthetic presentation-only header** copy of the live drawer read with depth 38 / target 39 / the bridge's saturated u32 Scrap + coal / 600 s. The header stays inside the padded drawer and shows the complete price/time; the level rows behind it are the live three-level read, not a fabricated 38-level state. | `1e746551423fbbe2cc6bf2e3b3f7aec3d5d3cf61f712ea279ee0fc86b0cfbcc5` |
| `wave-warning-three-zone-probe.png` | **Synthetic presentation-only wave** copied snapshot consistent with the director's own wave 20 rotation (`wave=19`, `next_wave=20`, regular interval, West/North/East). Shows all three full direction names in the visible warning row; not a real wave 20. | `1f1c9a265c0ad73a1aba05f07cc4ecab8a8977ac31a2755c3270163fa591af2e` |
| `wave-result-warning-probe.png` | **Synthetic presentation-only Result** through the production order (`refresh_session("Result")` then a frozen nonempty warning refresh): the outcome composition shows no wave card and no `Incoming` row. Not a gameplay win. | `63e72d8fe6696d373761e4e54b562d464436a075c6cac2b97dccdfc59c63b7bb` |

## Comparison with the approved mockups and prior captures

Frame names match `../access-shutdown-2026-11-15/` so the two directories can be
compared directly; those frames show the prior head (`0d522d9`).

- **Wave card (mockups 1–3, prior `framing-default.png`).** The card sits 4 px
  under the top bar in the top rail and is 258 px wide; a separate visible
  `Incoming: …` row spells the snapshot's full direction names (one to three
  zones), with the same names and exact wave indices in the readout and card
  accessible descriptions. The warning row is a HUD sibling, so its visibility
  is also gated on the card's in-match visibility: Result hides the card and
  the warning even while the frozen incoming zones are nonempty (see the
  synthetic Result frame), and Playing/Paused restore it with the full names.
  The card cannot be flush with the top bar: the northwest scout click point
  (screen `≈(283,108)`) and the Depot placement point (`≈(310,65)`) sit in the
  band above the old card top, so a flush card at the old width would cover
  them. Narrowing clears the scout click to the card's right (measured
  clearance rises from 11.60 to 15.33 px); the Depot keeps its unchanged
  9.25 px from the top bar. The retained tradeoff is the 4 px rail gap, not a
  lost input target.
- **Underground drawer (mockup 3, prior `underground-l3.png`).** The drawer
  begins flush under the top bar and its header is a bounded two-line block
  (title/depth/Esc line plus the snapshot's own complete price/time, or the
  live dig percent/remaining). Word wrap and `clip_text` bound the block; the
  saturated-price / 600-second probe stays inside the padded drawer; it does
  not cover the maximum projected duration. The room-kind tiles are 72 px tall
  with 34 px icons, the unchanged 32 px clear name/price band and the level
  badge chip; the 152 px viewport
  still shows three complete rows. Names and prices still come from the
  catalogue/snapshots.
- **Lower panels (mockups 1–3, prior `framing-default.png`).** The empty
  selection card keeps its single body-size `Nothing selected` line; the
  selection/command card geometry, the reserved contextual train strip and the
  minimap frame are unchanged. The command card cannot shrink to the mockups'
  ~190 px without hiding the always-available build commands or jumping the
  panel height with the selection, so its 400–710 px footprint stays.

## Verification and limits

At `2639dc1`, all seven headless Godot smokes, the focused shutdown scene and
the shutdown-runner contract exited 0. The economy journey printed
`northwest_scout=15.33 Depot_placement=9.25`; the main smoke's tightest opening
safe source remains the southeast Scrap at 9.3 px from the command card, which
was not moved. No Rust/gameplay file changed this round, so the `d74899f`
`cargo test --workspace --locked` pass (432 passed, 0 failed, one ignored)
still covers the unchanged Rust sources; `cargo fmt --all -- --check` exited 0.

The underground journey's prep margin is mode-dependent: the headless runs
printed `margin_s=35.60` (`logs/fix2-headless`) and `36.55`
(`logs/fix2-final-headless`), while the final windowed capture run printed
`margin_s=15.60`. All exited 0 and pass the same 300 s prep guard; the
windowed run spends more wall-clock time per rendered frame.

The four windowed evidence runs were invoked literally as:

```
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix2-final-captures" godot --path godot res://scenes/smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix2-final-captures" godot --path godot res://scenes/economy_smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix2-final-captures" godot --path godot res://scenes/underground_smoke_test.tscn
GRUS_CAPTURE_DIR="$PWD/.superpowers/sdd/2026-11-15-frostline-visual-comparison-followup/fix2-final-captures" godot --path godot res://scenes/wave_smoke_test.tscn
```

Each exited 0 and wrote the frames above; the retained raw captures and logs
stay under that directory's parent.

Limitations: Linux, export/e2e, screen-reader operation, keyboard-traversal
proof and a paid L4 remain out of scope. The accessible descriptions and the
visible warning row are asserted as values in the smoke tests; this pass does
not operate a screen reader. The five synthetic rows are widget probes; the
Result frame is a synthetic outcome composition, and the saturated-header and
Result probes' gameplay-derived rows/values are the live read and copied
snapshots, not paid gameplay. The wave card's 4 px rail gap and the command
card's height remain the disclosed composition compromises. The focused
shutdown regression passed but its runner deletes successful scratch logs, so
the retained output supports the runner result rather than independent
per-mode raw-log review.
