//! Timing check 7 (tier T3): the real kernel path on `vkms`, in the QEMU
//! guest. `cargo xtask kms-check` builds this program for the guest and
//! runs it with the device open on descriptor 3 and `NEPTRON_DRM_FD=3`, as
//! the launcher passes it.
//!
//! It commits real modes through `KmsDisplay`, reads each mode back and
//! compares it field by field, checks the frame CRC after each switch,
//! compares the writeback picture with the reference renderer, measures the
//! flips, checks the error contract of the fake display, turns the output
//! off and on again, checks that no modeset happens before the first
//! commit, and checks the state after drop. Each check prints `PASS` or `FAIL`. The exit code is the number of
//! failed checks.
//!
//! Two short runs come first, each in its own process because a process
//! takes the launcher descriptor once: `--not-drm` with descriptor 3 a pipe,
//! and `--standard-fd` with `NEPTRON_DRM_FD=1`. Both must be refused before
//! the process owns the descriptor.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        let failed = match std::env::args().nth(1).as_deref() {
            Some("--not-drm") => linux::not_drm(),
            Some("--standard-fd") => linux::standard_fd(),
            _ => linux::run(),
        };
        println!("kms-check: {failed} failed");
        ExitCode::from(u8::try_from(failed).unwrap_or(u8::MAX))
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("kms-check needs Linux and vkms; run it with cargo xtask kms-check");
        ExitCode::FAILURE
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::fs::{File, OpenOptions};
    use std::io::{Read as _, Write as _};
    use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
    use std::os::unix::fs::OpenOptionsExt as _;
    use std::path::PathBuf;

    use drm::control::{Device as ControlDevice, crtc};
    use neptron_backend::{Display, DisplayError};
    use neptron_gfx::gles::{GlesError, GlesRenderer};
    use neptron_gfx::{OutputBuffer, ReferenceRenderer};
    use neptron_kms::{KmsConfig, KmsDisplay, KmsError, ModeRecord};
    use neptron_safety::{ValidatedMode, validate};
    use neptron_types::{
        CrtRange, Filter, FlipEvent, Frame, GpuCaps, Modeline, MonitorKind, MonitorProfile,
        PixelFormat, Rect, ScanlineKind, Scanlines, Scene,
    };

    /// The device the shell opened on descriptor 3.
    const DEVICE: &str = "/dev/dri/card0";
    const BUFFER_WIDTH: u16 = 1920;
    const BUFFER_HEIGHT: u16 = 480;
    /// Flips measured per mode. The dev build draws with software GL at
    /// about 4 frames per second in the guest, so this is about 8 seconds;
    /// the period comes from the kernel's vblank count, not the flip rate.
    const TIMED_FLIPS: usize = 30;
    /// The largest distance of the measured vblank period from the period
    /// of the mode, in millionths: 1 %. `vkms` makes its vblank with a
    /// kernel timer from the mode's own timings.
    const PERIOD_TOLERANCE_PPM: u64 = 10_000;
    /// The strength of the scanline scene: a dark line keeps half of each
    /// channel, so it differs from both the bright line and black.
    const SCANLINE_STRENGTH_PERCENT: u8 = 50;
    /// CRC entries that must agree for a still picture.
    const CRC_SAMPLES: usize = 3;
    /// Flips shown before the CRCs are read, so the picture is on the
    /// output.
    const CRC_SETTLE_FLIPS: usize = 4;

    /// Wide enough for every test mode on `vkms`, which has no hardware
    /// limit.
    const CAPS: GpuCaps = GpuCaps {
        pclock_min_hz: 1_000_000,
        pclock_max_hz: 400_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 3840,
        max_height: 2160,
    };

    struct Check {
        failed: u32,
    }

    impl Check {
        fn report(&mut self, name: &str, result: Result<String, String>) {
            match result {
                Ok(note) if note.is_empty() => println!("PASS {name}"),
                Ok(note) => println!("PASS {name}: {note}"),
                Err(why) => {
                    println!("FAIL {name}: {why}");
                    self.failed += 1;
                }
            }
        }
    }

    fn config() -> KmsConfig {
        KmsConfig {
            caps: CAPS,
            output: None,
            crt: true,
            buffer_width: BUFFER_WIDTH,
            buffer_height: BUFFER_HEIGHT,
            writeback: true,
        }
    }

    /// Descriptor 3 is a pipe that holds `PIPE_TEXT`. The backend refuses
    /// it, leaves it open and usable, and does not mark the launcher
    /// descriptor as taken.
    pub fn not_drm() -> u32 {
        let mut check = Check { failed: 0 };
        check.report(
            "a pipe as the launcher descriptor is NotDrm",
            match KmsDisplay::from_env(config()) {
                Err(KmsError::NotDrm { reason }) => Ok(reason),
                other => Err(format!("{:?}", other.err())),
            },
        );
        check.report(
            "the refused pipe stays open and usable",
            std::fs::read_to_string("/proc/self/fd/3")
                .map_err(|e| e.to_string())
                .and_then(|text| {
                    if text.trim() == PIPE_TEXT {
                        Ok(format!("read {text:?}"))
                    } else {
                        Err(format!("read {text:?}"))
                    }
                }),
        );
        check.report(
            "a refused descriptor is not marked as taken",
            match KmsDisplay::from_env(config()) {
                Err(KmsError::NotDrm { .. }) => Ok(String::new()),
                other => Err(format!("{:?}", other.err())),
            },
        );
        check.failed
    }

    /// `NEPTRON_DRM_FD=1`: the backend refuses standard output and leaves
    /// it working.
    pub fn standard_fd() -> u32 {
        let mut check = Check { failed: 0 };
        check.report(
            "standard output as the launcher descriptor is StandardFd",
            match KmsDisplay::from_env(config()) {
                Err(KmsError::StandardFd { fd: 1 }) => Ok(String::new()),
                other => Err(format!("{:?}", other.err())),
            },
        );
        check.report(
            "standard output still works after the refusal",
            Ok(String::new()),
        );
        check.failed
    }

    /// What the guest command writes into the pipe of `--not-drm`.
    const PIPE_TEXT: &str = "neptron-pipe";

    pub fn run() -> u32 {
        let mut check = Check { failed: 0 };
        let probe = match Probe::open() {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL open {DEVICE} for the readback: {e}");
                return 1;
            }
        };
        let before = probe.crtc_modes();
        check.report(
            "no CRTC has a mode before the first commit",
            if before.iter().all(Option::is_none) {
                Ok(String::new())
            } else {
                Err(format!("{before:?}"))
            },
        );
        let config = config();
        let mut display = match KmsDisplay::from_env(config.clone()) {
            Ok(d) => d,
            Err(e) => {
                println!("FAIL open the display from NEPTRON_DRM_FD: {e}");
                return check.failed + 1;
            }
        };
        println!(
            "display: driver {}, connector {}, CRTC {}, buffers linear by {}",
            display.driver_name(),
            display.connector_name(),
            display.crtc_id(),
            if display.linear_by_modifier() {
                "the modifier list"
            } else {
                "the usage flag (the driver takes no modifiers)"
            }
        );
        check.report(
            "the driver is vkms",
            expect_eq(display.driver_name(), "vkms"),
        );
        check.report(
            "open sets no mode",
            match probe.mode_of(display.crtc_id()) {
                Ok(None) => Ok(String::new()),
                other => Err(format!("{other:?}")),
            },
        );
        check.report(
            "the launcher descriptor has one owner",
            match KmsDisplay::from_env(config.clone()) {
                Err(KmsError::LauncherFdTaken { .. }) => Ok(String::new()),
                Err(e) => Err(format!("wrong error: {e}")),
                Ok(_) => Err("a second display took the same descriptor".to_owned()),
            },
        );
        contract_before_mode(&mut check, &mut display);

        let mut headless = match GlesRenderer::headless() {
            Ok(r) => r,
            Err(e) => {
                println!("FAIL make a headless GLES renderer: {e}");
                return check.failed + 1;
            }
        };
        check.report(
            "a window draw on a headless renderer is NoWindow",
            match headless.draw_to_window(&Pictures::new(1).scene(&test_modes()[0].mode), 320, 240)
            {
                Err(GlesError::NoWindow) => Ok(String::new()),
                other => Err(format!("{other:?}")),
            },
        );
        let modes = test_modes();
        let mut crcs: HashMap<(usize, u8), String> = HashMap::new();
        let mut crc = Crc::find();
        let mut reference = ReferenceRenderer::new();
        for (index, test) in modes.iter().enumerate() {
            println!("mode {}: {:?}", test.name, test.mode);
            let Some(valid) = test.validated() else {
                check.report(
                    &format!("{} passes validate", test.name),
                    Err(format!(
                        "{:?}",
                        validate(&test.mode, &test.profile, &CAPS).err()
                    )),
                );
                continue;
            };
            let name = test.name;
            let readback = display.set_mode(&valid);
            check.report(
                &format!("{name}: set_mode returns the request field by field"),
                match &readback {
                    Ok(m) => same_mode(&test.mode, m),
                    Err(e) => Err(e.to_string()),
                },
            );
            if readback.is_err() {
                continue;
            }
            check.report(
                &format!("{name}: a second descriptor reads the same mode"),
                match probe.mode_of(display.crtc_id()) {
                    Ok(Some(m)) => same_mode(&test.mode, &m),
                    other => Err(format!("{other:?}")),
                },
            );
            for (seed, label) in [(1_u8, "picture A"), (2, "picture B")] {
                let pictures = Pictures::new(seed);
                let scene = pictures.scene(&test.mode);
                check.report(
                    &format!("{name}: writeback of {label} equals the reference renderer"),
                    writeback_equals_reference(&mut display, &mut reference, &scene, &test.mode),
                );
                match crc.as_mut().map(|c| c.stable(&mut display, &scene)) {
                    Some(Ok(value)) => {
                        check.report(
                            &format!("{name}: frame CRC of {label} is stable"),
                            Ok(value.clone()),
                        );
                        crcs.insert((index, seed), value);
                    }
                    Some(Err(e)) => {
                        check.report(&format!("{name}: frame CRC of {label} is stable"), Err(e))
                    }
                    None => check.report(
                        &format!("{name}: frame CRC"),
                        Err("no vkms CRC directory in debugfs".to_owned()),
                    ),
                }
            }
            if let (Some(a), Some(b)) = (crcs.get(&(index, 1)), crcs.get(&(index, 2))) {
                check.report(
                    &format!("{name}: a different picture has a different CRC"),
                    if a == b {
                        Err(format!("both {a}"))
                    } else {
                        Ok(String::new())
                    },
                );
            }
            if test.mode.vactive / SOURCE_H >= 2 {
                let pictures = Pictures::new(3);
                let scene = pictures.scanline_scene(&test.mode);
                check.report(
                    &format!("{name}: scanlines: writeback equals the reference renderer"),
                    writeback_equals_reference(&mut display, &mut reference, &scene, &test.mode),
                );
                check.report(
                    &format!(
                        "{name}: scanlines: the window path equals the read-back path of GLES"
                    ),
                    window_equals_read_back(&mut display, &mut headless, &scene, &test.mode),
                );
            }
            let pictures = Pictures::new(1);
            let scene = pictures.scene(&test.mode);
            check.report(
                &format!("{name}: flips"),
                flips(&mut display, &scene, &test.mode),
            );
            contract_with_mode(&mut check, &mut display, &probe, &scene, &valid, name);
        }

        if let Some(first) = modes.first().and_then(TestMode::validated) {
            let pictures = Pictures::new(1);
            let scene = pictures.scene(first.mode());
            let back = display.set_mode(&first).map_err(|e| e.to_string());
            check.report(
                "the first mode again: same CRC as before",
                back.and_then(|_| {
                    let now = crc.as_mut().ok_or("no CRC")?.stable(&mut display, &scene)?;
                    let then = crcs.get(&(0, 1)).ok_or("no first CRC")?;
                    if &now == then {
                        Ok(now)
                    } else {
                        Err(format!("{now} now, {then} before"))
                    }
                }),
            );
            blank_contract(&mut check, &mut display, &probe, &scene, &first);
        }

        let last = probe.mode_of(display.crtc_id());
        let crtc_id = display.crtc_id();
        drop(display);
        check.report(
            "drop keeps the last validated mode with no picture, or switches the output off",
            match (last, probe.mode_of(crtc_id), probe.plane_fb(crtc_id)) {
                (Ok(Some(before)), Ok(Some(after)), Ok(None)) if before == after => {
                    Ok("the mode stays, the plane is off".to_owned())
                }
                (_, Ok(None), _) => Ok("the output is off".to_owned()),
                other => Err(format!("{other:?}")),
            },
        );
        too_large(&mut check, &probe, &modes, crtc_id);
        check.failed
    }

    fn contract_before_mode(check: &mut Check, display: &mut KmsDisplay) {
        let pictures = Pictures::new(1);
        let scene = pictures.scene(&test_modes()[0].mode);
        check.report(
            "present before the first mode is NoMode",
            expect_eq(display.present(&scene), Err(DisplayError::NoMode)),
        );
        check.report(
            "wait_flip before the first mode is NoMode",
            expect_eq(display.wait_flip(), Err(DisplayError::NoMode)),
        );
        check.report(
            "capture before the first mode is NoMode",
            expect_eq(display.capture().map(|_| ()), Err(DisplayError::NoMode)),
        );
    }

    fn contract_with_mode(
        check: &mut Check,
        display: &mut KmsDisplay,
        probe: &Probe,
        scene: &Scene<'_>,
        valid: &ValidatedMode,
        name: &str,
    ) {
        check.report(
            &format!("{name}: wait_flip with no flip queued is a device error"),
            is_device(display.wait_flip()),
        );
        if let Err(e) = display.present(scene) {
            check.report(&format!("{name}: present"), Err(e.to_string()));
            return;
        }
        check.report(
            &format!("{name}: present while a flip is pending is a device error"),
            is_device(display.present(scene)),
        );
        check.report(
            &format!("{name}: set_mode while a flip is pending is a device error"),
            is_device(display.set_mode(valid)),
        );
        check.report(
            &format!("{name}: the pending flip completes"),
            display
                .wait_flip()
                .map(|_| String::new())
                .map_err(|e| e.to_string()),
        );
        // Each modeset makes a new mode blob, so an unchanged MODE_ID of the
        // CRTC proves that no modeset commit ran.
        let crtc = display.crtc_id();
        let before = probe.crtc_prop(crtc, b"MODE_ID");
        let result = display.set_mode(valid);
        let after = probe.crtc_prop(crtc, b"MODE_ID");
        check.report(
            &format!("{name}: set_mode with the same mode makes no commit"),
            match (result, before, after) {
                (Ok(_), Ok(b), Ok(a)) if a == b && b != 0 => Ok(format!("MODE_ID stays {b}")),
                other => Err(format!("{other:?}")),
            },
        );
    }

    /// `blank` with a flip pending is busy. Then it turns the output off,
    /// and `set_mode` of the same mode commits again.
    fn blank_contract(
        check: &mut Check,
        display: &mut KmsDisplay,
        probe: &Probe,
        scene: &Scene<'_>,
        valid: &ValidatedMode,
    ) {
        let crtc = display.crtc_id();
        if let Err(e) = display.present(scene) {
            check.report("blank: present", Err(e.to_string()));
            return;
        }
        check.report(
            "blank while a flip is pending is a device error",
            is_device(display.blank()),
        );
        check.report(
            "blank: the pending flip completes",
            display
                .wait_flip()
                .map(|_| String::new())
                .map_err(|e| e.to_string()),
        );
        check.report(
            "blank succeeds",
            display
                .blank()
                .map(|()| String::new())
                .map_err(|e| e.to_string()),
        );
        check.report(
            "blank: a second descriptor sees the CRTC inactive with no mode and no plane",
            match (
                probe.crtc_prop(crtc, b"ACTIVE"),
                probe.crtc_prop(crtc, b"MODE_ID"),
                probe.mode_of(crtc),
                probe.plane_fb(crtc),
            ) {
                (Ok(0), Ok(0), Ok(None), Ok(None)) => Ok(String::new()),
                other => Err(format!("{other:?}")),
            },
        );
        check.report(
            "present after blank is NoMode",
            expect_eq(display.present(scene), Err(DisplayError::NoMode)),
        );
        check.report(
            "wait_flip after blank is NoMode",
            expect_eq(display.wait_flip(), Err(DisplayError::NoMode)),
        );
        check.report(
            "a second blank succeeds",
            display
                .blank()
                .map(|()| String::new())
                .map_err(|e| e.to_string()),
        );
        // MODE_ID is 0 after the blank, so a blob there now proves that
        // set_mode made a real commit for the mode set before the blank.
        let readback = display.set_mode(valid);
        check.report(
            "set_mode after blank commits the same mode again",
            match (&readback, probe.crtc_prop(crtc, b"MODE_ID")) {
                (Ok(_), Ok(blob)) if blob != 0 => Ok(format!("MODE_ID {blob}")),
                other => Err(format!("{other:?}")),
            },
        );
        check.report(
            "set_mode after blank: the read back equals the request",
            match &readback {
                Ok(m) => same_mode(valid.mode(), m),
                Err(e) => Err(e.to_string()),
            },
        );
        check.report(
            "set_mode after blank: a second descriptor reads the same mode",
            match probe.mode_of(crtc) {
                Ok(Some(m)) => same_mode(valid.mode(), &m),
                other => Err(format!("{other:?}")),
            },
        );
        check.report(
            "set_mode after blank: a flip completes",
            display
                .present(scene)
                .and_then(|()| display.wait_flip())
                .map(|_| String::new())
                .map_err(|e| e.to_string()),
        );
    }

    /// A display with buffers smaller than 480 lines refuses the 480p mode
    /// and changes nothing.
    fn too_large(check: &mut Check, probe: &Probe, modes: &[TestMode], crtc_id: u32) {
        let Some(tall) = modes
            .iter()
            .find(|m| m.name == "480p")
            .and_then(TestMode::validated)
        else {
            return;
        };
        let fd: OwnedFd = match OpenOptions::new().read(true).write(true).open(DEVICE) {
            Ok(f) => f.into(),
            Err(e) => {
                check.report("a mode larger than the buffers", Err(e.to_string()));
                return;
            }
        };
        let config = KmsConfig {
            caps: CAPS,
            output: None,
            crt: true,
            buffer_width: BUFFER_WIDTH,
            buffer_height: 240,
            writeback: false,
        };
        let before = probe.mode_of(crtc_id);
        let result = KmsDisplay::open(fd, config)
            .map_err(|e| e.to_string())
            .and_then(|mut d| match d.set_mode(&tall) {
                Err(DisplayError::ModeRefused { .. }) => Ok(()),
                other => Err(format!("{other:?}")),
            });
        check.report(
            "a mode larger than the buffers is refused and changes nothing",
            result.and_then(|()| {
                let after = probe.mode_of(crtc_id);
                if format!("{before:?}") == format!("{after:?}") {
                    Ok(String::new())
                } else {
                    Err(format!("{before:?} before, {after:?} after"))
                }
            }),
        );
    }

    fn is_device<T: std::fmt::Debug>(r: Result<T, DisplayError>) -> Result<String, String> {
        match r {
            Err(DisplayError::Device { reason, .. }) => Ok(reason),
            other => Err(format!("{other:?}")),
        }
    }

    fn expect_eq<T: PartialEq + std::fmt::Debug>(actual: T, expected: T) -> Result<String, String> {
        if actual == expected {
            Ok(String::new())
        } else {
            Err(format!("{actual:?}, expected {expected:?}"))
        }
    }

    fn same_mode(requested: &Modeline, read: &Modeline) -> Result<String, String> {
        let pairs: [(&str, u64, u64); 13] = [
            (
                "pclock_hz",
                requested.pclock_hz.into(),
                read.pclock_hz.into(),
            ),
            ("hactive", requested.hactive.into(), read.hactive.into()),
            (
                "hsync_start",
                requested.hsync_start.into(),
                read.hsync_start.into(),
            ),
            (
                "hsync_end",
                requested.hsync_end.into(),
                read.hsync_end.into(),
            ),
            ("htotal", requested.htotal.into(), read.htotal.into()),
            ("vactive", requested.vactive.into(), read.vactive.into()),
            (
                "vsync_start",
                requested.vsync_start.into(),
                read.vsync_start.into(),
            ),
            (
                "vsync_end",
                requested.vsync_end.into(),
                read.vsync_end.into(),
            ),
            ("vtotal", requested.vtotal.into(), read.vtotal.into()),
            (
                "interlace",
                requested.interlace.into(),
                read.interlace.into(),
            ),
            (
                "doublescan",
                requested.doublescan.into(),
                read.doublescan.into(),
            ),
            (
                "hsync_positive",
                requested.hsync_positive.into(),
                read.hsync_positive.into(),
            ),
            (
                "vsync_positive",
                requested.vsync_positive.into(),
                read.vsync_positive.into(),
            ),
        ];
        let diff: Vec<String> = pairs
            .iter()
            .filter(|(_, a, b)| a != b)
            .map(|(n, a, b)| format!("{n}: {a} -> {b}"))
            .collect();
        if diff.is_empty() {
            Ok(String::new())
        } else {
            Err(diff.join(", "))
        }
    }

    fn writeback_equals_reference(
        display: &mut KmsDisplay,
        reference: &mut ReferenceRenderer,
        scene: &Scene<'_>,
        mode: &Modeline,
    ) -> Result<String, String> {
        display.present(scene).map_err(|e| e.to_string())?;
        display.wait_flip().map_err(|e| e.to_string())?;
        let capture = display.capture().map_err(|e| e.to_string())?;
        let (w, h) = (u32::from(mode.hactive), u32::from(mode.vactive));
        let mut expected = vec![0_u8; w as usize * h as usize * 4];
        let mut out = OutputBuffer {
            data: &mut expected,
            width: w,
            height: h,
            pitch: w as usize * 4,
        };
        reference
            .render(scene, &mut out)
            .map_err(|e| e.to_string())?;
        if (capture.width, capture.height) != (w, h) {
            return Err(format!("capture {}x{}", capture.width, capture.height));
        }
        compare(&capture.pixels, &expected, w)
    }

    /// The KMS path (window surface, scanout, writeback) against the GLES
    /// read-back path of the same scene, byte for byte in the colour bytes.
    fn window_equals_read_back(
        display: &mut KmsDisplay,
        headless: &mut GlesRenderer,
        scene: &Scene<'_>,
        mode: &Modeline,
    ) -> Result<String, String> {
        display.present(scene).map_err(|e| e.to_string())?;
        display.wait_flip().map_err(|e| e.to_string())?;
        let capture = display.capture().map_err(|e| e.to_string())?;
        let (w, h) = (u32::from(mode.hactive), u32::from(mode.vactive));
        let mut read_back = vec![0_u8; w as usize * h as usize * 4];
        let mut out = OutputBuffer {
            data: &mut read_back,
            width: w,
            height: h,
            pitch: w as usize * 4,
        };
        headless
            .render(scene, &mut out)
            .map_err(|e| e.to_string())?;
        compare(&capture.pixels, &read_back, w)
    }

    /// Compares two XRGB8888 pictures of width `w` in the colour bytes.
    fn compare(got: &[u8], want: &[u8], w: u32) -> Result<String, String> {
        if got.len() != want.len() {
            return Err(format!("{} bytes against {}", got.len(), want.len()));
        }
        let mut differ = 0_usize;
        let mut first = None;
        for (i, (a, b)) in got.chunks_exact(4).zip(want.chunks_exact(4)).enumerate() {
            if a[..3] != b[..3] {
                differ += 1;
                first.get_or_insert((
                    i % w as usize,
                    i / w as usize,
                    [a[0], a[1], a[2]],
                    [b[0], b[1], b[2]],
                ));
            }
        }
        let total = got.len() / 4;
        match first {
            None => Ok(format!("{total} pixels equal")),
            Some((x, y, got, want)) => Err(format!(
                "{differ} of {total} pixels differ; first at ({x}, {y}): BGR {got:?}, \
                 expected {want:?}"
            )),
        }
    }

    /// Presents `TIMED_FLIPS` frames and checks the events.
    fn flips(
        display: &mut KmsDisplay,
        scene: &Scene<'_>,
        mode: &Modeline,
    ) -> Result<String, String> {
        let mut events: Vec<FlipEvent> = Vec::with_capacity(TIMED_FLIPS);
        for _ in 0..TIMED_FLIPS {
            display.present(scene).map_err(|e| e.to_string())?;
            events.push(display.wait_flip().map_err(|e| e.to_string())?);
        }
        let mut missed = 0;
        for pair in events.windows(2) {
            if pair[1].sequence <= pair[0].sequence || pair[1].time_ns <= pair[0].time_ns {
                return Err(format!(
                    "events out of order: {:?} then {:?}",
                    pair[0], pair[1]
                ));
            }
            missed += pair[1].sequence - pair[0].sequence - 1;
        }
        let (first, last) = (events[0], events[events.len() - 1]);
        let vblanks = last.sequence - first.sequence;
        let measured_ns = (last.time_ns - first.time_ns) / vblanks;
        let pixels = u64::from(mode.htotal) * u64::from(mode.vtotal);
        // An interlaced frame has two fields; vkms runs its timer at twice
        // the field rate there (OP-030), so only progressive modes are
        // timed.
        if mode.interlace {
            return Ok(format!(
                "{vblanks} vblanks, {missed} missed, {measured_ns} ns per vblank; \
                 not timed: vkms gives interlaced vblanks at twice the field rate (OP-030)"
            ));
        }
        let expected_ns = pixels * 1_000_000_000 / u64::from(mode.pclock_hz);
        let distance_ppm = measured_ns.abs_diff(expected_ns) * 1_000_000 / expected_ns;
        let note = format!(
            "{vblanks} vblanks, {missed} missed, {measured_ns} ns per vblank against \
             {expected_ns} ns ({distance_ppm} ppm)"
        );
        if distance_ppm <= PERIOD_TOLERANCE_PPM {
            Ok(note)
        } else {
            Err(note)
        }
    }

    /// The test modes: each passes `validate` for its profile.
    struct TestMode {
        name: &'static str,
        mode: Modeline,
        profile: MonitorProfile,
    }

    impl TestMode {
        fn validated(&self) -> Option<ValidatedMode> {
            validate(&self.mode, &self.profile, &CAPS).ok()
        }
    }

    fn line(pclock_hz: u32, h: [u16; 4], v: [u16; 4], interlace: bool) -> Modeline {
        Modeline {
            pclock_hz,
            hactive: h[0],
            hsync_start: h[1],
            hsync_end: h[2],
            htotal: h[3],
            vactive: v[0],
            vsync_start: v[1],
            vsync_end: v[2],
            vtotal: v[3],
            interlace,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    /// `generic_15` and the 60 Hz range of `pc_31_120`, as in
    /// `data/monitors`.
    fn profile(id: &str) -> MonitorProfile {
        let range = match id {
            "generic_15" => CrtRange {
                h_min_mhz: 15_625_000,
                h_max_mhz: 15_750_000,
                v_min_mhz: 49_500,
                v_max_mhz: 65_000,
                h_front_porch_ns: 2_000,
                h_sync_ns: 4_700,
                h_back_porch_ns: 8_000,
                v_front_porch_ns: 64_000,
                v_sync_ns: 192_000,
                v_back_porch_ns: 1_024_000,
                hsync_positive: false,
                vsync_positive: false,
                progressive_lines_min: 192,
                progressive_lines_max: 288,
                interlaced_lines_min: 448,
                interlaced_lines_max: 576,
            },
            _ => CrtRange {
                h_min_mhz: 31_400_000,
                h_max_mhz: 31_600_000,
                v_min_mhz: 50_000,
                v_max_mhz: 65_000,
                h_front_porch_ns: 671,
                h_sync_ns: 2_683,
                h_back_porch_ns: 3_353,
                v_front_porch_ns: 34_000,
                v_sync_ns: 101_000,
                v_back_porch_ns: 436_000,
                hsync_positive: false,
                vsync_positive: false,
                progressive_lines_min: 400,
                progressive_lines_max: 512,
                interlaced_lines_min: 0,
                interlaced_lines_max: 0,
            },
        };
        MonitorProfile {
            id: id.to_owned(),
            kind: MonitorKind::Crt {
                ranges: vec![range],
            },
        }
    }

    fn test_modes() -> Vec<TestMode> {
        let v240 = [240, 241, 244, 262];
        vec![
            TestMode {
                name: "240p",
                mode: line(6_700_000, [320, 334, 366, 426], v240, false),
                profile: profile("generic_15"),
            },
            TestMode {
                name: "480p",
                mode: line(
                    25_175_000,
                    [640, 656, 724, 800],
                    [480, 482, 485, 525],
                    false,
                ),
                profile: profile("pc_31_120"),
            },
            TestMode {
                name: "480p, positive sync",
                mode: Modeline {
                    hsync_positive: true,
                    vsync_positive: true,
                    ..line(
                        25_175_000,
                        [640, 656, 724, 800],
                        [480, 482, 485, 525],
                        false,
                    )
                },
                profile: profile("pc_31_120"),
            },
            TestMode {
                name: "1920x240",
                mode: line(40_200_000, [1920, 2004, 2196, 2556], v240, false),
                profile: profile("generic_15"),
            },
            TestMode {
                name: "480i",
                mode: line(13_500_000, [640, 670, 734, 858], [480, 484, 490, 525], true),
                profile: profile("generic_15"),
            },
        ]
    }

    /// A 320x240 game frame and a 320x240 UI canvas, both XRGB8888.
    struct Pictures {
        game: Vec<u8>,
        ui: Vec<u8>,
    }

    const SOURCE_W: u16 = 320;
    const SOURCE_H: u16 = 240;

    impl Pictures {
        fn new(seed: u8) -> Pictures {
            let mut game = Vec::with_capacity(usize::from(SOURCE_W) * usize::from(SOURCE_H) * 4);
            let mut ui = Vec::with_capacity(game.capacity());
            for y in 0..SOURCE_H {
                for x in 0..SOURCE_W {
                    let (x8, y8) = (x as u8, y as u8);
                    game.extend_from_slice(&[
                        x8 ^ seed.wrapping_mul(37),
                        y8,
                        x8.wrapping_add(y8).wrapping_mul(seed),
                        0,
                    ]);
                    let border = x < 2 || y < 2 || x >= SOURCE_W - 2 || y >= SOURCE_H - 2;
                    let v = if border {
                        0xFF
                    } else {
                        (x8 / 8).wrapping_mul(seed)
                    };
                    ui.extend_from_slice(&[v, 0x40, 0xC0 ^ seed, 0]);
                }
            }
            Pictures { game, ui }
        }

        /// The game fills the mode with nearest scaling; the UI covers the
        /// top-left quarter.
        fn scene(&self, mode: &Modeline) -> Scene<'_> {
            let frame = source_frame;
            let (w, h) = (u32::from(mode.hactive), u32::from(mode.vactive));
            Scene {
                game: Some(frame(&self.game)),
                crop: Rect {
                    x: 0,
                    y: 0,
                    w: u32::from(SOURCE_W),
                    h: u32::from(SOURCE_H),
                },
                dest: Rect { x: 0, y: 0, w, h },
                filter: Filter::Nearest,
                ui: Some(frame(&self.ui)),
                ui_dest: Rect {
                    x: 0,
                    y: 0,
                    w: w / 2,
                    h: h / 2,
                },
                scanlines: Scanlines::NONE,
            }
        }

        /// The game alone, scaled by whole multiples and with black
        /// scanlines at half strength, so both paths draw dark lines.
        fn scanline_scene(&self, mode: &Modeline) -> Scene<'_> {
            let (w, h) = (u32::from(mode.hactive), u32::from(mode.vactive));
            let multiple = h / u32::from(SOURCE_H);
            Scene {
                dest: Rect {
                    x: 0,
                    y: 0,
                    w,
                    h: multiple * u32::from(SOURCE_H),
                },
                ui: None,
                scanlines: Scanlines {
                    kind: ScanlineKind::Black,
                    strength_percent: SCANLINE_STRENGTH_PERCENT,
                },
                ..self.scene(mode)
            }
        }
    }

    fn source_frame(data: &[u8]) -> Frame<'_> {
        Frame {
            data,
            width: SOURCE_W,
            height: SOURCE_H,
            pitch: usize::from(SOURCE_W) * 4,
            format: PixelFormat::Xrgb8888,
        }
    }

    /// The frame CRCs of CRTC 0 in debugfs.
    struct Crc {
        dir: PathBuf,
    }

    impl Crc {
        fn find() -> Option<Crc> {
            let entries = std::fs::read_dir("/sys/kernel/debug/dri").ok()?;
            for entry in entries.flatten() {
                let name = std::fs::read_to_string(entry.path().join("name")).unwrap_or_default();
                if name.starts_with("vkms") {
                    return Some(Crc {
                        dir: entry.path().join("crtc-0").join("crc"),
                    });
                }
            }
            None
        }

        /// Shows `scene` for a few flips, then reads the CRCs of the last
        /// frames. They must agree.
        fn stable(
            &mut self,
            display: &mut KmsDisplay,
            scene: &Scene<'_>,
        ) -> Result<String, String> {
            File::options()
                .write(true)
                .open(self.dir.join("control"))
                .and_then(|mut f| f.write_all(b"auto"))
                .map_err(|e| format!("crc control: {e}"))?;
            let mut data = OpenOptions::new()
                .read(true)
                .custom_flags(O_NONBLOCK)
                .open(self.dir.join("data"))
                .map_err(|e| format!("crc data: {e}"))?;
            for _ in 0..CRC_SETTLE_FLIPS {
                display.present(scene).map_err(|e| e.to_string())?;
                display.wait_flip().map_err(|e| e.to_string())?;
            }
            let mut text = String::new();
            let mut chunk = [0_u8; 256];
            loop {
                match data.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => text.push_str(&String::from_utf8_lossy(&chunk[..n])),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(format!("crc read: {e}")),
                }
            }
            let values: Vec<&str> = text
                .lines()
                .filter_map(|l| l.split_once(' ').map(|(_, crc)| crc.trim()))
                .collect();
            if values.len() < CRC_SAMPLES {
                return Err(format!("{} CRC entries: {text:?}", values.len()));
            }
            let last = &values[values.len() - CRC_SAMPLES..];
            if last.iter().all(|v| *v == last[0]) {
                Ok(last[0].to_owned())
            } else {
                Err(format!("the last CRCs differ: {last:?}"))
            }
        }
    }

    /// `O_NONBLOCK` on Linux for aarch64 and x86_64.
    const O_NONBLOCK: i32 = 0o4000;

    /// A second descriptor on the device, for reads only.
    struct Probe(File);

    impl AsFd for Probe {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl drm::Device for Probe {}
    impl ControlDevice for Probe {}

    impl Probe {
        fn open() -> std::io::Result<Probe> {
            let p = Probe(OpenOptions::new().read(true).write(true).open(DEVICE)?);
            drm::Device::set_client_capability(&p, drm::ClientCapability::UniversalPlanes, true)?;
            // The atomic properties, such as MODE_ID, show only with it.
            drm::Device::set_client_capability(&p, drm::ClientCapability::Atomic, true)?;
            Ok(p)
        }

        /// The value of the CRTC property `name`, such as `MODE_ID` (a blob
        /// id) or `ACTIVE`.
        fn crtc_prop(&self, crtc_id: u32, name: &[u8]) -> Result<u64, String> {
            let handle: crtc::Handle = drm::control::from_u32(crtc_id).ok_or("CRTC id 0")?;
            let set = self.get_properties(handle).map_err(|e| e.to_string())?;
            let (ids, values) = set.as_props_and_values();
            for (&id, &value) in ids.iter().zip(values) {
                let info = self.get_property(id).map_err(|e| e.to_string())?;
                if info.name().to_bytes() == name {
                    return Ok(value);
                }
            }
            Err(format!(
                "the CRTC has no {} property",
                String::from_utf8_lossy(name)
            ))
        }

        fn crtc_modes(&self) -> Vec<Option<Modeline>> {
            let Ok(res) = self.resource_handles() else {
                return vec![None];
            };
            res.crtcs()
                .iter()
                .map(|&c| self.mode_of(c.into()).ok().flatten())
                .collect()
        }

        fn mode_of(&self, crtc_id: u32) -> Result<Option<Modeline>, String> {
            let handle: crtc::Handle = drm::control::from_u32(crtc_id).ok_or("CRTC id 0")?;
            let info = self.get_crtc(handle).map_err(|e| e.to_string())?;
            info.mode()
                .map(|m| record_of(m).to_modeline().map_err(|e| e.to_string()))
                .transpose()
        }

        /// The framebuffer on the primary plane of the CRTC, if any.
        fn plane_fb(&self, crtc_id: u32) -> Result<Option<u32>, String> {
            for p in self.plane_handles().map_err(|e| e.to_string())? {
                let info = self.get_plane(p).map_err(|e| e.to_string())?;
                if info.crtc().map(u32::from) == Some(crtc_id) {
                    return Ok(info.framebuffer().map(u32::from));
                }
            }
            Ok(None)
        }
    }

    fn record_of(mode: drm::control::Mode) -> ModeRecord {
        let raw: drm_ffi::drm_mode_modeinfo = mode.into();
        let mut name = [0_u8; 32];
        for (dst, &src) in name.iter_mut().zip(raw.name.iter()) {
            *dst = src as u8;
        }
        ModeRecord {
            clock_khz: raw.clock,
            h: [raw.hdisplay, raw.hsync_start, raw.hsync_end, raw.htotal],
            v: [raw.vdisplay, raw.vsync_start, raw.vsync_end, raw.vtotal],
            hskew: raw.hskew,
            vscan: raw.vscan,
            vrefresh_hz: raw.vrefresh,
            flags: raw.flags,
            kind: raw.type_,
            name,
        }
    }
}
