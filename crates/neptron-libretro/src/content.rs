//! Content for `retro_load_game`: the path and, when the core takes it, the
//! data. Content is user data: it is opened read-only and never changed.

use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use neptron_libretro_sys::retro_game_info;

use crate::error::LoadError;

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

/// The content as the core gets it. The `retro_game_info` points into the
/// fields of this struct, which never change, so the pointers stay valid
/// for its life. The host keeps it until `retro_unload_game`.
#[derive(Debug)]
pub(crate) struct LoadedContent {
    // Owned so that the pointers in `info` stay valid.
    _path: CString,
    _data: Option<Vec<u8>>,
    info: retro_game_info,
}

impl LoadedContent {
    /// Prepares a plain file. With `need_fullpath` the core gets the path
    /// only; otherwise the host reads the file too.
    pub(crate) fn plain(path: &Path, need_fullpath: bool) -> Result<Self, LoadError> {
        let c_path = c_path(path)?;
        let data = if need_fullpath {
            None
        } else {
            Some(read_content(path)?)
        };
        Ok(Self::new(c_path, data))
    }

    fn new(path: CString, data: Option<Vec<u8>>) -> Self {
        let info = retro_game_info {
            path: path.as_ptr(),
            data: data
                .as_ref()
                .map_or(std::ptr::null(), |d| d.as_ptr().cast()),
            size: data.as_ref().map_or(0, Vec::len),
            meta: std::ptr::null(),
        };
        Self {
            _path: path,
            _data: data,
            info,
        }
    }

    pub(crate) fn info(&self) -> &retro_game_info {
        &self.info
    }

    #[cfg(test)]
    pub(crate) fn data(&self) -> Option<&[u8]> {
        self._data.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::ffi::CStr {
        &self._path
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
    use std::path::PathBuf;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-libretro-content-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_plain_file_gives_path_and_data() {
        let path = temp_file("game.nes", b"rom bytes");
        let content = LoadedContent::plain(&path, false).unwrap();
        let info = content.info();
        assert_eq!(content.path().to_str().unwrap(), path.to_str().unwrap());
        assert_eq!(content.data(), Some(&b"rom bytes"[..]));
        assert_eq!(info.path, content.path().as_ptr());
        assert_eq!(info.data, content.data().unwrap().as_ptr().cast());
        assert_eq!(info.size, 9);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn need_fullpath_gives_the_path_only() {
        let content = LoadedContent::plain(Path::new("/no/such/file.sfc"), true).unwrap();
        assert!(content.info().data.is_null());
        assert_eq!(content.info().size, 0);
    }

    #[test]
    fn a_missing_file_is_an_error() {
        assert!(matches!(
            LoadedContent::plain(Path::new("/no/such/file.sfc"), false),
            Err(LoadError::ReadContent { .. })
        ));
    }

    #[test]
    fn a_file_above_the_limit_is_an_error() {
        let path = temp_file("huge.md", b"");
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_CONTENT_BYTES + 1)
            .unwrap();
        assert!(matches!(
            LoadedContent::plain(&path, false),
            Err(LoadError::ContentTooLarge { len, .. }) if len == MAX_CONTENT_BYTES + 1
        ));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_path_that_is_not_utf8_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/roms/\xFF.nes"));
        assert!(matches!(
            LoadedContent::plain(path, true),
            Err(LoadError::InvalidPath { .. })
        ));
    }
}
