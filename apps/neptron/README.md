# neptron

The frontend binary. It holds only wiring, no logic: it joins the library, the settings, the UI, the session, the ODREN engine and its guard, the input and the control protocol on one frame loop.

## Must never depend on

- Logic of its own: logic belongs in the crates

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms` (Linux: KMS, SDL3 gamepads and audio, evdev keyboards), `window` (SDL3 window and audio), `control` (the commands that change the frontend). No feature is on by default.
- The frame thread does no file I/O. Every write (settings, the guard record, the library cache, the notice and pad files, the format request) goes to the writer thread (`outbox.rs`) over a bounded channel. A full channel keeps the job in a backlog on the frame thread, in order; nothing is dropped.
- Each frame reads the clock once and gives that time to every guard call, the hotkeys and the key repeat of the frame.

## Run it on a desktop

The window backend needs SDL3 (`brew install sdl3` on macOS, `libsdl3-dev` on Debian). From the root of the checkout:

```sh
mkdir -p /tmp/neptron/share/roms/nes /tmp/neptron/state /tmp/neptron/cores /tmp/neptron/run
cargo run -p neptron --features window -- --backend window \
  --share /tmp/neptron/share --state /tmp/neptron/state --data data \
  --cores /tmp/neptron/cores --run /tmp/neptron/run --no-control
```

- Put games in `share/roms/nes`, `share/roms/snes` or `share/roms/megadrive`, and the cores in the cores folder with the names that `data/cores` gives (`fceumm_libretro.dylib` named `fceumm_libretro.so`, and so on).
- Keys: arrows, X accepts (RetroPad A), Z goes back (B), S is the menu button (X). Right Shift and Return held together for 0.5 s open the quick menu; held for 5 s they are also the blind recovery.
- A launch and the end of a game end the process with exit code 75. Run it under the launcher to get the next run at once: `cargo run -p neptron-launch -- --no-device -- target/debug/neptron --backend window ...`.

`cargo xtask clippy -p neptron --features window` checks the window build. The `kms` build compiles only on Linux.

## A run

One run is one process. It shows the game list, or it runs one game, because `neptron-libretro` allows one core per process and the display descriptor closes with the display.

| Outcome | Exit code | What the launcher does |
| --- | --- | --- |
| Relaunch: a launch from the list, the end of a game, a game that did not start | 75 (`RELAUNCH_EXIT_CODE`) | Starts the next run at once |
| Quit: the window was closed, or SIGTERM or SIGINT | 0 | Stops |
| Reboot: `Settings > System > Reboot` or `Format share`, confirmed (`Outcome::Reboot`) | 80 (`power::REBOOT_EXIT_CODE`) | With `--power`: asks init for a reboot |
| Power-off: `Settings > System > Shut down`, confirmed (`Outcome::PowerOff`) | 81 (`power::POWER_OFF_EXIT_CODE`) | With `--power`: asks init for a power-off |
| No display answered (KMS) | 69 (`NO_DISPLAY_EXIT_CODE`) | Starts it again after a short fixed wait, with no backoff |
| The run cannot start (`BootError`) | 1 | Starts it again after its backoff |

The relaunch file `run/next.toml` (`relaunch.rs`) carries a run to the next one: the game to launch (or none for the list), a notice for the player, and the pad on each port (`pads`, `PadRecord`), so the next run keeps the order of the players. The next run takes and removes it. A game path must be relative, with no empty, `.` or `..` part, and the system a plain id (`GameRef::check`). A file that fails the check is removed, and the start is a new boot to the game list. The run folder is a RAM file system (`/run/neptron` on the image).

**A relaunch is not a new boot.** A run from a relaunch file calls `Guard::continue_boot`: no prompt, no count, no revert notice, so a change that was not kept never reaches a later run. A new boot calls `Guard::boot`, which sets the run marker of the guard record. A good picture or a clean end of the run (`Shell::finish`: a relaunch, a quit, a reboot, a power-off) clears it. A crash, a hang or a power cut before a good picture leaves it set, and the next new boot counts one failed boot (`docs/spec/display-guard.md`).

## Boot, before the first frame

The guard record is written before every step that can fail for a reason other than the display, so a start that fails later still counts. A sound device or a gamepad subsystem that does not open is a notice, and the run goes on with no sound or with keyboards only (`devices::MaybeSink`).

**The first-boot file.** First, before a backend reads a setting, `prestart::prepare` imports `neptron.toml` from the boot partition (`/boot` on the image, `--boot DIR`; `boot_file.rs`) once per process. The rules are in `docs/spec/raspberry-pi.md`, "`neptron.toml` on the boot partition". Details of the app:

- `boot_file::decide` is pure. `boot_file::import` writes through `Storage::apply`, then stores the hash in `state/neptron.toml.sha256` (temp file, fsync, rename).
- A file larger than 64 KiB (`MAX_FILE_BYTES`) or not UTF-8 changes nothing and keeps the stored hash, as a file that does not parse.
- A failed write of the settings is a notice, and the next start tries again. When the hash cannot be stored, each start copies the file again, so a menu change of a key that the file sets is lost at the next start; the notice says so.
- The import only writes settings. A display value from the file passes the monitor checks, the guard and the safety validator, as a hand edit of `global.toml` does.

**The display wait (KMS).** With `display.output` `auto`, the backend waits for a display before it opens one (`display_wait`). It reads the connectors in sysfs every 250 ms (`DISPLAY_POLL_NS`) for at most 15 s (`DISPLAY_WAIT_MAX_NS`), until one qualifies by `neptron_kms::choose`: connected, not writeback, and not an analog connector with no EDID (that one is used only when the settings name it).

- sysfs gives the type name of a connector, not its number. `display_wait::connector_class` maps the name, and a test keeps it equal to `ConnectorClass::of_type` for every DRM type. For DVI-I the wait uses `ConnectorClass::of_connector` with the sysfs EDID, as the open does.
- A digital connector whose status reads `unknown` counts as ready: the forced probe of the open decides.
- The app depends on `neptron-kms` with default features for this rule, which compile no DRM code.
- A named output does not wait. The open takes the launcher's descriptor and cannot be tried again, so the wait comes first. When the open finds no connected display, the frontend exits with code 69 before the guard record is written, so the start counts as no failed boot. A stop request ends the wait, with its own log line.

**The output rule of a Raspberry Pi** is in `docs/spec/raspberry-pi.md`, "The output rule". In the app: `display_wait::tv_fallback` finds a composite connector of a GPU with the `tv_encoder` quirk, the wait is then at most 3 s (`COMPOSITE_FALLBACK_WAIT_NS`), and `display_wait::auto_output` opens composite by name. `boot_output` keeps the connector of the first run in `run/output`, and each relaunch of the boot opens it by name, with no wait. A relaunch whose connector sysfs no longer shows, or shows as `disconnected`, applies the rule again, with a log line. `display.output` wins over the kept connector. A start that is not a relaunch removes the file.

**A TV output.** The KMS backend reads the `TV mode` property of the connector in use (`KmsDisplay::tv_standards`) into `Runtime::tv_standards`. Such a connector takes only the modes of a TV profile (ADR 0012, `docs/spec/raspberry-pi.md` section 3):

- The wanted monitor is the profile of `display.tv` (`auto` is NTSC, `TV_SAFE_STANDARD`). `display.monitor` has no effect, with a notice when it is not `auto`.
- The safe default (so the revert, safe mode and the blind recovery) is the NTSC profile. A guard record gives back only the TV profile of a standard the connector offers. A revert writes `display.tv`, never a `tv_` id into `display.monitor`.
- A preset file cannot take the id prefix `tv_` or the TV kind (`PresetError::Reserved`).
- The Display panel shows `TV standard` and `TV picture width` only on a TV, disables `Monitor profile` there (`Not used on a TV`), greys out the standards the connector does not offer, and names the standard on the mode line (`NTSC 240p 60.05 Hz`). Other outputs never name a standard.
- A `TV mode` property with no standard that NEPTRON knows would refuse every mode: the KMS backend logs it and exits with code 69, and `Detected::safe` gives `MonitorError::NoTvStandard`. sysfs does not show the property, so this shows only at the open.

**The steps, in order:**

1. The problems of the `neptron.toml` import (`Runtime::boot_file`): notices on a new boot, log lines on a relaunch. A missing file is a log line only.
2. The settings: `global.toml`, and for a game its system and game files. A file that cannot be read is an empty layer and a notice.
3. The monitor presets from `data/monitors` at run time (OP-044): the binary embeds no GPL data. A bad file is left out with a notice.
4. The detected output from sysfs for the connector the backend drives (`Runtime::output`, `Runtime::subconnector`): the connected connectors, the profile of that connector's own EDID by its class (the class rule is in the `neptron-platform` README), the GPU caps of the capability table. The named connector uses only its own EDID, whatever its status, and never borrows the EDID of another display. A name that no connector has (the window, the fakes) gives no EDID. Only `auto` takes the first connected display that gives a profile.
5. The flat-panel profile is cut to the display's caps and the panel limits (`PANEL_WIDTH_MAX_PX` 3840, `PANEL_HEIGHT_MAX_PX` 2160). A native mode above them starts the panel in its largest listed mode that fits, with a notice that names both modes. A panel with no mode that fits gives no profile, with a notice.
6. The guard record from `state/video/guard.toml`. A record that cannot be read is the default record. The guard gets the entry of this start's place only (`docs/spec/display-guard.md`, "The record").
7. The safe default (`SAFE_START_PROFILE`, `display.rs`, `display/bound.rs`): the range profile of a CRT that gives EDID; a trusted flat panel (`Detected::trusts`) in its native mode; else `generic_15`. A named CRT preset bounds it (`docs/spec/display-guard.md`, "Trust" and "A named preset bounds the safe default"). Without a safe default the run cannot start.
8. `wanted`: the monitor of `display.monitor` (`auto` is the detected profile, or `generic_15` with no EDID; `flat` is the flat-panel profile of the connector's EDID whatever its class, for an LCD with a VGA input; an unknown id, or `flat` with no listed mode, is the safe default with a notice), and the global picture mode and refresh rate.
9. The library cache. In the list, a scan runs on its own thread and replaces the index when it ends. The first scan thread of the process also reads the arcade index (`/usr/share/neptron/arcade/fbneo.idx`, `Paths::arcade_index`) into `ArcadeState`. A missing or damaged index never stops the boot: the arcade folders then list each zip by its file name. Clone folding (`Other versions (N)`) is in `docs/spec/arcade.md`.
10. The list: a `VideoEngine` for the menu source (320x240, 60 Hz, 4:3, `MENU_SOURCE`). A game: `Session::launch` with the resolved settings, the core options of `data/cores` (OP-055), `RunRateSource::ReadBack` on KMS and `Measured` in the window. A failed launch relaunches to the list with the reason as a notice (`Booted::Relaunch`). `LaunchError::Refused` and `LaunchError::CoreStopped` get the notices S1 and S2 of `docs/spec/arcade.md` section 10.1 (`start_failure.rs`). A GL core gets the notices of `docs/spec/hw-render.md` section 6.2: H1 for a refusal with the request that the machine lacks (before S1), H2 for `LaunchError::Graphics`. A game that runs after the host refused its context logs that the core draws in software. The app passes the internal resolution option of the core data and the machine (`MACHINE`: a Raspberry Pi on ARM, else a PC).
11. `Guard::boot` (a new boot) or `Guard::continue_boot` (a relaunch), with the trust of the display. The record it returns is written at once (temp file, fsync, rename) before the first modeset. When that write fails (a read-only or full state partition), the boot cannot count itself, so this start uses the safe default (`Guard::recover`, with a notice and no tone). In a game, `follow_settings` with the game's policy follows.

**The sound device** opens once per process, before the first frame (`devices::open_audio`). The choice and its fallbacks are in `docs/spec/audio-settings.md`. With ALSA the list comes from `/proc/asound`. The setting stays as it is. The log names the driver, the devices, the choice of `auto`, each device that failed with its error, and the device that opened.

## One frame

| Game list | Game |
| --- | --- |
| Poll the input once, ports, hotkeys. Blind recovery calls `Guard::recover` | The session polled the input in its last frame |
| Navigation from the events, with key repeat | Navigation from `Session::input_events`, only while the UI shows; else only the menu button after `menu_opened` |
| Screensaver: the idle timer (`shell/screensaver.rs`) | The same, from `Session::input_events`; `hide_picture` while it shows |
| Snapshot: confirm, display mode | Snapshot: confirm, session, `video_status` (the read back, never a proposal), display mode |
| `Screens::update`, then the intents | The same |
| `SceneCanvas::paint`: the canvas is drawn only when the scene changed | The same, while the UI shows |
| `begin_frame(MENU_SOURCE)`, `present(None, canvas)`, `wait_flip` | `run_frame(canvas or None)` |
| `Guard::tick`, also after a failed present | `Guard::tick` |
| No flip: the loop waits for itself one 60 Hz frame | `blanked`: the same |

- Key repeat (`nav.rs`): a direction repeats after 400 ms, then every 66 ms. On `PromptOpened` every repeat stops and every held key is ignored until its release (OP-048). A, B and X are accept, back and menu; Start and Select are left to the hotkeys.
- The screensaver follows `docs/spec/screensaver.md`. The hotkeys still see the input under it, so the blind recovery works. The time setting is read at boot and after each live change.
- The quick menu is never a toggle: the session ignores a second hotkey, and the app only opens it (OP-051). The UI intents `Resume` and `QuitGame` call `Session::resume` and `Session::quit`. The quick menu has no reset.
- The notice at the first picture of a game that is not exact (`shell/picture.rs`): one sentence for each reason of the policy (`Notice::reasons`), with the texts of `docs/spec/arcade.md` section 11. A fit or a crop with no reason of its own gives the words of the video status.
- The speed notices of a slow game (`speed_notice.rs`) follow `docs/spec/pacing-audio.md`. Both are Warning notices: they wait for the quick menu, as every notice in a game, and go to the log.
- The one-time notices of an interlaced picture across the lines, and their file `state/config/notices.toml` (`notices.rs`, `Job::Notices`), follow `docs/spec/arcade.md` section 11 and `docs/spec/tate.md` section 7.4: `vertical_interlaced` for a vertical game, `horizontal_interlaced` for a horizontal game across a turned screen. A missing or damaged file shows the notices again. The picture notice of the start of a horizontal game across a turned screen adds "This game is horizontal. It shows small on a turned screen." once at each start.
- The arcade rules of the policy (`PolicySettings::arcade`) apply to a game of a system whose core is the core of the arcade index (`library::runs_arcade_boards`, `GameContext::arcade`).
- `FrameReport::ended` or any `SessionError` (`DisplayStalled` included): the game ends with `quit()`, and the run relaunches to the list, which opens the display again in a new process. This is the one rule for a display that stops. The end notices S2 and S3 and their order against a lost save (`shell::intents::pick_notice`) are in `docs/spec/arcade.md` section 10.1. A GL core with no picture in 10 s ends with H3, which comes before a lost save as S2 does; `SessionError::ContextLost` ends with H4, after a lost save as S3 does. The notice that loses goes to the log.

## Flat panels

- The panel starts in its native mode, so the panel does no scaling. The policy puts the game at the largest whole vertical factor at which its display aspect fits, centred, with black around it. The width follows the core's aspect, often a fraction of the game width: the renderer then uses the sharp bilinear filter, which the video status shows as `sharp` and never as an alert. A 50 Hz game takes a listed 50 Hz mode of the native size.
- The menu is the 320x240 canvas at one whole scale, centred: 4x, 1280x960 at (320, 60) on 1080p.
- The display panel shows the panel's name and its start mode as the detail of `auto` in the monitor row; `flat` shows the same for the panel profile.
- Scanlines follow `scanlines_for`: `auto` is off on a panel.

### The first boot on a panel

- A trusted panel (ADR 0013, `docs/spec/display-guard.md`, "Trust") boots in its native mode with no prompt, and is the safe default of the run, so a revert and the blind recovery show a picture.
- A named preset on such a display is not trusted: the boot prompt opens, and with no answer the guard goes to `generic_15` (or to the 15 kHz ranges of a preset that does not hold `generic_15`).

## Settings and the guard

| Setting | What a change does |
| --- | --- |
| Safety none: scanlines, scanline strength, the Start hold | Written at once (writer thread) and applied live |
| Picture mode, refresh rate, TV width, while the guard is `Confirmed` | Written at once and given to `Guard::follow_settings`: no prompt |
| Monitor, TV standard; or a picture mode, refresh rate or TV width in a pending change or in safe mode | Changed in memory and given to `Guard::propose`. Written only on `Confirmed`. Taken back on `Reverted` and `SafeMode` |
| Output (`display.output`) | Shown, not changed from the menu or the protocol; a new connector at the next start opens the boot prompt |

- `PromptOpened` puts the confirm dialog in the snapshot with the guard's deadline. `KeepMode` calls `confirm`; a refused confirmation goes to the log as `keep refused: <reason>`, and the dialog stays. The dialog shows no Keep at 0 s, so an `Expired` refusal only comes from a fault. `RevertMode` calls `decline`.
- `Reverted { rewrite: Some(config) }`: the monitor, picture mode and refresh rate of that configuration go to the global settings, never `Guard::active()`.
- `SafeMode(Recovery)` plays a short tone: in the game list through the app's sink, in a game through `Session::write_cue`, so the whole tone plays with no added latency. The display panel shows "(safe mode)" after the mode, and the safe-mode rows (`Guard::keep_shown`, `Guard::retry_settings`) of `docs/spec/display-guard.md`. The notices of a start follow the same spec (`run/safe-notice-shown`). A revert notice shows only when something on screen went back, never for a superseded change.
- The picture mode and refresh rate rows, the pair write (`Config::write_pair`), the gate (`pages::gate`) and the keep prompt follow `docs/spec/video-policy-cases.md` section 8. The display panel writes the global level; the quick menu writes the game level, and its keep prompt moves a change to the system or global level.
- A row whose setting does not apply to the display of the run (`applies_to` in the schema) is disabled with the reason `Not used on a flat panel`. The class is that of the monitor the guard gave the engine. On a flat panel this is the refresh rate.
- The audio panel writes `audio.volume` and `audio.output` at the global level. The quick menu has a `Volume` row. A volume change reaches the running game at once (`Session::set_volume_percent`); in the game list it only reaches the file. A new output takes effect at the next run.
- The System panel (`pages::system_page`): `Reboot`, `Shut down` and, with an offer in `storage.toml`, `Format share` (`storage::FormatOffer`). The rules and the texts are in `docs/spec/ui-navigation.md`, "The System panel". In the app:
  - A confirm ends the run as a quit (`Shell::finish` writes the queued settings and the guard record, so it counts no failed boot; the threads are joined; the launcher holds the last validated mode) with `Outcome::Reboot` or `Outcome::PowerOff`. `end` in `main.rs` exits with code 80 or 81; `power::exit_code` gives the same codes to the tests.
  - Power control is `NEPTRON_POWER=1` from the launcher, read into `Runtime::power` by the KMS backend; the window backend never has it. `NEPTRON_POWER_FAILED` gives the Warning notice of `power::failed_notice`.
  - `power::takes_request` checks the rules again when the action arrives: the game list, power control, no display change waiting for its confirmation, and no outcome yet.
  - `Format share` writes `/state/share-format-request` on the writer thread (`Job::ShareFormatRequest`). The run ends only when `Outbox::request_result` reports the file written (`Shell::poll_format_request`). The frontend never formats (`os/REFERENCE.md`, "A format that the user confirms").
- Pads: `Shell::poll_pads` takes and logs the notices of the input source each frame. Who is told and when (`pad_notes.rs`, `run/pad-notes.toml`, `Job::PadNotes`) is in `docs/spec/input-generic-pads.md`. A notice that the shell's notice list drops (it holds 8, `NOTICES_MAX` in `shell/view.rs`) is not told, so the next notice of its pad shows. The KMS backend forwards the notices of the gamepads and of the keyboards.
- The Input panel (`pages::input_page`, `pad_page.rs`) follows `docs/spec/input-players.md`. The shell builds the page again when the roster counter changes. The pads open with `PortRule::Game` in a game run, `PortRule::Menus` otherwise, with the roster of the relaunch file held on their ports.
- The controller layouts (`pad_maps.rs`, `config/input/pads.toml`, `Job::PadMaps`) and the guided mapping (`shell/mapping.rs`, game list only) follow `docs/spec/input-remapping.md`. The file is read at the start of each run, before the first poll, and the maps go to `PadRoster::set_player_maps`.
- The Network panel (`pages::network_page`, `network.rs`, thread `network-watch`) follows `docs/spec/network-panel.md`. `Shell::network_counts` gives the reads asked for and the pages built, for the tests.
- The set check of an arcade game (thread `neptron-set-check`, `Library::start_check`, `Shell::poll_set_check`) follows `docs/spec/arcade.md` section 10. The frame thread never waits for it, and a second launch while a check runs is ignored.

## Control protocol

`control.rs`: the socket thread accepts one connection at a time (read and write timeouts of 2 s, socket file mode 0600) and sends each command to the frame thread over a bounded channel. The frame thread answers between two frames. `wait` stays on the frame thread until its condition holds or its time ends; a `timeout_ms` above 60 s (`WAIT_TIMEOUT_MAX_MS`) is cut to it. At the end of a run `ControlServer::stop` answers every open wait and queued command with `unavailable`, closes the frame thread's side, and stops the socket thread between two request lines, so a client never delays the end of a run.

| Command | Served |
| --- | --- |
| `hello`, `ui_tree`, `session_state`, `video_state`, `stats`, `settings_get`, `log_tail` | Always |
| `input_send`, `input_script`, `session_launch`, `settings_set` | With the `control` feature. A display setting goes through the guard, as a menu choice does |
| `wait` | Always, on the frame thread |
| `clock_step` | `unavailable`: the binary runs in real time. The tier 2 tests step the simulated clock themselves |
| `screenshot` | `unavailable`: the frontend has no PNG encoder |

## The log and the stop

- The lines of the protocol log (guard events, notices, every mode set with its read back) also go to the log of the run. The frame thread queues each line in a bounded channel (`logbook.rs`, 256 lines) and never waits: a full channel drops the line and counts it. A writer thread writes the lines to standard error; a failed write loses the line and never stops the run. At start the log names the binary and the folders.
- SIGTERM and SIGINT set a stop flag (`stop.rs`), registered before any SDL code starts, so SDL3 installs no handler of its own. The frame loop reads the flag each frame and ends the run as a quit: save RAM and settings written, the display dropped by its rules, exit code 0.

## Tests

- Unit tests of each module, among them the detection in `display/tests.rs` (panels, a TV, analog EDIDs on digital connectors, a DAC, a VGA CRT, a broken EDID, no EDID, a named 15 kHz preset with a PC-range EDID, the fit to the limits, `flat`).
- `tests/app/control.rs` (tier 1): the control socket with a real client.
- Tier 2, ignored in tier 1 and run by `cargo xtask verify --tier 2`, on the fakes of `neptron-testkit` with the built test core:
  - `panel.rs`: a fake 1080p HDMI panel: native mode, menu and game at 4x, a CRT preset that reverts with no answer.
  - `end_to_end.rs`: boot, list, launch, the core's mode switch, the quick menu, a display change reverted and then kept, the quit with the keep prompt, save RAM, one boot counted.
  - `named_crt.rs`: four presets on a VGA output with a PC-range analog EDID; every safe path stays inside the preset and the revert never writes `auto`.
  - `recovery.rs`: the 5 s hold to `Guard::recover` in the list and in a game, a hot plug, a failed sysfs read.
  - `faults.rs`: a display that stops flipping.
  - `start.rs`: every way a game fails to start or is stopped by its core, with its notice and log lines.
  - `audio.rs`, `system.rs`, `network.rs`: the audio panel, the System panel with the format row, the Network panel.
  - `generic_pads.rs`, `players.rs`, `remapping.rs`: the input notices, the player order across runs, the guided mapping.
  - `crawler.rs`: the UI crawler. It walks every menu path from home and the quick menu with the D-pad, accept and back. It fails on a dead end, a clipped row, a row outside the overscan-safe area, a schema setting that no screen shows, and a disabled state that does not follow `applies_to`. It never accepts `Reboot`, `Shut down` or `Format share`. It runs on the 15 kHz safe start (no row disabled), on a 1080p panel (only the refresh rate disabled), and with a format offer.

## Open points

- The output connector is not an input of the guard, and a new connector needs the display opened again. So the display panel shows it read-only and `settings_set display.output` is refused. It changes in `global.toml` or `neptron.toml` and takes effect at the next start, where the record's output check opens the boot prompt.
- `screenshot` is not served: the frontend has no PNG encoder.
- Favourites and recent games are not kept; their lists are empty.
- The KMS caps follow ADR 0010: `Detected::backend_caps` gives the caps by the output kind (`OutputKind`). The output is digital only for a flat panel on a digital link that `display.monitor` gives (`auto` or `flat`); every other case is analog. An analog output gets the table's caps, with buffers of at most 2560x1200. A digital output gets the table's digital limits (`Matrix::caps_for_output`), with buffers that also hold the panel's largest listed mode up to 3840x2160. The caps carry the buffer size as the largest mode. The validator is unchanged; the kernel's test-only commit is the last word, and a refused mode falls to the next listed one. Before the open the subconnector is not known, so a DP connector with a digital EDID opens with the digital caps; right after the open, an analog result (a `VGA` subconnector, or a digital adapter with no EDID) cuts the caps of the policy and the validator to the analog ones (`Detected::policy_caps`, `KmsDisplay::lower_caps`). No CRT path runs with the digital limits. The GPU caps come from the output of the settings, else the connector that gave the profile, else the first connected output. A 3840x2160 XRGB8888 buffer is 33 MB, and the GBM surface has at most four.
- Only `intel_gen9` has digital limits today: 300 MHz on HDMI, 165 MHz on DVI-D, 533.25 MHz on DP, 4096x2160. Every other GPU gives a digital output its analog values (165 MHz, 1920x1200), so a 1440p or 4K panel there runs in its largest listed mode of at least 50 Hz that fits, usually 1080p60, with a notice.
- An adapter that shows its own digital EDID with a CRT behind it, and no analog subconnector, is taken for a flat panel. ADR 0013 accepts that it gets the panel mode with no prompt. The protection: a monitor preset in `display.monitor` always wins, is never trusted, and makes `generic_15` the safe default.
- The menu source is 60 Hz also on a 50 Hz profile; the policy picks the mode.
- `video_state` gives an empty `verdicts` list.

## Status

Complete for the MVP on the fakes and the window. The KMS build runs on the i3-6100T (2026-11-13) and on a Pi 3 B on composite (2026-11-14).
