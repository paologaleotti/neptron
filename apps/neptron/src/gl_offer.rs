//! The context types that the display offers to a GL core, kept in the RAM
//! folder for the boot. The display finds them by making a context of each
//! API, which costs a GL setup at each launch; every launch is a new
//! process, so the first launch of a boot probes and the later ones read
//! the file. A power cycle clears the RAM folder, and the GPU does not
//! change while the machine runs.

use std::io;
use std::path::Path;

use neptron_backend::{ContextOffer, Display, GlVersion};
use serde::{Deserialize, Serialize};

use crate::atomic;

/// The format version of the file.
pub const GL_OFFER_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OfferFile {
    version: u32,
    /// `major.minor`, or absent for an API the display cannot make.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gles: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gl_core: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gl_compatibility: Option<String>,
}

/// The offer of `display`: the one in the file at `path`, or else the
/// probe of the display, which then goes to the file. A file that does not
/// parse is probed again and replaced. The error of a write goes to `warn`,
/// and the offer still holds.
pub fn kept_offer<D: Display>(
    path: &Path,
    display: &mut D,
    warn: &mut dyn FnMut(String),
) -> ContextOffer {
    if let Some(offer) = read(path) {
        return offer;
    }
    let offer = display
        .core_render()
        .map_or(ContextOffer::NONE, |render| render.offer());
    if let Err(e) = write(path, &offer) {
        warn(format!(
            "cannot keep the GL offer in {}: {e}",
            path.display()
        ));
    }
    offer
}

/// The offer in the file at `path`, or `None` when it is missing or does
/// not parse.
fn read(path: &Path) -> Option<ContextOffer> {
    let text = std::fs::read_to_string(path).ok()?;
    let file: OfferFile = toml::from_str(&text).ok()?;
    if file.version != GL_OFFER_VERSION {
        return None;
    }
    Some(ContextOffer {
        gles: version(file.gles.as_deref())?,
        gl_core: version(file.gl_core.as_deref())?,
        gl_compatibility: version(file.gl_compatibility.as_deref())?,
    })
}

/// `Some(None)` for no value, `Some(Some(v))` for `major.minor`, and
/// `None` for text that is not a version.
fn version(text: Option<&str>) -> Option<Option<GlVersion>> {
    let Some(text) = text else {
        return Some(None);
    };
    let (major, minor) = text.split_once('.')?;
    Some(Some(GlVersion::new(
        major.parse().ok()?,
        minor.parse().ok()?,
    )))
}

fn write(path: &Path, offer: &ContextOffer) -> io::Result<()> {
    let text = |v: Option<GlVersion>| v.map(|v| format!("{}.{}", v.major, v.minor));
    let file = OfferFile {
        version: GL_OFFER_VERSION,
        gles: text(offer.gles),
        gl_core: text(offer.gl_core),
        gl_compatibility: text(offer.gl_compatibility),
    };
    // A struct of an integer and strings always serialises.
    let toml = toml::to_string(&file).unwrap_or_default();
    atomic::write(path, toml.as_bytes())
}

#[cfg(test)]
mod tests {
    use neptron_testkit::preset::CAPS_LOW_CLOCK;
    use neptron_testkit::{DisplayOptions, FakeDisplay, SimClock};

    use super::*;

    const PC: ContextOffer = ContextOffer {
        gles: Some(GlVersion::new(3, 2)),
        gl_core: Some(GlVersion::new(4, 6)),
        gl_compatibility: None,
    };

    fn display(offer: Option<ContextOffer>) -> FakeDisplay {
        FakeDisplay::with_options(
            SimClock::new(),
            CAPS_LOW_CLOCK,
            DisplayOptions {
                core_offer: offer,
                ..DisplayOptions::default()
            },
        )
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neptron-gl-offer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join(name);
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn the_first_launch_probes_and_the_next_one_reads_the_file() {
        let path = temp("probe.toml");
        let mut warnings = Vec::new();
        let mut warn = |w: String| warnings.push(w);
        assert_eq!(kept_offer(&path, &mut display(Some(PC)), &mut warn), PC);
        // A display that would offer nothing: the file of the boot wins,
        // so no probe ran.
        assert_eq!(kept_offer(&path, &mut display(None), &mut warn), PC);
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_display_with_no_core_context_keeps_an_empty_offer() {
        let path = temp("none.toml");
        let mut warn = |_: String| {};
        assert_eq!(
            kept_offer(&path, &mut display(None), &mut warn),
            ContextOffer::NONE
        );
        assert_eq!(read(&path), Some(ContextOffer::NONE));
    }

    #[test]
    fn a_file_that_does_not_parse_is_probed_again() {
        for text in [
            "version = 2\n",
            "version = 1\ngles = \"three\"\n",
            "not toml",
        ] {
            let path = temp("bad.toml");
            std::fs::write(&path, text).expect("write");
            let mut warn = |_: String| {};
            assert_eq!(
                kept_offer(&path, &mut display(Some(PC)), &mut warn),
                PC,
                "{text}"
            );
            assert_eq!(read(&path), Some(PC), "{text}: replaced");
        }
    }
}
