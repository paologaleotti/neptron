# Frame pacing and audio

This spec settles how NEPTRON keeps the sound in step with a picture whose refresh is locked to the emulated system. The code is in `crates/neptron-audio`. The session (`crates/neptron-session`) calls it once per core frame.

## The problem

The core runs one frame per page flip. The display is the master clock. Each core frame makes a fixed amount of audio on average: the core's sample rate divided by the core's frame rate, for example 32040.5 / 60.0988 = 533.1 stereo frames for the SNES. The sound card plays at its own rate, from its own crystal. The two clocks never agree exactly. If the frontend resamples with a fixed ratio, the audio queue slowly fills up or runs empty. Blocking on a full queue would make the frame thread miss a flip. An empty queue is an audible gap.

Dynamic rate control solves this: it changes the resample ratio by a very small amount, in proportion to the distance of the queue from its target level. The pitch changes by the same small amount, which is not audible.

## Sources

- Hans-Kristian Arntzen, "Dynamic Rate Control for Retro Game Emulators", 12 December 2012. Read from `https://raw.githubusercontent.com/libretro/docs/master/archive/ratecontrol.pdf`. Equations (2) to (7) give the law, its stability and its fixed point. Section 4.1 reports that `0.002 <= d <= 0.005` works in RetroArch.
- RetroArch v1.19.1, `audio/audio_driver.c`, function `audio_driver_flush`: the law as code (`half_size`, `delta_mid`, `direction`, `adjust`, `source_ratio_current`).
- RetroArch v1.19.1, `retroarch.c`, function `audio_driver_monitor_adjust_system_rates`: the input rate is the core's sample rate times the display rate over the core's frame rate, when the skew is at most `audio_max_timing_skew`.
- RetroArch v1.19.1, `config.def.h`: `DEFAULT_RATE_CONTROL_DELTA 0.005f`, `DEFAULT_MAX_TIMING_SKEW 0.05f`, `DEFAULT_OUT_LATENCY 64` (milliseconds) on most platforms.

## Units and names

All rates at the API are integers, as the contracts require: core rates in millihertz, the device rate in hertz. A "frame" of audio is one stereo pair (left, right). Inside the pipeline, ratios and gains are `f64` and `f32` values; no float crosses the API.

| Name | Meaning | Unit | Example |
| --- | --- | --- | --- |
| `f_in` | Core sample rate (libretro `sample_rate`) | mHz | 32 040 500 |
| `f_core` | Core frame rate (libretro `fps`) | mHz | 60 099 (60.0988 Hz, rounded) |
| `f_run` | Run rate: core frames the session runs per second of real time | mHz | 60 099 |
| `f_dev` | Device rate: the rate of the sink (`AudioSink::rate_hz`) | Hz | 48 000 |
| `q` | Queue level: `AudioSink::queued_frames()`, read once per core frame, just before the write | output frames | 1536 |
| `A_B` | Control span (the paper's buffer capacity) | output frames | 3072 at 48 kHz |
| `d` | Maximum ratio deviation | none | 0.005 |

Rounding 60.0988 Hz to 60 099 mHz is an error of 3.3 ppm. Rate control absorbs it, as it absorbs the error of every crystal.

## Rates and the nominal ratio

The session gives the pipeline `f_in`, `f_core` and `f_run` at launch (`f_in` and `f_core` from the core's AV info; `f_run = f_core` when it knows no better figure yet), and a new `f_run` whenever it knows a better figure. The pipeline computes:

```
f_in_eff = f_in * f_run / f_core          (the rate at which audio arrives, in real time)
r0       = f_dev / f_in_eff
         = f_dev * f_core / (f_in * f_run) (nominal ratio: output frames per input frame)
```

This is RetroArch's `audio_driver_monitor_adjust_system_rates`. The session decides `f_run`; the pipeline does the arithmetic. `f_run` is the rate at which the session calls the core, not the refresh of the mode:

| Output | `f_run` |
| --- | --- |
| KMS, normal | The field rate of the mode read back from `set_mode` (`Modeline::field_rate_mhz`): the solver's achieved refresh, as the Definition asks |
| KMS, double refresh (`exact_x2`) | That field rate divided by 2: the core runs one frame per two flips |
| KMS, forced 50 or 60 Hz | The field rate of the forced mode |
| Desktop window | Measured from the flip times with `FlipRateMeter`, because the window flips at the desktop rate, which is not the mode rate (`crates/neptron-window/README.md`, Pacing) |

The pipeline refuses `f_run / f_core` outside `[1/2, 2]` (`AudioError::RunRatio`) at launch, and later keeps the old value. RetroArch applies the adjustment only when the skew is at most 5%. NEPTRON applies it at any skew in that range, because a forced 50 or 60 Hz rate changes the game speed by 20% and the audio must follow the game (see below).

`FlipRateMeter` takes each `FlipEvent::time_ns` and returns the mean flip rate, in mHz, over the last 120 flip intervals (2 s at 60 Hz). It returns nothing before 30 intervals. It uses a fixed array and does no allocation.

## The control law

Once per core frame, before the write, the pipeline reads `q` and sets the ratio. The session calls `end_frame` after the present of the frame and before the wait for its flip (see "Frame order"), so the read is at the same point of every frame:

```
direction = clamp((A_B - 2 q) / A_B, -1, 1)
r         = r0 * (1 + d * direction)
```

This is the paper's equation (2). In RetroArch's code, `avail = A_B - q` is the free space, `direction = (avail - A_B/2) / (A_B/2)`, which is the same value, and `source_ratio_current = source_ratio_original * (1 + rate_control_delta * direction)`.

- The queue below half the span gives a ratio above `r0`: more output frames per input frame, so the queue fills.
- The queue above half the span gives a ratio below `r0`, so the queue drains.
- The clamp is not in RetroArch, where `q` can never exceed the driver buffer. Here `A_B` is a control parameter, smaller than the sink queue, so `q` can exceed it, and the clamp keeps `|r / r0 - 1| <= d` at all times.

Constants (in `crates/neptron-audio/src/control.rs`):

| Constant | Value | Why |
| --- | --- | --- |
| `MAX_RATE_DEVIATION_PPM` (`d`) | 5000 (0.5%) | The task brief; RetroArch's default; the upper end of the paper's range. It covers a rate error of up to 0.5% |
| `CONTROL_SPAN_MS` (`A_B`) | 64 ms of output frames | RetroArch's default latency. 3072 frames at 48 kHz |
| Target level | `A_B / 2`: 32 ms, 1536 frames at 48 kHz | The paper's fixed point (7) with a correct estimate |

**Stability.** With `R` output frames played per core frame and `R'` the estimate, the paper's equation (5) gives an exponential approach with a time constant of `tau = A_B / (2 d R')` core frames: 3072 / (2 * 0.005 * 800) = 384 frames, 6.4 s at 60 Hz. The fixed point is equation (6). With the relative estimate error `e = R / R' - 1`:

```
q_c = A_B * (d - e) / (2 d)
```

| Estimate error `e` | Fixed point `q_c` |
| --- | --- |
| 0 | 0.50 `A_B` (32 ms) |
| +-0.1% (device crystal, test case) | 0.40 or 0.60 `A_B` |
| +-0.2% (display rate not given to the pipeline) | 0.30 or 0.70 `A_B` |
| +-0.5% or more | The law saturates. The queue runs empty or full |

So the session must give the real `f_run`. Then `e` is only the device crystal error, typically well under 0.01%.

## Latency

| Part | Typical | How it is measured |
| --- | --- | --- |
| Queue just before a write | 32 ms (target), 25.6 to 38.4 ms with a 0.1% crystal error | `queued_frames()` before each write; in the fakes, from `FakeAudio::writes` |
| Queue between writes | plus up to one core frame of audio (16.7 ms at 60 Hz) | Same |
| SDL device period | 1024 frames, 21.3 ms at 48 kHz, 23.2 ms at 44.1 kHz (`SDL_AUDIO_DEVICE_SAMPLE_FRAMES` hint, `DEVICE_PERIOD_FRAMES`). SDL's ALSA backend makes a buffer of two periods | `Sdl3Sink::device_buffer_frames` (one period) |
| Resampler delay | 95 output frames, 2.0 ms (sinc, at 48 kHz from 32 kHz); 2 frames (polynomial) | rubato `output_delay()`, tested |
| Total, mean | about 64 ms, plus up to one more device period with ALSA (UNVERIFIED) | The sum. The delay from the DAC to the speaker needs hardware to measure |

The input side adds no delay: each core frame is resampled in one call, whatever its length, so no input waits for a full chunk.

## Frame order

The session runs one frame in this order:

```text
flip event -> input -> core (retro_run) -> mute if asked -> present -> end_frame -> wait for the flip -> unmute when no longer asked -> run rate update
```

This is the wait pacing. When a core frame does not fit in one flip interval, the loop uses the late pacing of the section "A core slower than the field":

```text
end of the present before -> input -> core (retro_run) -> mute if asked -> wait for the flip before -> present -> end_frame -> unmute when no longer asked -> run rate update
```

The present uploads the frame, queues the GPU's work and asks for a nonblocking page flip, which the kernel makes once the GPU has drawn the frame. `end_frame` resamples the frame's audio (1.3 to 1.4 ms on the Raspberry Pi 3) while the GPU draws, so it is off the path from the flip event to the GPU submit. A frame with no core (the quick menu) calls `idle_frame` at the same place. A failed present still ends the audio of the frame. With double refresh only the first present of the core frame is followed by `end_frame`. A UI over the game (the quick menu, a dialog, a notice) costs no more than the game alone: the app draws the canvas only when its scene changes, the renderer sends only the changed canvas rows, and the game quad is drawn only outside the opaque UI.

A modeset happens inside the present and blocks it. The first flip of the new mode comes one field of that mode after it. The mute comes before the present: it fills the queue with silence up to the cover, `MUTE_COVER_MS` (100 ms), which covers the modeset and the wait for the first flip. On the Raspberry Pi 3 a change of the TV standard takes about 75 ms from the last flip of the old mode to the first flip of the new one (ftrace), so the device keeps playing silence through it and the pipeline counts no underrun. A pipeline test runs a gap of 75 ms and of 90 ms from the mute to the next frame and finds no underrun; a session test runs a 30 ms modeset into 50.007 Hz and loses no frame. A modeset longer than the cover empties the muted queue: the device plays silence it does not have, which is still silence, and the pipeline counts an underrun. The cover stays below the 128 ms queue of the SDL sink.

The pipeline reads `q` after the present, at the same point of every frame. The law holds `q` at its target at that point, so the target, the time constant and the fixed point are those above. The audio of a frame reaches the sink after the present, about 0.7 ms later on the Pi, while the picture keeps its timing. The latency in the table above is measured from the write. The session counts missed fields (`Counters::missed_fields`, `fields_missed` in the stats reply): each flip of a frame in which the core ran counts the vertical blanks since the flip before, beyond the blanks that one flip takes (1, or 2 for an interlaced mode on a driver that flips once per frame). The first flip of a mode counts nothing. The tests of timing check 6 run on the simulated clock, where a present takes no time, and check that the session counts each late blank as a missed field. A session test records, at each write of the sink, whether a flip of the fake display is pending, and so checks in every frame that the audio goes out between the present and the flip.

## A core slower than the field

### The problem

In the frame order above, a core frame starts when the flip of the frame before completes, at a vertical blank `V(n)` (`session/frame.rs`, `show`: `present`, then `wait_flip`). The present asks for a flip at the first blank after the commit (`neptron-kms`, `KmsDisplay::present`: an atomic commit with `NONBLOCK | PAGE_FLIP_EVENT`; `wait_flip` polls the DRM descriptor for the event). When the core and the present take less than one flip interval, the flip lands at `V(n+1)`. When they take a little more, the flip cannot land at `V(n+1)`. It lands at `V(n+2)`, the loop waits for it, and only then does the next core frame start. Each late frame costs a whole field, and the next frame starts with no head start, so it is late again. A core that needs 4% more than one field runs at half speed.

The audio has the same cause: with the run rate at the field rate, such a core makes the audio of half the frames, and the rate control can correct only 0.5%, so the queue runs dry again and again. `docs/hardware-checklist.md` has the measured case (Killer Instinct on FinalBurn Neo).

### Two pacings

The loop has two pacings.

- **Wait pacing.** The frame order above. Each frame waits for its last flip before the next frame reads the input. This is the default. The delay from the input poll to the flip of the frame is one flip interval.
- **Late pacing.** After the present of a core frame, the loop does not wait for its last flip. The next frame reads the input and runs the core at once, while the display still shows the frame before. After its core ran, the next frame waits for that flip (`VideoEngine::wait_flip`), then presents. The display takes one queued flip only (a second atomic commit before the flip event gives `EBUSY`), so at most one frame waits for its flip while the next frame runs. The present still draws after the flip of the frame before, as in the wait pacing, so the display needs no extra buffer.

With late pacing a frame that is late by a part of a field loses only that part: the next frame starts at the end of the present, not at the late blank.

The pacing uses three values of each core frame, on the session clock and the flip events:

- `work`: the time of the frame's own work, counted from the flip that it follows. In wait pacing it is the time from the flip of the frame before, moved to the session clock (`FlipClock`), to the end of the first present of this frame. In late pacing it is the core time (from the end of the frame before to the start of the wait for the flip before) plus the time from the later of that flip and the start of the wait to the end of the present. So the time from a flip to the return of its wait counts too, as in wait pacing. It does not hold the time the GPU draws after the commit: the flip waits for the GPU's fence.
- `interval`: the flip interval of the mode on screen, `10^12 / field_rate_mhz` nanoseconds times the blanks that one flip takes (1, or 2 for an interlaced mode on a driver that flips once per frame), from the mode read back.
- A late flip: the gap from the flip before is more than the blanks one flip takes, so the flip landed after the blank it aimed at. The gap is the difference of the vertical blank counts; when a driver does not advance the count, it is the time between the flips in whole fields, rounded (more than 1.5 fields is 2). When the `work` of its frame was below `interval`, the GPU or the display made the flip late (`Counters::late_flips`).

The rules (constants in `crates/neptron-session/src/pacing.rs`):

| From | To | When |
| --- | --- | --- |
| Wait | Late | A core frame with `work >= interval`: its flip cannot land on its target blank, so the loop does not wait for it |
| Wait | Late | `LATE_FLIPS_TO_ENTER` (2) late flips in a row of frames whose `work` fit: the commit was in time, but the GPU was not done at the blank. The next frame does not wait for its flip. The loop learns the `work` of that frame as the late work |
| Late | Wait | A hold of core frames in a row whose `work` is below `interval`, whose flip lands on its blank, and, when a late work is learned, whose `work` is at least `TRY_MARGIN_PERMILLE` (30, 3%) below it. The frame after the hold waits for its flip |
| Any | Wait | A frame with no core run (the quick menu, the output off), a frame with a modeset, a frame with a display error, a frame whose present first drained the flip of a failed wait (its work holds that wait, so it is not known), and a frame in which the session ends. Such a frame waits for its flip, the hold is back at its shortest, and the late work is forgotten |

The hold:

- Each entry into late pacing uses the hold for its stay, then doubles the hold of the next stay, from `LATE_HOLD_FRAMES` (30 frames, 0.5 s at 60 Hz) up to `LATE_HOLD_MAX_FRAMES` (1920 frames, 32 s).
- In wait pacing the hold halves, down to 30 frames, after each `HOLD_DECAY_FRAMES` (600 frames, 10 s) with no late frame.

Why these rules:

- A rule for each frame alone (wait when the frame fits) loses the head start: with a core at 1.04 intervals, a frame after a late one fits, waits for the next blank, and the frame after it is late again. That gives 2 frames in 3 fields, 67%, and the input delay changes from frame to frame. The hold keeps a core near the limit in one pacing.
- The first hold is short (0.5 s) because the input delay of late pacing is one interval more: after one long frame (a disk read, a shader compile) a fast core is back at one interval of delay within 0.5 s.
- A loop that changes its delay again and again is worse than a steady delay of two intervals for a core at the edge. Long frames that come again before the hold has decayed lengthen each stay until the loop stays in late pacing. The decay is slower than the growth: long frames closer than 10 s apart lead to a stable late pacing; rarer ones each cost one short stay.
- Inside late pacing the hold cannot see the GPU time (the GPU draws while the next core frame runs). A core that the GPU makes late would pass every hold and fail every try of wait pacing. The learned late work stops this: such a core tries wait pacing again only when its work drops 3% below the work at which its flips were late.
- The late flips are what the display did, so the loop also sees a frame that is late although its commit is in time. The work alone cannot see the time of the GPU after the commit.

### What the player gets

`x` is `work / interval` of a core with a fixed cost. The presented rate is a fraction of the flip rate of the mode.

| `x` | Pacing | Presented rate | Missed fields per presented frame | Input poll to flip | Wait pacing alone |
| --- | --- | --- | --- | --- | --- |
| 0.5, 0.9, 0.95, 0.99 | Wait | 100% | 0 | 1 interval | the same |
| 0.95, with a GPU that draws 0.1 interval after the commit | Late | 100% after the entry | 3 at the entry, then 0 | 2 intervals | 50%, 2 intervals |
| 1.02 | Late | 98.0% (`1 / x`) | 0.02 | `x` to `x + 1` intervals | 50%, 2 intervals |
| 1.04 | Late | 96.2% | 0.04 | `x` to `x + 1` intervals | 50%, 2 intervals |
| 1.3 | Late | 76.9% | 0.3 | `x` to `x + 1` intervals | 50%, 2 intervals |
| 2.0 | Late | 50% | 1 | `x` to `x + 1` intervals | 33%, 3 intervals |
| 2.5 | Late | 40% | 1.5 | `x` to `x + 1` intervals | 33%, 3 intervals |

The steady states, in plain words:

- **A game that fits.** It stays in wait pacing for the whole session. Each frame shows at the next blank, one interval after the input was read.
- **A game a little too slow, or a GPU that is a little late.** It stays in late pacing. It runs at `1 / x` of the full rate (or at the full rate when only the GPU was late), the input shows one more interval later, and nothing changes from then on.
- **A game that fits, with a long frame now and then.** A long frame costs the fields by which it was late, then 0.5 s of late pacing, then wait pacing again. When long frames come again and again (closer than about 10 s apart), each stay is longer than the one before, until the loop stays in late pacing: a steady delay of two intervals, no more changes.
- **A game at the edge.** A core whose cost jitters around one interval enters late pacing once and stays there.
- **A game below half speed.** As a slow game, with no sound.

### Audio of a slow game

The honest result for a core that is too slow is that the game runs slower than real time, and its sound with it. The core makes one frame of audio per core frame. In late pacing the session measures the real rate of core frames and gives it to the pipeline as `f_run` (constants in `crates/neptron-session/src/speed.rs`):

- The session keeps the cycle times of the last `SPEED_WINDOW_FRAMES` (120) frames of late pacing, in a fixed array made at launch: the time from the end of the frame before to the end of the frame. The measured rate is the frames over the sum of the cycles, in mHz. The cycles of a late core end at the ends of their presents, not at vertical blanks, so the measure has no step of a whole field. A core at 1.04 intervals gives 96.2% of the flip rate.
- The measure is used when the core is slow, decided from the window, not from a run of frames: the window holds `SPEED_MIN_FRAMES` (8) cycles or more and its mean cycle without its single largest cycle is above `interval` by more than `SPEED_SLOW_MARGIN_PERMILLE` (5, 0.5%, the error that the rate control corrects). So a single long frame of a fast core (a disk read, a state load, a shader compile) changes nothing in the audio, and a core that is slow on average is measured: frames of 0.97 and 1.05 intervals in turn, a long frame every few frames.
- A core that is clearly slow is measured at once: `SPEED_CLEAR_SLOW_FRAMES` (2) cycles in a row of at least `SPEED_CLEAR_SLOW_PERMILLE` (1200, 1.2 intervals). A single long frame never passes this.
- The window follows a change of speed. When the mean of its last `SPEED_RECENT_FRAMES` (30, 0.5 s) cycles differs from the mean of its older cycles (at least as many) by more than `SPEED_CHANGE_PERMILLE` (100, 10%), the window keeps only the last 30 cycles, so the measure follows a new speed within about 60 frames, in both directions. A core with a long frame every few frames has the same mean in both parts and stays measured. A clearly slow run that turns clearly fast, `SPEED_CLEAR_SLOW_FRAMES` (2) cycles in a row of at most `SPEED_CLEAR_FAST_PERMILLE` (1050), clears the window at once: the end of a heavy scene stops the measure after 2 frames.
- `f_run` is the lower of the run rate of the "Rates and the nominal ratio" section and the measure. In wait pacing, and while the core is not slow, the target is the run rate of that section alone, so a game at full speed keeps the audio of the rate control alone.
- `f_run` moves to its target by at most `RUN_RATE_SLEW_PERMILLE_PER_S` (4000) of the mode's run rate per second, at the start of a measure, during it, and at its end, until it is back at the mode's rate. A change `D` (as a fraction) over `D / s` seconds puts the queue off by `D * D / (2 s)` seconds of audio: at 4 per second the largest change before the mute, 50%, costs at most 31 ms, the queue's target level, half of its span, and the change of a scene of Killer Instinct (33%) costs 14 ms. The pitch glides over at most 125 ms (for a change of 50%), and does not jump. A change inside the steady measure (the window moves) is a small fraction of a percent per frame.
- The queue corrects the run rate while a measure is in use, and until the run rate is back at the mode's rate with the queue inside its band. The band is `QUEUE_BAND_PERMILLE` (500): 16 ms either way of the 32 ms target, where the rate control alone holds it; a core with a long frame every few frames swings the level before a write inside it. Beyond the band, in proportion to the distance beyond it: the level smoothed over `QUEUE_SMOOTH_FRAMES` (16) frames trims the run rate by up to `QUEUE_TRIM_PERMILLE` (60, 6%) in either direction, a time constant of about 0.5 s; and a last level above twice the target (the span) moves the rate from the measure towards the mode's rate at once, all the way at three times the target, because then the core makes audio faster than measured. So after any change of speed the level before a write is back within half the target of its target within `QUEUE_SETTLE_S` (3 s, in the test of the scenes of Killer Instinct), and the queue never holds more than the 128 ms of the sink (`Sdl3Sink`); more is an overrun, which the sink drops, and the test asserts none. The pitch is up to a semitone off while the queue is far off. A game at full speed (no measure) keeps the audio of the rate control alone.
- So a game at 96% speed plays its sound at 96% of its tempo and pitch (0.7 semitones lower), with no underrun in the steady state. The rate control keeps its `d` of 0.5% and absorbs the rest.
- The pipeline refuses `f_run` below half the core's frame rate (`AudioError::RunRatio`). When the measure goes below `SOUND_MUTE_PERMILLE` (500, the bound of the pipeline) of the core's frame rate, the session mutes the audio (`AudioPipeline::mute`: a 5 ms ramp to silence), feeds it no core audio and calls `idle_frame` in place of `end_frame`, as under the quick menu. The device plays the ramp and the silence after it, then runs dry, which is not counted as an underrun. The sound comes back only when the measure is above `SOUND_RESUME_PERMILLE` (520): a core that hovers at half speed does not chop the sound. When the measure stops (wait pacing, a core no longer slow, no mode on screen), the mute ends too. The next frame's `end_frame` tops the muted queue up with silence, and the audio fades in at the frame after it. Below half speed the sound is more than an octave low and is no longer the game's sound.
- The entry into late pacing costs the queue audio once. The queue level is read after the present. In wait pacing that is just after the blank; a late frame reads it `x` intervals after the blank. Until the measure is used (2 frames for a clearly slow core) and the run rate has glided to it, each frame also brings `1 / x` of the audio the device plays. The queue target is 32 ms, about 1.9 intervals at 60 Hz. So the change of a scene from 1.0 to 1.5 intervals costs at most one underrun (one prime with a fade-in), and a core near or above the length of the queue (from about 1.9 intervals) costs a few at the entry. After that, none.
- A long frame that is longer than the 64 ms span of the rate control (4 intervals at 60 Hz), and that comes every few frames, makes the deepest dips of the queue touch zero now and then, for about 1 ms (about every 5 s in the test). This is the size of the queue, not the run rate: a frame of 3 intervals every 10 frames plays with no underrun.

### Notice and statistics

- `Session::sound_too_slow` is true while the audio is muted because the game runs below half speed.
- `Session::slow_speed_percent` returns the speed of the game in percent of the run rate of the mode on screen while the measure is used, the window is full (120 frames, about 2 s), and the speed is below `SLOW_NOTICE_PERMILLE` (980, 98%). Else `None`. A single long frame does not give it.
- The owner approved the two notice texts: "This game runs at {p}% speed. This machine is too slow for it." (from `slow_speed_percent`) and "Sound off: the game runs at less than half speed." (from `sound_too_slow`).
- `frames_presented` (`Counters::flips`) counts every flip that completed, also the flip that the next frame waits for in late pacing and the flip that the engine drains after a timed-out wait.
- A flip that the engine drains in a present after a failed wait (`FrameReport::drained_flips`) is no progress of the display for `FLIPLESS_FRAMES_LIMIT`: a display whose flips take 500 to 1000 ms ends the session with `DisplayStalled`. A flip that a wait of the loop returned, also the wait for the flip of the frame before in late pacing, is progress.
- `fields_missed` keeps its rule. Each flip counts its gap for the frame it shows: a flip that the next frame waits for counts for the frame that queued it.
- `frames_dropped` (`Counters::flipless_frames`) keeps its meaning: a frame whose flip the loop did not wait for is shown at the next blank, and is not flipless.
- `Counters::late_frames` counts the core frames whose flip the loop did not wait for, and `Counters::late_flips` the late flips of frames whose work fit. The stats reply gives both as `late_frames` and `late_flips` from protocol version 3 (`LATE_PACING_VERSION`), append-only.
- `FrameReport::pacing` gives each change of pacing. The app writes one log line at each change, through the log of the run. With the hold rules the loop changes pacing a few times at most, then reaches a steady state.
- `Counters::input_to_flip_ns` is the last delay from an input poll that returned at least one event to the first flip of the frame that ran the core with it, on the session clock. The flip event can come from another clock with another origin (the kernel's on KMS). Both clocks are monotonic, so the session keeps the lowest offset that a wait shows (the session time after the wait less the flip time: a wait that returns at the flip) and moves each flip time to its own clock with it (`FlipClock`). In late pacing the next frame waits for a flip that completed earlier, and the offset still gives the time of the flip. The app gives it as `input_to_frame_ns` in the stats reply.

#### The speed notices

The app decides when the two notices show in pure code (`apps/neptron/src/speed_notice.rs`, `SpeedNotices::step`). Each frame of a game gives it the two values of the session and the time of the clock of the run.

- The speed notice shows when `slow_speed_percent` first gives a value, with that value. A change of the value does not show it again.
- It shows again only after `slow_speed_percent` gave `None` (the game was no longer slow), and at least `SLOW_NOTICE_GAP_NS` (60 s) after the speed notice before. A game that is slow again before the 60 s pass gets the notice when they pass, if it is still slow.
- The sound notice shows at each turn of `sound_too_slow` to true. A modeset stops the measure and so ends the mute, so a slow game can get the sound notice again after a modeset.
- When both start in one frame, the sound notice comes first and the speed notice at the next frame.
- Both are Warning notices of the app, with the mechanism of every other notice: a notice in a game waits for the quick menu, and goes to the log once when it is added. Only the frame that adds a notice allocates its text.
- Tests: the unit tests of `SpeedNotices`; the tier 2 test `a_slow_game_tells_its_speed_and_its_sound_off` (`apps/neptron/tests/app/audio.rs`), a game whose frames take 40 ms on the simulated clock; and the scene `screens/notice-slow-speed`, which shows that the longest speed notice fits the notice box at 320x240.

### Safety and other paths

The pacing decides only when the loop waits for a flip. It sets no mode, calls no `Display` method of its own, and bypasses nothing.

| Path | In late pacing |
| --- | --- |
| A mode change | The engine sets every mode in its present, through `ValidatedMode`, after the frame waited for the flip of the frame before. A frame with a modeset waits for its flip and goes back to wait pacing; the rate meter starts again |
| Display guard and blind recovery | They change the engine's inputs (monitor, settings) between two frames. The next present applies them, after the wait for the flip before. The guard's timers are on the session clock and do not change |
| Quick menu | The frame that opens the menu waits for the flip of the frame before, presents the menu and waits for its own flip. Under the menu the loop is in wait pacing, with the shortest hold and nothing learned. After `resume` it starts in wait pacing |
| Output off (blank) | The engine blanks only with no flip queued; the frame waits for the flip of the frame before first. A blanked frame runs no core and is in wait pacing. With no mode there is no measure, and a mute for a slow core ends |
| Interlaced mode | The driver decides on which blank a flip lands; the loop changes only when it commits. `interval` is two fields on a driver that flips once per frame. Nothing in the commit chooses a field. Whether the field order stays right in late pacing is not proven: the fake display puts every flip on a blank where a flip can land by construction, so its test cannot show a swapped field. The 15 kHz CRT check of `docs/hardware-checklist.md` decides it |
| Double refresh (`repeat` 2) | Only the last flip of a core frame is not waited for. The first flip is waited for before the second present, because the display takes one queued flip. `work` ends at the first present, so the limit is one flip interval, as in wait pacing |
| Hot unplug, a display that stops flipping | The wait for the flip of the frame before is the bounded wait of the backend (500 ms on KMS). A wait that fails ends the frame with no present; the frame still ends its audio, and it is flipless. A drained flip is no progress. `FLIPLESS_FRAMES_LIMIT` ends the session. A new monitor profile is a modeset |
| A driver that does not count blanks | The gap between flips comes from their times, so late flips and missed fields are still seen |
| End of the session | A frame in which the core asks to stop waits for its flip, so no flip stays queued after the last frame |
| Drivers | The loop uses only what the atomic KMS API gives every driver: a flip completes at a vertical blank with an event, and one flip can be queued. There is no code for one board |

No thread is added. The frame thread waits in the same bounded flip wait, at a different point of the frame. The pacing state and the window of cycles are made at launch; a frame allocates nothing more.

On the desktop window the interval comes from the read back of the window's mode, while the window flips at the desktop rate. There the change to late pacing follows the mode rate, not the desktop rate. The window is a development output, and this is accepted.

### Tests

The session tests give each frame a cost on the simulated clock: the scripted input of the tests moves the clock forward by the cost in each poll, so the frame's work takes that time before the core runs. The test core cannot move the simulated clock: it is a separate library loaded by the host. The fake display models the GPU with `DisplayOptions::gpu_ns` (a flip lands only on a blank after the present plus that time) and a driver that does not count blanks with `DisplayOptions::frozen_vblank_count`. Each test runs a 60 Hz test core on the fake display.

| Case | Asserts |
| --- | --- |
| `x` 0.5, 0.9, 0.95, 0.99 | Wait pacing in every frame; one flip per core frame; no missed field; input poll to flip is one interval within 1 ns; no `slow_speed_percent` |
| `x` 1.02, 1.04, 1.3, 2.0, 2.5 | One change to late pacing and none back; presented rate `1 / x` of the flip rate within 0.5% after the entry; missed fields equal the repeated blanks of the fake display; no frame dropped (`flipless_frames` 0, every presented frame shown once, only the last flip still queued); input poll to flip from `x` to `x + 1` intervals |
| Sound at `x` 1.02, 1.04, 1.3, 2.0 | `f_run` within 0.5% of the measured rate; no underrun at 1.02 and 1.04, at most one at 1.3 and four at 2.0, all in the first second; no overrun |
| Sound at `x` 2.5 | `sound_too_slow`; at most four underruns at the entry; no `end_frame` and no underrun in the 60 frames after the run |
| `slow_speed_percent` | `100 / x` within 1 below 98%, else `None`; none for a jitter around 1.0, for 0.9, and after a single long frame |
| `x` 1.0, jitter of 5% | One change to late pacing and none back; presented rate above 96%; no underrun, no overrun |
| `x` 0.5 and 1.5 in turn | Presented rate above 99%; no frame dropped; at most one underrun |
| 60 s of 0.97 and 1.05 in turn; of 1.0 with 3.5 every 4 frames; of 0.5 with 3 every 10 frames; of 0.5 with 4 every 10 frames | `f_run` within 0.5% of the real rate of core frames; at most 2 pacing changes; no underrun after the first 3 s (at most 15 of about 1 ms for the frames of 4 intervals, longer than the span); no overrun |
| 200 frames fast, 300 frames at 1.3, 400 fast | The change of `f_run` in each frame is at most `RUN_RATE_SLEW_PERMILLE_PER_S` of the mode's rate per second of the frame; no overrun |
| The scenes of Killer Instinct: 12 s at 0.97 (jitter 3%), 17 s at 1.5 (jitter 5%), twice | In each second the queue holds at most the 6144 frames of the sink; from 3 s after each change the level before a write is within half the target of the target; at most one underrun per scene; no overrun |
| `x` 0.95, GPU 0.1 interval | Late pacing from the late flips; presented rate at least 98%; `late_flips` at least 2; at most one underrun; then 4000 frames (67 s) with no change of pacing, no missed field and no underrun |
| The same, with a vertical blank count of 0 | Presented rate at least 98% |
| The same, then the quick menu and a resume | One entry after the resume (3 late flips), then quiet |
| A long frame of 3 intervals every 180 frames | At most 10 changes of pacing in the first minute, none in the next 4 minutes |
| One frame of 2.5 fields, and one of 300 ms, of a fast core | For 240 frames after it: `f_run` is the mode's run rate in every frame, no mute, no overrun, at most one underrun, the queue at 0.4 to 0.6 of the span at the end |
| One frame of 1.5 intervals, then `x` 0, 0.5 or 0.95 | Late pacing for the shortest hold plus at most 2 frames, then wait pacing with an input delay of one interval; one missed field |
| A core at 49%, then 51%, then 55% of its rate | Muted at 49%, still muted at 51% in every frame, sound at 55% |
| Flips of 700 ms (two timed-out waits of 350 ms each) | The session ends with `DisplayStalled` after `FLIPLESS_FRAMES_LIMIT` frames; drained flips still count as presented |
| A failed wait for the flip of the frame before | The frame ends its audio; one flipless frame; the game goes on |
| Quick menu, modeset, a change of monitor and its revert, the output off and on, interlaced, double refresh, a lost flip, the end of the session | The rules of the safety table; at 2.5 intervals the mute ends while the output is off |

### Measured results

From the session tests on the simulated clock and the fake display (no GPU time unless the row says so): 600 frames with each cost after 30 frames with none, a 60.002 Hz mode. Hardware can differ: see `docs/hardware-checklist.md`.

| Case | Presented rate | Missed fields | Input poll to flip | Underruns | `f_run` at the end |
| --- | --- | --- | --- | --- | --- |
| `x` 0.5, 0.9, 0.95, 0.99 | 100% | 0 | 1 interval | 0 | 60 002 mHz |
| `x` 1.02 | 98.13% | 12 | 1.50 to 2.00 intervals | 0 | 58 825 mHz |
| `x` 1.04 | 96.17% | 24 | 2.00 intervals | 0 | 57 694 mHz |
| `x` 1.3 | 76.96% | 180 | 1.50 to 2.00 intervals | 1, at the entry | 46 155 mHz |
| `x` 2.0 | 50.00% | 599 | 2.00 intervals | 3, at the entry | 30 001 mHz |
| `x` 2.5 | 40.00% | 899 | 2.50 to 3.00 intervals | 1 counted, at the entry; then muted | not changed (muted) |
| `x` 1.0, jitter of 5% | 99.83% | | | 0 | |
| `x` 0.5 and 1.5 in turn | 100.00% | | | 0 | |
| 0.97 and 1.05 in turn, 3600 frames | | | | 0 | 59 398 mHz, the real rate 59 398 |
| 1.0 with 3.5 every 4 frames, 3600 frames | | | | 3, in the first 3 s | 36 924 mHz, the real rate 36 924 |
| 0.5 with 3 every 10 frames, 3600 frames | | | | 2, in the first 3 s | 51 800 mHz, the real rate 51 800 |
| 0.5 with 4 every 10 frames, 3600 frames | | | | 3 in the first 3 s, then 12 of about 1 ms | 48 323 mHz, the real rate 48 162 |
| The scenes of Killer Instinct, 58 s | | | | 1 at each entry into a heavy scene, 2 in all | the queue at most 2902 frames (60 ms) in the per-second samples, the level back in its band within 3 s of each change; no overrun |
| `x` 0.95, GPU 0.1 interval, 1200 frames | 100% after the entry | 3 | | 1 | |
| The same, vertical blank count 0 | 100% | | | | |
| A long frame of 3 intervals every 180 frames | | | | | 7 changes of pacing, then none |
| One frame of 2.5 fields of a fast core | | | | 1 | the mode's rate; queue at 1753 of 3072 frames after 240 frames |
| One frame of 300 ms of a fast core | | | | 1 | the mode's rate; queue at 1539 of 3072 frames |

Double refresh with a core at 1.3 fields keeps one core frame per two fields with a flip on every field: the game rate. An interlaced mode with one flip per frame and a core at 1.3 frames presents 76.9% of the frames; the fake display cannot show the field order (see the safety table).

## Resampler

`rubato` 5.0.1, `Async` resampler, fixed input, stereo, `f32` samples.

| Setting | `Resampling::Sinc` (default) | `Resampling::Polynomial` |
| --- | --- | --- |
| Interpolation | Windowed sinc, 128 taps, Blackman-Harris window, linear interpolation between 128 oversampled points | Cubic polynomial, no anti-alias filter |
| Cut-off | Automatic: 0.895 of the input Nyquist frequency (14.3 kHz for 32 kHz input) | None |
| Delay | 95 output frames at 32 kHz to 48 kHz | 2 output frames |
| CPU, this machine (M1 Pro, one core), release | 0.33% of real time | 0.08% |
| CPU, this machine, dev profile (`opt-level = 0`) | 21% of real time | 0.64% |
| CPU, Raspberry Pi 3 class (Cortex-A53, 1.2 GHz), estimate | 3% to 4% of one core, with NEON | under 1% |

The Pi 3 figure is an estimate: the M1 release figure times 10, the usual ratio for a NEON inner loop between these cores. On a Pi 3, `end_frame` as a whole (resampler, rate control, write) takes 1.3 to 1.4 ms per core frame, about 8 % of one core at 60 Hz (commit `9ef1978`). The resampler part alone is not measured.

- The sinc filter is what dynamic rate control needs: a variable ratio with band-limited interpolation. 256 taps with cubic interpolation cost 2.4 times more (0.78%) for a cut-off at 0.947; retro sound chips have little content above 14 kHz, so 128 taps is the choice.
- The resampler is made with a maximum relative ratio of 4.04: `f_run` may go from one end of its range (half the core rate) to the other (twice the core rate) after launch, a factor of 4, and `d` comes on top of it.
- The ratio changes once per core frame, with rubato's ramp over the chunk, so the pitch has no steps.
- The chunk size is set to the input of each core frame (`Resizable::set_chunk_size`), up to 100 ms of input. More input in one frame is cut, and the frames cut are counted.
- The filter is built once, for the ratio at launch, with the `f_run` the session gives then: a game forced to 60 Hz gets a filter for its real ratio. When a later `f_run` moves the ratio far from it, the cut-off stays where it was; for downsampling this can let a little content near the output Nyquist frequency alias. A new resampler at each change of `f_run` would remove it, at the cost of a click.
- `Resampling::Polynomial` exists for a CPU that cannot afford the sinc filter, and for the ten-minute timing tests in the dev profile (see Tests).

Licences: MIT OR Apache-2.0 (rubato, audioadapter, audioadapter-buffers, audioadapter-sample, num-integer, num-traits), MIT (windowfunctions), 0BSD OR Apache-2.0 (audio-codec-algorithms), Zlib OR MIT OR Apache-2.0 (visibility). `cargo deny` accepts all of them. With `default-features = false` the FFT resampler and its dependencies (`realfft`, `rustfft`) are not built. The only proc-macro crate is `visibility`. It uses `syn` 2, next to the `syn` 3 of the workspace: one more proc-macro build.

## Threads

The device side uses SDL3's push model, not a callback.

| Thread | Owner | Runs |
| --- | --- | --- |
| Frame thread (the main thread, which also initialises SDL) | The session | The core, `AudioPipeline::push_samples` and `end_frame`, the resampler, `Sdl3Sink::write` (`SDL_PutAudioStreamData`) and `Sdl3Sink::queued_frames` (`SDL_GetAudioStreamQueued`) |
| SDL audio device thread | SDL: started when the device opens, joined when it closes | Takes data from the audio stream, converts the format if the device needs it, and gives it to the OS |

No NEPTRON code runs on the audio thread, so the real-time rule (no lock, no allocation, no I/O in the audio callback) holds by construction. A pull callback would put our code on that thread and would need a lock-free single-producer single-consumer ring between the two; the push model gives the same queue inside SDL with no code of ours on the real-time side.

What the frame thread takes from SDL:

- SDL's stream lock, also taken by the device thread while it takes one device period (1024 frames). The device thread holds it for the copy and the format conversion of one period, so the frame thread waits at most microseconds.
- SDL's queue memory. SDL keeps the queue in chunks. This is SDL's code, not ours, and whether it allocates in the steady state is a point to check on the target.

The stream is opened at the device's own rate (`SDL_GetAudioDeviceFormat`), in 16-bit stereo, so SDL does not resample: rubato does all of it. SDL's queue has no limit, so `Sdl3Sink` sets one: 128 ms of frames (`SINK_CAPACITY_MS`), twice the control span. A write takes only what fits, as the `AudioSink` contract asks. An SDL error makes the sink act as full: `write` takes nothing and `queued_frames` reports the capacity.

Only the SDL audio subsystem is initialised. `Sdl3Sink::open` must run on the thread that initialised SDL first (the `sdl3` crate checks this); the sink is not `Send`.

## Behaviour

**Start.** The first `end_frame` finds an empty queue. The pipeline writes `A_B / 2` frames of silence first (priming), then the frame's audio with a 5 ms fade-in (`FADE_MS`). The level starts at the target, so the law has nothing to correct.

**Underrun.** When `end_frame` finds the queue empty, the device has run dry or is about to. When the frame before left audio in the queue, the pipeline counts an underrun (`AudioStats::underruns`). The first frame and the first frame after `idle_frame` let the queue run empty on purpose and do not count. A muted queue that runs dry counts. The count is per pipeline. When not muted, the pipeline primes again (silence to the target), fades in, and counts a restart. When muted, it writes the frame's silence and tops up to the target. It does not prime at any level above zero: inserting silence while audio still plays would add a gap. A level that is low but not zero is the law's job.

**Overrun.** When the sink takes fewer frames than offered, the rest are lost and counted (`AudioStats::overrun_frames`). With the session following the rules, the queue never reaches the sink limit: the law keeps it near 32 ms, and the limit is 128 ms. An overrun means a rate figure far from the truth, or a core that makes much more audio than its AV info says.

**Mute** (`mute()`), for a mode switch and for pause. The pipeline writes, at once, a 5 ms linear ramp from the last sample it wrote to zero, then silence up to the cover (`MUTE_COVER_MS`, 100 ms). So a mute silences at least about 100 ms of output. While muted, each `end_frame` still runs the resampler, so its state stays continuous, and writes silence of the same length, plus silence up to the target when the queue is below it. While the queue is above the target, a muted frame drops its silence down to the target and keeps one frame, so every frame still writes after its present. So the cover lasts only through a present that blocks: after a mute with frames running (a refused mode, a frame that the engine mutes) the level is back at the target within about 5 frames, the late pacing reads the true level, and a frame with more than 28 ms of output does not overrun the 128 ms sink. A TV standard change stays covered, because no frame runs during its modeset. The law keeps running.

**Unmute** (`unmute()`). The next `end_frame` fades the audio in over 5 ms. If the queue is empty, it primes first. If the queue is above the target, because the modeset was shorter than the cover, the next frames drop the start of their output until the level is back at the target, at most the cover less the target. Each of these frames keeps its last output frame, which the fade-in makes near silence, so every frame still writes after its present. So the cover adds no delay to the sound after a short modeset, and no old sound plays.

**Mode switch.** The engine mutes the video for the frames the monitor needs to resync. The session calls `mute()` before it starts the modeset and `unmute()` after the first flip in the new mode. A blocking modeset longer than the cover (100 ms) lets the device run dry. It plays silence, so nothing is heard, and the pipeline counts one underrun. The `end_frame` of the modeset frame runs after the present, still muted, and tops the queue up to the target with silence. From the next frame the level before each write is back at the target, and the law has nothing to recover. After the switch the session gives the new `f_run`.

**Pause** (quick menu). The session calls `mute()`. While the menu is open, each frame calls `idle_frame` in place of `end_frame`: it writes a pending cue with silence after it up to the target, and nothing when no cue is pending. The queue plays out the ramp and the silence, then runs dry; this is not counted as an underrun. On resume, the first core frame's `end_frame` runs still muted and fills the queue to the target with silence. The session unmutes after that frame's flip, and the next frame fades the audio in.

**Late flip without mute.** A missed vblank means one core frame of audio less in that time. The queue drops by one frame of audio (16.7 ms), and the law brings it back with the time constant `tau` (6.4 s). If the queue runs dry, this is an underrun (above).

**Forced 50 or 60 Hz.** The Definition says that without a region option the game runs at the forced rate and its speed changes by the ratio. The session gives the forced rate as `f_run`, so `r0` follows: the audio keeps in step with the game, and its pitch changes by the same ratio. A 50 Hz game forced to 60 Hz plays 20% faster and 20% higher (3.2 semitones); a 60 Hz game forced to 50 Hz plays 16.7% slower and lower. The resampler does not correct the pitch: that needs time stretching, which is not in the MVP. The video status shows a `speed` alert for the change. It does not mention the pitch.

**Double refresh.** The session gives half the flip rate as `f_run`. Nothing else changes.

**Fast-forward and slow motion.** Not in the MVP. When they come, the session mutes during fast-forward (the queue cannot take audio at several times the rate), as RetroArch does by default.

**Core rate change** (libretro `SET_SYSTEM_AV_INFO`). The session makes a new `AudioPipeline` with the same sink, the user volume and the rest of a pending cue. It keeps the run rate when the new core rate accepts it, else it uses the core's frame rate. The counters, `underruns` included, start from 0. A core rate change and `write_cue` (a frontend cue, rare) are the only times the pipeline allocates after launch.

## API for the session

```rust
// Pure: no device, no unsafe.
pub struct CoreTiming { pub sample_rate_mhz: u64, pub frame_rate_mhz: u64 }
pub enum Resampling { Sinc, Polynomial }
pub struct AudioPipeline<S: AudioSink> { /* resampler, buffers, sink */ }
impl<S: AudioSink> AudioPipeline<S> {
    pub fn new(sink: S, core: CoreTiming, run_rate_mhz: u64, resampling: Resampling)
        -> Result<Self, AudioError>;                  // allocates
    pub fn set_run_rate_mhz(&mut self, run_rate_mhz: u64) -> Result<(), AudioError>;
    pub fn push_samples(&mut self, stereo: &[i16]);   // the frame's audio from the host; any number of calls per frame
    pub fn end_frame(&mut self);                      // once per core frame, after retro_run and the present
    pub fn idle_frame(&mut self);                     // a frame with no core: writes a pending cue only
    pub fn write_cue(&mut self, stereo: &[i16]) -> usize; // a frontend cue, mixed after the gain
    pub fn set_volume_percent(&mut self, percent: u32); // ramps over VOLUME_RAMP_MS
    pub fn run_rate_mhz(&self) -> u64;
    pub fn mute(&mut self);
    pub fn unmute(&mut self);
    pub fn stats(&self) -> &AudioStats;
    pub fn sink(&self) -> &S;
}
pub struct RateControl;      // the law alone
pub struct FlipRateMeter;    // f_run from flip times, for the window
pub enum AudioError;

// Feature `sdl3`.
pub struct Sdl3Sink;         // impl AudioSink
impl Sdl3Sink { pub fn open() -> Result<Self, SinkError>; pub fn device_buffer_frames(&self) -> Option<u32>; }
```

`push_samples` and `end_frame` do no allocation, no I/O and take no lock of ours. Every buffer is made in `new`.

## Tests (the audio part of timing check 6)

All run on `FakeAudio` and `SimClock`, with no sleep and no thread. A test harness plays the session: at each simulated flip it moves the clock, pushes one core frame of audio and calls `end_frame`. Flip times and audio per frame come from exact rational rates (60.0988 Hz is 600 988 / 10 000), so the harness has no rounding drift; the pipeline gets the rounded millihertz values, as in the product. The sink queue holds 128 ms, as `Sdl3Sink` does.

| Test | Scenario | Asserts |
| --- | --- | --- |
| Steady | 60.0988 Hz core, 32 040.5 Hz audio, 48 000 Hz device, display at the core's rate, 120 simulated seconds | No underrun, no overrun; level before each write inside `[0.45, 0.55] A_B` after the first second; ratio deviation never above 0.5% |
| Device fast and slow | Same, device 0.1% fast, and 0.1% slow | Level inside `[0.25, 0.75] A_B` |
| Display off, rate given | Display 0.2% off the core's rate, `f_run` given | Level inside `[0.45, 0.55] A_B` |
| Display off, rate not given | Display 0.2% off, the pipeline not told | No underrun, no overrun; level inside `[0.15, 0.85] A_B` |
| Mode switch | `mute()`, one frame, 100 ms with no frame, `unmute()` | The device runs dry on silence only; one underrun, inside the pause; the first frame after the pause primes the queue, and from the second frame the level before each write is inside `[0.45, 0.55] A_B`; the output stays continuous |
| Pause, stall, late flip | A 3 s muted pause; a 100 ms stall without mute; one missed flip | The pause and the stall restart the queue once each. After a missed flip the law brings the level back within 0.01 `A_B` of the target in 30 s (about 5 `tau`), with no underrun |
| Continuity | 440 Hz sine | Each step between two output samples is at most the slope of the sine plus the fade slope plus 2 LSB |
| Pitch | 440 Hz sine, at the core's rate and with the device 0.1% fast and slow | Frequency of the output, on the device's real clock, within 0.5% in each 1 s window. Over the run: within 0.01% plus the part the level drift explains. While the law moves the queue from the target to its fixed point, the device plays the frames the queue loses on top of the frames written, so the mean pitch moves by that change over the frames played (0.019% in 29 s for a 0.1% crystal error) |
| Forced 60 Hz | 50 Hz core shown at 60 Hz, `f_run` given | Level inside `[0.45, 0.55] A_B`; pitch 1.2 times the input, within 0.5% |
| Random rates | Fixed seed, twelve pairs of 30 s: core sample rates, frame rates, device rates, crystal and display errors | Level inside `[0.25, 0.75] A_B` for each pair |
| Determinism | The steady run twice: 5 s polynomial, 3 s sinc | The same sample hash |

`[0.45, 0.55]` applies where every rate is exact and `f_run` is given: there the measured levels are 0.499 to 0.501, and an error of 0.1% in `r0` moves the fixed point to 0.40 or 0.60, so it fails. A crystal or display error keeps the wider bounds, which hold the fixed points of the table above with margin.

**Run lengths.** The law's time constant `tau` is 6.4 s. A run of 120 simulated seconds is about 19 `tau`: the queue has reached its fixed point long before the end, and a longer run proves nothing more about convergence or bounds. Timing check 6 asks for ten minutes; one ten-minute run of the steady scenario is an ignored test, run on demand with `cargo xtask test -p neptron-audio --run-ignored only`. The random pairs run 30 s each, about 4.7 `tau`.

**Resampling modes in the tests.** The ten-minute part of check 6 (and its 120 s runs) is proven with `Resampling::Polynomial`, because the sinc filter costs 21% of real time in the dev profile. Rate control sees only frame counts, which are the same for both modes except for the constant resampler delay. The default, `Resampling::Sinc`, runs for 30 simulated seconds (about 4.7 `tau`) in the fixed-rate scenarios (the core's own rate, the device 0.1% fast and slow, the forced 60 Hz game) and for 15 s around the mode switch; these are the pitch and continuity tests. The sample hash is the same in every run on one machine. It is not the same across CPUs with the sinc filter, because rubato picks AVX, SSE, NEON or scalar code at run time and the sums are made in a different order.

The SDL3 sink has one smoke test, ignored by default because it needs a sound device: a quiet 440 Hz tone through `AudioPipeline` and `Sdl3Sink` for one second, paced by the device's own queue. `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only` runs it. A person must listen to know whether the tone is heard.

## Measured results

On the fakes, 120 s polynomial runs (the random pairs 30 s), levels as fractions of `A_B`, judged after the first second:

| Scenario | Levels before a write | Largest deviation after 1 s |
| --- | --- | --- |
| Core rate, device exact | 0.499 to 0.501 | 0.00001 |
| Device 0.1% fast / slow | 0.400 to 0.485 / 0.514 to 0.600 | 0.00100 |
| Display 0.2% off, rate given | 0.499 to 0.501 | 0.00001 |
| Display 0.2% fast / slow, not given | 0.528 to 0.700 / 0.299 to 0.471 | 0.00201 |
| 50 Hz game forced to 60 Hz | 0.500 to 0.500 | 0.00000 |
| Twelve random rate pairs | 0.424 to 0.591 | |

The fixed points are those of equation (6): 0.40, 0.60, 0.30 and 0.70. The deviation is 0.5% only on the first frame, which finds the queue empty. Every run has no underrun and no overrun.

- Pitch of a 440 Hz tone: worst 1 s window 0.00006% at the core's rate, 0.080% with the device 0.1% off, 0.00033% at 528 Hz forced to 60 Hz.
- Largest step of the 440 Hz tone at a quarter of full scale: 473, against a bound of 511 (567 against 606 at 528 Hz).
- Mode switch: the first frame after the 100 ms pause primes the queue; from the second frame the level before each write is 0.500 to 0.501 of the span.
- Late flip: 0.497 to 0.498 of the span 30 s after one missed flip.

## Open points

- `AudioSink` has no pause. With one, the session could stop the device during a modeset, and the queue (silence) would survive the switch, with no dry device. Default: mute, let it run dry, prime again. Where: `AudioPipeline::mute`. How to change: add `pause` and `resume` to `AudioSink` (a contract change, rule 4).
- The paper's section 2.5 re-estimates `R'` from the mean level. RetroArch does not. Default: no re-estimation; the session gives `f_run`. Where: `RateControl`. How to change: add a slow estimate of the mean level and correct `r0` with equation (8).
- The latency from the queue to the speaker, SDL's queue allocations, and the cost of the resampler alone on a Pi 3 need hardware. On the Pi 3 analog output, periods of 1024 frames play with no ALSA underrun in 60 s (commit `dff9f44`), and `end_frame` takes 1.3 to 1.4 ms.
