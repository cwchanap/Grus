# Frostline battlefield framing and command tiles

> Evidence retention: this checkout keeps a representative subset of the captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/framing-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 viewport captures from implementation head `7a66955`, taken 2026-11-15 on Godot 4.7.1, OpenGL 4.1 Metal compatibility renderer, Apple M1 Pro. Times are source-file modification times in UTC; the images do not embed a commit. Three windowed smoke journeys completed with success markers on their first final attempts.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `framing-default.png` | Real starting fixture, default orthographic size 54 | 19:58:07 | `c1a50e3ae50db0f8c81ebbaff5772cfa642e488c7ebff14a8b0d486f86fd8b04` |
| `framing-zoomout.png` | Same run and camera transform, three real wheel-out events to 72 | 19:58:08 | `dd3ba74a73ff23b142d1f1c530404f89af58103acc104d18f09a3e80cc564b95` |
| `command-tiles-normal.png` | Real selected worker; ordinary and research-locked build tiles | 19:58:40 | `a7e823c43320c8f50a1b8caef319c71978c9a11f0aecb0aa4da875edb1c84687` |
| `command-tiles-hover.png` | Same journey, real pointer motion over the affordable Depot tile | 19:58:40 | `e68fe65ed7d937523cc4619698438e0c10f073f4a631c207fd8edce93f024de8` |
| `command-tiles-armed.png` | Real Bunkhouse arming; separate Wall focus ring set by test | 19:58:40 | `9d0bbb416f999e232659072141c524c508904b1b43905feecb1f8e6ad6969dc4` |
| `command-tiles-unaffordable.png` | Real unaffordable Guard click and Rust rejection | 19:59:13 | `6b1d2ceaab61408517af8c36c7d7d721893f604757aa8758130518ef83072d78` |
| `underground-l3.png` | Real paid L3, three complete rows; no L4 row | 19:59:30 | `ec551b142cd4f9f908936a00baf0beeddf366d419cbc9ce138cc554c7e1a77dc` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits; not paid L4 gameplay | 19:59:31 | `7ef7f962e417d8dff4ad2c6c93b9f0e41b684300042c45cbd833bd2f2ce75d30` |
| `underground-long-name.png` | **Synthetic presentation-only L4** long-name/construction probe | 19:59:31 | `cf2c631c6f32308917de04b4164d9a456b2cdbfb3ae3ce94a7e0b3638726a9c5` |

These frames predate the follow-up that edged the battlefield command hint with
a dark outline; `command-tiles-armed.png` here is the retained before frame. See
[the command-hint contrast fix](../command-status-2026-11-15/README.md).

## Comparison and tradeoffs

The 54 and 72 frames show the same fixture and camera position/orientation in one run; live countdown and research advance between captures, so they are not frozen-state images. The closer view increases projected cell size by one third; wheel-out restores the former framing. Compared with mockups 1–3, the actual starting base remains much sparser than the illustrated city. No fixture buildings, models or fog were fabricated or resized.

Ordinary build/train tiles now use quieter flat plates, with distinctive painted armed, locked and unaffordable states. Compared with the mockups, the ordinary sockets are darker, icons remain small and the exceptional-state frames are still ornate. The rendered unaffordable image shows an actual rejection; unaffordable hover was tested through its style, not separately captured. Test-set focus does not prove keyboard traversal or screen-reader behavior.

At the default zoom some scout/resource targets fall behind HUD panels; users can pan or zoom out. One tested Depot placement anchor clears the wave card by only 0.91 pixels at size 54 and requires particular care; it clears to the card's right at 66. The smoke uses a real click there, not an automatic zoom fallback.

## Verification and limitations

At `7a66955`, `cargo fmt --all -- --check`, `cargo test --workspace --locked` (432 passed, one ignored), all seven headless Godot smokes and `git diff --check 30fc36d..7a66955` exited 0. The final rendered smoke, economy and underground journeys exited 0 on their first attempts; the underground marker reports 21.80 simulated seconds of prep margin.

Successful final headless runs still reported ObjectDB leaks and resources in use at shutdown: main, economy and underground each reported 4 / 2, and defenses reported 8 / 4. These diagnostics occurred in earlier runs too; their cause remains undiagnosed, and success markers do not establish clean teardown.

Earlier camera work had an observed capture stall, hover failure, test-only parse error and occluded-target failures before successful runs; an alleged cadence-failure log and a transitional image were overwritten and cannot be independently verified. Earlier tile runs also overwrote preliminary logs and frames; the retained final artifacts and these fresh captures are separate. A gray duplicate-looking off-map render appears in earlier and new framing images; its cause was not investigated. No final-head Linux, screen-reader, export/e2e or full keyboard-traversal check was run. Linux checks were waived.
