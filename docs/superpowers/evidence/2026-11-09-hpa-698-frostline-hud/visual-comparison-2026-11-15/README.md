# Frostline visual comparison follow-up

Unretouched 1280 × 720 captures from the presentation-only follow-up head
(commits `10be6aa` and `e27c77e`; base `efe40ad`) taken 2026-11-15 with Godot 4.7.1, OpenGL
4.1 Metal compatibility renderer on Apple M1 Pro. Source PNG mtimes are
2026-11-15 17:03 MST (2026-11-16 00:03 UTC); the files were copied into this directory
without re-encoding. Images do not embed a commit.

Every frame comes from an existing real-input smoke journey unless the row is
labelled **synthetic**. Captures are opt-in (`GRUS_CAPTURE_DIR`), use
`SmokeHelpers.capture_frame`, and wait for `RenderingServer.frame_post_draw` on
a real renderer. No frame manufactures gameplay state; the two synthetic L4
frames are widget probes in `underground_smoke_test` (no command, spend or sim
state) and are labelled as such.

| Frame | Journey / provenance | sha256 |
| --- | --- | --- |
| `framing-default.png` | `smoke_test` real opening fixture at the authored 54.0 framing, nothing selected: compact wave card in the top rail, quiet empty-selection hint. | `bfa0a303eafadef37e5d3a4252d0b7a901732fd52f00539e48132ab8ec4bb9c2` |
| `command-tiles-armed.png` | `smoke_test` real worker selection, real Bunkhouse arming; the separate Wall focus ring is set by the test. | `41c72ac3e1d64e1685a5194035d419052566dd6e776115c63a820287b4a87fed` |
| `command-tiles-unaffordable.png` | `economy_smoke_test` real unaffordable Guard click and Rust `InsufficientResources` rejection with the selected Barracks queue row empty. | `07f6bbaffdd167f78cac98b7b755da9b65af4269686e032653f28ae9db8a8677` |
| `economy-queue-barracks.png` | `economy_smoke_test` real Barracks Guard→Rifleman FIFO: pressed head socket with timer/progress, painted queued socket, recessed empties. | `90ad6851de1b0a982db8504a62c41aeb03eb8eccd5b99e9d2e7688ef127547cf` |
| `economy-queue-overflow.png` | `economy_smoke_test` Research Center after six real Worker clicks: five sockets plus the amber `+1` overflow chip. | `7e8e59a4eee8fd1eff9363c7e89c4201af5f3d7c7130ccf1ae763259e3c4a240` |
| `underground-l3.png` | `underground_smoke_test` real paid L3 with completed Lab/Tap and Tap armed: one-line header, 34 px kind icons over the clear name/price band, level badges, lock/occupied/armed states, three complete rows and a live North warning. | `c1eb70fb28536e6075a0867bf2176e00a84d634ceae5df8fda86decb3ce3e9a8` |
| `underground-unarmed.png` | `underground_smoke_test` after disarming the Tap: every empty slot is the flat recess, completed rooms keep their plates, names and benefit bands. | `7a947975e0a138d96c40c019e97dc6291fa6a41cb994eebafa967ec598b3c45c` |
| `underground-construction.png` | `underground_smoke_test` real Lab construction: percent badge top-right, room name centred. | `6e9399ccbb2e4cc881e05c5b1c8012763abe81170d32a62bce2246c40298a8b7` |
| `underground-dig-progress.png` | `underground_smoke_test` real L1 dig: progress fill and amber remaining readout outside the level viewport, one-line header carrying the same values. | `0a343249f5b08c05bd3c473d2ab1d81c7e27325e79df9d5afa6613b6c0011d2f` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid gameplay. | `95aa023afdec7f796b0e0ff5b2a723b5b6b6743628380ac89d99293ed94c6c2d` |
| `underground-long-name.png` | **Synthetic presentation-only L4** long-name/construction probe; the real journey never funds L4. | `972c83f601f84139090b3fa64e1ef12dabbb5baaf004386312ef60e2d5c43c87` |
| `wave-warning.png` | `wave_smoke_test` real first-wave North warning: amber ring and edge glow with the compact card's fourth compass-tag line (`Incoming: N`; the full `North` stays on the card tooltip). | `33e5d0037464dc8062ba38c1f0dec8e6b4cdf9014d80d6d1288d2cb1bf21d163` |

## Comparison with the approved mockups and prior captures

Frame names match `../access-shutdown-2026-11-15/` so the two directories can be
compared directly; those frames show the prior head (`0d522d9`).

- **Wave card (mockups 1–3, prior `framing-default.png`).** The card used to
  float 64 px below the top bar, with a wide empty right half and a detached
  outline. It now sits 4 px under the top bar in the same top rail as the
  research card, is 258 px wide instead of 358 px, and puts the incoming-zone
  warning on its own fourth line. The warning uses the mockup's compass tag
  (`Incoming: N`, `Incoming: W, N, E`) so the authored three-zone warning
  (wave 20+) fits the compact label; the full zone names remain on the card
  tooltip and the directional edge glows. The card cannot be made flush with
  the top bar: the northwest scout click point (screen `≈(283,108)`) and the
  Depot placement point (`≈(310,65)`) sit in the band between the bar and the
  old card top, so a card that reached upward at its old width would cover
  them. Narrowing it clears the scout click to its right, which raises the
  scout's measured clearance from 11.60 to 15.33 px; the Depot stays at its
  unchanged 9.25 px from the top bar. This is the retained tradeoff: a 4 px
  rail gap instead of the mockup's contact edge.
- **Underground drawer (mockup 3, prior `underground-l3.png`).** The room-kind
  tiles were 56 px tall with a 20 px icon squeezed above a 32 px label band, so
  the icons read as fragments. They are now 76 px tall with 34 px icons, the
  unchanged 32 px clear name/price band, and an 11 px required-level badge on
  its own dark chip. The two header lines (title line plus next-dig line) are
  now one digest line, which is where the extra height comes from; the same
  line carries the live dig percent/remaining time while digging and says
  `inspection only` outside Playing. Names and prices still come from the
  catalogue; locked, occupied and armed states are unchanged apart from the
  badge plate.
- **Lower panels (mockups 1–3, prior `framing-default.png`).** The empty
  selection card no longer paints a 22 px `Nothing selected` title: the empty
  state is one body-size line under the empty portrait bezel, leaving the
  composed title area to real selections. The selection and command card
  geometries, the reserved train strip and the minimap frame are unchanged.
  The command card cannot be shortened to the mockups' ~190 px without either
  hiding the always-available build commands when a producer is selected or
  jumping the panel height with the selection; both would remove or destabilise
  working controls, so its 400–710 px footprint is a retained compromise.

## Verification and limits

At commit `e27c77e`, `cargo fmt --all -- --check`, `cargo test --workspace
--locked` (11 suites, 432 passed, 0 failed, one ignored), all seven headless Godot smokes,
the focused shutdown scene, the shutdown-runner contract and the four
real-render smoke journeys (main, economy, underground, wave) exited 0. The
economy journey printed `northwest_scout=15.33 Depot_placement=9.25`; the main
smoke's tightest opening safe source remains the southeast Scrap at 9.3 px from
the command card, which was not moved. The wave smoke also renders a
presentation-only three-zone warning probe on a copied snapshot and asserts the
compass line fits the label and the tooltip keeps the full names.

Limitations: Linux, export/e2e, screen-reader behaviour, keyboard-traversal
proof and a paid L4 remain out of scope. The two L4 frames are synthetic widget
probes. The wave card's 4 px rail gap, the compass-tag warning wording and the
command card's height are the disclosed visual compromises. The final runs of
this pass printed no leak or drain diagnostics; the failed underground re-tests
that established the RED state printed the ticket's existing `2 resources still
in use at exit` on their non-zero exits, and the focused shutdown regression
passed.
