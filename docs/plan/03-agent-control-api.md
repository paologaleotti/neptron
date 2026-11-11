## Agent control API

`neptron-ipc` gives an agent eyes and hands: one JSON-lines socket, wrapped by `neptronctl`, through which it can see the screen, read the widget tree, press buttons and step time. It is compiled in only with the `control` feature, which release images do not enable.

| Command | Returns or does | Used for |
| --- | --- | --- |
| `screenshot --stage ui` | PNG of the 320x240 logical canvas | Layout and text checks, goldens |
| `screenshot --stage output` | PNG after scaling, rotation and shaders | Scaling and shader checks |
| `screenshot --stage scanout` | PNG of what the display received (QEMU only, from `vkms` writeback) | End-to-end proof |
| `ui tree` | JSON: screen id, focused widget, every visible row with its label, value and source level | Assertions without pixels |
| `input send`, `input script` | Presses buttons as a given player; plays a recorded sequence | Driving the UI and games |
| `clock step N` | Advances exactly N frames of simulated time | Determinism |
| `wait <condition>` | Blocks until a screen, state or frame count is reached, with a timeout | Replaces every sleep |
| `session launch`, `session state` | Starts a game with a given core; reports lifecycle state | Scripted play |
| `video state` | Current modeline with every field, derived line and field rates, active profile, last validator verdicts, switch count | Timing engine checks |
| `stats` | Frames presented, dropped and repeated; audio underruns; buffer fill; input-to-frame delay | Pacing and audio checks |
| `settings get`, `settings set` | Reads or writes a value at a given level | Setup and assertions |
| `log tail` | Recent log lines from the RAM ring | Diagnosis |

**Looking at the UI without running the app**

- Every screen registers a **scene**: the screen plus fixture data. `cargo xtask shot settings/display` renders one scene to a PNG in under a second, also in a 4x enlarged copy that is easier to read.
- `cargo xtask gallery` renders every scene, for every skin and canvas size, onto one contact sheet. An agent opens that single image to review a UI change.
- Goldens live in `tests/golden/`. `cargo xtask golden --bless` updates them, and CI attaches a before/after diff image for every changed golden to the pull request, so a human approves visual changes in seconds.

**The UI crawler.** One test walks every menu path using only D-pad and two buttons through the control API. It fails on dead ends, on text the layout engine reports as clipped, on anything outside the overscan-safe area, and on any schema setting it could not reach.
