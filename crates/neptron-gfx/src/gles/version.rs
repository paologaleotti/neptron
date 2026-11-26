use super::GlesError;

/// Checks that `text`, the `GL_VERSION` string, names OpenGL ES 2 or later
/// in the form that glow parses (`OpenGL ES <major>.<minor> ...`). glow
/// panics on a string it cannot parse, so the renderer checks it first.
pub(super) fn check(text: &str) -> Result<(), GlesError> {
    match es_version(text) {
        Some((major, _)) if major >= 2 => Ok(()),
        _ => Err(GlesError::GlVersion {
            text: text.to_owned(),
        }),
    }
}

/// The API and version of the `GL_VERSION` string of a context, in the
/// form that glow parses: `(true, major, minor)` for OpenGL ES 2 or later,
/// `(false, major, minor)` for desktop OpenGL 2 or later, else `None`.
/// glow panics on a string it cannot parse, so the context of a core is
/// checked with this before the loader reads it.
pub(super) fn api_version(text: &str) -> Option<(bool, u32, u32)> {
    if let Some((major, minor)) = es_version(text) {
        return (major >= 2).then_some((true, major, minor));
    }
    if text.starts_with("WebGL ") {
        return None;
    }
    let number = text.split(' ').next()?;
    let (major, minor) = major_minor(number)?;
    (major >= 2).then_some((false, major, minor))
}

/// The major and minor version of an OpenGL ES version string, with the
/// rules of the parser in glow 0.18: the text after the last ` ES `, up to
/// the first space, split at dots; trailing zeros of the minor version are
/// dropped unless it starts with a zero.
fn es_version(text: &str) -> Option<(u32, u32)> {
    if text.starts_with("WebGL ") {
        return None;
    }
    let rest = &text[text.rfind(" ES ")? + " ES ".len()..];
    let rest = rest
        .find("GLSL ES ")
        .map_or(rest, |at| &rest[at + "GLSL ES ".len()..]);
    let number = rest.split(' ').next()?;
    major_minor(number)
}

/// `major.minor` of a version number, with the minor rule of glow 0.18.
fn major_minor(number: &str) -> Option<(u32, u32)> {
    let mut parts = number.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?;
    let minor = if minor.starts_with('0') {
        "0"
    } else {
        minor.trim_end_matches('0')
    };
    Some((major, minor.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesa_and_driver_strings_are_accepted() {
        for text in [
            "OpenGL ES 3.1 Mesa 26.1.8",
            "OpenGL ES 2.0 Mesa 24.0.0",
            "OpenGL ES 3.2 build 1.13@5776728",
            "OpenGL ES 3.0",
        ] {
            assert_eq!(check(text), Ok(()), "{text}");
        }
        assert_eq!(es_version("OpenGL ES 3.10 V@0502"), Some((3, 1)));
    }

    #[test]
    fn a_core_context_names_its_api_and_version() {
        assert_eq!(api_version("OpenGL ES 3.2 Mesa 26.1.8"), Some((true, 3, 2)));
        assert_eq!(
            api_version("4.6 (Core Profile) Mesa 26.1.8"),
            Some((false, 4, 6))
        );
        assert_eq!(
            api_version("4.5 (Compatibility Profile) Mesa 24.0"),
            Some((false, 4, 5))
        );
        assert_eq!(api_version("2.1 Mesa 24.0.0"), Some((false, 2, 1)));
        assert_eq!(api_version("3.3.0 NVIDIA 550.54"), Some((false, 3, 3)));
        for text in ["", "1.4 Mesa", "OpenGL ES 1.1", "WebGL 2.0", "x.y", "4"] {
            assert_eq!(api_version(text), None, "{text:?}");
        }
    }

    #[test]
    fn strings_that_glow_cannot_parse_are_refused() {
        for text in [
            "",
            "4.6 (Core Profile) Mesa 26.1.8",
            "OpenGL ES",
            "OpenGL ES x.y",
            "OpenGL ES 3",
            "OpenGL ES 1.1 Mesa",
            "WebGL 2.0",
        ] {
            assert_eq!(
                check(text),
                Err(GlesError::GlVersion {
                    text: text.to_owned()
                }),
                "{text:?}"
            );
        }
    }
}
