# 0016: The host holds initial core option values and applies them when the core declares its options

- Status: accepted
- Date: 2026-11-16
- Task: M06 (decision); M07a builds it

## Context

The host contract (`docs/spec/libretro-host.md`, "The order of calls" and "Core options") has the session call `Host::set_option` with the stored values after `Host::open` and before `load_game`. The three MVP cores declare their options in `retro_init` or `retro_get_system_info`, so this works for them.

FinalBurn Neo declares its options inside `retro_load_game` (its `set_environment()` runs there), because the DIP switches of the game are core options and are known only when the game is loaded. Before `load_game` the host knows no FBNeo key, so `set_option` gives `OptionError::UnknownKey` for every value, the session counts each one in `refused_options`, and the core starts with its defaults. The default values of `data/cores/fbneo.toml` (`fbneo-diagnostic-input`) and every user value under `[core.fbneo]` would be lost.

A core reads its options with `GET_VARIABLE` right after it declares them, inside `retro_load_game`. The session cannot call `set_option` between the two: no host call runs inside the core's call.

## Decision

1. `Host` gets `hold_options(&mut self, values: &[(String, String)])`. The session calls it after `Host::open` and before `load_game`, with the defaults of `data/cores` and the stored values, the stored ones winning. It replaces the loop of `set_option` calls before `load_game`.
2. The host keeps a copy of the held values for its whole life, at most `MAX_OPTIONS` (1,024) pairs. More pairs are cut and logged once.
3. When `hold_options` runs, each held key that is declared already gets its held value at once, if the value is in the key's value list.
4. At each later `SET_VARIABLES`, after the host has read the new list and kept the current values that are still valid (the present rule), each newly declared key with a held value gets that value, if the value is in its list. A key that was declared before keeps its current value: a change from the quick menu is never undone by a second declaration.
5. A held value that applies at a declaration does not set the flag that `GET_VARIABLE_UPDATE` reads: the core reads its values after it declares them. A held value for a key that is already declared applies at once and sets the flag, as `set_option` does, so a core that declares its options at init behaves as before.
6. After `load_game`, `Host::held_option_report()` gives the held keys that the core has not declared and the held values that its list does not have. The session counts them in `refused_options` and logs them once, as it does today for refused `set_option` calls.
7. `set_option` keeps its meaning: it changes a declared option now. The quick menu uses it after the load.
8. `core_trace` writes an `option` line for each option declared during `load_game` too, so a test can check the default values of `data/cores` for a core that declares its options late.

## Consequences

- A core that declares its options at load gets the frontend's values, as RetroArch gives them from its options file.
- The present cores behave as before: their options are declared at open, and decision 3 applies the held values then, as `set_option` did.
- Per-game DIP switch values can be stored under `[core.fbneo]` at the game level and reach the core. Their schema is still OP-022.
- The host keeps one more bounded list. `SET_VARIABLES` can allocate already, so the frame path rule does not change.
- `docs/spec/libretro-host.md` changes in M07a: the order of calls, the core option section, the public API.

## Alternatives

- Call `retro_load_game` twice (a first load to learn the keys): rejected. A load can have side effects, the host allows one game per process, and FBNeo opens the zip each time.
- Answer `GET_VARIABLE` from the stored settings directly, with no declared list: rejected. The host would give the core a value it never offered, which the present rule (a value must be in the list) forbids for good reason.
- Leave FBNeo at its defaults in the MVP: rejected. The service-menu combination must move away from "Hold Start" (`docs/spec/arcade.md`, section 13), and DIP switches are a user's normal arcade setting.
