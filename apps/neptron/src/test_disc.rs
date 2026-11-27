//! A test system with discs and BIOS files, and a folder for test files,
//! for the unit tests of the app.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use neptron_library::bios;
use neptron_library::{Catalog, Source};

pub(crate) const US: &[u8] = b"synthetic us 16b";
pub(crate) const EU: &[u8] = b"synthetic eu 16b";

pub(crate) fn catalog() -> Catalog {
    let system = format!(
        "format = 1\nfolder = \"testdisc\"\naliases = []\nname = \"Test Disc\"\nextensions = [\"chd\", \"cue\", \"m3u\"]\ndiscs = [\"chd\", \"cue\", \"m3u\"]\n\n[[bios]]\nfile = \"td-us.bin\"\nregion = \"ntsc-u\"\nsize = 16\ncrc32 = [\"{:08x}\"]\n\n[[bios]]\nfile = \"td-eu.bin\"\nregion = \"pal\"\nsize = 16\ncrc32 = [\"{:08x}\"]\n",
        bios::crc32(US),
        bios::crc32(EU)
    );
    let core = "format = 1\nid = \"td\"\nname = \"TD\"\nlibrary = \"td.so\"\nsystems = [\"testdisc\"]\nlicence = \"MIT\"\n";
    Catalog::from_sources(
        &[Source {
            name: "testdisc",
            text: &system,
        }],
        &[Source {
            name: "td",
            text: core,
        }],
    )
    .expect("the test catalog")
}

pub(crate) struct Dir(pub(crate) PathBuf);

impl Dir {
    pub(crate) fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "neptron-disc-launch-{label}-{}-{n}",
            std::process::id()
        ));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn put(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        // A folder left in the temporary directory does no harm.
        let _ = fs::remove_dir_all(&self.0);
    }
}
