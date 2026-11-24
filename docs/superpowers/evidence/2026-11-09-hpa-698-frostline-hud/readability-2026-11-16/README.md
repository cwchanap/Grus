# Frostline readability follow-up

> Evidence retention: this checkout keeps none of these captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/readability-2026-11-16) is available at the pre-cleanup commit.

Unretouched 1280 × 720 viewport captures from Godot 4.7.1, OpenGL 4.1 Metal
compatibility renderer on an Apple M1 Pro. Frames use
`SmokeHelpers.capture_frame` and were copied without re-encoding. The initial
captures use the production tree committed as `3188783`; the two fix-round
captures use the test tree committed as `fa2839f`, with the same production
files. Images do not embed a commit.

| Frame | Provenance | SHA-256 |
| --- | --- | --- |
| `framing-default.png` | Real opening fixture, no selection. | `ed66cd137031bd098eb8c3f69516958352c3a9a070f4b595a1c1c5b10cb80643` |
| `command-tiles-normal.png` | Real Worker selection, normal command tiles. | `d915df6046defe0b2fc986036c395e8cd549130c9b704606a92dd5f2e8492a91` |
| `command-tiles-hover.png` | Real pointer motion over the Depot tile. | `01e6e98d441ae15117322f5467d00a54eae62ea6c4f0b3f8205add3da08e3d9b` |
| `command-tiles-keyboard-focus.png` | Real Bunkhouse arming; Wall reached with keyboard arrows, retaining the armed state. | `8dc50968332d16c6d775e3436daf1f9d89f8c7217a848140be3389c01d232905` |
| `command-tiles-unaffordable.png` | Real unaffordable Guard click and Rust rejection. | `9321c8c40feb918cef888cbe408cee2ad5b4c47e8aaeaa4c6abc884e78b4ede1` |
| `economy-queue-barracks.png` | Real Barracks Guard→Rifleman queue. | `b1ad643d4ba55a4b1f6e5d16ef9dd7a3a4f46d4f53ebf2ae791243ed43e0a72a` |
| `underground-l3.png` | Real paid L3, completed Lab and Tap, Tap armed. | `d87ddfeb061ca217de61c51ea43140c187e26333d576ef8d8f420e566718e7af` |
| `underground-unarmed.png` | Real L3 drawer after disarming Tap. | `2115d25f67d9785ae7b10c741b8932c655635c2884ddf39721b338e6b78c0f4b` |
| `research-all-reached-probe.png` | **Synthetic presentation-only research card:** supplied points and ETA; bar, percent and reached markers derive from those inputs. Live build locks were restored before capture; not a paid milestone or gameplay victory. | `c28879c37369c4779985623083574e6bc772f9a4e486d61ce2cc2bd587311148` |

## Comparison and verification

Compared with the approved mockups and the
[prior visual follow-up](../visual-comparison-2026-11-15/): research text and
the progress tube are larger, command prices have dark bands, and underground
names/prices are larger against darker backing. The tube is still shorter and
the markers lower than in the mockup. The band covers the middle of the
painted lower bevel; armed, focused, locked and unaffordable states retain
distinct visible cues.

At `3188783`, all seven headless smokes, the focused shutdown scene and
runner, Rust formatting and locked workspace tests passed. An independent run
also passed all seven verbose smokes and the shutdown checks at that tree.
At `fa2839f`, the changed main smoke passed headless and windowed; the
windowed focus-pixel check used real keyboard-navigation events and reported
456 bright pixels around the focused Wall versus 0 around an unfocused tile.
An independent headless main smoke and diff check also passed at `fa2839f`.
The focus-pixel thresholds are calibrated to this macOS renderer and are
explicitly skipped headless, not proof of cross-platform appearance.

The initial windowed smoke, economy and underground journeys were recaptured
after layout adjustments; earlier frame files were overwritten, while their
logs were retained locally. Failed early layout checks and failure-exit resource
diagnostics were retained locally as well. No leak diagnostics appeared in the
final independent headless runs.

The screenshots do not establish a live reached milestone or real Victory.
No Linux, export/e2e or screen-reader operation was performed. The camera,
fixture, fog, game rules and panel footprints were not changed.
