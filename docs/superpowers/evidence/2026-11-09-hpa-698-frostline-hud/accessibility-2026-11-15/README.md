# Frostline HUD accessibility follow-up — evidence

> Evidence retention: this checkout keeps none of these captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/accessibility-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 captures from implementation head `de47663`, after adding alternatives to non-Start rich-text icons and accessible names to the live production queue. Captured on 2026-11-15 with Godot 4.7.1 and OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro. Source-file modification times are UTC; PNGs do not embed their source commit.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `smoke-start.png` | Real Start; coordinator's post-restart general smoke | 05:22:40 | `f18a347e170e17361d1c1c10067597c95f40576bc13978bd48aa237a85b90259` |
| `combat-defeat.png` | Real alien-caused Defeat; coordinator's post-restart combat smoke | 05:23:28 | `3263e356bed72fff1ecbe919baa06b661d55ba5c755fa62d9c90d3262784783f` |
| `combat-victory-probe.png` | **Presentation-only Victory probe**, not a gameplay win | 05:23:28 | `ede35b5fd01fdae0df9e1aa7ed1c6a30f0a85ebe1a965f3798fa9576a08f0e9f` |
| `economy-queue-overflow.png` | Real six-Worker queue and +1; implementer's fix-wave economy smoke | 03:54:25 | `0a040944d9078920906a9976673fd73976ae0d0daeddfa16a395f7d7796e2a43` |
| `economy-queue-barracks.png` | Real Guard→Rifleman FIFO; implementer's fix-wave economy smoke | 03:54:37 | `7d502b6fa66b5e70b1f9e84f440a99322166cb49183b880fdd69465c95db8bd4` |

The [previous visual-polish captures](../visual-polish-2026-11-15/README.md) include the drawer, Pause and synthetic L4 probes, but are from the earlier `da298ec` head. The accessibility fix did not change drawer code or scene geometry. Its headless underground journey passed, but the later fix-head windowed underground attempts did not complete, so those older images are not represented as successful final-head captures.

## Verification and limits

At `de47663`, the coordinator reran `cargo fmt --all -- --check`, `cargo test --workspace --locked` (432 passed, one ignored), all seven headless Godot smokes with success markers, and `git diff --check b8a2d80..de47663`; all exited 0. The implementer also recorded successful windowed economy and combat journeys at the fix head. After a server restart interrupted a subsequent capture sequence, a new windowed general and combat run exited 0, while economy and underground each timed out after 180 seconds. Separate retries with an activation command also timed out after 180 seconds. Economy logs stopped after plugin startup; underground logs captured dig and Pause frames but no success marker. The exact stalled stages and causes are unconfirmed; the successful earlier economy run does not diagnose them.

Some successful headless runs emitted shutdown diagnostics: the final general and economy runs each reported 4 ObjectDB instances and 2 resources, and defenses reported 8 and 4. Their causes remain unconfirmed. Two earlier intermediate test-harness failure logs were overwritten during the fix wave; the subsequent focused RED and GREEN logs are retained locally.

The pinned Godot parser's image alternatives and the queue controls' accessibility-name properties are tested; no screen-reader session was run. The selection-loss assertion currently runs after the queue has drained, so it does not independently test clearing populated names. Genuine gameplay Victory, paid L4 excavation and final-head Linux checks were not verified; Linux checks were waived.
