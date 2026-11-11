//! The host with the real test core: the built `neptron-testcore` dynamic
//! library, driven by a script, with the fakes of `neptron-testkit` for
//! input, audio and time.
//!
//! The host allows one core per process, and the test core reads its
//! script path from an environment variable, so each test needs its own
//! process. `cargo nextest` runs every test in its own process.

#![deny(unsafe_code)]

use std::path::{Path, PathBuf};

use neptron_backend::{AudioSink, InputSource};
use neptron_libretro::{
    AvInfo, ContentRequest, Event, GameGeometry, Host, HostConfig, LoadError, LogLevel, LogSource,
    OpenError, Pad, Timing, Video, ZipError, read_save_file, save_file_path, write_save_file,
};
use neptron_testcore::{
    PixelFormat as CoreFormat, SCRIPT_ENV, Size, expected_hash, frames_after, hash, sample,
};
use neptron_testkit::{FakeAudio, ScriptedInput, SimClock};
use neptron_types::{Control, PixelFormat};

/// The built test core, next to the test binary, as the test core's own
/// load test finds it.
fn core_path() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let deps = exe.parent().expect("deps directory");
    let name = libloading_name();
    [
        deps.join(&name),
        deps.parent().expect("profile directory").join(&name),
    ]
    .into_iter()
    .find(|p| p.exists())
    .unwrap_or_else(|| panic!("no {name} next to {}", exe.display()))
}

fn libloading_name() -> String {
    if cfg!(target_os = "macos") {
        "libneptron_testcore.dylib".to_owned()
    } else {
        "libneptron_testcore.so".to_owned()
    }
}

/// A fresh folder for this test process.
fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("neptron-libretro-it-{}-{name}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Writes the script and points the test core at it.
#[allow(unsafe_code)]
fn use_script(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("game.ntc");
    std::fs::write(&path, text).expect("write the script");
    // SAFETY: the test has started no thread that reads the environment.
    unsafe { std::env::set_var(SCRIPT_ENV, &path) };
    path
}

fn config(dir: &Path) -> HostConfig {
    HostConfig {
        system_dir: dir.join("bios"),
        save_dir: dir.join("saves"),
    }
}

fn open(dir: &Path) -> Host {
    Host::open(&core_path(), config(dir)).expect("open the test core")
}

fn load(host: &mut Host, content: &Path) -> AvInfo {
    let exts = ["ntc".to_owned()];
    host.load_game(Some(ContentRequest {
        path: content,
        extensions: &exts,
    }))
    .expect("load the script")
}

fn core_format(format: PixelFormat) -> CoreFormat {
    match format {
        PixelFormat::Xrgb8888 => CoreFormat::Xrgb8888,
        PixelFormat::Rgb565 => CoreFormat::Rgb565,
        PixelFormat::Xrgb1555 => CoreFormat::Rgb1555,
    }
}

const FRAME_NS: u64 = 16_666_667;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60000 48000
save_ram 16

at 2 dupe
at 3 geometry 320 240 1333
at 5 size 300 200
at 6 av_info 320 240 0 512 480 50000 44100
at 7 size 512 480
at 8 unknown_env
at 9 unknown_env
at 10 get_variable neptron_testcore_mode 4
at 10 get_variable neptron_testcore_level 5
at 11 shutdown
";

/// Stereo frames that the test core sends in frame `n` of `SCRIPT`. The
/// AV change at frame 6 starts the count again at the new rate.
fn audio_frames(n: u64) -> u64 {
    let ntsc = neptron_testcore::Timing {
        fps_mhz: 60_000,
        sample_rate_hz: 48_000,
    };
    let pal = neptron_testcore::Timing {
        fps_mhz: 50_000,
        sample_rate_hz: 44_100,
    };
    if n < 6 {
        frames_after(n + 1, ntsc) - frames_after(n, ntsc)
    } else {
        frames_after(n - 5, pal) - frames_after(n - 6, pal)
    }
}

/// The gate: a scripted run of the test core through the host. Every frame
/// hash equals the hash that the test core's generator gives for that
/// frame and the buttons that the scripted input pressed.
#[test]
fn a_scripted_run_matches_the_test_core() {
    let dir = temp_dir("run");
    let script = use_script(&dir, SCRIPT);
    let mut host = open(&dir);
    assert_eq!(host.system_info().name, "neptron-testcore");
    assert_eq!(host.system_info().extensions, ["ntc"]);
    assert!(host.system_info().need_fullpath);
    host.set_option("neptron_testcore_mode", "gamma").unwrap();
    host.set_option("neptron_testcore_level", "2").unwrap();

    let av = load(&mut host, &script);
    assert_eq!(
        av,
        AvInfo {
            geometry: GameGeometry {
                base_width: 256,
                base_height: 224,
                max_width: 320,
                max_height: 240,
                aspect_milli: 1333,
            },
            timing: Timing {
                fps_mhz: 60_000,
                sample_rate_mhz: 48_000_000,
            },
        }
    );
    assert_eq!(host.pixel_format(), PixelFormat::Rgb565);

    let clock = SimClock::new();
    let mut input = ScriptedInput::new(clock.clone(), Vec::new());
    input.press(FRAME_NS, 0, Control::Start);
    input.press(4 * FRAME_NS, 0, Control::A);
    input.set(4 * FRAME_NS, 0, Control::LeftX, -300);
    input.release(7 * FRAME_NS, 0, Control::Start);
    input.press(9 * FRAME_NS, 0, Control::R3);
    let mut audio = FakeAudio::new(clock.clone(), 48_000, 1 << 20);

    let mut pad = Pad::default();
    let mut events = Vec::new();
    let mut next_sample = 0u64;
    let mut last_hash = None;
    let mut unknown_logs = Vec::new();
    for n in 0..12u64 {
        let mut polled = Vec::new();
        input.poll(&mut polled);
        for e in &polled {
            assert_eq!(e.device, 0);
            pad.apply(e.control, e.value);
        }
        host.set_pad(0, pad).unwrap();
        let out = host.run_frame().unwrap();

        let frame = match out.video {
            Video::Frame(f) => {
                assert_ne!(n, 2, "frame 2 is a duplicate");
                f
            }
            Video::Duplicate(Some(f)) => {
                assert_eq!(n, 2, "only frame 2 is a duplicate");
                f
            }
            other => panic!("frame {n}: {other:?}"),
        };
        let size = Size {
            width: u32::from(frame.width),
            height: u32::from(frame.height),
        };
        let format = core_format(frame.format);
        let got = hash(frame.data, frame.pitch, size, format).unwrap();
        if n == 2 {
            assert_eq!(Some(got), last_hash, "the duplicate is the last frame");
        } else {
            let want = expected_hash(size, format, n, pad.buttons).unwrap();
            assert_eq!(got, want, "frame {n} hash");
        }
        last_hash = Some(got);
        let want_size = match n {
            0..=2 => (256, 224),
            3 | 4 | 6 => (320, 240),
            5 => (300, 200),
            _ => (512, 480),
        };
        assert_eq!((frame.width, frame.height), want_size, "frame {n} size");

        assert_eq!(
            out.audio.len() as u64,
            2 * audio_frames(n),
            "frame {n} audio"
        );
        for pair in out.audio.chunks_exact(2) {
            assert_eq!(pair, sample(next_sample), "sample {next_sample}");
            next_sample += 1;
        }
        assert_eq!(out.dropped.audio_frames, 0);
        assert_eq!(audio.write(out.audio), out.audio.len() / 2);

        for e in out.events {
            events.push((n, e.clone()));
        }
        for log in out.logs {
            if log.text.contains("0x24e54") {
                unknown_logs.push((n, log.level, log.source));
            }
        }
        clock.advance(FRAME_NS);
    }

    assert_eq!(audio.written_frames(), next_sample);
    let pal = AvInfo {
        geometry: GameGeometry {
            base_width: 320,
            base_height: 240,
            max_width: 512,
            max_height: 480,
            aspect_milli: 1333,
        },
        timing: Timing {
            fps_mhz: 50_000,
            sample_rate_mhz: 44_100_000,
        },
    };
    assert_eq!(
        events,
        [
            (
                3,
                Event::Geometry(GameGeometry {
                    base_width: 320,
                    base_height: 240,
                    max_width: 320,
                    max_height: 240,
                    aspect_milli: 1333,
                })
            ),
            (6, Event::AvInfo(pal)),
            (11, Event::Shutdown),
        ]
    );
    assert_eq!(host.av_info(), Some(pal));
    assert_eq!(
        unknown_logs,
        [(8, LogLevel::Warn, LogSource::Host)],
        "the unknown call is logged once"
    );

    let mut ram = Vec::new();
    assert_eq!(host.copy_save_ram(&mut ram).unwrap(), 16);
    assert_eq!(ram[4], 3, "mode is gamma, the third value");
    assert_eq!(ram[5], 2, "level is 2, the second value");
    drop(host);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn core_options_are_declared_with_their_defaults() {
    let dir = temp_dir("options");
    use_script(&dir, "neptron-testcore 1\n");
    let mut host = open(&dir);
    let options: Vec<(String, Vec<String>, String)> = host
        .options()
        .iter()
        .map(|o| {
            (
                o.key().to_owned(),
                o.values().map(str::to_owned).collect(),
                o.value().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        options,
        [
            (
                "neptron_testcore_mode".to_owned(),
                vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()],
                "alpha".to_owned()
            ),
            (
                "neptron_testcore_level".to_owned(),
                vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
                "1".to_owned()
            ),
        ]
    );
    assert_eq!(host.options()[0].description(), "Test mode");
    assert!(host.set_option("neptron_testcore_mode", "delta").is_err());
    assert!(host.set_option("neptron_testcore_speed", "1").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_ram_round_trips_through_a_file() {
    let dir = temp_dir("save");
    let script = use_script(
        &dir,
        "neptron-testcore 1\nsave_ram 16\n\nat 0 sram_write 3 42\n",
    );
    let saves = dir.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    let srm = save_file_path(&saves, &script).unwrap();
    assert_eq!(srm, saves.join("game.srm"));
    let original: Vec<u8> = (1..=16).collect();
    write_save_file(&srm, &original).unwrap();

    let mut host = open(&dir);
    load(&mut host, &script);
    let bytes = read_save_file(&srm).unwrap().unwrap();
    let loaded = host.load_save_ram(&bytes).unwrap();
    assert_eq!(
        (loaded.loaded, loaded.file_len, loaded.ram_len),
        (16, 16, 16)
    );
    host.run_frame().unwrap();

    let mut ram = Vec::new();
    host.copy_save_ram(&mut ram).unwrap();
    let mut expected = original.clone();
    expected[3] = 42;
    assert_eq!(ram, expected);
    write_save_file(&srm, &ram).unwrap();
    assert_eq!(read_save_file(&srm).unwrap(), Some(expected));
    assert_eq!(
        std::fs::read(saves.join("game.srm.bak")).unwrap(),
        original,
        "one backup of the previous file"
    );

    let short = host.load_save_ram(&[9; 4]).unwrap();
    assert_eq!((short.loaded, short.file_len, short.ram_len), (4, 4, 16));
    host.copy_save_ram(&mut ram).unwrap();
    assert_eq!(ram[..5], [9, 9, 9, 9, 5], "the smaller length loads");
    drop(host);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_refused_load_gives_the_core_log() {
    let dir = temp_dir("refused");
    use_script(&dir, "neptron-testcore 1\nbogus_line 1\n");
    let mut host = open(&dir);
    assert!(matches!(host.load_game(None), Err(LoadError::Refused)));
    let errors: Vec<_> = host
        .logs()
        .iter()
        .filter(|l| l.source == LogSource::Core && l.level == LogLevel::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{:?}", host.logs());
    assert!(errors[0].text.contains("line 2"), "{}", errors[0].text);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_zip_needs_a_core_that_takes_data() {
    let dir = temp_dir("zip");
    use_script(&dir, "neptron-testcore 1\n");
    let zip = dir.join("game.zip");
    std::fs::write(&zip, minimal_zip("game.ntc", b"neptron-testcore 1\n")).unwrap();
    let mut host = open(&dir);
    let exts = ["ntc".to_owned()];
    let request = ContentRequest {
        path: &zip,
        extensions: &exts,
    };
    assert!(matches!(
        host.load_game(Some(request)),
        Err(LoadError::ZipNeedsFullPath { entry, .. }) if entry == "game.ntc"
    ));
    let exts = ["nes".to_owned()];
    assert!(matches!(
        host.load_game(Some(ContentRequest {
            path: &zip,
            extensions: &exts
        })),
        Err(LoadError::Zip {
            source: ZipError::NoMatchingEntry { .. },
            ..
        })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_second_host_in_the_process_is_refused() {
    let dir = temp_dir("second");
    use_script(&dir, "neptron-testcore 1\n");
    let first = open(&dir);
    assert!(matches!(
        Host::open(&core_path(), config(&dir)),
        Err(OpenError::ProcessAlreadyUsed)
    ));
    drop(first);
    assert!(matches!(
        Host::open(&core_path(), config(&dir)),
        Err(OpenError::ProcessAlreadyUsed)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_file_that_is_not_a_library_is_refused() {
    let dir = temp_dir("not-a-library");
    let fake = dir.join("core.so");
    std::fs::write(&fake, b"this is text, not a library").unwrap();
    assert!(matches!(
        Host::open(&fake, config(&dir)),
        Err(OpenError::Library { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_library_that_is_not_a_core_is_refused() {
    let dir = temp_dir("not-a-core");
    let libc = if cfg!(target_os = "macos") {
        Path::new("/usr/lib/libSystem.B.dylib")
    } else {
        Path::new("libc.so.6")
    };
    assert!(matches!(
        Host::open(libc, config(&dir)),
        Err(OpenError::MissingSymbol {
            name: "retro_set_environment",
            ..
        })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

/// A zip archive with one stored entry.
fn minimal_zip(name: &str, data: &[u8]) -> Vec<u8> {
    let crc = crc32(data);
    let mut out = Vec::new();
    out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(data);
    let directory = out.len() as u32;
    out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    out.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    let directory_len = out.len() as u32 - directory;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    out.extend_from_slice(&directory_len.to_le_bytes());
    out.extend_from_slice(&directory.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |mut crc, &b| {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
        crc
    })
}
