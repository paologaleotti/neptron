//! A temporary folder for tests, removed on drop.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "neptron-library-{label}-{}-{n}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove a stale test folder");
        }
        fs::create_dir_all(&path).expect("make the test folder");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Makes an empty file and its parent folders.
    pub fn file(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("make the parent folder");
        }
        fs::write(&path, b"").expect("write the test file");
        path
    }

    pub fn dir(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(&path).expect("make the test folder");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // A folder left in the temporary directory does no harm.
        let _ = fs::remove_dir_all(&self.path);
    }
}
