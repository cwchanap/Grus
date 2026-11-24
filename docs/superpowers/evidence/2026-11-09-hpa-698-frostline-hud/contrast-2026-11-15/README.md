# Frostline HUD contrast pass — rendered evidence

> Evidence retention: this checkout keeps none of these captures. Filenames and results below describe the original capture set. The [complete original evidence](https://github.com/cwchanap/Grus/tree/31b1c91469745dd33126d61986a645b2b3679fea/docs/superpowers/evidence/2026-11-09-hpa-698-frostline-hud/contrast-2026-11-15) is available at the pre-cleanup commit.

Unretouched 1280 × 720 viewport PNGs from implementation head `0c8664b` on `hpa-698-frostline-hud`. Captured 2026-11-15 with Godot 4.7.1, OpenGL 4.1 Metal compatibility renderer on Apple M1 Pro, using the existing opt-in smoke capture helper. Times are source-file modification times in UTC; PNGs do not embed a source commit.

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `pause.png` | Real Esc pause during paid L1 dig | 07:52:11 | `6c5f3c83f1b3ff9495003052242770793fd3995f688ee483c075250fee7a464d` |
| `underground-dig-progress.png` | Real paid L1 dig | 07:52:11 | `cf42c3c4551c2d0f7313db6a5e4e2bfbc895320963036bf38cbc0e146d2b95ae` |
| `underground-construction.png` | Real L3 Lab construction | 07:52:22 | `f6cecc0edbd43fdffe75da3c8995e0dc984ba2ec48a337d2e1dcd4c4bbf8b5f2` |
| `underground-l3.png` | Real paid L3 Lab and Tap; Tap armed | 07:52:25 | `32e3810b12fc8a5a10262488760dae91dea906c13d3d7cdad2b0f64f85c83830` |
| `underground-unarmed.png` | Real paid L3, Tap disarmed | 07:52:25 | `64146c4ce0b8063808a1832257dc7f88e2b027cc09b5f9d5c730e1ad0c3e167f` |
| `underground-synthetic-l4-benefits.png` | **Synthetic presentation-only L4** benefits, not paid L4 gameplay | 07:52:26 | `b28e21270ab2e850d6f884337c7dbb22aa5350a4e59f7dfce9aa0c25b10de909` |
| `underground-long-name.png` | **Synthetic presentation-only L4** Shelter at 99%, not a real purchase | 07:52:26 | `b12649f88de4b5f3490d9cec51d132af6f69f3511bab934ebc809561f5a5cdaf` |
| `economy-queue-overflow.png` | Real six-Worker queue and +1; context checked before and after capture, not atomically at draw time | 07:52:38 | `890397237e23362401cc13172c08d012719c42bc6766322e8b62a77c3e51b562` |
| `economy-queue-barracks.png` | Real Guard→Rifleman FIFO | 07:52:49 | `d2a8f8536ed2655adcd35a7a3d9c4b8158bf85675c9b88595be87109e2242b21` |
| `pause-unfocused.png` | Real Pause, before test-set focus | 07:53:16 | `fc829f70eebc7bacfa32edb80738b8e36ff6dbc374507907a0e72b6e91b8abab` |
| `pause-focused.png` | Same Pause; focus set by test before Enter activation | 07:53:16 | `62569c9c551c346cbdfc967ae86966694e97c094374793b050e8a4cc178c7371` |

## Comparison

- **Underground, mockup 3:** Room names have a larger, renderer-supported dark outline; benefits use a bolder face on a darker band. Three complete rows, state text and real-input controls remain. Names and icons are still compact, and occupied frames remain more ornate than the mockup. The scrolled synthetic L4 scrollbar still meets the right edge of the last slot.
- **Pause:** Resume is a brighter primary action with dark text; the focused image shows a separate outside ring. There is no dedicated Pause mockup. Focus was set by the test; Enter activation was exercised afterward, but Tab traversal and other keyboard routes were not established.
- **Queue, mockup 2:** A lighter active-cell ground, slightly larger icon area and smaller timer chip expose more of the head portrait while preserving the queued frames, progress and overflow. The reference's arrow and portrait treatment remain different. The test's overlap percentage measures the timer against the icon control rectangle, not all artwork pixels or the progress overlay.

## Verification and limits

At `0c8664b`, `cargo fmt --all -- --check`, `cargo test --workspace --locked` (432 passed, one ignored), all seven headless Godot smokes and `git diff --check 0f015e7..0c8664b` exited 0. The final windowed underground, economy and combat journeys each exited 0 on their first attempts; the underground run retained 18.00 simulated seconds of prep margin.

During implementation, an initial economy assertion had a parse error and timed out; later test-harness attempts timed out because a void coroutine was mistakenly treated as a Boolean, then passed after that test-only mistake was corrected. These are retained in local logs. Earlier capture timeouts, intermittent input/cadence failures and varying ObjectDB/resource diagnostics remain undiagnosed. In this final headless sequence general, economy and underground each reported 4 ObjectDB instances and 2 resources at shutdown, while defenses reported 10 and 4; success markers do not establish clean teardown.

The queue timer-fit check does not yet deduct its chip's content margins or explicitly pin the widest current countdown; no current clipping was demonstrated. The populated-queue selection-loss test now exercises real input, but adds timing pressure at 20×. No screen-reader session or full keyboard-navigation check was run. Genuine gameplay Victory, paid L4 excavation and final-head Linux checks were not verified; Linux checks were waived. Earlier visuals and failed capture attempts remain documented in the [prior accessibility evidence](../accessibility-2026-11-15/README.md).
