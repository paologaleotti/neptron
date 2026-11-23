//! `cargo xtask release-names`: the file names of the assets of a GitHub
//! Release, for the release job of CI.
//!
//! Each image of a release goes out as a compressed image and a sources
//! archive with the same name stem, so a person who has one file can find
//! the other: `<artifact>-<version>.img.xz` and
//! `<artifact>-<version>-sources.tar`.

/// An error of `release-names`.
#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    /// The arguments are wrong.
    #[error("{0}\nusage: {USAGE}")]
    Usage(String),
    /// The tag is not `v` and a version.
    #[error("{0:?} is not a release tag (v0.1.0, v0.2.0-rc.1)")]
    BadTag(String),
    /// The artifact name has a character other than a-z, 0-9, `-` and `_`.
    #[error("{0:?} is not an artifact name (lowercase letters, digits, - and _)")]
    BadArtifact(String),
    /// The image file is not one raw `.img` file.
    #[error("{0:?} is not a raw image file name (NAME.img)")]
    BadImage(String),
}

/// The usage of the command.
pub const USAGE: &str = "cargo xtask release-names --tag vX.Y.Z --artifact NAME --image FILE.img";

/// The asset names of one image of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNames {
    /// The version: the tag without its `v`.
    pub version: String,
    /// The compressed image.
    pub image: String,
    /// The sources archive of the image.
    pub sources: String,
}

/// The asset names for the image file `image` of the CI artifact
/// `artifact` in the release `tag`.
pub fn names(tag: &str, artifact: &str, image: &str) -> Result<ReleaseNames, ReleaseError> {
    if !is_release_tag(tag) {
        return Err(ReleaseError::BadTag(tag.to_owned()));
    }
    let plain = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    };
    if !plain(artifact) {
        return Err(ReleaseError::BadArtifact(artifact.to_owned()));
    }
    let stem = image.strip_suffix(".img").filter(|s| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    });
    if stem.is_none() {
        return Err(ReleaseError::BadImage(image.to_owned()));
    }
    let version = tag.trim_start_matches('v').to_owned();
    Ok(ReleaseNames {
        image: format!("{artifact}-{version}.img.xz"),
        sources: format!("{artifact}-{version}-sources.tar"),
        version,
    })
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

/// Runs `release-names` with `args` and prints `version=`, `image=` and
/// `sources=` lines, in the format of `$GITHUB_OUTPUT`.
pub fn run(args: &[&str]) -> Result<(), ReleaseError> {
    let (mut tag, mut artifact, mut image) = (None, None, None);
    let mut it = args.iter();
    while let Some(&flag) = it.next() {
        let value = it
            .next()
            .copied()
            .ok_or_else(|| ReleaseError::Usage(format!("{flag} needs a value")))?;
        match flag {
            "--tag" => tag = Some(value),
            "--artifact" => artifact = Some(value),
            "--image" => image = Some(value),
            other => return Err(ReleaseError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    let missing = |flag: &str| ReleaseError::Usage(format!("{flag} is missing"));
    let names = names(
        tag.ok_or_else(|| missing("--tag"))?,
        artifact.ok_or_else(|| missing("--artifact"))?,
        image.ok_or_else(|| missing("--image"))?,
    )?;
    print!("{}", output_lines(&names));
    Ok(())
}

/// The `key=value` lines of `names`.
pub fn output_lines(names: &ReleaseNames) -> String {
    format!(
        "version={}\nimage={}\nsources={}\n",
        names.version, names.image, names.sources
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_and_its_sources_share_the_artifact_and_version_stem() {
        let n = names("v0.1.0", "neptron-pc-x86_64", "neptron-x86_64.img").unwrap();
        assert_eq!(n.image, "neptron-pc-x86_64-0.1.0.img.xz");
        assert_eq!(n.sources, "neptron-pc-x86_64-0.1.0-sources.tar");
        let n = names("v0.2.0-rc.1", "neptron-rpi-arm64", "neptron-rpi-arm64.img").unwrap();
        assert_eq!(
            n,
            ReleaseNames {
                version: "0.2.0-rc.1".to_owned(),
                image: "neptron-rpi-arm64-0.2.0-rc.1.img.xz".to_owned(),
                sources: "neptron-rpi-arm64-0.2.0-rc.1-sources.tar".to_owned(),
            }
        );
        assert_eq!(
            output_lines(&n),
            "version=0.2.0-rc.1\nimage=neptron-rpi-arm64-0.2.0-rc.1.img.xz\n\
             sources=neptron-rpi-arm64-0.2.0-rc.1-sources.tar\n"
        );
    }

    #[test]
    fn a_tag_an_artifact_or_an_image_of_another_form_is_refused() {
        for tag in [
            "0.1.0",
            "v0.1",
            "v0.1.0.0",
            "v0.x.0",
            "v0.1.0-",
            "v0.1.0-rc 1",
            "v",
            "main",
        ] {
            assert!(
                matches!(names(tag, "a", "a.img"), Err(ReleaseError::BadTag(_))),
                "{tag}"
            );
        }
        for artifact in ["", "-a", "A", "a/b", "a b", "a.b"] {
            assert!(
                matches!(
                    names("v1.0.0", artifact, "a.img"),
                    Err(ReleaseError::BadArtifact(_))
                ),
                "{artifact}"
            );
        }
        for image in [
            "Image",
            "rootfs.ext4",
            ".img",
            "a b.img",
            "../a.img",
            "a.img.xz",
        ] {
            assert!(
                matches!(names("v1.0.0", "a", image), Err(ReleaseError::BadImage(_))),
                "{image}"
            );
        }
        for good in ["v0.1.0", "v10.20.30", "v0.2.0-rc.1", "v1.0.0-beta-2"] {
            assert!(is_release_tag(good), "{good}");
        }
    }

    #[test]
    fn the_command_needs_every_flag() {
        assert!(run(&["--tag", "v0.1.0", "--artifact", "a", "--image", "a.img"]).is_ok());
        for bad in [
            &["--tag", "v0.1.0", "--artifact", "a"][..],
            &["--tag"],
            &["--nope", "x"],
        ] {
            assert!(matches!(run(bad), Err(ReleaseError::Usage(_))), "{bad:?}");
        }
    }
}
