//! The pure parts of `cargo xtask core-trace`: the cases, the guest script,
//! the split of the guest output into traces, the comparison with the
//! committed traces, the ROM checksums, and the zip and image files that
//! the check writes.

use std::collections::BTreeMap;

/// Frames that each case runs: 15 s of a 60 Hz system. The input scripts
/// reach the audio sync test at about frame 450, and it beeps a few times
/// before the end.
pub const FRAMES: u32 = 900;
/// A frame hash and an audio hash every this many frames.
pub const HASH_EVERY: u32 = 30;
/// Where the image installs the cores.
pub const CORE_DIR: &str = "/usr/lib/libretro";
/// Where the guest finds the shared directory.
pub const GUEST_SHARE: &str = "/mnt/host";
/// The program that the guest runs, in the shared directory.
pub const PROGRAM: &str = "core_trace";
/// The trace line that names the architecture. The comparison skips it:
/// the traces must be equal on every architecture.
pub const ARCH_PREFIX: &str = "arch ";

/// One core, one content file and one input script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Case {
    /// The case name, also the trace file name.
    pub name: &'static str,
    /// The core library in [`CORE_DIR`].
    pub library: &'static str,
    /// The ROM, relative to `tests/roms`.
    pub rom: &'static str,
    /// The extensions of the system, as in `data/systems`.
    pub extensions: &'static str,
    /// Load the ROM from a zip file that holds it, not from the plain file.
    pub zip: bool,
    /// Button presses on port 0: `FRAME:BUTTON[+BUTTON...]:FRAMES`.
    pub presses: &'static [&'static str],
}

const NES_PRESSES: &[&str] = &[
    "120:b:6",
    "240:right:6",
    "260:down:4",
    "272:down:4",
    "284:down:4",
    "296:down:4",
    "308:down:4",
    "320:down:4",
    "332:down:4",
    "344:down:4",
    "356:down:4",
    "368:down:4",
    "400:a:6",
    "460:a:6",
];
const SNES_PRESSES: &[&str] = &[
    "240:down:6",
    "270:down:6",
    "300:a:6",
    "340:down:6",
    "380:a:6",
];
const MD_PRESSES: &[&str] = &[
    "240:down:6",
    "270:down:6",
    "300:y:6",
    "340:down:6",
    "380:y:6",
];

/// Every case. A zip case runs the same script as the plain case of its
/// core, so its frames and audio must be the same.
pub const CASES: [Case; 6] = [
    Case {
        name: "fceumm",
        library: "fceumm_libretro.so",
        rom: "nes/240pee.nes",
        extensions: "nes,zip",
        zip: false,
        presses: NES_PRESSES,
    },
    Case {
        name: "fceumm-zip",
        library: "fceumm_libretro.so",
        rom: "nes/240pee.nes",
        extensions: "nes,zip",
        zip: true,
        presses: NES_PRESSES,
    },
    Case {
        name: "snes9x",
        library: "snes9x_libretro.so",
        rom: "snes/240pSuite.sfc",
        extensions: "bs,fig,sfc,smc,swc,zip",
        zip: false,
        presses: SNES_PRESSES,
    },
    Case {
        name: "snes9x-zip",
        library: "snes9x_libretro.so",
        rom: "snes/240pSuite.sfc",
        extensions: "bs,fig,sfc,smc,swc,zip",
        zip: true,
        presses: SNES_PRESSES,
    },
    Case {
        name: "genesis_plus_gx",
        library: "genesis_plus_gx_libretro.so",
        rom: "megadrive/240pSuite-1.32.bin",
        extensions: "bin,gen,md,sg,smd,zip",
        zip: false,
        presses: MD_PRESSES,
    },
    Case {
        name: "genesis_plus_gx-zip",
        library: "genesis_plus_gx_libretro.so",
        rom: "megadrive/240pSuite-1.32.bin",
        extensions: "bin,gen,md,sg,smd,zip",
        zip: true,
        presses: MD_PRESSES,
    },
];

impl Case {
    /// The plain case of a zip case.
    pub fn plain(&self) -> Option<&'static Case> {
        if !self.zip {
            return None;
        }
        CASES.iter().find(|c| !c.zip && c.rom == self.rom)
    }

    /// The zip file of this case in the shared directory.
    pub fn zip_name(&self) -> String {
        format!("zips/{}.zip", self.name)
    }

    /// The content path in the guest.
    pub fn guest_content(&self) -> String {
        if self.zip {
            format!("{GUEST_SHARE}/{}", self.zip_name())
        } else {
            format!("{GUEST_SHARE}/roms/{}", self.rom)
        }
    }

    /// The file name of the ROM.
    pub fn rom_file(&self) -> &'static str {
        self.rom.rsplit('/').next().unwrap_or(self.rom)
    }
}

/// The shell script that the guest runs: one process for each case, since
/// a process hosts one core. Each case ends with its exit code in the trace.
pub fn guest_script(cases: &[Case], dumps: &[u32]) -> String {
    let mut s = String::from("#!/bin/sh\ncd /\nrm -rf /tmp/core-trace\n");
    for case in cases {
        let mut words = vec![
            format!("{GUEST_SHARE}/{PROGRAM}"),
            "--case".to_owned(),
            case.name.to_owned(),
            "--core".to_owned(),
            format!("{CORE_DIR}/{}", case.library),
            "--content".to_owned(),
            case.guest_content(),
            "--ext".to_owned(),
            case.extensions.to_owned(),
            "--frames".to_owned(),
            FRAMES.to_string(),
            "--hash-every".to_owned(),
            HASH_EVERY.to_string(),
            "--work".to_owned(),
            format!("/tmp/core-trace/{}", case.name),
        ];
        for press in case.presses {
            words.push("--press".to_owned());
            words.push((*press).to_owned());
        }
        if !case.zip {
            for frame in dumps {
                words.push("--dump".to_owned());
                words.push(frame.to_string());
            }
        }
        s.push_str(&words.join(" "));
        s.push_str(&format!("\necho \"TRACE {} exit $?\"\n", case.name));
    }
    s
}

/// The guest output, split by case.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// The trace lines of each case, without the `TRACE <case> ` prefix.
    pub traces: BTreeMap<String, Vec<String>>,
    /// The frame dumps of each case and frame.
    pub dumps: BTreeMap<(String, u32), Dump>,
    /// Lines that belong to no case: kernel messages and the like.
    pub other: Vec<String>,
}

/// A frame from the dump lines.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dump {
    /// Width in pixels.
    pub width: u32,
    /// Height in lines.
    pub height: u32,
    /// The pixel format, as the program prints it.
    pub format: String,
    /// The hex chunks, by index.
    pub chunks: BTreeMap<usize, String>,
    /// The number of chunks.
    pub count: usize,
}

impl Output {
    /// Sorts the console lines of the command.
    pub fn parse(lines: &[String]) -> Self {
        let mut out = Self::default();
        for line in lines {
            if let Some(rest) = line.strip_prefix("TRACE ")
                && let Some((case, text)) = rest.split_once(' ')
            {
                out.traces
                    .entry(case.to_owned())
                    .or_default()
                    .push(text.to_owned());
                continue;
            }
            if let Some(rest) = line.strip_prefix("FRAME ")
                && out.add_dump(rest).is_some()
            {
                continue;
            }
            out.other.push(line.clone());
        }
        out
    }

    fn add_dump(&mut self, rest: &str) -> Option<()> {
        let f: Vec<&str> = rest.split(' ').collect();
        let [case, frame, width, height, format, part, hex] = f[..] else {
            return None;
        };
        let (index, count) = part.split_once('/')?;
        let dump = self
            .dumps
            .entry((case.to_owned(), frame.parse().ok()?))
            .or_default();
        dump.width = width.parse().ok()?;
        dump.height = height.parse().ok()?;
        format.clone_into(&mut dump.format);
        dump.count = count.parse().ok()?;
        dump.chunks.insert(index.parse().ok()?, hex.to_owned());
        Some(())
    }
}

impl Dump {
    /// The frame as 8-bit RGB, or `None` when a chunk is missing or damaged.
    pub fn rgb(&self) -> Option<Vec<u8>> {
        if self.chunks.len() != self.count {
            return None;
        }
        let hex: String = self.chunks.values().map(String::as_str).collect();
        let bytes = decode_hex(&hex)?;
        let pixels = usize::try_from(self.width.checked_mul(self.height)?).ok()?;
        let mut rgb = Vec::with_capacity(pixels * 3);
        match self.format.as_str() {
            "Xrgb8888" => {
                for px in bytes.chunks_exact(4) {
                    rgb.extend_from_slice(&[px[2], px[1], px[0]]);
                }
            }
            "Rgb565" => {
                for px in bytes.chunks_exact(2) {
                    let v = u16::from_le_bytes([px[0], px[1]]);
                    rgb.extend_from_slice(&[
                        expand(v >> 11, 5),
                        expand((v >> 5) & 0x3f, 6),
                        expand(v & 0x1f, 5),
                    ]);
                }
            }
            "Xrgb1555" => {
                for px in bytes.chunks_exact(2) {
                    let v = u16::from_le_bytes([px[0], px[1]]);
                    rgb.extend_from_slice(&[
                        expand((v >> 10) & 0x1f, 5),
                        expand((v >> 5) & 0x1f, 5),
                        expand(v & 0x1f, 5),
                    ]);
                }
            }
            _ => return None,
        }
        (rgb.len() == pixels * 3).then_some(rgb)
    }
}

fn expand(value: u16, bits: u32) -> u8 {
    let max = (1u32 << bits) - 1;
    u8::try_from(u32::from(value) * 255 / max).unwrap_or(u8::MAX)
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The text of a trace file: the lines, each ended by a newline.
pub fn trace_text(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// How a trace differs from the committed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Line number in the committed trace, from 1.
    pub line: usize,
    /// The committed line, if any.
    pub expected: Option<String>,
    /// The line of this run, if any.
    pub actual: Option<String>,
}

/// The lines that differ, the `arch` line excluded. Empty means equal.
pub fn compare(expected: &str, actual: &[String]) -> Vec<Difference> {
    let keep = |l: &&str| !l.starts_with(ARCH_PREFIX);
    let e: Vec<&str> = expected.lines().filter(keep).collect();
    let a: Vec<&str> = actual.iter().map(String::as_str).filter(keep).collect();
    (0..e.len().max(a.len()))
        .filter_map(|i| {
            let (x, y) = (e.get(i).copied(), a.get(i).copied());
            (x != y).then(|| Difference {
                line: i + 1,
                expected: x.map(str::to_owned),
                actual: y.map(str::to_owned),
            })
        })
        .collect()
}

/// The lines of a trace that the content path cannot change: the frame
/// and audio hashes, and the video, event, save RAM and exit lines.
pub fn emulation_lines(trace: &str) -> Vec<&str> {
    trace
        .lines()
        .filter(|l| {
            [
                "hash ",
                "audio ",
                "video ",
                "event ",
                "save_ram size",
                "av ",
                "exit ",
            ]
            .iter()
            .any(|p| l.starts_with(p))
        })
        .collect()
}

/// Checks that a zip trace and its plain trace show the same emulation.
pub fn zip_matches_plain(plain: &str, zip: &str) -> Result<(), String> {
    let (p, z) = (emulation_lines(plain), emulation_lines(zip));
    if p.is_empty() {
        return Err("the plain trace has no frame hashes".to_owned());
    }
    match p.iter().zip(&z).position(|(a, b)| a != b) {
        Some(i) => Err(format!("plain {:?}, zip {:?}", p[i], z[i])),
        None if p.len() != z.len() => Err(format!(
            "plain has {} emulation lines, zip has {}",
            p.len(),
            z.len()
        )),
        None => Ok(()),
    }
}

/// The entries of a `SHA256SUMS` file: hash and relative path.
pub fn parse_sums(text: &str) -> Result<Vec<(String, String)>, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (hash, path) = line
                .split_once("  ")
                .ok_or_else(|| format!("{line:?} is not \"<sha256>  <path>\""))?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("{hash:?} is not a SHA-256"));
            }
            if path.starts_with('/') || path.split('/').any(|p| p == "..") {
                return Err(format!("{path:?} leaves the folder"));
            }
            Ok((hash.to_ascii_lowercase(), path.to_owned()))
        })
        .collect()
}

/// A zip file with one stored entry, as a ROM collection holds it.
pub fn stored_zip(name: &str, data: &[u8]) -> Option<Vec<u8>> {
    let name_len = u16::try_from(name.len()).ok()?;
    let size = u32::try_from(data.len()).ok()?;
    let crc = crc32(data);
    let mut out = Vec::with_capacity(data.len() + 2 * name.len() + 100);
    let common = |out: &mut Vec<u8>| {
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra length
    };
    out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    common(&mut out);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(data);
    let central = u32::try_from(out.len()).ok()?;
    out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes()); // version made by
    common(&mut out);
    out.extend_from_slice(&[0; 14]); // comment, disk, attributes, offset 0
    out.extend_from_slice(name.as_bytes());
    let central_len = u32::try_from(out.len()).ok()? - central;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]); // disks, entries
    out.extend_from_slice(&central_len.to_le_bytes());
    out.extend_from_slice(&central.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    Some(out)
}

/// CRC-32 (IEEE), the checksum of zip entries.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn repo_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
    }

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn every_case_has_a_plain_twin_and_a_unique_name() {
        let mut names: Vec<&str> = CASES.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CASES.len());
        for case in CASES.iter().filter(|c| c.zip) {
            let plain = case.plain().unwrap();
            assert_eq!(plain.library, case.library);
            assert_eq!(plain.presses, case.presses, "{}", case.name);
            assert_eq!(format!("{}-zip", plain.name), case.name);
        }
    }

    #[test]
    fn the_cases_match_the_core_and_system_files() {
        for case in CASES {
            let core = std::fs::read_to_string(
                repo_dir()
                    .join("data/cores")
                    .join(format!("{}.toml", case.name.trim_end_matches("-zip"))),
            )
            .unwrap();
            assert!(
                core.contains(&format!("library = \"{}\"", case.library)),
                "{}",
                case.name
            );
            let system = case.rom.split('/').next().unwrap();
            let text =
                std::fs::read_to_string(repo_dir().join(format!("data/systems/{system}.toml")))
                    .unwrap();
            let listed: Vec<String> = case
                .extensions
                .split(',')
                .map(|e| format!("\"{e}\""))
                .collect();
            assert!(
                text.contains(&format!("extensions = [{}]", listed.join(", "))),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_guest_script_runs_each_case_in_its_own_process() {
        let case = Case {
            presses: &["10:a+b:2"],
            ..CASES[0]
        };
        let zip = Case {
            presses: &[],
            ..CASES[1]
        };
        let script = guest_script(&[case, zip], &[59]);
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(
            "/mnt/host/core_trace --case fceumm --core /usr/lib/libretro/fceumm_libretro.so \
             --content /mnt/host/roms/nes/240pee.nes --ext nes,zip --frames 900 \
             --hash-every 30 --work /tmp/core-trace/fceumm --press 10:a+b:2 --dump 59\n\
             echo \"TRACE fceumm exit $?\"\n"
        ));
        assert!(script.contains(
            "--content /mnt/host/zips/fceumm-zip.zip --ext nes,zip --frames 900 \
             --hash-every 30 --work /tmp/core-trace/fceumm-zip\n"
        ));
    }

    #[test]
    fn output_is_split_by_case_and_other_lines_are_kept() {
        let out = Output::parse(&lines(
            "TRACE a format 1\n[   1.0] random: crng init\nTRACE b format 1\nTRACE a end\n\
             FRAME a 5 2 1 Xrgb8888 0/2 00000000\nFRAME a 5 2 1 Xrgb8888 1/2 ffffff00\n\
             FRAME broken",
        ));
        assert_eq!(out.traces["a"], ["format 1", "end"]);
        assert_eq!(out.traces["b"], ["format 1"]);
        assert_eq!(out.other, ["[   1.0] random: crng init", "FRAME broken"]);
        let dump = &out.dumps[&("a".to_owned(), 5)];
        assert_eq!(dump.rgb().unwrap(), [0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn a_dump_with_a_lost_chunk_gives_no_image() {
        let out = Output::parse(&lines("FRAME a 5 2 1 Xrgb8888 0/2 00000000"));
        assert_eq!(out.dumps[&("a".to_owned(), 5)].rgb(), None);
    }

    #[test]
    fn sixteen_bit_pixels_expand_to_full_range() {
        let dump = Dump {
            width: 2,
            height: 1,
            format: "Rgb565".to_owned(),
            chunks: BTreeMap::from([(0, "ffff1f00".to_owned())]),
            count: 1,
        };
        assert_eq!(dump.rgb().unwrap(), [255, 255, 255, 0, 0, 255]);
        let dump = Dump {
            format: "Xrgb1555".to_owned(),
            chunks: BTreeMap::from([(0, "007c1f00".to_owned())]),
            ..dump
        };
        assert_eq!(dump.rgb().unwrap(), [255, 0, 0, 0, 0, 255]);
    }

    #[test]
    fn compare_skips_the_arch_line_and_reports_each_difference() {
        let expected = "format 1\narch aarch64\nhash 29 1\nend\n";
        assert!(compare(expected, &lines("format 1\narch x86_64\nhash 29 1\nend")).is_empty());
        let d = compare(expected, &lines("format 1\narch aarch64\nhash 29 2"));
        assert_eq!(
            d,
            [
                Difference {
                    line: 2,
                    expected: Some("hash 29 1".to_owned()),
                    actual: Some("hash 29 2".to_owned()),
                },
                Difference {
                    line: 3,
                    expected: Some("end".to_owned()),
                    actual: None,
                },
            ]
        );
    }

    #[test]
    fn a_zip_trace_must_show_the_same_emulation() {
        let plain = "content a.nes\nhash 29 x 1\naudio 0-29 2\nexit 0\n";
        let zip = "content a.zip\nhash 29 x 1\naudio 0-29 2\nexit 0\n";
        assert_eq!(zip_matches_plain(plain, zip), Ok(()));
        assert!(zip_matches_plain(plain, "hash 29 x 2\naudio 0-29 2\nexit 0\n").is_err());
        assert!(zip_matches_plain(plain, "hash 29 x 1\n").is_err());
        assert!(zip_matches_plain("content a.nes\n", zip).is_err());
    }

    #[test]
    fn sums_are_parsed_and_unsafe_paths_refused() {
        let h = "04f01d7372f66ea2befe325b9bd655a9fcc395a31fa46d9466286bb8f9d2e62e";
        assert_eq!(
            parse_sums(&format!("{h}  nes/a.nes\n\n")).unwrap(),
            [(h.to_owned(), "nes/a.nes".to_owned())]
        );
        for bad in [
            format!("{h} one-space"),
            "abc  a.nes".to_owned(),
            format!("{h}  ../a.nes"),
            format!("{h}  /a.nes"),
        ] {
            assert!(parse_sums(&bad).is_err(), "{bad}");
        }
    }

    /// The ROMs that the repository holds. The others have art of other
    /// owners and stay in the machine cache; see `tests/roms/README.md`.
    const COMMITTED_ROMS: [&str; 1] = ["nes/240pee.nes"];

    #[test]
    fn only_the_cleared_roms_are_committed_and_they_match_their_checksums() {
        let dir = repo_dir().join("tests/roms");
        let sums = parse_sums(&std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap()).unwrap();
        assert_eq!(sums.len(), 3);
        for case in CASES {
            assert!(sums.iter().any(|(_, p)| p == case.rom), "{}", case.rom);
        }
        for (hash, path) in sums {
            if COMMITTED_ROMS.contains(&path.as_str()) {
                let data = std::fs::read(dir.join(&path)).unwrap();
                assert_eq!(crate::sha256::hex(&data), hash, "{path}");
            } else {
                assert!(!dir.join(&path).exists(), "{path} must not be committed");
            }
        }
    }

    #[test]
    fn crc32_and_the_stored_zip_follow_the_format() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let zip = stored_zip("a.nes", b"NES").unwrap();
        assert_eq!(&zip[..4], b"PK\x03\x04");
        assert_eq!(&zip[30..35], b"a.nes");
        assert_eq!(&zip[35..38], b"NES");
        let end = zip.len() - 22;
        assert_eq!(&zip[end..end + 4], b"PK\x05\x06");
        let central = u32::from_le_bytes(zip[end + 16..end + 20].try_into().unwrap());
        assert_eq!(central, 38);
        assert_eq!(&zip[38..42], b"PK\x01\x02");
    }
}
