## Verification ladder

Every work package names the tier that gates it, and `cargo xtask verify --tier N` runs that tier and everything below it. The command exits non-zero on failure and writes `target/verify/report.json` plus screenshots and logs.

| Tier | What runs | What it proves | Runs on | Time budget |
| --- | --- | --- | --- | --- |
| T0 Static | `fmt`, `clippy` with warnings denied, dependency-rule check, `unsafe` allow-list, licence check | Hygiene and the crate boundaries | macOS, Linux | 1 min |
| T1 Logic | Unit and property tests for every crate; golden modelines; settings and parser fixtures | All pure logic, including the whole timing maths | macOS, Linux | 2 min |
| T2 Headless app | The real frontend with a fake display, CPU renderer, test cores and a simulated clock; scripted sessions; screenshot goldens; UI-tree assertions | UI, sessions, ODREN decisions, pacing and audio logic | macOS, Linux | 5 min |
| T3 Emulated OS | The Buildroot image booted in QEMU with a virtual display, virtual gamepads and virtual sound card | Kernel config, init, the real DRM path, input, audio, updates, power loss, boot time, memory | macOS and Linux, through a container | 5 to 15 min |
| T4 Real cores | Real libretro cores running free test ROMs for a fixed frame count; frame hashes and audio checks | Core hosting and compatibility | CI nightly; on demand locally | 10 to 30 min |
| T5 Hardware | Self-test reports and the sync probe on real machines | The actual signal on the wire | Self-hosted runners, optional | Nightly |

**Which tier an agent must reach.** T2 for every change. T3 when the change touches `neptron-kms`, the update glue, device input, audio output or anything under `os/`. T4 and T5 run in CI and are read, not run, by the agent.

**Three properties make the ladder trustworthy**

- **Deterministic.** A simulated clock drives frames, so tests never sleep and a 30-minute play session runs in seconds.
- **Identical on both hosts.** T0 to T2 use no OS graphics, audio or input APIs, so macOS and Linux give bit-identical results.
- **Observable.** Anything the agent might need to judge (pixels, widget tree, current modeline, dropped frames) is a query on one control socket.
