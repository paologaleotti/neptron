//! The network status of the image for the Network panel: the address
//! file of the run folder and the switch file of the share, read on a
//! thread of their own.
//!
//! A lease can come or go at any time after the boot, so the panel reads
//! the file again while it is open. The frame thread does no file I/O: it
//! asks the thread for a read and takes the result in a later frame.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use neptron_platform::{NETWORK_FILE_MAX_BYTES, NetworkStatus, parse_network};

/// The address file in the run folder, which the address script of the
/// image writes.
pub const STATUS_FILE: &str = "network.toml";

/// The file on the state partition that turns the network share off at
/// the start of the share script.
pub const SHARE_OFF_FILE: &str = "network-share-off";

/// The time between two reads while the Network panel is open, in
/// nanoseconds of the clock of the frame loop.
pub const NETWORK_REFRESH_NS: u64 = 1_000_000_000;

/// What the Network panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkInfo {
    /// The address file is valid.
    Known {
        /// The host name and the addresses.
        status: NetworkStatus,
        /// False when the switch file of the share is there.
        share_on: bool,
    },
    /// The address file is missing, cannot be read, or is not valid.
    Unknown {
        /// Why, for the log.
        reason: String,
    },
}

/// Reads the address file in the run folder `run` and the switch file of
/// the share in the state folder `state`.
pub fn read(run: &Path, state: &Path) -> NetworkInfo {
    let path = run.join(STATUS_FILE);
    let text = match read_bounded(&path) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return NetworkInfo::Unknown {
                reason: format!(
                    "{} is larger than {NETWORK_FILE_MAX_BYTES} bytes",
                    path.display()
                ),
            };
        }
        Err(e) => {
            return NetworkInfo::Unknown {
                reason: format!("cannot read {}: {e}", path.display()),
            };
        }
    };
    match parse_network(&text) {
        Ok(status) => NetworkInfo::Known {
            status,
            share_on: !state.join(SHARE_OFF_FILE).exists(),
        },
        Err(e) => NetworkInfo::Unknown {
            reason: format!("{}: {e}", path.display()),
        },
    }
}

/// The text of `path`, or `None` when it is larger than
/// [`NETWORK_FILE_MAX_BYTES`]: a damaged file must not fill the memory.
fn read_bounded(path: &Path) -> io::Result<Option<String>> {
    let file = std::fs::File::open(path)?;
    let limit = u64::try_from(NETWORK_FILE_MAX_BYTES).unwrap_or(u64::MAX);
    let mut text = String::new();
    file.take(limit.saturating_add(1))
        .read_to_string(&mut text)?;
    Ok((text.len() <= NETWORK_FILE_MAX_BYTES).then_some(text))
}

/// The thread that reads the network status when the frame thread asks.
/// Drop ends the thread and joins it: a read is one small file in RAM, so
/// the join does not wait long.
#[derive(Debug)]
pub struct NetworkWatch {
    requests: Option<SyncSender<()>>,
    results: Receiver<NetworkInfo>,
    thread: Option<JoinHandle<()>>,
}

impl NetworkWatch {
    /// Starts the thread for the run folder `run` and the state folder
    /// `state`.
    ///
    /// # Errors
    ///
    /// The error of the thread start.
    pub fn start(run: PathBuf, state: PathBuf) -> io::Result<Self> {
        let (requests, asked) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("network-watch".to_owned())
            .spawn(move || watch(&run, &state, &asked, &sender))?;
        Ok(Self {
            requests: Some(requests),
            results,
            thread: Some(thread),
        })
    }

    /// Asks for a read. It never waits: when a request waits already, this
    /// one merges with it.
    pub fn request(&self) {
        if let Some(requests) = &self.requests {
            // Full: a read is asked for already. Disconnected: the thread
            // has ended, and the panel keeps what it has.
            let _ = requests.try_send(());
        }
    }

    /// The result of a read that differs from the one before, when one
    /// came. It never waits.
    pub fn poll(&self) -> Option<NetworkInfo> {
        // Empty: no new result. Disconnected: the thread has ended, and the
        // panel keeps what it has.
        self.results.try_recv().ok()
    }
}

impl Drop for NetworkWatch {
    fn drop(&mut self) {
        // With no sender left, the wait of the thread ends.
        self.requests = None;
        if let Some(thread) = self.thread.take() {
            // A thread that panicked has nothing left to stop.
            let _ = thread.join();
        }
    }
}

/// The loop of the thread: one read for each request, and a result only
/// when it differs from the last one sent.
fn watch(run: &Path, state: &Path, asked: &Receiver<()>, sender: &SyncSender<NetworkInfo>) {
    let mut sent: Option<NetworkInfo> = None;
    while asked.recv().is_ok() {
        let info = read(run, state);
        if sent.as_ref() == Some(&info) {
            continue;
        }
        match sender.try_send(info.clone()) {
            Ok(()) => sent = Some(info),
            // The frame thread has not taken the last result: this one is
            // dropped, and the next read sends again.
            Err(TrySendError::Full(_)) => sent = None,
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::time::Duration;

    use neptron_platform::PortAddress;

    use super::*;

    /// The longest wait for a result of the thread. A read takes far less;
    /// the bound only keeps a broken test from hanging.
    const RESULT_WAIT_MAX: Duration = Duration::from_secs(30);

    const ONE_PORT: &str = "version = 1\nhost_name = \"neptron\"\n[[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.112\"\n";
    const NO_ADDRESS: &str = "version = 1\nhost_name = \"neptron\"\n";

    /// The folder of one test, removed when the test ends.
    struct Folders {
        dir: PathBuf,
        run: PathBuf,
        state: PathBuf,
    }

    impl Drop for Folders {
        fn drop(&mut self) {
            // A folder that cannot be removed only leaves a file in the
            // temp folder.
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// A run folder and a state folder for one test.
    fn folders(name: &str, status: Option<&str>) -> Folders {
        let dir =
            std::env::temp_dir().join(format!("neptron-network-{}-{name}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        let (run, state) = (dir.join("run"), dir.join("state"));
        std::fs::create_dir_all(&run).expect("the run folder");
        std::fs::create_dir_all(&state).expect("the state folder");
        if let Some(text) = status {
            std::fs::write(run.join(STATUS_FILE), text).expect("the status");
        }
        Folders { dir, run, state }
    }

    fn known(addresses: &[(&str, [u8; 4])], share_on: bool) -> NetworkInfo {
        NetworkInfo::Known {
            status: NetworkStatus {
                host_name: Some("neptron".to_owned()),
                addresses: addresses
                    .iter()
                    .map(|&(port, ipv4)| PortAddress {
                        port: port.to_owned(),
                        ipv4: Ipv4Addr::from(ipv4),
                    })
                    .collect(),
            },
            share_on,
        }
    }

    impl NetworkWatch {
        /// Asks for a read, and waits until the request is queued.
        fn request_queued(&self) {
            self.requests
                .as_ref()
                .expect("a sender")
                .send(())
                .expect("the thread runs");
        }

        /// Waits for the next result.
        fn next(&self) -> NetworkInfo {
            self.results
                .recv_timeout(RESULT_WAIT_MAX)
                .expect("a result of the thread")
        }
    }

    #[test]
    fn a_valid_file_with_the_share_on_or_off() {
        let f = folders("valid", Some(ONE_PORT));
        assert_eq!(
            read(&f.run, &f.state),
            known(&[("eth0", [192, 168, 0, 112])], true)
        );
        std::fs::write(f.state.join(SHARE_OFF_FILE), b"").expect("the switch");
        assert_eq!(
            read(&f.run, &f.state),
            known(&[("eth0", [192, 168, 0, 112])], false)
        );
    }

    /// The reason of an unknown status.
    fn reason(f: &Folders) -> String {
        match read(&f.run, &f.state) {
            NetworkInfo::Unknown { reason } => reason,
            known => panic!("not unknown: {known:?}"),
        }
    }

    #[test]
    fn a_missing_damaged_newer_or_large_file_is_unknown_with_its_reason() {
        let missing = reason(&folders("missing", None));
        assert!(missing.starts_with("cannot read "), "{missing}");
        assert!(missing.contains("network.toml"), "{missing}");

        let damaged = reason(&folders("damaged", Some("version = 1\nhost_name = \n")));
        assert!(damaged.contains("does not parse"), "{damaged}");

        let newer = reason(&folders(
            "newer",
            Some("version = 2\nhost_name = \"neptron\"\n"),
        ));
        assert!(newer.ends_with("this build reads version 1"), "{newer}");

        let padding = "#".repeat(NETWORK_FILE_MAX_BYTES);
        let large = reason(&folders("large", Some(&format!("{NO_ADDRESS}{padding}\n"))));
        assert!(large.ends_with("is larger than 65536 bytes"), "{large}");
    }

    #[test]
    fn the_watch_sends_only_a_changed_result() {
        let f = folders("watch", Some(NO_ADDRESS));
        let watch = NetworkWatch::start(f.run.clone(), f.state.clone()).expect("the thread");
        assert_eq!(watch.poll(), None, "nothing before a request");
        watch.request_queued();
        assert_eq!(watch.next(), known(&[], true));
        watch.request_queued();
        std::fs::write(f.run.join(STATUS_FILE), ONE_PORT).expect("a lease");
        watch.request_queued();
        assert_eq!(
            watch.next(),
            known(&[("eth0", [192, 168, 0, 112])], true),
            "the unchanged read sent nothing"
        );
        std::fs::remove_file(f.run.join(STATUS_FILE)).expect("the file goes");
        watch.request_queued();
        assert!(matches!(watch.next(), NetworkInfo::Unknown { .. }));
        drop(watch);
    }

    #[test]
    fn a_request_never_waits() {
        let f = folders("merge", Some(NO_ADDRESS));
        let watch = NetworkWatch::start(f.run.clone(), f.state.clone()).expect("the thread");
        for _ in 0..100 {
            watch.request();
        }
        assert_eq!(watch.next(), known(&[], true));
    }
}
