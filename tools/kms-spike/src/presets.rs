//! The monitor presets of `data/monitors`, embedded at build time.

use neptron_types::{CrtRange, MonitorKind, MonitorProfile};

/// One shipped monitor preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// The preset name, equal to its file name in `data/monitors`.
    pub id: &'static str,
    /// The Switchres ranges of the preset, in file order.
    pub ranges: &'static [CrtRange],
}

impl Preset {
    /// The preset as the profile that `validate` takes.
    pub fn profile(&self) -> MonitorProfile {
        MonitorProfile {
            id: self.id.to_owned(),
            kind: MonitorKind::Crt {
                ranges: self.ranges.to_vec(),
            },
        }
    }
}

/// Every shipped preset, sorted by name.
pub fn all() -> &'static [Preset] {
    PRESETS
}

/// The preset with this exact name.
pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

include!(concat!(env!("OUT_DIR"), "/presets.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_file_is_embedded() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/monitors");
        let files = std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "toml")
            })
            .count();
        assert_eq!(all().len(), files);
    }

    #[test]
    fn ids_are_sorted_and_unique() {
        let ids: Vec<&str> = all().iter().map(|p| p.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn ntsc_keeps_its_values() {
        let ntsc = find("ntsc").unwrap();
        assert_eq!(ntsc.ranges.len(), 1);
        let r = ntsc.ranges[0];
        assert_eq!((r.h_min_mhz, r.h_max_mhz), (15_734_260, 15_734_260));
        assert_eq!((r.v_min_mhz, r.v_max_mhz), (59_940, 59_940));
        assert_eq!(
            (r.h_front_porch_ns, r.h_sync_ns, r.h_back_porch_ns),
            (1_500, 4_700, 4_700)
        );
        assert_eq!(
            (r.v_front_porch_ns, r.v_sync_ns, r.v_back_porch_ns),
            (191_000, 191_000, 953_000)
        );
        assert_eq!((r.interlaced_lines_min, r.interlaced_lines_max), (448, 480));
    }

    #[test]
    fn profile_carries_the_id_and_ranges() {
        let p = find("pc_31_120").unwrap().profile();
        assert_eq!(p.id, "pc_31_120");
        match p.kind {
            MonitorKind::Crt { ranges } => assert_eq!(ranges.len(), 2),
            MonitorKind::FlatPanel { .. } => panic!("a preset is a CRT"),
        }
    }

    #[test]
    fn unknown_name_finds_nothing() {
        assert!(find("NTSC").is_none());
        assert!(find("").is_none());
    }
}
