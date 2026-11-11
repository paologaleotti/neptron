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

Once per core frame, before the write, the pipeline reads `q` and sets the ratio:

```
direction = clamp((A_B - 2 q) / A_B, -1, 1)
r         = r0 * (1 + d * direction)
```

This is the paper's equation (2). In RetroArch's code, `avail = A_B - q` is the free space, `direction = (avail - A_B/2) / (A_B/2)`, which is the same value, and `source_ratio_current = source_ratio_original * (1 + rate_control_delta * direction)`.

- The queue below half the span gives a ratio above `r0`: more output frames per input frame, so the queue fills.
- The queue above half the span gives a ratio below `r0`, so the queue drains.
- The clamp is new. In RetroArch `q` can never exceed the driver buffer. Here `A_B` is a control parameter, smaller than the sink queue, so `q` can exceed it, and the clamp keeps `|r / r0 - 1| <= d` at all times.

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
| SDL device buffer | 512 frames, 10.7 ms at 48 kHz (`SDL_AUDIO_DEVICE_SAMPLE_FRAMES` hint) | `Sdl3Sink::device_buffer_frames` |
| Resampler delay | 95 output frames, 2.0 ms (sinc, at 48 kHz from 32 kHz); 2 frames (polynomial) | rubato `output_delay()`, tested |
| Total, mean | about 53 ms | The sum. The delay from the DAC to the speaker needs hardware to measure |

The input side adds no delay: each core frame is resampled in one call, whatever its length, so no input waits for a full chunk.

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

The Pi 3 figure is an estimate: the M1 release figure times 10, the usual ratio for a NEON inner loop between these cores. It needs a measurement on the Pi.

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

- SDL's stream lock, also taken by the device thread while it takes one device buffer (512 frames). The device thread holds it for the copy and the format conversion of one buffer, so the frame thread waits at most microseconds.
- SDL's queue memory. SDL keeps the queue in chunks. This is SDL's code, not ours, and whether it allocates in the steady state is a point to check on the target.

The stream is opened at the device's own rate (`SDL_GetAudioDeviceFormat`), in 16-bit stereo, so SDL does not resample: rubato does all of it. SDL's queue has no limit, so `Sdl3Sink` sets one: 128 ms of frames (`SINK_CAPACITY_MS`), twice the control span. A write takes only what fits, as the `AudioSink` contract asks. An SDL error makes the sink act as full: `write` takes nothing and `queued_frames` reports the capacity.

Only the SDL audio subsystem is initialised. `Sdl3Sink::open` must run on the thread that initialised SDL first (the `sdl3` crate checks this); the sink is not `Send`.

## Behaviour

**Start.** The first `end_frame` finds an empty queue. The pipeline writes `A_B / 2` frames of silence first (priming), then the frame's audio with a 5 ms fade-in (`FADE_MS`). The level starts at the target, so the law has nothing to correct.

**Underrun.** When `end_frame` finds the queue empty, the device has run dry or is about to: the session missed frames. The pipeline primes again (silence to the target) and fades in, and counts a restart. It does not prime at any level above zero: inserting silence while audio still plays would add a gap. A level that is low but not zero is the law's job.

**Overrun.** When the sink takes fewer frames than offered, the rest are lost and counted (`AudioStats::overrun_frames`). With the session following the rules, the queue never reaches the sink limit: the law keeps it near 32 ms, and the limit is 128 ms. An overrun means a rate figure far from the truth, or a core that makes much more audio than its AV info says.

**Mute** (`mute()`), for a mode switch and for pause. The pipeline writes, at once, a 5 ms linear ramp from the last sample it wrote to zero, then silence up to the target level. While muted, each `end_frame` still runs the resampler, so its state stays continuous, and writes silence of the same length, plus silence up to the target when the queue is below it. The law keeps running.

**Unmute** (`unmute()`). The next `end_frame` fades the audio in over 5 ms. If the queue is empty, it primes first.

**Mode switch.** The engine mutes the video for the frames the monitor needs to resync. The session calls `mute()` before it starts the modeset and `unmute()` after the first flip in the new mode. A blocking modeset longer than the queue (32 ms plus one frame) lets the device run dry, but it plays silence, so nothing is heard; the first frame after the switch primes the queue again. That first frame finds the queue empty and fills it to the target, so from the second frame the level before each write is back at the target, and the law has nothing to recover. After the switch the session gives the new `f_run`.

**Pause** (quick menu). The session calls `mute()` and stops calling `end_frame`. The queue plays out the ramp and the silence, then runs dry. On resume it calls `unmute()`; the first frame primes.

**Late flip without mute.** A missed vblank means one core frame of audio less in that time. The queue drops by one frame of audio (16.7 ms), and the law brings it back with the time constant `tau` (6.4 s). If the queue runs dry, this is an underrun (above).

**Forced 50 or 60 Hz.** The Definition says that without a region option the game runs at the forced rate and its speed changes by the ratio. The session gives the forced rate as `f_run`, so `r0` follows: the audio keeps in step with the game, and its pitch changes by the same ratio. A 50 Hz game forced to 60 Hz plays 20% faster and 20% higher (3.2 semitones); a 60 Hz game forced to 50 Hz plays 16.7% slower and lower. The resampler does not correct the pitch: that needs time stretching, which is not in the MVP. The quick menu already says that the speed changes; it should also say that the pitch changes.

**Double refresh.** The session gives half the flip rate as `f_run`. Nothing else changes.

**Fast-forward and slow motion.** Not in the MVP. When they come, the session mutes during fast-forward (the queue cannot take audio at several times the rate), as RetroArch does by default.

**Core rate change** (libretro `SET_SYSTEM_AV_INFO`). The session makes a new `AudioPipeline`. That is the only time the pipeline allocates after launch.

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
    pub fn push_samples(&mut self, stereo: &[i16]);   // from the core's audio callbacks, any number per frame
    pub fn end_frame(&mut self);                      // once per core frame, after retro_run
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

The SDL3 sink has one smoke test, ignored by default because it needs a sound device: a quiet 440 Hz tone through `AudioPipeline` and `Sdl3Sink` for one second, paced by the device's own queue. `cargo xtask test -p neptron-audio --features sdl3 --run-ignored only` runs it.

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
- SDL3 smoke test on this machine (macOS, built-in output): the device opened at 44 100 Hz with a 512-frame buffer and played one second of samples in 1.01 s, with no overrun. A person must listen to know whether the tone is heard.

## Open points

- `AudioSink` has no pause. With one, the session could stop the device during a modeset, and the queue (silence) would survive the switch, with no dry device. Default: mute, let it run dry, prime again. Where: `AudioPipeline::mute`. How to change: add `pause` and `resume` to `AudioSink` (a contract change, rule 4).
- The paper's section 2.5 re-estimates `R'` from the mean level. RetroArch does not. Default: no re-estimation; the session gives `f_run`. Where: `RateControl`. How to change: add a slow estimate of the mean level and correct `r0` with equation (8).
- The latency from the queue to the speaker, the SDL device buffer on the target, SDL's queue allocations, and the CPU cost on a Pi 3 need hardware.
