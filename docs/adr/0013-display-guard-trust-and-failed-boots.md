# 0013: The display guard trusts a display by its class, and counts only runs that end abnormally

- Status: accepted
- Date: 2026-11-16
- Task: M01

## Context

The keep or revert dialog of the display showed in too many places, came back after the settings were kept, and sometimes said that a kept change went back (OP-105, OP-076). The causes are in `docs/spec/display-guard.md`, section "Facts that drive the design". Four of the fixes change a safety rule of `docs/definition/05-crt-safety-model.md` or of `docs/flows/03-guard.md`:

- Row 4, "Confirm or revert": "Any user-initiated change to output, profile or geometry applies for 10 seconds. No button press means automatic revert."
- Row 5, "Last known good": "Two boots that never reach a confirmed picture trigger safe mode." The code counted every start until the confirmed configuration showed a picture for 10 s, and no safe state ever made a boot good.
- The flat panel first-boot rule of `docs/flows/03-guard.md`: a panel is never the safe default until the user keeps it, so an unanswered first boot went to `generic_15`, and a panel shows no picture there.
- The first boot of an image opened the boot prompt also on a composite TV, where the wanted and the safe configuration are the same.

The validator is the only gate for signal safety, and none of the changes below touches it, the hard rails or the blind recovery hold.

## Decision

1. **D1, trust an EDID flat panel at boot.** A boot applies the wanted configuration with no prompt when the driven connector is a digital link to a display whose EDID gives a flat-panel profile (`OutputKind::Digital`, `Detected::auto_is_panel`, the EDID read from the driven connector), and `display.monitor` is `auto` or `flat`. Such a panel is then also the safe default of the run, in its native mode. A named preset on such a display is not trusted: it opens the boot prompt, as on any unknown display, and the safe default of that run stays the 15 kHz safe start, because the preset says that a CRT can be behind an adapter.
2. **D2, no boot prompt on a TV output.** A connector with the `TV mode` property takes only the fixed table modes of its standards, so a boot on it never prompts. A user change of the TV standard still opens the dialog.
3. **No boot prompt for the safe default.** A wanted monitor that is the monitor of the safe default needs no confirmation (OP-076).
4. **D4, the window is 15 s, counted from the first settled frame of the change.** `CONFIRM_WINDOW_NS` becomes 15 s. Until the first settled frame, the deadline is the time of the change plus 15 s, so a change that never settles still reverts. The chain limit stays 30 s from the first change (`PENDING_LIMIT_NS`, now a value of its own), so an unconfirmed mode stays on the tube at most 30 s, as before. A change that gives no valid picture still reverts after 1 s (`NO_PICTURE_LIMIT_NS`).
5. **D3, no dialog for a picture style, a refresh policy or a TV width on a confirmed monitor.** These change only the policy's choice among modes that the validator accepts for the same monitor profile. In a pending change or in safe mode they still open the dialog. On a confirmed CRT profile with more than one range, such as `arcade_15_25_31`, a style change can move the signal from one line rate to another with no prompt. Each mode is still one that the validator accepts for that profile, which the user kept.
6. **D5, a failed boot is a run that ended abnormally before a good picture.** A new boot sets a run marker in the record. A settled picture held for 3 s (`GOOD_BOOT_PICTURE_NS`, was 10 s) clears it, in `Confirmed` and in every safe state. A clean exit of the frontend (relaunch, quit, SIGTERM, reboot, power-off) clears it. A boot that finds the marker set counts one failed boot. Two in a row start the next boot in `Safe(FailedBoots)`. A relaunch never sets the marker and never counts.
7. **D6, safe mode ends with any kept configuration.** The picture on screen in safe mode can be kept from the Display panel ("Keep this picture"), and after failed boots "Try my settings again" ends safe mode and applies the settings, with the dialog when their monitor is not known.
8. **D10 is kept.** An unconfirmed CRT profile on an unknown display (no EDID, an analog link, or a named preset) keeps the boot prompt and the timeout revert.
9. **The record keeps one last known good per place, in format version 1.** The new fields `run_open` and `[[places]]` are optional. A build before this ADR ignores fields it does not know (`RecordFile` has no `deny_unknown_fields`), and it still finds `output`, `monitor` and `last_known_good` at the top, filled from the place used last. So no version 2 is needed: an older build reads a new file, and a new build reads an old file as one place.

## Consequences

- A boot on a flat panel, a composite TV, or a CRT that gives EDID shows no dialog and no notice. The boot-check golden of tier 3 (`os/tests/golden/boot-game-list.ppm`) and the boot check script (`xtask/src/os/boot_check.rs`), which wait for the revert notice of the first boot, must change with the next tier 3 run.
- A digital adapter with its own flat-panel EDID and a CRT behind it gets the panel mode with no prompt, and the blind recovery gives the panel mode too. The owner accepts this risk (D1). Such a user names a preset, which is not trusted: the boot prompt opens, and the 15 kHz safe start is the safe default of that run. A list of known adapter EDIDs is a possible later item.
- A 3 s good picture clears the failed-boot marker sooner. A frontend that crashes after 3 s with a picture no longer counts toward safe mode; a crash loop before any picture still does.
- Power cuts on a working display never lead to safe mode.
- A record of this build that an older build rewrites loses its `places` and keeps only the top place.
- `power::gives_back_boot` and `record::for_continued_boot` go away: the clean-exit rule replaces both.

## Alternatives

- Keep the prompt and fix only the notice text (option (a) of the analysis): the panel and the TV still prompt at every first boot, and a panel switched on late sees no picture.
- Start the window at the first input: silence cannot tell an absent user from a tube that shows nothing, so a CRT would keep an unconfirmed mode for as long as nobody presses a key.
- A record version 2: an older build refuses the file and starts with no last known good. Optional fields in version 1 avoid that.
- `neptron.toml` as the only way to set a monitor: it needs a second computer, and a wrong preset in it stays on the tube with no revert. Its import is out of scope (D9).
