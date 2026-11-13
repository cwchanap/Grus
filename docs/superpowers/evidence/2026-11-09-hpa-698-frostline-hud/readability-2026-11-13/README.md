# Frostline HUD readability pass — rendered evidence

These are unretouched 1280 × 720 viewport PNGs from the implementation head `d9f9957` on `hpa-698-frostline-hud`, captured on 2026-11-13 with Godot 4.7.1's OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro. Each capture uses the opt-in `SmokeHelpers.capture_frame` and the existing smoke journey; timestamps below are source-file modification times in UTC, not copy times. The PNG files themselves do not embed the source commit.

| Frame | Source / provenance | Captured UTC | SHA-256 |
| --- | --- | --- | --- |
| `smoke-start.png` | Real Start phase; general-smoke retry | 17:51:11 | `edae3febe4d52a9d8bd6686ac166bd8329f580a77b427ae011ee53016e3ef31c` |
| `smoke-map-edge.png` | Real gameplay; general-smoke retry | 17:51:11 | `a2f3b70eb84e25c2ce46ddcc5865d814b89715970c8014a1f2b8e3188ba8b378` |
| `combat-defeat.png` | Real alien-caused Defeat | 17:49:46 | `dc8660a465ace2af31eb191e27f7864cac53a92eaed365bc07a4d9771ebdb16f` |
| `combat-victory-probe.png` | **Presentation-only Victory probe**, not a gameplay win; real Defeat state restored afterwards | 17:49:46 | `ee34041950b5baa4b072de257dc54e6d6332b01293e077862a60abe0ebbfb2dc` |
| `pause.png` | Real closed-panel Esc pause during paid L1 excavation | 17:50:16 | `161fdf6900288e8473668d918c87cdfd85ed7e05225d3b5fd75f416efbbe8d86` |
| `economy-queue-barracks.png` | Real Guard→Rifleman FIFO, snapshot progress | 17:50:10 | `bfb61fe37bfc193e454b1a21e7275bc13c6ce6d140490623aa8f30167c9bcb89` |
| `economy-queue-overflow.png` | Six real Worker enqueues, five sockets and +1; predicate checked immediately before and after capture, not atomically at draw time | 17:49:59 | `79c454bc22c204e36214f59da5e2d7c946caa3d9a6880bc309ad7706ad5e7ea0` |
| `underground-dig-progress.png` | Real paid L1 dig, live progress | 17:50:16 | `cf42c3c4551c2d0f7313db6a5e4e2bfbc895320963036bf38cbc0e146d2b95ae` |
| `underground-construction.png` | Real L3 Lab under construction | 17:50:27 | `81f69e65cf8f421d2e8306d3ee8db157297abac99906ae77b5a38ad85ac0a108` |
| `underground-l3.png` | Real paid L3 Lab and Tap, Tap armed | 17:50:30 | `af999e89de19e73869a63ba162fbf787e98d1c91abe019688072f6ea2e64d504` |
| `underground-unarmed.png` | Real L3, Tap disarmed | 17:50:30 | `db356cb07f4dad875a1d8e4e7d5cac8cc36d558e01fac7239cf9d0da4bbc93f0` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4 row:** catalogue-derived benefits, no paid L4 gameplay | 17:50:31 | `15e16e82e8b068974d4931094ef249a497a538101e6fa3f1a1b33bfc88b119f3` |
| `underground-long-name.png` | **Synthetic presentation-only L4:** Shelter at 99%, not an actual room purchase | 17:50:31 | `aeffefb88d36364c49ddb304b0ae3649a8cf8e6da2cb3ac324a9e37b8be9ae80` |

## Direct mockup comparison

- **Mockup 1, in-match HUD:** existing Frostline framing is retained. The live fog-of-war and sparse map are different from the illustrative base; game-world models and data have not been fabricated to match it.
- **Mockup 2, queue:** the head has a larger, bold timer and selected frame, with live progress, normal queued sockets, recessed empties and the +1 overflow. Five equal sockets and no arrow remain deliberate differences; the painted frames are still busier than the reference.
- **Mockup 3, underground:** names, prices, lock text, percentages and benefits use larger type and stronger contrast, while three rows remain visible. The longest population/output benefits fit in the scrolled synthetic L4 probe. Slot frames remain ornate and icons small; the separate benefit chips, plus glyphs and striped excavation bar in the illustration are not reproduced.
- **Mockup 4, Start and Result:** the Start instructions are reflowed into two larger lines; the outcome banner/title is larger and the shared Pause/Result dim is lighter. The controls remain text rather than separate glyph cells; the real Result battlefield is much sparser and darker than the illustration, with RESTART/QUIT instead of its replay/exit treatments. The Pause capture checks legibility, not a dedicated mockup match.

## Verification and limits

On `d9f9957`, `cargo fmt --all -- --check` and `cargo test --workspace --locked` passed (432 tests, one ignored), and all seven headless Godot smokes passed with their success markers. The pass-range `git diff --check 6930a62..d9f9957` passed. Linux checks were skipped as requested.

The first real-render general smoke saved Start/map-edge frames, then failed its interpolation cadence check (`max visual step 0.611` across 42 changes); the separate retry passed (`0.141`). The cause of the failed run is unconfirmed. The Start and map-edge frames above are from the successful retry. Combat, economy and underground render journeys each passed in the initial final sequence; the underground success reported 28.95 seconds of remaining preparation margin. Earlier task-level failed/retried captures and their causes are recorded in the local task reports; a successful retry does not establish their cause.

Some successful runs emit ObjectDB/resource shutdown diagnostics, with varying counts and unconfirmed cause. Real-render `frame_post_draw` has stalled in an earlier capture attempt; exact cause was not established. The final PNGs and success markers do not independently prove an absence of such intermittent behavior.

Neither genuine gameplay Victory nor paid L4 excavation was verified in these captures. The result frame and L4 rows are explicitly labelled presentation-only/synthetic. The previous map-edge before/after images used different camera framing, so their pixel values do not constitute a controlled comparison.
