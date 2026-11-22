# Frostline HUD visual polish — rendered evidence

> Evidence retention: this checkout keeps a representative subset of the captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/visual-polish-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 viewport PNGs from implementation head `da298ec` on `hpa-698-frostline-hud`. Captured 2026-11-15 with Godot 4.7.1, OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro, using the existing opt-in smoke capture helper. Times are source-file modification times in UTC; images do not embed a commit identifier.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `smoke-start.png` | Real boot Start | 03:08:31 | `f18a347e170e17361d1c1c10067597c95f40576bc13978bd48aa237a85b90259` |
| `smoke-map-edge.png` | Real gameplay | 03:08:31 | `a474ce3ab4ac537c57915cd4218be1f0ae05b86dfae14232f61e09456cb59167` |
| `combat-defeat.png` | Real alien-caused Defeat | 03:09:11 | `c324e5f37ef873b373c8cbf1ca4064b568276fa5eb39c8d963e2378e44b0509c` |
| `combat-victory-probe.png` | **Presentation-only Victory probe**, not a gameplay win | 03:09:11 | `ede35b5fd01fdae0df9e1aa7ed1c6a30f0a85ebe1a965f3798fa9576a08f0e9f` |
| `economy-queue-overflow.png` | Real six-Worker queue, five sockets and +1; context checked before and after capture, not atomically at draw time | 03:09:23 | `db1653bf24af9f9bbf2fd2a1d0d9a92045874dde268f72ff116b746392ccdb34` |
| `economy-queue-barracks.png` | Real Guard→Rifleman FIFO | 03:09:35 | `3771859255b4d25a5c735e6b5d4df868e0f057b78cf362a0edf9b5acb20d87fa` |
| `pause.png` | Real Esc pause during paid L1 dig | 03:09:41 | `52b7fe49e4d8df873a44b81a335d5d8d764bf15f21bf154d1770adc3f4d75c08` |
| `underground-dig-progress.png` | Real paid L1 dig | 03:09:41 | `cf42c3c4551c2d0f7313db6a5e4e2bfbc895320963036bf38cbc0e146d2b95ae` |
| `underground-construction.png` | Real L3 Lab construction | 03:09:52 | `122d100a93deccc9d335360c3dcb57c6fd87a181dd628f405aca287d792c8d05` |
| `underground-l3.png` | Real paid L3 Lab and Tap; Tap armed | 03:09:55 | `5aaa1e05618c0ada318cfa5209595029c099ef8aa4d69ed7bb06e21bf41371ee` |
| `underground-unarmed.png` | Real paid L3, Tap disarmed | 03:09:55 | `8663b61eec81d4ea4c483cbcd388dba52d0550020400a818dd78cfa274b40e74` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits; not paid L4 gameplay | 03:09:55 | `8ab2671d77d3dff028ab84c315245555b0b71db84b9556b45bb452aa0a5829c4` |
| `underground-long-name.png` | **Synthetic presentation-only L4** Shelter at 99%; not a real purchase | 03:09:55 | `cfc453d2a68df7977f6c99261b9a9eddab2870874d2432112eb8ce162f0bc99a` |

## Comparison

- **Underground, mockup 3:** Locked slots are quiet dark cells with lock glyphs and requirements; actionable empties have cyan outlines and text; occupied rooms retain painted frames. The previous capture used ornate frames for all three states. Three rows remain visible. Solid borders, textual requirements and the existing excavation bar still differ from the reference's dashed cells and striped bar.
- **Pause:** A compact, centered Paused/Resume card replaces the large mostly empty panel. There is no dedicated Pause mockup. Automated tests resize the window while paused and check centering, but no intermediate-size screenshot or manual drag was tested.
- **Queue, mockup 2:** The active portrait now has a distinct larger cyan-bordered plate, timer chip and visible progress track; queued portraits retain painted frames and empties remain recessed. Five sockets plus overflow exactly fill the existing row; the reference's arrow and portrait treatment remain different.
- **Start and Result, mockup 4:** Start uses shipped mouse and Pause cues alongside the existing words, not individual keycap chips. Result has a centered action bar and outcome tint while retaining the real battlefield and snapshot statistics. Its battlefield remains darker and sparser than the illustrated city; no city or victory was invented. The Result actions remain RESTART/QUIT rather than the mockup's replay/exit treatment.

## Verification and limits

At `da298ec`, `cargo fmt --all -- --check` and `cargo test --workspace --locked` passed (432 tests, one ignored); all seven headless Godot smokes passed with their success markers. `git diff --check b8a2d80..da298ec` passed. All four final windowed capture journeys passed on their first attempts, including a general-smoke interpolation maximum step of 0.090 and underground preparation margin of 26.95 seconds.

During development, an underground capture attempt timed out after two frames, and a later Start capture run failed its Stop-input assertion before passing on retry. A headless combat attempt also failed an alien/Crystal guard before a retry. Their causes are unconfirmed; the final passes do not establish a cause. Some headless runs reported shutdown diagnostics: the final general smoke reported 4 ObjectDB instances and 2 resources, while the final defenses smoke reported 10 and 4 respectively. Causes remain unconfirmed.

The Start glyphs have BBCode text alternatives checked against the pinned parser, but no screen-reader session was run. Genuine gameplay Victory and paid L4 excavation were not verified by these captures. Linux checks were skipped as requested. Earlier branch evidence and limitations remain in the [previous readability pass](../readability-2026-11-13/README.md).
