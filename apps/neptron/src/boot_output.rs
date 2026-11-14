//! The output of a boot: chosen once, by the first run of the boot, and
//! kept by every relaunch after it.
//!
//! Each game launch and each return to the game list is a new process. If
//! each one ran the output rule again, a Pi with composite only would wait
//! [`crate::display_wait::COMPOSITE_FALLBACK_WAIT_NS`] at every launch, and
//! an HDMI display that answers late could take a game away from the TV
//! that shows the menu: HDMI and composite share one pixel valve, so the
//! TV goes black. So the first run writes the connector it drives to
//! `run/output` ([`crate::Paths::boot_output`]), and a relaunch opens that
//! connector by name, with no wait.
//!
//! The name is a file of its own and not a field of the relaunch file: the
//! KMS backend chooses and opens the connector, and it is the only part of
//! the run that knows whether the name came from the settings or from the
//! rule; the relaunch file is written by the shell, from many places. The
//! run folder is a RAM folder, and the first run of a boot removes the file
//! before it chooses, so a name never outlives its boot.

use std::io;
use std::path::Path;

use neptron_platform::{ConnectorStatus, Platform};

use crate::atomic;

/// Where the output of a run comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputPlan {
    /// `display.output` names the connector: it wins over everything.
    Named(String),
    /// The connector of this boot, which the first run chose: no wait.
    Kept(String),
    /// The output rule: the wait, the connector choice of the backend and
    /// the composite fallback.
    Rule,
}

/// The plan of a run. `setting` is `display.output` when it names a
/// connector, `kept` the connector of this boot (`None` on the first run
/// of a boot). A kept connector that sysfs no longer shows, or shows as
/// `disconnected`, is no longer usable, and the run takes the rule.
pub fn plan(setting: Option<&str>, kept: Option<&str>, platform: &Platform) -> OutputPlan {
    if let Some(name) = setting {
        return OutputPlan::Named(name.to_owned());
    }
    match kept {
        Some(name) if usable(platform, name) => OutputPlan::Kept(name.to_owned()),
        _ => OutputPlan::Rule,
    }
}

/// Whether sysfs shows the connector `name`, and not as `disconnected`.
/// A composite connector always reads `unknown`.
fn usable(platform: &Platform, name: &str) -> bool {
    platform
        .connectors
        .iter()
        .any(|c| c.name == name && c.status != ConnectorStatus::Disconnected)
}

/// The connector of this boot in the file at `path`, or `None` when there
/// is no such file or it holds no name.
pub fn read(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let name = text.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Writes `name` as the connector of this boot.
///
/// # Errors
///
/// The error of the write.
pub fn write(path: &Path, name: &str) -> io::Result<()> {
    atomic::write(path, format!("{name}\n").as_bytes())
}

/// Removes the connector of an earlier boot. No file is no error.
///
/// # Errors
///
/// The error of the removal.
pub fn clear(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use neptron_platform::Connector;

    use super::*;

    fn pi3(hdmi: ConnectorStatus, composite: bool) -> Platform {
        let connector = |name: &str, kind: &str, status| Connector {
            card: "card1".to_owned(),
            name: name.to_owned(),
            kind: kind.to_owned(),
            status,
            edid: None,
        };
        let mut connectors = vec![connector("HDMI-A-1", "HDMI-A", hdmi)];
        if composite {
            connectors.push(connector(
                "Composite-1",
                "Composite",
                ConnectorStatus::Unknown,
            ));
        }
        Platform {
            gpus: Vec::new(),
            connectors,
        }
    }

    #[test]
    fn a_relaunch_keeps_the_connector_of_the_boot_with_no_wait() {
        let p = pi3(ConnectorStatus::Disconnected, true);
        assert_eq!(
            plan(None, Some("Composite-1"), &p),
            OutputPlan::Kept("Composite-1".to_owned())
        );
    }

    #[test]
    fn an_hdmi_display_that_answers_later_does_not_take_the_boot_away() {
        let p = pi3(ConnectorStatus::Connected, true);
        assert_eq!(
            plan(None, Some("Composite-1"), &p),
            OutputPlan::Kept("Composite-1".to_owned())
        );
    }

    #[test]
    fn a_connector_that_is_gone_or_disconnected_gives_the_rule() {
        let p = pi3(ConnectorStatus::Connected, false);
        assert_eq!(plan(None, Some("Composite-1"), &p), OutputPlan::Rule);
        let unplugged = pi3(ConnectorStatus::Disconnected, true);
        assert_eq!(plan(None, Some("HDMI-A-1"), &unplugged), OutputPlan::Rule);
        assert_eq!(plan(None, None, &p), OutputPlan::Rule, "the first run");
    }

    #[test]
    fn a_named_output_wins_over_the_connector_of_the_boot() {
        let p = pi3(ConnectorStatus::Connected, true);
        assert_eq!(
            plan(Some("HDMI-A-1"), Some("Composite-1"), &p),
            OutputPlan::Named("HDMI-A-1".to_owned())
        );
        assert_eq!(
            plan(Some("Composite-1"), None, &Platform::default()),
            OutputPlan::Named("Composite-1".to_owned()),
            "a name is used whatever sysfs shows"
        );
    }

    #[test]
    fn the_file_holds_one_name_and_clears() {
        let dir = std::env::temp_dir().join(format!("neptron-boot-output-{}", std::process::id()));
        let path = dir.join("output");
        assert_eq!(read(&path), None);
        clear(&path).expect("no file is no error");
        write(&path, "Composite-1").expect("write");
        assert_eq!(read(&path).as_deref(), Some("Composite-1"));
        std::fs::write(&path, "  \n").expect("write");
        assert_eq!(read(&path), None, "no name");
        clear(&path).expect("clear");
        assert!(!path.exists());
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
