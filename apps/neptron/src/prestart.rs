//! What a process does before a backend opens the display: the import of
//! `neptron.toml`, then the settings that the backend reads to choose and
//! open the output. The import comes first, so the first start after a
//! change of the file already opens the output that the file names.

use neptron_settings::{DISPLAY_MONITOR, DISPLAY_OUTPUT, Schema, Storage, Target};

use crate::boot_file::{self, Imported};
use crate::display::AUTO;
use crate::paths::Paths;

/// The import and the settings of the backend, read once per process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prestart {
    /// What the import found, for the notices and the log of the run.
    pub imported: Imported,
    /// `display.output` when it names a connector.
    pub output: Option<String>,
    /// `display.monitor`, `auto` when unset.
    pub monitor: String,
}

/// Imports `neptron.toml`, then reads the global settings that the backend
/// needs. A settings file that cannot be read gives `auto` for both keys;
/// the boot of the shell reports it.
pub fn prepare(paths: &Paths) -> Prestart {
    let schema = Schema::builtin();
    let storage = Storage::new(paths.config());
    let imported = boot_file::import(
        &schema,
        &paths.boot_file(),
        &paths.boot_file_hash(),
        &storage,
    );
    let global = storage.load(&schema, Target::Global).ok();
    let setting = |key: &str| {
        global
            .as_ref()
            .and_then(|l| l.parsed.layer.get(key).map(ToString::to_string))
            .unwrap_or_else(|| AUTO.to_owned())
    };
    Prestart {
        imported,
        output: Some(setting(DISPLAY_OUTPUT)).filter(|o| o != AUTO),
        monitor: setting(DISPLAY_MONITOR),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use neptron_platform::Platform;

    use super::*;
    use crate::boot_output::{OutputPlan, plan};

    fn paths(root: &Path) -> Paths {
        Paths {
            share: root.join("share"),
            state: root.join("state"),
            data: root.join("data"),
            cores: root.join("cores"),
            run: root.join("run"),
            sysfs: root.join("sys"),
            boot: root.join("boot"),
        }
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-prestart-{name}-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("boot")).expect("the folder");
        dir
    }

    #[test]
    fn the_first_start_plans_the_output_that_the_file_names() {
        let dir = folder("output");
        let paths = paths(&dir);
        std::fs::write(
            paths.boot_file(),
            "[display]\noutput = \"Composite-1\"\nmonitor = \"generic_15\"\n",
        )
        .expect("neptron.toml");
        let prestart = prepare(&paths);
        assert_eq!(prestart.output.as_deref(), Some("Composite-1"));
        assert_eq!(prestart.monitor, "generic_15");
        assert_eq!(
            plan(prestart.output.as_deref(), None, &Platform::default()),
            OutputPlan::Named("Composite-1".to_owned())
        );
        assert!(prestart.imported.problems.is_empty());
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn with_no_file_and_no_settings_both_keys_are_auto() {
        let dir = folder("none");
        let prestart = prepare(&paths(&dir));
        assert_eq!(prestart.output, None);
        assert_eq!(prestart.monitor, AUTO);
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
