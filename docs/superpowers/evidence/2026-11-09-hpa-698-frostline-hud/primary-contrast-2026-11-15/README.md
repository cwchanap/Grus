# Frostline HUD primary-action contrast — final evidence

> Evidence retention: this checkout keeps a representative subset of the captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/primary-contrast-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 viewport PNGs from implementation head `0cfa7ff` on `hpa-698-frostline-hud`. Captured 2026-11-15 with Godot 4.7.1, OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro, using the existing smoke capture helper. Times are source-file modification times in UTC; PNGs do not embed a source commit.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `pause.png` | Real Esc pause during paid L1 dig | 08:26:36 | `0cc62d5e78540d9f0acf233f3fca3401be4e0ff6e46051e0554f0cb6f22222a8` |
| `underground-dig-progress.png` | Real paid L1 dig | 08:26:35 | `802e5a5fd3fc8611bb5588609fd477c3ae0dcbb88d8f1a4c7795001e40338fdb` |
| `underground-construction.png` | Real L3 Lab construction | 08:26:47 | `373d87b776e266c9ddf2ad7d8d109ecb23e26da9deecf33749dc1e020bb0c917` |
| `underground-l3.png` | Real paid L3 Lab and Tap; Tap armed | 08:26:50 | `e6db923a06909a84108944b2f2e42e9fa1431ecdde15baea31100bcd084c1147` |
| `underground-unarmed.png` | Real paid L3, Tap disarmed | 08:26:50 | `1bc9a7806c4c5829686bbdaedff614c998ac036892893ea4f7283a8ebcca4a9e` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid L4 gameplay | 08:26:50 | `918f7f13ed4d025a07b0575fa2422ea350b96bbc37e14a3361ccfec70d807ab1` |
| `underground-long-name.png` | **Synthetic presentation-only L4** Shelter at 99%, not a real purchase | 08:26:50 | `474e22aa3c9bdf95f9a1e1f1febd871cd01a1b45499b77fbd1e5c3f81c5908a7` |
| `economy-queue-overflow.png` | Real six-Worker queue and +1; context checked before and after capture, not atomically at draw time | 08:27:02 | `ada415a7b3a32194efae724b39f7ab1be7a06e3e5d22087ce70c846d7ddf1908` |
| `economy-queue-barracks.png` | Real Guard→Rifleman FIFO | 08:27:14 | `fa45ab86ba1e4615d9b0db924e1a30d98dbe90afb0fa8efa15a9438e6bf25926` |
| `combat-start.png` | Real Start screen | 08:27:20 | `1f6000300dcb05db574a07395346993375e19884083292db8626045bddcf03dd` |
| `start-focused.png` | Real Start; focus set by test | 08:27:20 | `d60cf38d6a309071c523d670dca76a2d2d6257c24faa888150b1c7e0ddbd78ff` |
| `pause-unfocused.png` | Real Pause, before test-set focus | 08:27:41 | `fc829f70eebc7bacfa32edb80738b8e36ff6dbc374507907a0e72b6e91b8abab` |
| `pause-focused.png` | Real Pause; focus set by test before Enter activation | 08:27:41 | `62569c9c551c346cbdfc967ae86966694e97c094374793b050e8a4cc178c7371` |
| `combat-defeat.png` | Real alien-caused Defeat | 08:27:46 | `b93e3c13433fe9223abb1a231631efec05f88e87740cf56485b8f167ddd0ce28` |
| `defeat-focused.png` | Real Defeat; Restart focus set by test | 08:27:46 | `15c5b326eab9e8a38ec1560797c32867f28955380a22640b5ea54039350b693e` |
| `combat-victory-probe.png` | **Presentation-only Victory probe**, not a gameplay win | 08:27:46 | `f6716dfaed16671b0c3ec9713f6e0f842d51046d52a65485464711f3fea5ce82` |

## Comparison and verification

The [prior contrast pass](../contrast-2026-11-15/README.md) strengthened room readouts, Resume and the active queue portrait. This follow-up gives Start and Restart the same dark ink and outside focus ring as Resume. The rendered solid ink and cyan plate sample at approximately 6.20:1 contrast; this is a reference measurement, not a formal accessibility certification. Room icons and names remain compact, occupied frames remain more ornate than mockup 3, and the queue still differs from mockup 2's arrow and portrait treatment. The Result battlefield remains sparser than mockup 4's illustration.

At `0cfa7ff`, the coordinator reran `cargo fmt --all -- --check`, `cargo test --workspace --locked` (432 passed, one ignored), all seven headless Godot smokes and `git diff --check 0f015e7..0cfa7ff`; all exited 0. The final rendered underground, economy and combat journeys each exited 0 on their first attempts; the underground run retained 18.05 simulated seconds of prep margin. The rendered economy run also emitted 4 ObjectDB instances / 2 resources at shutdown. In the final headless sequence general, economy and underground each reported 4 / 2, while defenses reported 10 / 4. Their causes remain unconfirmed; success markers do not establish clean teardown.

All focused frames use test-set focus. Resume activation was exercised with injected Enter after that setup; user keyboard traversal and focused Start/Restart keyboard activation were not established. No screen-reader session was run. The timer-fit guard still measures the label box rather than deducting chip content margins and does not explicitly pin the widest current countdown; no current clipping was demonstrated. The timer-overlap assertion measures the control rectangle, not visible artwork pixels.

Earlier test-harness timeouts and historic rendered-capture stalls remain documented in the [previous contrast evidence](../contrast-2026-11-15/README.md) and [accessibility evidence](../accessibility-2026-11-15/README.md); a successful retry does not establish their cause. Genuine gameplay Victory, paid L4 excavation and final-head Linux checks were not verified; Linux checks were waived.
