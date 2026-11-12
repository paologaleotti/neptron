//! The options of `selftest video`, parsed by hand like the rest of the
//! command line.

use std::path::PathBuf;

/// The monitor presets on the image.
pub const DEFAULT_MONITORS_DIR: &str = "/usr/share/neptron/monitors";
/// The sysfs root for the GPU and EDID detection.
pub const DEFAULT_SYSFS_DIR: &str = "/sys";
/// The report file when the command line names none.
pub const DEFAULT_REPORT_PATH: &str = "neptron-selftest-video.json";
/// The value of `--profile` that takes the profile from the monitor's EDID.
pub const EDID_PROFILE: &str = "edid";

/// The options of one `selftest video` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoArgs {
    /// Where the monitor profile comes from.
    pub profile: ProfileChoice,
    /// The folder of the monitor presets.
    pub monitors: PathBuf,
    /// The sysfs root.
    pub sysfs: PathBuf,
    /// The connector named with `--connector`.
    pub connector: Option<String>,
    /// The report file.
    pub report: PathBuf,
}

/// The monitor profile of the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileChoice {
    /// A preset of the monitor folder, by id.
    Preset(String),
    /// The profile built from the EDID of the connected monitor.
    Edid,
}

/// Parses the words after `selftest video`. The error text says what is
/// wrong; the caller adds the pointer to the usage.
pub fn parse(words: &[&str]) -> Result<VideoArgs, String> {
    let mut profile = None;
    let mut monitors = PathBuf::from(DEFAULT_MONITORS_DIR);
    let mut sysfs = PathBuf::from(DEFAULT_SYSFS_DIR);
    let mut connector = None;
    let mut report = PathBuf::from(DEFAULT_REPORT_PATH);
    let mut rest = words;
    while let Some((flag, tail)) = rest.split_first() {
        let Some((value, tail)) = tail.split_first() else {
            return Err(format!("{flag} needs a value"));
        };
        match *flag {
            "--profile" => profile = Some(profile_choice(value)?),
            "--monitors" => monitors = PathBuf::from(value),
            "--sysfs" => sysfs = PathBuf::from(value),
            "--connector" => connector = Some((*value).to_owned()),
            "--report" => report = PathBuf::from(value),
            other => return Err(format!("selftest video has no option {other:?}")),
        }
        rest = tail;
    }
    Ok(VideoArgs {
        profile: profile.ok_or("selftest video needs --profile ID or --profile edid")?,
        monitors,
        sysfs,
        connector,
        report,
    })
}

fn profile_choice(value: &str) -> Result<ProfileChoice, String> {
    if value == EDID_PROFILE {
        return Ok(ProfileChoice::Edid);
    }
    // The id names a file in the preset folder; no other character may
    // reach the path.
    let id_chars = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_';
    if value.is_empty() || !value.chars().all(id_chars) {
        return Err(format!(
            "the profile {value:?} is not a preset id (a-z, 0-9 and _) or {EDID_PROFILE:?}"
        ));
    }
    Ok(ProfileChoice::Preset(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    #[test]
    fn the_defaults_need_only_a_profile() {
        let args = parse(&words("--profile pc_31_120")).unwrap();
        assert_eq!(args.profile, ProfileChoice::Preset("pc_31_120".into()));
        assert_eq!(args.monitors, PathBuf::from(DEFAULT_MONITORS_DIR));
        assert_eq!(args.sysfs, PathBuf::from(DEFAULT_SYSFS_DIR));
        assert_eq!(args.connector, None);
        assert_eq!(args.report, PathBuf::from(DEFAULT_REPORT_PATH));
    }

    #[test]
    fn every_option_parses() {
        let args = parse(&words(
            "--profile edid --monitors /m --sysfs /s --connector DP-1 --report /r.json",
        ))
        .unwrap();
        assert_eq!(args.profile, ProfileChoice::Edid);
        assert_eq!(args.monitors, PathBuf::from("/m"));
        assert_eq!(args.sysfs, PathBuf::from("/s"));
        assert_eq!(args.connector.as_deref(), Some("DP-1"));
        assert_eq!(args.report, PathBuf::from("/r.json"));
    }

    #[test]
    fn bad_options_are_refused() {
        for line in [
            "",
            "--profile",
            "--profile ../etc/passwd",
            "--profile PC",
            "--profile pc_31_120 --warp 9",
            "--profile pc_31_120 --report",
        ] {
            assert!(parse(&words(line)).is_err(), "{line}");
        }
    }
}
