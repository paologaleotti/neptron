# The display guard: when it asks, when it reverts, what it keeps

This spec settles OP-105 and OP-076. It describes when the guard opens the keep or revert dialog, how a boot counts as failed, what the record keeps, and which notices the user sees. ADR 0013 records the changes to the safety rules (rows 4 and 5 of `docs/definition/05-crt-safety-model.md`, and the flat panel first-boot rule). `docs/flows/03-guard.md` describes the guard as built.

## Facts that drive the design

- The validator is the only gate for signal safety. Every mode that the guard causes is a winner that the validator accepted for the current monitor profile and caps (`neptron_safety::validate`, `ValidatedMode`). The dialog adds one thing: a person checks that the profile matches the real monitor. A wrong preset (for example `pc_31_120` on a 15 kHz tube) passes the validator.
- On a Raspberry Pi 3 with a composite TV, the safe default is `tv_ntsc`, and `display.tv = auto` also gives `tv_ntsc`. The first boot opened a prompt for the safe default itself, and its timeout "reverted" to the same mode with a notice.
- A return from a game to the game list called `Guard::boot`, so each return opened the boot prompt again until the user pressed Keep.
- The record held one place. A boot at another place (a monitor that is off at boot, the composite fallback of a Pi) erased the confirmation of the first place.
- A safe state never made a boot good. On a TV that nobody kept, each power cut added one failed boot, and the third power-on was sticky safe mode.
- The picture on screen in safe mode could not be kept: a `propose` of the active configuration wrote it with no prompt.
- The dialog ignored Left and Right until it saw 60 frames with no direction. A quick "Right, then Accept" was a Revert. The app (`Navigator::hold_back`) drops key repeats and keys held when the dialog opens, so the UI needs no such rule.

## Rules

1. Signal safety stays with the validator. The guard never sends a mode that the validator did not accept. The hard rails do not change.
2. Each boot classifies the display at its place as Trusted, Confirmed or Unknown. A place is the pair (connector name, monitor identity). The monitor identity is `edid-` and the FNV-1a 64 hash of the EDID in 16 hex digits, or `none` with no EDID (`Detected::identity`).
3. At boot the guard applies the wanted configuration with no prompt when one of these holds:
   - its monitor is the monitor of the safe default;
   - its monitor is trusted (see "Trust");
   - its monitor is the last known good of this place.

   Otherwise it opens the boot prompt. The boot prompt reverts to the last known good monitor with the wanted settings, or else to the safe default.
4. A user change of the output, the monitor profile or the TV standard opens the dialog. The window is 15 s (`CONFIRM_WINDOW_NS`), counted from the first settled frame of the change. Until that frame, the deadline is the time of the change plus 15 s. A chain of changes with no Keep ends at the latest 30 s after the first (`PENDING_LIMIT_NS`). The default action is Revert. The focus starts on Revert, and a fresh press of Right moves it at once.
5. A change of the picture mode, the refresh rate or the TV picture width applies with no prompt when the guard is in `Confirmed` (the monitor is trusted, confirmed or the safe default). In a pending change or in safe mode it follows rule 4.
6. A run that follows a relaunch file (into a game, or back to the game list) calls `Guard::continue_boot`. It never prompts and never counts a boot. A wanted monitor that is not known starts on the revert target with no event.
7. A revert whose target equals the configuration on the engine changes nothing that the user can see. It shows no notice.
8. Notices:
   - `Timeout`: "Display change undone: not kept in time."
   - `Declined`: "Display change undone."
   - `NoPicture`: "Display change undone: no picture."
   - `ClockWentBack`: "Display change undone: the clock went back."
   - `Superseded`: no notice.
   - The safe-mode notice of a start (`FailedBoots`, `Recovery`) shows once per power cycle. A blind recovery hold during a run always shows its notice and plays its tone.
9. A failed boot is a new boot that ended with no clean exit before its first settled picture held for 3 s (`GOOD_BOOT_PICTURE_NS`). Each new boot sets the run marker (`run_open`). A settled picture for 3 s in `Confirmed` or in a safe state clears it. A clean exit clears it. A new boot that finds the marker set counts one failed boot. After 2 failed boots in a row (`SAFE_MODE_FAILED_BOOTS`) the boot starts in `Safe(FailedBoots)`.
10. Safe mode ends when the user keeps a configuration: a change kept in the dialog, the picture on screen kept with the row "Keep this picture", or the row "Try my settings again" after `FailedBoots`. The blind recovery does not change.
11. The record keeps one last known good per place. A boot at another place never deletes the last known good of a different place.

Rule 12 of the analysis (the import of `neptron.toml`) is out of scope (D9).

## Trust

A display is trusted at boot by its class. The function is `Detected::trusts(setting)` in `apps/neptron/src/display.rs`, where `setting` is the value of `display.monitor`.

- A TV output: the connector has the `TV mode` property (`Detected::is_tv`). Every TV profile has fixed timings from a table, and the validator accepts only those modes. A boot on a TV never prompts, whatever `display.tv` holds. A user change of the TV standard still opens the dialog (rule 4).
- A trusted flat panel: all of these hold.
  - The output kind of the driven connector is `OutputKind::Digital`. `display_class` gives it only for a digital connector kind with no analog subconnector (`VGA`, `DVI-A`) and an EDID with a digital input and no analog listing.
  - The driven connector itself reports the EDID (`Detected::edid_hash` is set), the EDID parses with a good checksum on every block, and its profile is a flat panel (`Detected::auto_is_panel`).
  - `display.monitor` is `auto` or `flat`.
- Any other display is not trusted: no EDID; an analog connector or subconnector; a CRT EDID; a named preset, also on a trusted panel. A CRT EDID with `auto` needs no trust: its profile is the safe default.

A trusted flat panel is also the safe default of the run, in its native mode as fitted to the caps. So a revert, safe mode and the blind recovery on such a display give the panel mode. The boot decides the safe default from the setting at boot. The app decides it again, and gives it to the guard (`Guard::set_safe`), in two cases:

- After a kept change (a Keep in the dialog, or "Keep this picture"): from the detection of the run and the stored `display.monitor`. It reads no file. So a monitor preset kept in the run bounds the safe default of a later safe state of the same run, as it bounds that of the next boot.
- At the blind recovery, before the safe default goes to the engine. The run handles no hot plug, so another monitor can be on the connector. The recovery reads the display on the driven connector again from sysfs, on the frame thread: one small read, only in this rare action that the user holds for 5 s. A read that fails keeps the detection of the run and the identity of the place, and the log says so. A named preset on a panel tells the frontend that a CRT can be behind an adapter, so its safe default stays the 15 kHz safe start. On every other display the safe default does not change: the CRT EDID profile, or `generic_15`, or the TV profile of NTSC (or of the first standard the output offers).

## A named preset bounds the safe default

A named preset (`display.monitor` is not `auto` or `flat`) says which monitor is on the wire. The EDID can come from another device: an EDID dongle, a VGA KVM, or a splitter whose DDC comes from a PC monitor on its other port. So a 15 kHz CRT can show an analog EDID of 30 to 70 kHz. A 31 kHz signal can harm such a tube. A 15 kHz signal on a PC monitor gives no picture and harms nothing. Harm ranks above no picture.

The rule: every mode of the safe default is a mode of the named CRT preset. `Detected::safe` decides it in `apps/neptron/src/display/bound.rs`. "Inside" compares the line rates, the field rates and the line counts (progressive and interlaced), range by range: a preset with several ranges is inside when each of its ranges lies inside one range of the other profile.

| Named preset P and analog EDID E | Safe default | Boot prompt for P |
| --- | --- | --- |
| E inside P | the ranges of E, with the pixel clock limit of E. Its id is `<P>@<E id>` | yes; its revert writes P's id, never `auto` |
| P inside E | the ranges of P, with the pixel clock limit of E. Its id is `<P>@<E id>` | yes; its revert writes P's id |
| P and E have a part in common | P cut to E: the common line rates, field rates and line counts of each pair of ranges, the porches, sync and polarities of P, the pixel clock limit of E. Its id is `<P>@<E id>` | yes; its revert writes P's id |
| No part in common, a panel EDID, or no EDID | `generic_15` when it lies inside P; else the ranges of P under 19 kHz (the 15 kHz class of the hard rails), with the id `<P>@15khz`, or P itself when all its ranges are under 19 kHz; else `generic_15` | yes, unless the safe default is P itself |

The mark `@` is reserved: no preset file may hold it in its id (`PresetError::Reserved`). A revert to a safe default made from P writes P's id to `display.monitor` (`Detected::monitor_setting`), never `auto` and never an id that no preset has. So while a preset is named, a new EDID (a KVM, a dongle, another monitor) gives a new safe default inside the preset, never the unbounded EDID profile. The guard record keeps such a profile by its id, and `Detected::rebuild` makes it again only from the EDID detected now: another EDID drops the entry, and it never comes back wider than the preset and that EDID.

Examples with the shipped presets:

| Named preset | EDID 30 to 71 kHz, 50 to 160 Hz | No EDID |
| --- | --- | --- |
| `arcade_15`, `generic_15`, `arcade_25` | `generic_15` | `generic_15` |
| `pc_31_120`, `pc_70_120` | the ranges of the preset (inside the EDID), with the pixel clock limit of the EDID | `generic_15` |
| `arcade_31` | its 31 kHz range with field rates from 50 Hz | `generic_15` |
| `arcade_15_31` | its 31 kHz range | `generic_15` |
| `pstar` | its 31.5 to 31.6 kHz range | its two ranges under 19 kHz |
| `ntsc`, `pal` | the preset itself | the preset itself |
| `d9200` on an EDID of 31 to 38 kHz | its ranges 31 to 32 kHz and 37 to 38 kHz, no mode from 32 to 37 kHz | `generic_15` |

With `auto` or `flat` the rules of "Trust" apply as before: the CRT EDID profile is the safe default. A name that no preset has gives no bound: the EDID profile, as with `auto`. A named preset with no CRT range (a custom panel or TV profile) bounds no line rate: the safe default is `generic_15`.

So the revert target of a boot prompt, safe mode, the blind recovery and the next boots stay inside the named preset:

- An unanswered boot prompt for `arcade_15` on a display with a PC-range analog EDID reverts to `generic_15`, and the global settings name `generic_15`, never `auto`. The next boot starts on `generic_15` with no prompt, because it is the safe default.
- `generic_15` named on such a display is its own safe default, so it opens no boot prompt. `pc_31_120` on a PC CRT whose EDID holds it opens the boot prompt, and its revert keeps its ranges with the pixel clock limit of the EDID.
- `Safe(FailedBoots)`, `Safe(Recovery)` and the blind recovery give the same safe default.

The validator does not change. The rule only chooses which monitor profile the validator gets for a fallback.

## States

| State | Event | Next | Notice |
| --- | --- | --- | --- |
| Boot | the record is in safe mode, or 2 failed boots | `Safe(FailedBoots)` or `Safe(Recovery)` | once per power cycle |
| Boot | wanted monitor is the safe default, trusted, or the last known good here | `Confirmed` | none |
| Boot | otherwise | `Pending` (boot prompt) | dialog |
| Relaunch | any | as Boot, but never `Pending`: the last known good with the wanted settings, or the safe default | none |
| `Confirmed` or `Safe` | user change of profile, output or standard | `Pending` | dialog |
| `Confirmed` | style, refresh or TV width change | `Confirmed` (`follow_settings`) | none |
| `Safe` or `Pending` | style, refresh or TV width change | `Pending` | dialog |
| `Pending` | Keep, with a settled picture before the deadline | `Confirmed`. Last known good here = the monitor. Failed boots 0. Safe mode off | none |
| `Pending` | timeout, Revert, no picture, clock went back | revert target | rule 8; none when the target equals the active configuration |
| `Pending` | superseded | revert target | none |
| `Safe` | "Keep this picture" | `Confirmed` on the safe default. Last known good here = its monitor. The global settings name it | none |
| `Safe(FailedBoots)` | "Try my settings again" | safe mode off, failed boots 0, run marker set, then as Boot for the settings | dialog only when the monitor is not known |
| `Confirmed` or `Safe` | settled picture for 3 s | same; run marker cleared; failed boots 0 unless the record is in safe mode | none |
| any | clean exit | run marker cleared | none |

A clean exit is every end of the frame loop: a relaunch (exit 75), a quit by SIGTERM, SIGINT or a closed window (exit 0), a reboot (80) and a power-off (81). The app calls `Guard::close_run` in `Shell::finish`. A crash, a SIGKILL, a power cut and a start that fails after the record write are not clean.

While the record is in safe mode, the failed boots stay as they are after a good picture. So a start in `Safe(FailedBoots)` stays in it until the user keeps a configuration.

## The dialog

- Title "Keep this picture?". Buttons "Revert" (focus) and "Keep".
- A fresh press of Right moves the focus at once. The app ignores a key that is held when the dialog opens, and it stops every key repeat (`Navigator::hold_back`). The UI keeps no quiet-frame rule.
- The time left counts from the deadline of the prompt, which moves once, at the first settled frame of the change.
- At 0 s the dialog shows "The old picture comes back now." with the Revert button only. Keep is hidden, because the guard refuses it at the deadline.
- Every Keep that the guard refuses is logged: `keep refused: <reason>`, and the dialog stays open. A reachable case is a Keep while the change shows no valid picture (`ConfirmError::NoPicture`) or no settled frame yet (`NotShown`). The UI cannot send a Keep at 0 s, so `ConfirmError::Expired` only guards against a fault in the app. A Keep on a frame whose time is earlier than one the guard saw is refused too (`ClockWentBack`), and that frame reverts the change.

## The safe-mode rows

In safe mode the Display panel shows action rows at its end:

- "Keep this picture", in every safe state. It keeps the configuration on screen (`Guard::keep_shown`). The guard refuses it with no settled picture. The app then writes the safe default's monitor, picture mode and refresh rate to the global settings, as for a reverted boot prompt.
- "Try my settings again", in `Safe(FailedBoots)` only. It ends safe mode and applies the configuration of the settings (`Guard::retry_settings`). The run counts again from there: the run marker is set and the run is not good, so a hang on those settings counts as a failed boot, and a good picture clears the marker. A monitor that is not known opens the dialog; its revert goes to the safe default, outside safe mode. In any other state the guard ignores the call.

## The record

The file is `state/video/guard.toml`, written as temp file, fsync, rename. The format stays version 1 and is append-only. An older build ignores `run_open` and `places` and reads the other fields.

```toml
version = 1
failed_boots = 0
safe_mode = false
run_open = false
output = "HDMI-A-1"
monitor = "edid-0123456789abcdef"

[last_known_good]
kind = "edid"
id = "panel_npt_1080"

[[places]]
output = "Composite-1"
monitor = "none"

[places.last_known_good]
kind = "profile"
id = "tv_ntsc"

[[places]]
output = "HDMI-A-1"
monitor = "edid-0123456789abcdef"

[places.last_known_good]
kind = "edid"
id = "panel_npt_1080"
```

- `run_open` (optional, default `false`): the run marker of rule 9.
- `places` (optional): one entry per place with a last known good, oldest first, at most 8 (`PLACES_MAX`). A confirmation or a boot that uses a place moves it to the end. When a ninth place comes, the first entry goes.
- `output`, `monitor` and `last_known_good` at the top: the last entry of `places`, so an older build finds the place that was used last.
- A file with no `places` gives one place from the top fields when all three are there. A file with `places` ignores the top fields.
- The boot reads the last known good of its place only. It keeps the other entries as they are and writes them back with every save. An entry that no longer builds (a preset that was removed) stays in the file.

## Notices of a start

| Case | Notice |
| --- | --- |
| `Safe(FailedBoots)` at a start | Warning "Safe mode: the last starts gave no picture. Keep a picture in Settings, Display to leave it." Once per power cycle. |
| `Safe(Recovery)` at a start | Warning "Safe mode: the picture was reset. Keep a picture in Settings, Display to leave it." Once per power cycle. No tone. |
| The blind recovery hold | The same Recovery text, with the tone. Always. |

The run folder (`/run/neptron`) is a RAM folder. The marker `run/safe-notice-shown` says that this power cycle has shown the notice of a start.

## Tests

The tests of the rules:

- `a_tv_first_boot_opens_no_prompt_and_no_reverted_notice` (app, tier 2)
- `a_return_to_the_list_after_an_unanswered_prompt_opens_no_prompt` (app, tier 2)
- `a_boot_at_another_place_keeps_the_confirmation_of_the_first` (app record unit test, and app tier 2)
- `power_cuts_on_a_kept_or_trusted_tv_never_reach_safe_mode` (guard)
- `clean_restarts_count_no_failed_boot` (guard)
- `a_fresh_right_then_accept_keeps` (UI)
- `a_refused_keep_is_logged_and_the_dialog_stays` (app, tier 2), and `an_accept_on_keep_at_the_deadline_reverts` (app, tier 2)
- `a_revert_to_the_same_configuration_shows_no_notice` (guard)
- `safe_mode_ends_when_the_picture_on_screen_is_kept` (guard, and app tier 2)
- `a_trusted_panel_boots_native_with_no_prompt` (app, tier 2)
- `a_change_window_starts_at_the_first_settled_frame` (guard)
- `a_named_15_khz_preset_never_falls_back_to_a_pc_range_edid`, `an_analog_edid_inside_the_named_preset_stays_the_safe_default`, `a_preset_under_the_safe_start_is_its_own_safe_default`, `a_preset_inside_the_edid_keeps_the_pixel_clock_limit_of_the_edid`, `a_revert_on_an_edid_inside_the_preset_keeps_the_preset_named`, `a_record_entry_of_a_cut_is_dropped_or_narrowed_after_a_change_of_edid`, `a_part_in_common_is_the_safe_default`, `with_nothing_in_common_the_safe_default_is_15_khz` and `every_shipped_preset_bounds_its_safe_default` (app unit tests)
- `recovery_uses_the_safe_default_of_the_monitor_detected_now`, `a_revert_to_the_safe_default_uses_the_one_given_last` and `no_valid_mode_uses_the_safe_default_given_last` (guard), and `the_blind_recovery_after_a_hot_plug_is_safe_for_the_new_monitor`, `a_failed_read_at_the_blind_recovery_keeps_the_detection_of_the_boot` and `a_kept_preset_bounds_the_safe_default_of_the_run` (app, tier 2)
- `an_unanswered_boot_prompt_stays_inside_the_named_preset`, `a_reverted_menu_change_stays_inside_the_named_preset`, `safe_mode_after_failed_boots_stays_inside_the_named_preset` and `the_blind_recovery_stays_inside_the_named_preset` (app, tier 2): `arcade_15`, `generic_15`, `pc_31_120` and `arcade_31` on a VGA output with an analog EDID of 30 to 71 kHz. Every mode that reaches the fake display passes the validator for the named preset, and the safe default is the one of the table.

These stay green:

- An unknown VGA display with a hand-set 31 kHz preset still prompts at boot and reverts with no input.
- A panel EDID with a named CRT preset prompts at boot.
- The guard rig checks after every frame that the mode on the wire passes the validator.
