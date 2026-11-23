//! What an image says about the source that built it: the NEPTRON commit,
//! the commit date and the release tag. `os build` passes them to the
//! Buildroot container, and `board/common/install-licences.sh` writes them
//! to `/etc/neptron-release` and to the head of `LICENCES.txt`.
//!
//! The date is the commit date, not the time of the build, so two builds
//! of one commit give the same files.

use std::path::Path;

use crate::repo;

/// The facts of the source of one build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    /// The 40-hex commit, with [`MODIFIED_SUFFIX`] when the tree has
    /// changes that are not in it, or [`UNKNOWN`].
    pub commit: String,
    /// The commit date, `YYYY-MM-DDTHH:MM:SSZ` in UTC, or [`UNKNOWN`].
    pub date: String,
    /// The release tag (`v` and a version) that names exactly this commit,
    /// or [`NO_RELEASE`].
    pub release: String,
}

/// The value of a fact that the build cannot know, for example outside a
/// git checkout.
pub const UNKNOWN: &str = "unknown";
/// The release of a commit that no release tag names.
pub const NO_RELEASE: &str = "none";
/// Added to the commit of a tree with changes that are not committed.
pub const MODIFIED_SUFFIX: &str = "-modified";

impl SourceInfo {
    /// The facts of a build outside git.
    pub fn unknown() -> Self {
        Self {
            commit: UNKNOWN.to_owned(),
            date: UNKNOWN.to_owned(),
            release: NO_RELEASE.to_owned(),
        }
    }

    /// The facts from the outputs of git: `rev-parse HEAD`, `status
    /// --porcelain`, `show -s --format=%ct HEAD` and `describe --tags
    /// --exact-match` (`None` for a command that failed). A value that is
    /// not of the expected form gives the fallback of that fact.
    pub fn from_git(
        head: Option<&str>,
        status: Option<&str>,
        commit_time: Option<&str>,
        tag: Option<&str>,
    ) -> Self {
        let mut info = Self::unknown();
        if let Some(head) = head.map(str::trim).filter(|h| is_commit(h)) {
            info.commit = head.to_owned();
            // An unknown state is taken as modified: the claim stays true.
            if status.is_none_or(|s| !s.trim().is_empty()) {
                info.commit.push_str(MODIFIED_SUFFIX);
            }
            if let Some(seconds) = commit_time.and_then(|t| t.trim().parse::<i64>().ok()) {
                info.date = utc_date(seconds);
            }
            if let Some(tag) = tag.map(str::trim).filter(|t| is_release_tag(t)) {
                info.release = tag.to_owned();
            }
        }
        info
    }

    /// Reads the facts of the checkout at `root` with git.
    pub fn of_checkout(root: &Path) -> Self {
        let run = |args: &[&str]| repo::output(repo::git(root).args(args)).ok();
        Self::from_git(
            run(&["rev-parse", "HEAD"]).as_deref(),
            run(&["status", "--porcelain", "--untracked-files=normal"]).as_deref(),
            run(&["show", "-s", "--format=%ct", "HEAD"]).as_deref(),
            run(&[
                "describe",
                "--tags",
                "--exact-match",
                "--match",
                "v[0-9]*",
                "HEAD",
            ])
            .as_deref(),
        )
    }

    /// The `docker run` arguments that pass the facts to the build.
    pub fn env_args(&self) -> Vec<String> {
        [
            ("NEPTRON_SOURCE_COMMIT", &self.commit),
            ("NEPTRON_SOURCE_DATE", &self.date),
            ("NEPTRON_SOURCE_RELEASE", &self.release),
        ]
        .iter()
        .flat_map(|(key, value)| ["--env".to_owned(), format!("{key}={value}")])
        .collect()
    }
}

/// Whether `text` is a full commit hash: 40 lower-case hex digits.
pub fn is_commit(text: &str) -> bool {
    text.len() == 40
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `text` is a release tag: `v`, then `MAJOR.MINOR.PATCH`, and an
/// optional pre-release part of letters, digits, dots and hyphens after a
/// hyphen (`v0.1.0`, `v0.2.0-rc.1`).
pub fn is_release_tag(text: &str) -> bool {
    let Some(version) = text.strip_prefix('v') else {
        return false;
    };
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let numbers = parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let pre_ok = pre.is_none_or(|p| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    });
    numbers && pre_ok
}

/// The UTC date and time of `seconds` after the Unix epoch, as
/// `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_date(seconds: i64) -> String {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let rest = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

const SECONDS_PER_DAY: i64 = 86_400;

/// The proleptic Gregorian date of a day count from 1970-01-01 (the
/// algorithm `civil_from_days` of Howard Hinnant).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "c4243863aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";

    #[test]
    fn the_date_is_the_utc_time_of_the_commit() {
        assert_eq!(utc_date(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_date(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_date(1_795_437_296), "2026-11-23T12:34:56Z");
        assert_eq!(utc_date(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn a_clean_tagged_checkout_names_its_commit_date_and_release() {
        let info =
            SourceInfo::from_git(Some(HEAD), Some(""), Some("1795437296\n"), Some("v0.1.0\n"));
        assert_eq!(
            info,
            SourceInfo {
                commit: HEAD.trim().to_owned(),
                date: "2026-11-23T12:34:56Z".to_owned(),
                release: "v0.1.0".to_owned(),
            }
        );
        assert_eq!(
            info.env_args().join(" "),
            "--env NEPTRON_SOURCE_COMMIT=c4243863aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
             --env NEPTRON_SOURCE_DATE=2026-11-23T12:34:56Z \
             --env NEPTRON_SOURCE_RELEASE=v0.1.0"
        );
    }

    #[test]
    fn a_changed_or_unreadable_tree_is_marked_modified() {
        let changed = SourceInfo::from_git(Some(HEAD), Some(" M os/README.md\n"), Some("0"), None);
        assert_eq!(changed.commit, format!("{}-modified", HEAD.trim()));
        assert_eq!(changed.release, "none");
        let unread = SourceInfo::from_git(Some(HEAD), None, Some("0"), None);
        assert!(unread.commit.ends_with("-modified"));
    }

    #[test]
    fn a_build_outside_git_or_with_odd_output_gets_the_fixed_fallback() {
        assert_eq!(
            SourceInfo::from_git(None, None, None, None),
            SourceInfo::unknown()
        );
        assert_eq!(
            SourceInfo::from_git(Some("not a commit"), Some(""), Some("0"), Some("v1.0.0")),
            SourceInfo::unknown()
        );
        let odd = SourceInfo::from_git(Some(HEAD), Some(""), Some("yesterday"), Some("nightly"));
        assert_eq!(odd.date, "unknown");
        assert_eq!(odd.release, "none");
        assert_eq!(
            SourceInfo::unknown().env_args().join(" "),
            "--env NEPTRON_SOURCE_COMMIT=unknown --env NEPTRON_SOURCE_DATE=unknown \
             --env NEPTRON_SOURCE_RELEASE=none"
        );
    }

    #[test]
    fn a_release_tag_is_v_and_a_version() {
        for good in ["v0.1.0", "v10.20.30", "v0.2.0-rc.1", "v1.0.0-beta-2"] {
            assert!(is_release_tag(good), "{good}");
        }
        for bad in [
            "0.1.0",
            "v0.1",
            "v0.1.0.0",
            "v0.x.0",
            "v0.1.0-",
            "v0.1.0-rc 1",
            "v",
            "",
        ] {
            assert!(!is_release_tag(bad), "{bad}");
        }
    }
}
