# neptron-launch

The launcher. It holds the DRM descriptor, starts the frontend, passes the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a backoff.

## Usage

```text
neptron-launch [--device PATH | --no-device] -- PROGRAM [ARGS...]
```

- `--device PATH`: the DRM device. The default is `/dev/dri/card0`.
- `--no-device`: hold no device. For the desktop window, where no frontend takes a descriptor.
- `PROGRAM [ARGS...]`: the frontend and its arguments, for example `/usr/bin/neptron --backend kms`.

The image starts it from BusyBox init: `neptron-launch -- /usr/bin/neptron --backend kms`.

## What it does

1. Opens the device for reading and writing. It refuses a path that is not a character device of the DRM major (226), and a descriptor number of 0, 1 or 2. It clears close-on-exec on the descriptor, which the standard library sets, so each child gets the descriptor under the same number.
2. Starts the frontend with `NEPTRON_DRM_FD` set to the number. `KmsDisplay::from_env` of `neptron-kms` takes it. The frontend and the launcher share one open file description: DRM master, the framebuffers and the mode on the wire.
3. Waits for the frontend to end, then:

| Exit of the frontend | What the launcher does |
| --- | --- |
| Code 0 | Stops, with code 0 |
| Code 75 (`RELAUNCH_EXIT_CODE`, `EX_TEMPFAIL`) | Starts it again at once. The frontend uses it for a planned relaunch: into a game, or back to the game list |
| Code 69 (`NO_DISPLAY_EXIT_CODE`, `EX_UNAVAILABLE`) | No display answered in the frontend's own bounded wait. Starts it again after 1 s (`NO_DISPLAY_WAIT_NS`). It is no crash: the wait does not grow, the crash count stays, and it uses up no relaunch of the window |
| Any other code, or a signal | A crash: starts it again after a wait |

A frontend that relaunches more than 10 times in 10 s (`RELAUNCHES_MAX`, `RELAUNCH_WINDOW_NS`) is in a loop: each further relaunch counts as a crash and waits. The wait after a crash is 250 ms, and it doubles with each crash in a row up to 8 s (`BACKOFF_FIRST_NS`, `BACKOFF_MAX_NS`). A run of 30 s or more (`STABLE_RUN_NS`) ends the series, so the next crash waits 250 ms again.

The launcher keeps the device open the whole time. The kernel removes the framebuffers and some drivers switch the CRTC off only when the last holder closes the descriptor, so the last validated mode stays on the wire across a crash, a relaunch and a restart (`docs/spec/kms.md`).

SIGTERM or SIGINT stops the launcher cleanly: it forwards SIGTERM to the frontend once, waits for it to end, starts no new run and exits. The frontend ends its run as a quit on SIGTERM. A frontend still there 5 s after the SIGTERM (`STOP_WAIT_NS`), for example one stuck inside a core, gets SIGKILL, with a log line, so the launcher always stops. No orphan frontend keeps DRM master. The launcher looks at its child and at the signal every 20 ms (`CHILD_POLL`). A signal during the backoff wait ends the wait at once.

When the DRM device does not open (for example `/dev/dri/card0` is missing because the driver has not bound yet), the launcher logs one line, waits 2 s (`DEVICE_MISSING_WAIT_NS`; a stop signal ends the wait) and exits with code 1. Init starts it again, so it tries the device every 2 s and does not spin. The flag comes from `signal-hook` and the forward from `rustix`, both workspace dependencies, with no `unsafe` code.

A frontend that cannot start stops the launcher with an error. The init system restarts the launcher, which opens the device again.

## What it does not do

- It counts no failed boot and sets no safe mode. Those belong to the guard of the frontend (`neptron-video`), which keeps its own record. The launcher counts crashes only for its backoff.
- It reads no input. The blind recovery before `Guard::boot` is the frontend's job: the frontend boots the guard before its first frame, and from that first frame the input hotkeys count the 5 s hold of Select and Start, also for a combination held from power-on (the input sources report the keys held when they open). The gap is the start time of the frontend, about one second.
- A failed boot is counted by the frontend, not here. The frontend writes its guard record, with this start counted, before the first modeset and before every step that can fail for a reason other than the display; a missing sound device or gamepad does not stop it. So a frontend that keeps failing after that write starts in safe mode at its third start (two failed boots, then safe mode). A start that fails before the write is not counted: the display cannot open (`KmsDisplay::from_env`), or no safe monitor profile is installed. Such a start sets no mode, and the launcher keeps the last mode on the wire while it restarts the frontend: with its backoff after a crash, or after the fixed 1 s wait after code 69. A start with no connected display waits for one first (`neptron::display_wait`) and then exits with code 69, so a monitor that answers late never makes a long backoff. The launcher logs the first start with no display, then at most one line per minute (`NO_DISPLAY_LOG_INTERVAL_NS`), with the hint that an analog connector with no EDID needs a named output: a VGA CRT with no EDID on `auto` never gives a display.
- It sets no mode and makes no commit. Blanking an analog output before the first frontend commits (the safety model, "Shortening the 31 kHz window on x86") needs DRM code here and is not done in the MVP.

## Must never depend on

- `neptron-ui`
- Any frontend logic

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- Dependencies: `neptron-kms` with default features for the DRM node check (no DRM code compiles), `rustix` (`process`) to clear close-on-exec and to forward a signal, `signal-hook` for the stop flag, `thiserror`.

## Tests

- `backoff.rs`: the exit codes, the doubling wait and its limit, the reset after a stable run, a relaunch, a quit, the fixed wait after no display, and the once-a-minute line for it.
- `supervise.rs`: the loop with a fake frontend and a simulated clock: crashes, relaunches, a long run, a frontend that cannot start. No test sleeps.
- `device.rs`: a regular file and `/dev/null` are refused, a missing device, close-on-exec cleared.
- `stopping.rs`: SIGTERM once at the stop, SIGKILL once after the wait, counted from the SIGTERM, with simulated times.
- `main.rs`: a real child process reports its exit code and a signal, reads the descriptor of the launcher under its number, and a child that ignores SIGTERM gets SIGKILL after a short stop wait; a stop signal ends the backoff wait at once; a device that does not open gives the wait before the exit.

## Status

Complete for the MVP. It has not run on the target.
