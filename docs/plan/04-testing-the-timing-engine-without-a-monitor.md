## Testing the timing engine without a monitor

A modeline is numbers, and whether a monitor accepts it is arithmetic on those numbers, so most of the CRT engine can be proven with no display at all. Seven checks cover it, from pure maths to the kernel. Switchres calculates and the Rust validator judges, so each already checks the other; a second monitor model and mutation testing were dropped as more machinery than the risk needs.

| # | Check | How | Tier |
| --- | --- | --- | --- |
| 1 | Modes stay stable | A golden list of about 40 systems across every preset, run through the Switchres wrapper. Any change after a Switchres upgrade or a policy change shows up as a diff to review | T1 |
| 2 | Nothing unsafe gets through | Property tests over random width, height and refresh, for every profile and GPU capability entry: each result validates or is rejected with a reason. No panics | T1 |
| 3 | Policy follows the decision procedure | Table-driven tests written from the eight steps in the Definition: given a source, picture style, profile and GPU, expect this rung and this mode family | T1 |
| 4 | Per-GPU behaviour | The suite reruns for each entry in `hardware/matrix.toml` and compares with per-GPU goldens, such as super widths for Intel | T1 |
| 5 | Switching logic | `neptron-testcore`, a scripted libretro core written in Rust, changes geometry, refresh and interlace at given frames. The resulting list of mode switches is compared with a golden trace. Asserts include: no modeset on a width-only change under a fixed super width, debounce, mute during resync | T2 |
| 6 | Frame pacing and audio | Simulated clock plus a fake display with a programmable refresh (50.00, 59.94, 60.10 Hz, with jitter and late vblanks). 100,000 frames run in well under a second. Asserts: zero dropped or repeated frames, audio buffer inside its bounds, rate control converges, pitch error under 0.5% | T2 |
| 7 | The real kernel path | In QEMU on `vkms`: commit real modes through `neptron-kms`, read the active mode back and compare field by field, check the frame CRC after each switch, and assert that no modeset happens before the frontend's first commit | T3 |

**Core traces.** When a real core runs in T4, the harness records everything it reports about video: initial geometry, every geometry change, frame sizes and rates. Those traces are saved as fixtures and replayed in T1 and T2 forever, so regressions in handling a real game's behaviour are caught without running the core.

**Safety behaviours as scripts.** Confirm-or-revert, last-known-good, blind recovery and supervisor restore are each a scripted scenario: change the profile and press nothing, kill the frontend mid-game, fail two boots in a row. The assertion is always the same: the active mode at the end is the expected validated one.
