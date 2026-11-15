# Opening target access and audio shutdown

Unretouched 1280 × 720 captures from implementation head `0d522d9`, taken 2026-11-15 using Godot 4.7.1, OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro. Times are source-file modification times in UTC; images do not embed a commit.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `framing-default.png` | Real starting fixture, size 54 and no selection | 22:49:21 | `c42fe206e9b42451e579f2e7334c73376f4a847ed1276df1fc874a4d8ec043ff` |
| `framing-zoomout.png` | Same journey after three real wheel-out steps to 72 | 22:49:22 | `043663b8f70c58e424a83c096e63797b4e889063e2e5a4655db0d8e769b9d410` |
| `command-tiles-armed.png` | Real Bunkhouse arming; separate Wall focus set by test | 22:49:29 | `b49c7b4272e2f5ee7a0fa8b6e8c2ab00571edf0cbbb0b34c032855a342e39882` |
| `command-tiles-unaffordable.png` | Real unaffordable Guard action and Rust rejection | 22:50:02 | `f2baa8cdded2c4162dd686354513c6c17b2576d9a3d5ed3912783df48d055ea4` |
| `underground-l3.png` | Real paid L3, three complete rows and live wave warning | 22:50:19 | `b68512b5048ecf75c6e530e0248034a242a3b27bbc798417190af5114ea001bf` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid gameplay | 22:50:20 | `66dae919b6f8f91c22491dac7b560f22c2dbc2b89ed71bc2d5c55c56a7cec870` |
| `underground-long-name.png` | **Synthetic presentation-only L4** long-name/construction probe | 22:50:20 | `cb073167e1f183deb24a3ff01527f1acefbe7fb445f29c706bf66dc03d41ec2c` |

## Opening view

The camera shifts slightly and the existing wave card moves down, leaving the starting sources and the previously obscured scout and Depot click points clear of the HUD without zooming. The smoke sends a real Gather click to each of the six live starting sources and checks the selected worker's authoritative source ID. At 1280 × 720, the closest tested safe-source center clears the HUD by 9.3 px; the scout and Depot click points clear by 11.60 and 9.25 px respectively. These are measured click-point clearances, not a guarantee for all possible placements or display sizes.

The wave card is now visibly separated from the top bar and covers a different patch of the map; the camera keeps the closer size 54. The far northwest expansion salvage, which is not an opening safe source, still requires pan or zoom in the tested journey. The [earlier framing captures](../framing-2026-11-15/README.md) show the prior layout; these frames differ in camera and card positions, so they are not a controlled pixel-only comparison.

## Verification and limits

At `0d522d9`, independent `cargo fmt --all -- --check`, `cargo test --workspace --locked` (432 passed, one ignored), all seven verbose headless smokes, the focused shutdown scene, the shutdown-runner contract and `git diff --check 1e7f2d7..0d522d9` exited 0. No leak or drain-timeout warning appeared in those runs. Three final windowed smoke, economy and underground journeys also exited 0 on their first attempts without those warnings; the underground run reported 21.80 simulated seconds of prep margin.

Verbose investigation connected the previously observed shutdown diagnostics to active Ogg playback references and mixer teardown timing. The quit path stops cues and makes a **bounded best-effort** attempt to let audio processing advance. Driver timing is not an engine acknowledgement that every playback has been reclaimed: unusual callback timing, missed observations or a stalled driver can still hit the deadline and leave diagnostics. The timeout emits a warning and quit continues. Failure exits and forced/engine-driven termination can bypass the normal drain.

Earlier investigations included two interpolation failures, temporary probe/test parse errors and an intermediate benchmark leak; retries, source/provenance qualifications and some overwritten intermediate artifacts are recorded in the local investigation ledger. A success marker by itself does not prove clean teardown. This pass did not test Linux, actual macOS title-bar/Cmd+Q/Dock gestures, screen-reader behavior, release/export performance or paid L4. The shutdown regression is not wired into CI. Focus in the armed frame was test-set, not a keyboard-traversal demonstration.
