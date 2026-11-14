# neptron

The frontend binary. It holds only wiring, no logic: it joins the library, the settings, the UI, the session, the ODREN engine and its guard, the input and the control protocol on one frame loop.

## Run it on a desktop

The window backend needs SDL3 (`brew install sdl3` on macOS, `libsdl3-dev` on Debian). From the root of the checkout, with a folder of ROMs and a state folder of your own:

```sh
mkdir -p /tmp/neptron/share/roms/nes /tmp/neptron/state /tmp/neptron/cores /tmp/neptron/run
cargo run -p neptron --features window -- --backend window \
  --share /tmp/neptron/share --state /tmp/neptron/state --data data \
  --cores /tmp/neptron/cores --run /tmp/neptron/run --no-control
```

- Put games in `share/roms/nes`, `share/roms/snes` or `share/roms/megadrive`, and the cores (`fceumm_libretro.dylib` named `fceumm_libretro.so`, and so on, as `data/cores` names them) in the cores folder.
- Keys: arrows, X accepts (RetroPad A), Z goes back (B), S is the menu button (X), Right Shift and Return together open the quick menu, and held for 5 s they are the blind recovery.
- A launch, and the end of a game, end the process with exit code 75: one process holds one core. Run it under the launcher to get the next run at once: `cargo run -p neptron-launch -- --no-device -- target/debug/neptron --backend window ...`.

`cargo xtask clippy -p neptron --features window` checks the window build. The `kms` build compiles only on Linux.

## A run

One run is one process. It shows the game list, or it runs one game, because `neptron-libretro` allows one core per process and the display descriptor closes with the display. A run ends with an outcome:

| Outcome | Exit code | What the launcher does |
| --- | --- | --- |
| Relaunch: a launch from the list, the end of a game, a game that did not start | 75 (`RELAUNCH_EXIT_CODE`) | Starts the next run at once |
| Quit: the window was closed | 0 | Stops |
| Reboot: `Settings > System > Reboot`, confirmed (`Outcome::Reboot`) | 80 (`power::REBOOT_EXIT_CODE`) | With `--power`: asks init for a reboot |
| Power-off: `Settings > System > Shut down`, confirmed (`Outcome::PowerOff`) | 81 (`power::POWER_OFF_EXIT_CODE`) | With `--power`: asks init for a power-off |
| The run cannot start (`BootError`) | 1 | Starts it again after its backoff |

Before a relaunch the run writes `run/next.toml` (the relaunch file, `relaunch.rs`): the game to launch, or none for the list, and a notice for the player. The next run takes and removes it. A game path must be relative, with no empty, `.` or `..` part, and the system a plain id (`GameRef::check`): the run joins it to the ROM folder. A file that fails the check is removed, and the start is a new boot to the game list. The run folder is a RAM file system (`/run/neptron` on the image), so a power cut leaves no file.

**A relaunch is not a new boot.** `Guard::boot` counts one failed boot for each start. A run that starts from a relaunch file gives back the count of the run before (`record::for_continued_boot`), so quick game changes never reach safe mode. A crash, a power cut or a restart of the launcher leaves no relaunch file, and that start counts.

## Boot, before the first frame

The guard boots, and its record is written, before every step that can fail for a reason other than the display, so a start that fails later still counts. A sound device or a gamepad subsystem that does not open is a notice, and the run goes on with no sound or with keyboards only (`devices::MaybeSink`). The sound device opens once per process, before the first frame, through `devices::open_audio`: the device of `audio.output`; when that one is absent or does not open, the device that `auto` takes (the one HDMI or DisplayPort output with a monitor, else the first analog output, else the default device), then the first analog output, then the default device, with a notice at each step. With ALSA the list is read from `/proc/asound`, so HDMI and DisplayPort outputs are in it. The setting stays as it is. The log names the driver, the devices, the choice of `auto`, each device that failed with its error, and the device that opened (`docs/spec/audio-settings.md`). A game run follows a relaunch file, so the run before it counted the boot already.

Before all this, the KMS backend waits for a display when `display.output` is `auto` (`display_wait`). It reads the connectors in sysfs every 250 ms (`DISPLAY_POLL_NS`) for at most 15 s (`DISPLAY_WAIT_MAX_NS`) until one qualifies by `neptron_kms::choose` itself: connected, not a writeback connector, and not an analog connector with no EDID, which is used only when the settings name it. sysfs gives the type name of a connector, not its number, so `display_wait::connector_class` maps the name, and a test keeps it equal to `ConnectorClass::of_type` for every DRM type. DVI-I carries both signals, so for it the wait uses `ConnectorClass::of_connector` with the sysfs EDID, as the open does: a DVI-I panel is digital, a DVI-I CRT or a DVI-I with no EDID analog. A digital connector whose status reads `unknown` has had no probe yet and counts as ready; the forced probe of the display open decides. The app depends on `neptron-kms` with its default features for this rule, which compile no DRM code. A named output does not wait. The display open takes the launcher's descriptor and cannot be tried again, so the wait comes before it. When the open still finds no connected display, the frontend exits with code 69 (`NO_DISPLAY_EXIT_CODE`) before the guard record is written: the start counts as no failed boot, and the launcher starts it again after a short fixed wait, with no backoff. A stop request ends the wait, with its own log line.

The output rule of a Raspberry Pi (`docs/spec/raspberry-pi.md`, "The output rule"). A composite connector reports no display (`unknown`, no EDID), so the connector choice never takes it under `auto`. When a composite connector of a GPU whose capability table entry has the `tv_encoder` quirk exists (`display_wait::tv_fallback`), the wait is at most 3 s (`COMPOSITE_FALLBACK_WAIT_NS`, a first value that the bench sets), and a start that then finds no display that answers opens that connector by name (`display_wait::auto_output`). An HDMI display that answers in the wait wins, through the panel path. On a Pi 3 HDMI and composite share one pixel valve, so only one is driven. `display.output = "Composite-1"` gives composite with an HDMI display connected. Each look reads the limit again, so connectors that appear while the driver loads set it. The first run of a boot chooses the output once (`boot_output`): under `auto` it writes the connector it drives to `run/output`, and every relaunch of the boot (each game launch and each return to the game list) opens that connector by name, with no wait. So nothing switches between HDMI and composite while the frontend runs, also when an HDMI display starts to answer later. A relaunch whose connector sysfs no longer shows, or shows as `disconnected`, applies the rule again, with a log line. `display.output` wins over the kept connector. A start that is not a relaunch (a new boot, or a start after a crash) removes the file and applies the rule.

A TV output. The KMS backend reads the `TV mode` property of the connector in use (`KmsDisplay::tv_standards`) into `Runtime::tv_standards`. A connector with the property takes only the modes of a TV profile (ADR 0012), so the run decides every monitor from it: the wanted monitor is the profile of `display.tv` (`auto` is NTSC, `TV_SAFE_STANDARD`), and `display.monitor` has no effect, with a notice when it is not `auto`; the safe default, so also the boot prompt's revert, safe mode and the blind recovery, is the NTSC profile; a guard record gives back only the TV profile of a standard the connector offers. A preset file cannot take the id prefix `tv_` or the TV kind (`PresetError::Reserved`). A revert of the boot prompt writes `display.tv`, never a `tv_` id into `display.monitor`. The Display panel shows `TV standard` and `TV picture width` only on a TV, disables `Monitor profile` there (`Not used on a TV`), greys out the standards the connector does not offer, and names the standard on the mode line (`NTSC 240p 60.05 Hz`). Other outputs never name a standard.

1. The settings: `global.toml`, and for a game its system and game files. A file that cannot be read is an empty layer and a notice.
2. The monitor presets from `data/monitors` at run time (OP-044): the binary embeds no GPL data. A bad file is left out with a notice.
3. The detected output from sysfs, for the connector the backend drives (`Runtime::output`) and its `subconnector` property (`Runtime::subconnector`, read by the KMS backend): the connected connectors, the profile of that connector's own EDID by its class, the GPU caps of the capability table. The class rule is in the `neptron-platform` README: an analog EDID, a `VGA` or `DVI-A` subconnector or an analog-only connector is a CRT; a digital EDID on a digital connector is a flat panel. The named connector uses only its own EDID, whatever its status (`connected`, `unknown` as a VGA output with no EDID often is, or `disconnected`); it never borrows the EDID of another display. A name that no connector has (the window, the fakes) gives no EDID. Only `auto`, which the KMS path uses before the display opens, takes the first connected display that gives a profile.
4. The flat-panel profile is cut to the modes that the display's caps and the panel limits (`PANEL_WIDTH_MAX_PX` 3840, `PANEL_HEIGHT_MAX_PX` 2160) allow. A panel whose native mode is above them starts in its largest listed mode that fits, with a notice that names both modes. A panel with no mode that fits gives no profile, with a notice.
5. The guard record from `state/video/guard.toml`. A record that cannot be read is the default record. The file also holds `output`, the connector its last known good was confirmed on (an optional field, written at each confirmation). When the output of this start differs, or the file names none, the guard gets no last known good: it opens the boot prompt and reverts to the safe default for the new output, so a profile confirmed on one connector never drives another display without a prompt.
6. The safe default, with `auto` and `exact` (`SAFE_START_PROFILE`): the range profile of a CRT that gives EDID; a flat panel only when the record's last known good on this output is that panel; else `generic_15`. So with no EDID, a broken EDID, a DAC whose EDID is digital, and every panel that the user has not kept on this output, the safe start is `generic_15`. Without it the run cannot start.
7. `wanted`: the monitor of `display.monitor` (`auto` is the detected profile, a panel included, or `generic_15` with no EDID; `flat` is the flat-panel profile of the connector's EDID whatever its class, for an LCD with a VGA input; an unknown id, or `flat` with no listed mode, is the safe default with a notice) and the global picture style and refresh policy.
8. The library cache. In the list, a scan starts on its own thread and replaces the index when it ends.
9. The list: a `VideoEngine` for the menu source (320x240, 60 Hz, 4:3, `MENU_SOURCE`). A game: `Session::launch` with the resolved settings, the core options of `data/cores` (OP-055), `RunRateSource::ReadBack` on KMS and `Measured` in the window. A launch that fails ends the run with a relaunch to the list and the reason as a notice.
10. `Guard::boot` (the game list) or `Guard::continue_boot` (a game: no prompt, so a change that was not kept in the game list never reaches a game, and no revert notice shows), and the record it returns is written at once (temp file, fsync, rename) before the first modeset. When that write fails (a read-only or full state partition), the boot cannot count itself, so it could fail again and again and never reach safe mode: this start uses the safe default (`Guard::recover`, with a notice and no tone). In a game, `follow_settings` with the game's policy follows.

## One frame

Each frame reads the clock once and gives that time to every guard call, the hotkeys and the key repeat of the frame.

| Game list | Game |
| --- | --- |
| Poll the input once, ports, hotkeys. Blind recovery calls `Guard::recover` | The session polled the input in its last frame |
| Navigation from the events, with key repeat | Navigation from `Session::input_events`, only while the UI shows; else only the menu button after `menu_opened` |
| Snapshot: confirm, display mode | Snapshot: confirm, session, `video_status` (the read back, never a proposal), display mode |
| `Screens::update`, then the intents | The same |
| `begin_frame(MENU_SOURCE)`, `present(None, canvas)`, `wait_flip` | `run_frame(canvas or None)` |
| `Guard::tick`, also after a failed present | `Guard::tick` |
| No flip: the loop waits for itself one 60 Hz frame | `blanked`: the same |

- Key repeat (`nav.rs`): a direction repeats after 400 ms, then every 66 ms. On `PromptOpened` every repeat stops and every held key is ignored until its release (OP-048). A, B and X are accept, back and menu; Start and Select are left to the hotkeys.
- The quick menu is never a toggle: the session ignores a second hotkey, and the app only opens it (OP-051). Resume, reset and quit go to the session.
- A notice at the first picture of a game that is filtered, cropped or small, in the words of the video status.
- `FrameReport::ended` or any `SessionError` (`DisplayStalled` included): the game ends with `quit()`, and the run relaunches to the list, which opens the display again in a new process. This is the one rule for a display that stops.

## Flat panels

- The panel starts in its native mode, so the panel does no scaling. The policy puts the game at the largest whole vertical factor at which its display aspect fits, centred, with black around it. The width follows the core's aspect, so it is often a fraction of the game width: the renderer then uses the sharp bilinear filter, which the video status shows as `sharp` and never as an alert. A 50 Hz game takes a listed 50 Hz mode of the native size.
- The menu is the 320x240 canvas at one whole scale, centred: 4x, 1280x960 at (320, 60) on 1080p.
- The display panel shows the panel's name and the mode it starts in as the detail of `auto` in the monitor row; `flat` shows the same for the panel profile.
- Scanlines follow `scanlines_for`: `auto` is off on a panel.

### The first boot on a panel

- With no last known good on this output, the boot asks for the panel (`wanted`) under the boot prompt, and the safe default is `generic_15`.
- Kept: the panel becomes the last known good. Later boots on the same output start in it with no prompt, and the panel is then also the safe default of that output, so the blind recovery shows a picture.
- Not kept within 10 s: the guard goes to `generic_15` (`Safe(NothingConfirmed)`). A real panel then shows no signal, or the engine blanks when the driver refuses the mode. The revert does not write `generic_15` over `display.monitor = auto`, so the next boot asks again.
- The way back (`offer_panel`): one offer per run. A fresh press of a button (value 1: not a key repeat, not a stick move) proposes `display.monitor = auto`, the panel, under a new prompt with a full window, when all hold: the guard is in safe mode but not after a blind recovery (`Safe(Recovery)` always ends in the safe start and stays there); nothing is confirmed on this output; `display.monitor` is `auto` and gives a panel (an explicit preset is never overridden); and no offer came yet in this run. The press that starts the offer does not also move the menu; after the one offer, every press goes to the menu. So a run puts a panel mode on the wire for at most two confirm windows (`PANEL_WINDOWS_PER_RUN`): the boot prompt and one offer. On a 15 kHz CRT behind an adapter with its own digital EDID, the menu works after that one offer, and a CRT preset can be chosen.

## Settings and the guard

| Setting | What a change does |
| --- | --- |
| Safety none: scanlines, scanline strength, the Start hold | Written at once (writer thread) and applied live |
| Picture style, refresh policy, monitor | Changed in memory and given to `Guard::propose`. Written only on `Confirmed`. Taken back on `Reverted` and `SafeMode` |
| Output (`display.output`) | Shown, not changed from the menu or the protocol; a new connector at the next start opens the boot prompt |

- `PromptOpened` puts the confirm dialog in the snapshot with the guard's deadline. `KeepMode` calls `confirm` with the open prompt; a confirmation after the deadline is dropped. `RevertMode` calls `decline`.
- `Reverted { rewrite: Some(config) }`: the monitor, picture style and refresh policy of that configuration go to the global settings, never `Guard::active()`.
- `SafeMode(Recovery)` plays a short tone: in the game list through the app's sink, in a game mixed into the next core frames by the session's audio pipeline (`Session::write_cue`), so the whole tone plays and adds no latency. The display panel shows "(safe mode)" after the mode while the guard is in safe mode.
- The picture style options show the line rate and the field rate of the mode each style gives, and a style the monitor cannot show is grey with "Needs another monitor" (the help line under the list reads "Not available. Needs another monitor"). Both come from `VideoEngine::style_rows` for the source on screen (the game, or the menu source), the same policy, caps and validator as the run.
- The display panel writes the global level; the quick menu writes the game level, and its keep prompt moves a change to the system or global level.
- A row whose setting does not apply to the display of the run (`applies_to` in the schema) is disabled with the reason `Not used on a flat panel`. The class is that of the monitor the guard gave the engine. On a flat panel these are the picture style and the refresh policy.
- The audio panel (home, `Audio`) writes `audio.volume` and `audio.output` at the global level. The quick menu has a `Volume` row. A volume change reaches the running game at once (`Session::set_volume_percent`); in the game list it only reaches the file. A new output takes effect at the next run, which is a new process.
- The System panel (`Settings > System`, `pages::system_page`) holds `Reboot` and `Shut down`, each behind a confirm question with the focus on `Cancel`. A confirm ends the run as a quit (`Shell::finish` writes the queued settings and the guard record, the threads are joined, the display is left as after a quit: the last validated mode, held by the launcher) with `Outcome::Reboot` or `Outcome::PowerOff`, and the binary exits with code 80 or 81 (`end` in `main.rs`; `power::exit_code` gives the same codes to the tests). A request before the boot is good, on a boot that the guard does not hold to be failed (`Confirmed` or `Safe(NothingConfirmed)`, `power::gives_back_boot`), writes the guard record with the failed boot of this start given back (`record::for_continued_boot`). In `Safe(NoValidMode)` the count stays, so two boots with no valid picture still reach `Safe(FailedBoots)`; `Safe(FailedBoots)` and `Safe(Recovery)` keep their own rules. The rows are enabled only in the game list and only when the launcher gave power control (`NEPTRON_POWER=1`, read into `Runtime::power` by the KMS backend; the window backend never has it). In a game the reason is `Not during a game`: the player quits the game first, so its save RAM is written by its own quit. With no power control the reason is `Needs the launcher`. When the launcher could not carry out the request of the run before, the next start gets `NEPTRON_POWER_FAILED`, and the KMS backend shows the Warning notice `The restart did not work.` or `The shut down did not work.` (`power::failed_notice`). The app checks the rules again when the action arrives (`power::takes_request`, `Shell::request_power`): the game list, power control, no display change waiting for its confirmation, and no outcome yet, so a second request in the same frame changes nothing.
- Every write (settings, the guard record, the library cache) goes to the writer thread (`outbox.rs`) over a bounded channel. A full channel keeps the job in a backlog on the frame thread, in order; nothing is dropped. The frame thread does no file I/O.

## Control protocol

`control.rs`: the socket thread accepts one connection at a time (read and write timeouts of 2 s, socket file mode 0600) and sends each command to the frame thread over a bounded channel. The frame thread answers between two frames. `wait` stays on the frame thread until its condition holds or its time ends; a `timeout_ms` above 60 s (`WAIT_TIMEOUT_MAX_MS`) is cut to it. At the end of a run `ControlServer::stop` answers every open wait and queued command with `unavailable`, closes the frame thread's side, and stops the socket thread, which ends a connection between two request lines, so a waiting or busy client never delays the end of a run.

| Command | Served |
| --- | --- |
| `hello`, `ui_tree`, `session_state`, `video_state`, `stats`, `settings_get`, `log_tail` | Always |
| `input_send`, `input_script`, `session_launch`, `settings_set` | With the `control` feature. A display setting goes through the guard, as a menu choice does |
| `wait` | Always, on the frame thread |
| `clock_step` | `unavailable`: the binary runs in real time. The tier 2 tests step the simulated clock themselves |
| `screenshot` | `unavailable`: no PNG encoder in the frontend yet |

## The log and the stop

- The lines of the protocol log (guard events, notices, every mode set with its read back) also go to the log of the run. The frame thread queues each line in a bounded channel (`logbook.rs`, 256 lines) and never waits: a full channel drops the line and counts it. A writer thread writes the lines to standard error; a failed write loses the line and never stops the run or panics. At start the log names the binary and the folders.
- SIGTERM and SIGINT set a stop flag (`stop.rs`), registered before any SDL code starts, so SDL3 installs no handler of its own for them. The frame loop reads the flag at each frame and ends the run as a quit: save RAM and settings written, the display dropped by its rules, exit code 0.

## Tests

- Unit tests of each module: the detection (`display/tests.rs`: the safe start of a 1080p panel, a 4K panel, a 720p TV, DP and HDMI with an analog EDID, a DAC with a digital EDID, a VGA CRT, a broken EDID and no EDID; the fit to the limits; `flat`), the record file, the relaunch file, the presets, the settings view, the pages, the navigation, the writer thread, the library numbers, the command line.
- `tests/app/control.rs` (tier 1): the control socket with a real client.
- Tier 2, ignored in tier 1 and run by `cargo xtask verify --tier 2`, on the fakes of `neptron-testkit` with the built test core:
  - `panel.rs`: a fake 1080p HDMI panel (EDID in `tests/fixtures/edid`): the boot picks the panel profile in its native mode, the menu at 4x, a game at 4x with black borders, and a change to a CRT preset that reverts with no answer.
  - `end_to_end.rs`: a new boot confirms its settings, the game list, a launch, the mode switch of the core, the quick menu (not a toggle), a display change that reverts with no answer, the same change kept and written to the game's file, the quit with the keep prompt, the save RAM at the quit, and the list again with one boot counted.
  - `recovery.rs`: the 5 s hold of Select and Start from `neptron-input` to `Guard::recover`, in the list and in a game, with the record in safe mode and the tone.
  - `faults.rs`: a display that stops flipping ends the game and relaunches the list with the reason.
  - `audio.rs`: the audio panel writes the volume and the output to `global.toml`; the quick menu volume reaches the game at once, and the next run of the game starts at it.
  - `system.rs`: confirm `Reboot` ends the run with `Outcome::Reboot` (code 80), confirm `Shut down` with `Outcome::PowerOff` (code 81) after a volume change that reaches `global.toml`; cancel and back change nothing; in a game and with no power control both rows are disabled and ask nothing.
  - `crawler.rs`: the UI crawler. It walks every menu path from home and from the quick menu with the D-pad, accept and back only, through the UI tree of the control protocol. It fails on a dead end, on a clipped settings row, on a row outside the overscan-safe area, on a schema setting that no screen shows, and on a setting row whose disabled state does not follow `applies_to`. It reaches the System panel and never accepts `Reboot` or `Shut down`. It runs twice: on the CRT of the 15 kHz safe start, where no row is disabled, and on a 1080p panel, where only the picture style and the refresh policy are.

## Must never depend on

- Logic of its own: logic belongs in the crates

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms` (Linux: KMS, SDL3 gamepads and audio, evdev keyboards), `window` (SDL3 window and audio), `control` (the commands that change the frontend). No feature is on by default.

## Open points

- The output connector is not an input of the guard, and a new connector needs the display opened again, so the display panel shows it read-only and `settings_set display.output` is refused. It changes in `global.toml` or `neptron.toml` and takes effect at the next start, where the guard record's output check below opens the boot prompt.
- `screenshot` is not served: the frontend has no PNG encoder.
- Favourites and recent games are not kept; their lists are empty.
- The KMS build: `Detected::backend_caps` gives the caps of the display, by the output kind (`OutputKind`, ADR 0010). The output is digital only when the class rule finds a flat panel on a digital link and `display.monitor` gives that panel (`auto` or `flat`); every other case is analog: no EDID, a CRT, a DAC, `flat` on an analog output, a CRT preset on a digital output. An analog output gets the table's caps, and buffers of 2560x1200 at most cut to them, as before panels. A digital output gets the table's digital limits for its connector (`Matrix::caps_for_output`), and buffers that also hold the panel's largest listed mode up to 3840x2160, cut to those limits. The caps carry the buffer size as the largest mode, so the boot cuts the panel's modes to the buffers. The validator is unchanged: a panel mode must be one of its listed modes, inside these caps; the kernel's test-only commit is the last word, and a refused mode falls to the next listed one. The connector rule gets the class of `display.monitor`. Before the display opens the subconnector is not known, so a DP connector with a digital EDID opens with the digital caps. Right after the open, the class of the driven connector is read again with its subconnector; when the output is analog (a `VGA` subconnector, or a digital adapter with no EDID), `Detected::policy_caps` and `KmsDisplay::lower_caps` cut the caps of the policy and the validator to the analog ones. No CRT path runs with the digital limits; only the buffers stay larger. The GPU caps come from the output of the settings, else the connector that gave the profile, else the first connected output. A 3840x2160 XRGB8888 buffer is 33 MB, and the GBM surface has at most four.
- Only `intel_gen9` has digital limits today: 300 MHz on HDMI, 165 MHz on DVI-D, 533.25 MHz on DP, 4096x2160. Every other GPU gives a digital output its analog values (165 MHz, 1920x1200), so a 1440p or 4K panel there runs in its largest listed mode of at least 50 Hz that fits, usually 1080p60, with a notice.
- An adapter that shows its own digital EDID with a CRT behind it, and no analog subconnector, is taken for a flat panel. The protections: a monitor preset in `display.monitor` always wins; the panel mode is only `wanted` under the boot prompt and never the safe default until it is kept on this output, so with no answer it is on the wire for one confirm window (10 s) per boot and the guard then goes to `generic_15`; two boots with no confirmation start in safe mode.
- The menu source is 60 Hz also on a 50 Hz profile; the policy picks the mode.
- `video_state` gives no validator verdicts yet.

## Status

Complete for the MVP on the fakes and the window. The KMS build has not run.
