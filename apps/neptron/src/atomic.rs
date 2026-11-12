//! The write rule for user data and machine state: temp file, fsync,
//! rename, then fsync of the folder.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to `path` so that a crash leaves the old file or the new
/// one. The folder is made when it is missing. An entry at the temporary
/// path is removed first, and the temporary file is made with
/// `create_new`, so the write never follows a link.
///
/// # Errors
///
/// The error of the first step that failed. The file at `path` then holds
/// its old content, or the new one when only the folder sync failed.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".tmp");
    let temp = dir.join(name);
    match fs::remove_file(&temp) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path)?;
    File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_replaces_the_file_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("neptron-atomic-{}", std::process::id()));
        let path = dir.join("sub").join("file.toml");
        write(&path, b"one").expect("first write");
        fs::write(dir.join("sub").join("file.toml.tmp"), b"stale").expect("stale temp");
        write(&path, b"two").expect("second write");
        assert_eq!(fs::read(&path).expect("read"), b"two");
        assert!(!dir.join("sub").join("file.toml.tmp").exists());
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = fs::remove_dir_all(&dir);
    }
}
