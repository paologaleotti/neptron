//! Content for `retro_load_game`: the path and, when the core takes it, the
//! data. Content is user data: it is opened read-only and never changed.

use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use neptron_libretro_sys::{retro_game_info, retro_game_info_ext};

use crate::error::LoadError;
use crate::zip::{ZipError, read_zip_entry};

/// The largest content that the host reads into memory, in bytes.
pub const MAX_CONTENT_BYTES: u64 = 64 * 1024 * 1024;

/// What to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRequest<'a> {
    /// A plain file or a `.zip` file.
    pub path: &'a Path,
    /// The extensions that the system accepts, without dots. They choose
    /// the entry of a zip file.
    pub extensions: &'a [String],
}

/// The content rules of the core: from `retro_system_info` and from
/// `SET_CONTENT_INFO_OVERRIDE`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ContentRules<'a> {
    pub need_fullpath: bool,
    pub block_extract: bool,
    pub overrides: &'a [ContentOverride],
}

/// `need_fullpath` for one extension, from `SET_CONTENT_INFO_OVERRIDE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContentOverride {
    /// Lower case, without a dot.
    pub extension: String,
    pub need_fullpath: bool,
}

impl ContentRules<'_> {
    fn need_fullpath(&self, extension: &str) -> bool {
        self.overrides
            .iter()
            .find(|o| o.extension.eq_ignore_ascii_case(extension))
            .map_or(self.need_fullpath, |o| o.need_fullpath)
    }
}

/// The content as the core gets it. `info` and `info_ext` point into the
/// fields of this struct, which never change, so the pointers stay valid
/// for its life. `info_ext` is boxed so that its own address stays the same
/// when the struct moves. The host keeps it until `retro_unload_game`.
#[derive(Debug)]
pub(crate) struct LoadedContent {
    // Owned so that the pointers in `info` and `info_ext` stay valid.
    _path: CString,
    _data: Option<Vec<u8>>,
    _strings: [Option<CString>; 5],
    info: retro_game_info,
    info_ext: Box<retro_game_info_ext>,
}

/// The parts of a content path that `GET_GAME_INFO_EXT` gives.
struct Names {
    full_path: String,
    archive: Option<(String, String)>,
    dir: String,
    name: String,
    ext: String,
}

impl LoadedContent {
    /// Prepares the content of `request`: a plain file, or the first
    /// matching entry of a zip file (unless the core wants archives whole).
    pub(crate) fn prepare(
        request: &ContentRequest<'_>,
        rules: &ContentRules<'_>,
    ) -> Result<Self, LoadError> {
        let path = request.path;
        let text = path.to_str().ok_or_else(|| LoadError::InvalidPath {
            path: path.to_owned(),
        })?;
        let is_zip = extension(text).eq_ignore_ascii_case("zip") && !rules.block_extract;
        if is_zip {
            Self::zip_entry(path, text, request.extensions, rules)
        } else {
            Self::plain(path, text, rules)
        }
    }

    fn plain(path: &Path, text: &str, rules: &ContentRules<'_>) -> Result<Self, LoadError> {
        let names = Names {
            full_path: text.to_owned(),
            archive: None,
            dir: parent(text).to_owned(),
            name: stem(file_name(text)).to_owned(),
            ext: extension(text).to_ascii_lowercase(),
        };
        let data = if rules.need_fullpath(&names.ext) {
            None
        } else {
            Some(read_content(path)?)
        };
        Self::new(path, names, data)
    }

    fn zip_entry(
        path: &Path,
        text: &str,
        extensions: &[String],
        rules: &ContentRules<'_>,
    ) -> Result<Self, LoadError> {
        let zip_error = |source| LoadError::Zip {
            path: path.to_owned(),
            source,
        };
        let mut file = File::open(path).map_err(|e| zip_error(ZipError::Io(e)))?;
        let entry = read_zip_entry(&mut file, extensions, MAX_CONTENT_BYTES).map_err(zip_error)?;
        let ext = extension(&entry.name).to_ascii_lowercase();
        if rules.need_fullpath(&ext) {
            return Err(LoadError::ZipNeedsFullPath {
                path: path.to_owned(),
                entry: entry.name,
            });
        }
        let names = Names {
            full_path: format!("{text}#{}", entry.name),
            dir: parent(text).to_owned(),
            name: stem(file_name(&entry.name)).to_owned(),
            ext,
            archive: Some((text.to_owned(), entry.name)),
        };
        Self::new(path, names, Some(entry.data))
    }

    fn new(path: &Path, names: Names, data: Option<Vec<u8>>) -> Result<Self, LoadError> {
        let c = |s: &str| {
            CString::new(s).map_err(|_| LoadError::InvalidPath {
                path: path.to_owned(),
            })
        };
        let full_path = c(&names.full_path)?;
        let (archive_path, archive_file) = match &names.archive {
            Some((archive, entry)) => (Some(c(archive)?), Some(c(entry)?)),
            None => (None, None),
        };
        let strings = [
            archive_path,
            archive_file,
            Some(c(&names.dir)?),
            Some(c(&names.name)?),
            Some(c(&names.ext)?),
        ];
        let ptr = |s: &Option<CString>| s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
        let (data_ptr, size) = data
            .as_ref()
            .map_or((std::ptr::null(), 0), |d| (d.as_ptr().cast(), d.len()));
        let info = retro_game_info {
            path: full_path.as_ptr(),
            data: data_ptr,
            size,
            meta: std::ptr::null(),
        };
        let info_ext = Box::new(retro_game_info_ext {
            full_path: full_path.as_ptr(),
            archive_path: ptr(&strings[0]),
            archive_file: ptr(&strings[1]),
            dir: ptr(&strings[2]),
            name: ptr(&strings[3]),
            ext: ptr(&strings[4]),
            meta: std::ptr::null(),
            data: data_ptr,
            size,
            file_in_archive: names.archive.is_some(),
            persistent_data: true,
        });
        Ok(Self {
            _path: full_path,
            _data: data,
            _strings: strings,
            info,
            info_ext,
        })
    }

    pub(crate) fn info(&self) -> &retro_game_info {
        &self.info
    }

    /// The extended information for `GET_GAME_INFO_EXT`. The address stays
    /// the same for the life of the content.
    pub(crate) fn info_ext(&self) -> *const retro_game_info_ext {
        &raw const *self.info_ext
    }

    #[cfg(test)]
    pub(crate) fn data(&self) -> Option<&[u8]> {
        self._data.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::ffi::CStr {
        &self._path
    }

    #[cfg(test)]
    pub(crate) fn ext_strings(&self) -> Vec<Option<String>> {
        self._strings
            .iter()
            .map(|s| s.as_ref().map(|s| s.to_string_lossy().into_owned()))
            .collect()
    }
}

/// The overrides of one `SET_CONTENT_INFO_OVERRIDE` entry: each extension
/// of `extensions` (`"a|b"`) with `need_fullpath`.
pub(crate) fn overrides_of(extensions: &[u8], need_fullpath: bool) -> Vec<ContentOverride> {
    String::from_utf8_lossy(extensions)
        .split('|')
        .filter(|e| !e.is_empty())
        .map(|e| ContentOverride {
            extension: e.to_ascii_lowercase(),
            need_fullpath,
        })
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn stem(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

fn extension(path: &str) -> &str {
    let name = file_name(path);
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext,
        _ => "",
    }
}

/// The path as a C string. libretro.h says paths are UTF-8.
pub(crate) fn c_path(path: &Path) -> Result<CString, LoadError> {
    path.to_str()
        .and_then(|s| CString::new(s).ok())
        .ok_or_else(|| LoadError::InvalidPath {
            path: path.to_owned(),
        })
}

fn read_content(path: &Path) -> Result<Vec<u8>, LoadError> {
    let read_error = |source| LoadError::ReadContent {
        path: path.to_owned(),
        source,
    };
    let too_large = |len| LoadError::ContentTooLarge {
        path: path.to_owned(),
        len,
        limit: MAX_CONTENT_BYTES,
    };
    let file = File::open(path).map_err(read_error)?;
    let len = file.metadata().map_err(read_error)?.len();
    if len > MAX_CONTENT_BYTES {
        return Err(too_large(len));
    }
    let mut data = Vec::new();
    file.take(MAX_CONTENT_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(read_error)?;
    if data.len() as u64 > MAX_CONTENT_BYTES {
        return Err(too_large(data.len() as u64));
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::tests::{TestEntry, build};
    use std::path::PathBuf;

    const PLAIN: ContentRules<'static> = ContentRules {
        need_fullpath: false,
        block_extract: false,
        overrides: &[],
    };
    const FULLPATH: ContentRules<'static> = ContentRules {
        need_fullpath: true,
        ..PLAIN
    };

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-libretro-content-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn prepare(path: &Path, rules: &ContentRules<'_>) -> Result<LoadedContent, LoadError> {
        let exts = ["nes".to_owned(), "fds".to_owned()];
        LoadedContent::prepare(
            &ContentRequest {
                path,
                extensions: &exts,
            },
            rules,
        )
    }

    #[test]
    fn a_plain_file_gives_path_and_data() {
        let path = temp_file("game.nes", b"rom bytes");
        let content = prepare(&path, &PLAIN).unwrap();
        let info = content.info();
        assert_eq!(content.path().to_str().unwrap(), path.to_str().unwrap());
        assert_eq!(content.data(), Some(&b"rom bytes"[..]));
        assert_eq!(info.path, content.path().as_ptr());
        assert_eq!(info.data, content.data().unwrap().as_ptr().cast());
        assert_eq!(info.size, 9);
        let dir = path.parent().unwrap().to_str().unwrap().to_owned();
        assert_eq!(
            content.ext_strings(),
            [
                None,
                None,
                Some(dir),
                Some("game".to_owned()),
                Some("nes".to_owned())
            ]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn need_fullpath_gives_the_path_only() {
        let content = prepare(Path::new("/no/such/file.nes"), &FULLPATH).unwrap();
        assert!(content.info().data.is_null());
        assert_eq!(content.info().size, 0);
    }

    #[test]
    fn an_override_wins_for_its_extension() {
        let overrides = overrides_of(b"NES|unf", false);
        let rules = ContentRules {
            overrides: &overrides,
            ..FULLPATH
        };
        assert!(!rules.need_fullpath("nes"));
        assert!(!rules.need_fullpath("UNF"));
        assert!(rules.need_fullpath("fds"));
        let path = temp_file("over.nes", b"data");
        assert!(prepare(&path, &rules).unwrap().data().is_some());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_zip_entry_gives_data_and_a_named_path() {
        let zip = build(&[
            TestEntry {
                name: "readme.txt",
                data: b"hello",
                deflate: false,
            },
            TestEntry {
                name: "roms/Game.NES",
                data: b"NES\x1a deflated",
                deflate: true,
            },
        ]);
        let path = temp_file("Game.zip", &zip);
        let content = prepare(&path, &PLAIN).unwrap();
        let text = path.to_str().unwrap();
        assert_eq!(
            content.path().to_str().unwrap(),
            format!("{text}#roms/Game.NES")
        );
        assert_eq!(content.data(), Some(&b"NES\x1a deflated"[..]));
        let ext = content.info_ext();
        // Comparing pointers needs no unsafe.
        assert!(!ext.is_null());
        let dir = path.parent().unwrap().to_str().unwrap().to_owned();
        assert_eq!(
            content.ext_strings(),
            [
                Some(text.to_owned()),
                Some("roms/Game.NES".to_owned()),
                Some(dir),
                Some("Game".to_owned()),
                Some("nes".to_owned())
            ]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn zip_errors_are_typed() {
        let zip = build(&[TestEntry {
            name: "game.sfc",
            data: b"snes",
            deflate: false,
        }]);
        let path = temp_file("wrong.zip", &zip);
        assert!(matches!(
            prepare(&path, &PLAIN),
            Err(LoadError::Zip {
                source: ZipError::NoMatchingEntry { .. },
                ..
            })
        ));
        let path2 = temp_file("broken.zip", b"PK not really");
        assert!(matches!(
            prepare(&path2, &PLAIN),
            Err(LoadError::Zip {
                source: ZipError::NoEndOfCentralDirectory,
                ..
            })
        ));
        assert!(matches!(
            prepare(Path::new("/no/such.zip"), &PLAIN),
            Err(LoadError::Zip {
                source: ZipError::Io(_),
                ..
            })
        ));
        let good = temp_file(
            "good.zip",
            &build(&[TestEntry {
                name: "game.nes",
                data: b"nes",
                deflate: false,
            }]),
        );
        assert!(matches!(
            prepare(&good, &FULLPATH),
            Err(LoadError::ZipNeedsFullPath { entry, .. }) if entry == "game.nes"
        ));
        for p in [path, path2, good] {
            std::fs::remove_file(p).unwrap();
        }
    }

    #[test]
    fn block_extract_passes_the_archive_whole() {
        let path = temp_file("arcade.zip", b"any bytes");
        let rules = ContentRules {
            block_extract: true,
            ..PLAIN
        };
        let content = prepare(&path, &rules).unwrap();
        assert_eq!(content.data(), Some(&b"any bytes"[..]));
        assert_eq!(content.path().to_str().ok(), path.to_str());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_missing_file_is_an_error() {
        assert!(matches!(
            prepare(Path::new("/no/such/file.nes"), &PLAIN),
            Err(LoadError::ReadContent { .. })
        ));
    }

    #[test]
    fn a_file_above_the_limit_is_an_error() {
        let path = temp_file("huge.nes", b"");
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_CONTENT_BYTES + 1)
            .unwrap();
        assert!(matches!(
            prepare(&path, &PLAIN),
            Err(LoadError::ContentTooLarge { len, .. }) if len == MAX_CONTENT_BYTES + 1
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_path_that_is_not_utf8_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/roms/\xFF.nes"));
        assert!(matches!(
            prepare(path, &FULLPATH),
            Err(LoadError::InvalidPath { .. })
        ));
    }

    #[test]
    fn path_parts() {
        assert_eq!(extension("/a/b.c/file.NES"), "NES");
        assert_eq!(extension("/a/.hidden"), "");
        assert_eq!(extension("/a/noext"), "");
        assert_eq!(stem("Game.nes"), "Game");
        assert_eq!(stem(".nes"), ".nes");
        assert_eq!(parent("/a/b/c.nes"), "/a/b");
        assert_eq!(parent("c.nes"), "");
        assert_eq!(file_name("/a/b/c.nes"), "c.nes");
    }
}
