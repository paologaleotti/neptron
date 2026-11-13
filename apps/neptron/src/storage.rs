//! The status of the state and share partitions, as the storage init script
//! of the image found them at boot.
//!
//! The script mounts a RAM folder in place of a partition that fails its
//! check or its mount, so the frontend runs, and writes `storage.toml`
//! into the run folder. Everything written to a RAM folder is lost at
//! power-off, and the player must know it.

use std::io;
use std::path::Path;

use serde::Deserialize;

/// The status file in the run folder.
pub const STATUS_FILE: &str = "storage.toml";

/// What the script mounted at one mount point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mounted {
    /// The partition: what is written is kept.
    Partition,
    /// A RAM folder: what is written is lost at power-off.
    Ram,
}

/// The status of both mount points.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StorageStatus {
    /// What is at `/state`.
    pub state: Mounted,
    /// Why `/state` is a RAM folder.
    #[serde(default)]
    pub state_reason: Option<String>,
    /// What is at `/share`.
    pub share: Mounted,
    /// Why `/share` is a RAM folder.
    #[serde(default)]
    pub share_reason: Option<String>,
    /// `format` when a format that the user confirms may make the share
    /// partition usable; the four fields after it name the partition.
    #[serde(default)]
    pub share_fix: Option<String>,
    /// The device of the partition that the offer names.
    #[serde(default)]
    pub share_fix_device: Option<String>,
    /// Its first sector, in 512-byte sectors.
    #[serde(default)]
    pub share_fix_start: Option<u64>,
    /// Its size, in 512-byte sectors.
    #[serde(default)]
    pub share_fix_size: Option<u64>,
    /// Its MBR type, in hexadecimal digits as sfdisk prints it.
    #[serde(default)]
    pub share_fix_type: Option<String>,
    /// What came of a format request at this boot: `done`, `refused` or
    /// `failed`. `None` when there was no request.
    #[serde(default)]
    pub share_format: Option<String>,
    /// Why the request was not carried out.
    #[serde(default)]
    pub share_format_reason: Option<String>,
}

/// The notice after a format that the user asked for: an Info notice,
/// where the other notices of this file are warnings.
pub const FORMATTED_NOTICE: &str = "The share partition is formatted.";

/// The value of `share_fix` that offers a format.
pub const FIX_FORMAT: &str = "format";

/// The file on the state partition that asks the storage script for the
/// format of one partition at the next boot.
pub const REQUEST_FILE: &str = "share-format-request";

/// A format of the share partition that the user may ask for: the
/// partition as the storage script found it at this boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOffer {
    /// The device, for example `/dev/sda4`.
    pub device: String,
    /// The first sector, in 512-byte sectors.
    pub start: u64,
    /// The size, in 512-byte sectors.
    pub sectors: u64,
    /// The MBR type, in hexadecimal digits.
    pub kind: String,
}

/// The status file cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The file cannot be read.
    #[error("cannot read the storage status {path}: {source}")]
    Read {
        /// The file.
        path: String,
        /// The error.
        #[source]
        source: io::Error,
    },
    /// The file does not parse.
    #[error("the storage status {path} does not parse: {reason}")]
    Parse {
        /// The file.
        path: String,
        /// The parser's message.
        reason: String,
    },
}

impl StorageStatus {
    /// Reads `storage.toml` in the run folder `run`. `None` when the file
    /// is not there: a desktop run, or an image with no such script.
    ///
    /// # Errors
    ///
    /// [`StorageError`] for a file that cannot be read or parsed.
    pub fn read(run: &Path) -> Result<Option<Self>, StorageError> {
        let path = run.join(STATUS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(StorageError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        toml::from_str(&text)
            .map(Some)
            .map_err(|e| StorageError::Parse {
                path: path.display().to_string(),
                reason: e.to_string(),
            })
    }

    /// True when `/state` is a RAM folder: settings, saves and the guard
    /// record are lost at power-off.
    pub fn state_in_ram(&self) -> bool {
        self.state == Mounted::Ram
    }

    /// The format offer, when the script made one for a share in a RAM
    /// folder and every field of it is well formed. A malformed offer is
    /// no offer: the request carries these values to the script.
    pub fn format_offer(&self) -> Option<FormatOffer> {
        if self.share != Mounted::Ram || self.share_fix.as_deref() != Some(FIX_FORMAT) {
            return None;
        }
        let device = self.share_fix_device.clone()?;
        let kind = self.share_fix_type.clone()?;
        let start = self.share_fix_start?;
        let sectors = self.share_fix_size?;
        let device_ok = device.starts_with("/dev/")
            && device.len() <= DEVICE_MAX_BYTES
            && device
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'/' || b == b'_' || b == b'-');
        let kind_ok = (1..=2).contains(&kind.len()) && kind.bytes().all(|b| b.is_ascii_hexdigit());
        (device_ok && kind_ok && start > 0 && sectors > 0).then_some(FormatOffer {
            device,
            start,
            sectors,
            kind,
        })
    }

    /// The notices for the player, one for each RAM folder, and one for
    /// the result of a format request.
    pub fn notices(&self) -> Vec<String> {
        let mut notices = Vec::new();
        if self.state == Mounted::Ram {
            notices.push(format!(
                "The state partition is not in use ({}). Settings, saves and display choices are lost at power-off.",
                reason(self.state_reason.as_deref())
            ));
        }
        match self.share_format.as_deref() {
            Some("done") => notices.push(FORMATTED_NOTICE.to_owned()),
            Some(_) => notices.push(format!(
                "The share partition was not formatted ({}).",
                reason(self.share_format_reason.as_deref())
            )),
            None => {}
        }
        if self.share == Mounted::Ram {
            let fix = if self.format_offer().is_some() {
                " Settings > System can format it."
            } else {
                ""
            };
            notices.push(format!(
                "The share partition is not in use ({}). The game folder is empty, and files copied to it are lost at power-off.{fix}",
                reason(self.share_reason.as_deref())
            ));
        }
        notices
    }
}

impl FormatOffer {
    /// The size for the player, in decimal units with one decimal, as a
    /// disk label shows it: `56.6 GB`, `512.0 MB`.
    pub fn size_text(&self) -> String {
        let bytes = self.sectors.saturating_mul(SECTOR_BYTES);
        let (unit, name) = if bytes >= GB_BYTES {
            (GB_BYTES, "GB")
        } else {
            (MB_BYTES, "MB")
        };
        let tenths = bytes / (unit / 10);
        format!("{}.{} {name}", tenths / 10, tenths % 10)
    }

    /// The request file for this partition, which the storage script reads
    /// at the next boot: one `key=value` line each.
    pub fn request_text(&self) -> String {
        format!(
            "device={}\nstart={}\nsize={}\ntype={}\n",
            self.device, self.start, self.sectors, self.kind
        )
    }
}

/// Bytes in one sector of the numbers of the storage script.
const SECTOR_BYTES: u64 = 512;
/// Bytes in a decimal megabyte, as a disk label counts them.
const MB_BYTES: u64 = 1_000_000;
/// Bytes in a decimal gigabyte, as a disk label counts them.
const GB_BYTES: u64 = 1_000_000_000;
/// The longest device path that the request carries.
const DEVICE_MAX_BYTES: usize = 64;

fn reason(text: Option<&str>) -> &str {
    text.filter(|t| !t.is_empty()).unwrap_or("no reason given")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, text: Option<&str>) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-storage-{}-{name}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the folder");
        if let Some(text) = text {
            std::fs::write(dir.join(STATUS_FILE), text).expect("the status");
        }
        dir
    }

    #[test]
    fn no_file_is_no_status() {
        let dir = folder("none", None);
        assert_eq!(StorageStatus::read(&dir).expect("read"), None);
    }

    #[test]
    fn both_partitions_give_no_notice() {
        let dir = folder("ok", Some("state = \"partition\"\nshare = \"partition\"\n"));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert!(!status.state_in_ram());
        assert!(status.notices().is_empty());
    }

    #[test]
    fn a_ram_folder_gives_a_notice_with_its_reason() {
        let text = "state = \"ram\"\nstate_reason = \"e2fsck /dev/sda3 left errors (exit 4)\"\n\
                    share = \"ram\"\nshare_reason = \"no share partition\"\n";
        let dir = folder("ram", Some(text));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert!(status.state_in_ram());
        let notices = status.notices();
        assert_eq!(notices.len(), 2);
        assert!(notices[0].contains("(e2fsck /dev/sda3 left errors (exit 4))"));
        assert!(notices[0].contains("lost at power-off"));
        assert!(notices[1].contains("(no share partition)"));
    }

    fn offer_text(device: &str, kind: &str) -> String {
        format!(
            "state = \"partition\"\nshare = \"ram\"\nshare_reason = \"/dev/sda4 is empty\"\n\
             share_fix = \"format\"\nshare_fix_device = \"{device}\"\nshare_fix_start = 10240\n\
             share_fix_size = 120117248\nshare_fix_type = \"{kind}\"\n"
        )
    }

    #[test]
    fn an_offer_names_the_partition_and_its_size() {
        let dir = folder("offer", Some(&offer_text("/dev/sda4", "7")));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        let offer = status.format_offer().expect("an offer");
        assert_eq!(
            offer,
            FormatOffer {
                device: "/dev/sda4".to_owned(),
                start: 10240,
                sectors: 120_117_248,
                kind: "7".to_owned(),
            }
        );
        assert_eq!(offer.size_text(), "61.5 GB");
        assert_eq!(
            offer.request_text(),
            "device=/dev/sda4\nstart=10240\nsize=120117248\ntype=7\n"
        );
        let notices = status.notices();
        assert_eq!(notices.len(), 1);
        assert!(
            notices[0].ends_with("lost at power-off. Settings > System can format it."),
            "{notices:?}"
        );
    }

    #[test]
    fn the_size_is_in_decimal_units_with_one_decimal() {
        let offer = |sectors| FormatOffer {
            device: "/dev/sda4".to_owned(),
            start: 1,
            sectors,
            kind: "7".to_owned(),
        };
        assert_eq!(offer(110_546_944).size_text(), "56.6 GB");
        assert_eq!(offer(1_000_000_000 / 512 + 1).size_text(), "1.0 GB");
        assert_eq!(offer(1_000_000).size_text(), "512.0 MB");
        assert_eq!(offer(131_072).size_text(), "67.1 MB");
    }

    #[test]
    fn a_malformed_or_partial_offer_is_no_offer() {
        for (device, kind) in [
            ("sda4", "7"),
            ("/dev/sda4\\nstart=0", "7"),
            ("/dev/sda 4", "7"),
            ("/dev/sda4", "x7"),
            ("/dev/sda4", "783"),
            ("/dev/sda4", ""),
        ] {
            let dir = folder("bad-offer", Some(&offer_text(device, kind)));
            let status = StorageStatus::read(&dir).expect("read").expect("a status");
            assert_eq!(status.format_offer(), None, "{device:?} {kind:?}");
            assert!(!status.notices()[0].contains("Settings > System"));
        }
        let partial = "state = \"partition\"\nshare = \"ram\"\nshare_fix = \"format\"\n";
        let dir = folder("partial-offer", Some(partial));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert_eq!(status.format_offer(), None);
        let mounted =
            offer_text("/dev/sda4", "7").replace("share = \"ram\"", "share = \"partition\"");
        let dir = folder("mounted-offer", Some(&mounted));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert_eq!(
            status.format_offer(),
            None,
            "a mounted share needs no format"
        );
    }

    #[test]
    fn the_result_of_a_request_is_a_notice() {
        let done = "state = \"partition\"\nshare = \"partition\"\nshare_format = \"done\"\n";
        let dir = folder("format-done", Some(done));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert_eq!(status.notices(), ["The share partition is formatted."]);
        let refused = "state = \"partition\"\nshare = \"partition\"\nshare_format = \"refused\"\n\
                       share_format_reason = \"it holds a filesystem\"\n";
        let dir = folder("format-refused", Some(refused));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert_eq!(
            status.notices(),
            ["The share partition was not formatted (it holds a filesystem)."]
        );
    }

    #[test]
    fn a_bad_file_is_an_error() {
        let dir = folder("bad", Some("state = \"floppy\"\n"));
        assert!(matches!(
            StorageStatus::read(&dir),
            Err(StorageError::Parse { .. })
        ));
    }
}
