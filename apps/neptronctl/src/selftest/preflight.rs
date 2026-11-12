//! The checks before the first modeset: the person confirmed this profile,
//! there is a mode to show, and the frontend does not hold the display.

use std::path::Path;

/// The process names of the frontend and the launcher. While one of them
/// runs, it holds the DRM device, and the self-test must not take it.
pub const FRONTEND_PROCESSES: [&str; 2] = ["neptron", "neptron-launch"];

/// What the system shows about the frontend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontend {
    /// The control socket accepts a connection.
    pub socket_answers: bool,
    /// The names of the running processes.
    pub processes: Vec<String>,
    /// `NEPTRON_DRM_FD` is set: the tool runs under the launcher.
    pub under_launcher: bool,
}

/// Why the run on a display must not start, or `None` when it can.
pub fn refusal(
    confirmed: &str,
    profile_id: &str,
    modes: usize,
    frontend: &Frontend,
) -> Option<String> {
    if confirmed != profile_id {
        return Some(format!(
            "the confirmation names {confirmed:?}, but the profile is {profile_id:?}; run the \
             dry run, check the ranges against the monitor, and name the profile it prints"
        ));
    }
    if frontend.under_launcher {
        return Some(
            "NEPTRON_DRM_FD is set: the self-test runs with the frontend and the launcher \
             stopped, and opens the device itself; the launcher would start it again after a \
             failure"
                .to_owned(),
        );
    }
    if frontend.socket_answers {
        return Some(
            "the frontend answers on the control socket; stop the frontend and the launcher \
             first"
                .to_owned(),
        );
    }
    if let Some(name) = frontend
        .processes
        .iter()
        .find(|p| FRONTEND_PROCESSES.contains(&p.as_str()))
    {
        return Some(format!(
            "the process {name} runs and holds the display; stop the frontend and the launcher \
             first"
        ));
    }
    if modes == 0 {
        return Some("the profile and the GPU give no test mode".to_owned());
    }
    None
}

/// Why the connector of the display backend cannot be used, or `None`.
/// The GPU values and the EDID profile come from the connector that the
/// detection chose; the backend must drive the same one.
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
pub fn connector_refusal(detected: Option<&str>, backend: &str) -> Option<String> {
    match detected {
        Some(name) if name != backend => Some(format!(
            "the GPU values are for {name}, but the display backend chose {backend}; name the \
             connector with --connector"
        )),
        _ => None,
    }
}

/// Looks at the control socket, the processes under `proc_root` and the
/// environment.
pub fn look(socket: &Path, proc_root: &Path) -> Frontend {
    Frontend {
        socket_answers: std::os::unix::net::UnixStream::connect(socket).is_ok(),
        processes: process_names(proc_root),
        under_launcher: std::env::var_os(DRM_FD_ENV).is_some(),
    }
}

/// The variable the launcher sets for the frontend.
const DRM_FD_ENV: &str = "NEPTRON_DRM_FD";

/// The `comm` of every process under `proc_root`. A process that ends
/// while the list is read is left out; a host with no `/proc` gives none.
fn process_names(proc_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .filter_map(|e| std::fs::read_to_string(e.path().join("comm")).ok())
        .map(|name| name.trim_end().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn quiet() -> Frontend {
        Frontend::default()
    }

    #[test]
    fn a_confirmed_profile_with_modes_and_no_frontend_can_start() {
        assert_eq!(refusal("pc_31_120", "pc_31_120", 4, &quiet()), None);
    }

    #[test]
    fn the_confirmation_must_name_the_profile() {
        let why = refusal("ntsc", "pc_31_120", 4, &quiet()).unwrap();
        assert!(why.contains("\"pc_31_120\""), "{why}");
    }

    #[test]
    fn a_running_frontend_is_refused() {
        for frontend in [
            Frontend {
                socket_answers: true,
                ..quiet()
            },
            Frontend {
                processes: vec!["sh".into(), "neptron-launch".into()],
                ..quiet()
            },
            Frontend {
                processes: vec!["neptron".into()],
                ..quiet()
            },
            Frontend {
                under_launcher: true,
                ..quiet()
            },
        ] {
            assert!(refusal("x", "x", 1, &frontend).is_some(), "{frontend:?}");
        }
        let others = Frontend {
            processes: vec!["neptronctl".into(), "neptron-update".into()],
            ..quiet()
        };
        assert_eq!(refusal("x", "x", 1, &others), None);
    }

    #[test]
    fn no_mode_is_refused() {
        assert!(refusal("x", "x", 0, &quiet()).is_some());
    }

    #[test]
    fn the_backend_must_drive_the_detected_connector() {
        assert_eq!(connector_refusal(Some("DP-1"), "DP-1"), None);
        assert_eq!(connector_refusal(None, "Virtual-1"), None);
        let why = connector_refusal(Some("VGA-1"), "DP-1").unwrap();
        assert!(why.contains("--connector"), "{why}");
    }

    #[test]
    fn process_names_come_from_comm() {
        let root = std::env::temp_dir().join(format!("neptronctl-proc-{}", std::process::id()));
        for (pid, comm) in [("1", "init\n"), ("42", "neptron-launch\n")] {
            std::fs::create_dir_all(root.join(pid)).unwrap();
            std::fs::write(root.join(pid).join("comm"), comm).unwrap();
        }
        std::fs::create_dir_all(root.join("self")).unwrap();
        std::fs::create_dir_all(root.join("7")).unwrap();
        let mut names = process_names(&root);
        names.sort();
        assert_eq!(names, ["init", "neptron-launch"]);
        assert!(process_names(&PathBuf::from("/nonexistent/proc")).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_socket_with_no_listener_does_not_answer() {
        let f = look(
            Path::new("/nonexistent/control.sock"),
            Path::new("/nonexistent"),
        );
        assert!(!f.socket_answers);
        assert!(f.processes.is_empty());
    }
}
