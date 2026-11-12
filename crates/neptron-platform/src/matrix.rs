//! The GPU capability table, `data/hardware/matrix.toml`, and the lookup
//! from a detected GPU and connector to its capabilities.

use neptron_types::{GpuCaps, OutputKind};
use serde::Deserialize;
use thiserror::Error;

use crate::gpu::Gpu;
use crate::panel::ANALOG_CONNECTOR_KINDS;

/// The capabilities of a GPU that matches no entry of the table.
///
/// The lowest pixel clock is 25 MHz, the floor that old Intel chipsets and
/// older NVIDIA cards are reported to need, and there is no interlace. The
/// other values are a conservative baseline: the 165 MHz single-link TMDS
/// limit and a 1920 by 1200 mode.
pub const UNKNOWN_GPU_CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: UNKNOWN_GPU_PCLOCK_MIN_HZ,
    pclock_max_hz: 165_000_000,
    interlace: false,
    flip_per_field: false,
    max_width: 1920,
    max_height: 1200,
};

/// The lowest pixel clock of an unknown GPU, in hertz.
pub const UNKNOWN_GPU_PCLOCK_MIN_HZ: u32 = 25_000_000;

/// The versions of the table format that this crate reads.
pub const MATRIX_VERSION: u32 = 1;

/// The parsed and checked capability table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    entries: Vec<GpuEntry>,
}

/// One entry of the table: what it matches and what it gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuEntry {
    /// A stable id, for example `intel_gen9`.
    pub id: String,
    /// A name for people.
    pub name: String,
    /// The support tier: 1, 2 or 3.
    pub tier: u8,
    /// The kernel driver, as `DRIVER=` in the uevent gives it.
    pub driver: String,
    /// The PCI vendor id, when the entry matches PCI device ids.
    pub pci_vendor: Option<u16>,
    /// The PCI device ids the entry matches; empty for any.
    pub pci_devices: Vec<u16>,
    /// The device tree compatible strings the entry matches, any one of
    /// them; empty for any.
    pub of_compatible: Vec<String>,
    /// The connector types the entry matches, for example `DPI`; empty for
    /// any.
    pub connectors: Vec<String>,
    /// What the GPU can drive on these connectors.
    pub caps: GpuCaps,
    /// Known limits and fixes.
    pub quirks: Vec<Quirk>,
    /// The limits of a digital link on some connectors, when a source
    /// gives them. They apply only to an [`OutputKind::Digital`] output; an
    /// analog output, also a DAC on the same connector, keeps `caps`.
    pub digital: Vec<DigitalLimits>,
}

/// The highest pixel clock and the largest mode of a digital link on some
/// connectors of a GPU. The other values of [`GpuEntry::caps`] stay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalLimits {
    /// The connector types, for example `HDMI-A` or `DP`. Never an analog
    /// one.
    pub connectors: Vec<String>,
    /// The highest pixel clock, in hertz.
    pub pclock_max_hz: u32,
    /// The widest mode, in pixels.
    pub max_width: u16,
    /// The tallest mode, in lines.
    pub max_height: u16,
}

/// A known limit or fix of a GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quirk {
    /// A pixel clock below 25 MHz is reported but not proven on this output
    /// path, so the lowest pixel clock stays at 25 MHz.
    LowPixelClockUnproven,
    /// Interlace is reported to work under conditions that are not proven,
    /// so interlace is off.
    InterlaceUnproven,
    /// Interlaced scanout needs linear or X-tiled buffers.
    InterlaceNeedsLinearScanout,
    /// Interlace needs a kernel patch that is not in the stock kernel.
    InterlaceNeedsKernelPatch,
    /// Interlace runs through the RP1 PIO block, which then owns GPIO 1 to
    /// 3 and gives composite sync or vertical sync, not both.
    InterlaceUsesPio,
    /// The low pixel clock and interlace need the 15 kHz kernel patch set.
    #[serde(rename = "needs_15khz_patches")]
    Needs15khzPatches,
    /// The encoder runs at one pixel clock.
    FixedPixelClock,
}

/// Why the table was refused.
#[derive(Debug, Error)]
pub enum MatrixError {
    /// The text is not valid TOML for the format.
    #[error("capability table does not parse: {source}")]
    Parse {
        /// The TOML error, with the line.
        #[source]
        source: toml::de::Error,
    },
    /// The format version is not one this crate reads.
    #[error("capability table version {found} is not supported, only {supported}")]
    Version {
        /// The version in the file.
        found: u32,
        /// The version this crate reads.
        supported: u32,
    },
    /// An entry breaks a rule of the format.
    #[error("capability table entry {id:?}: {rule}")]
    Entry {
        /// The id of the entry.
        id: String,
        /// The rule it breaks.
        rule: &'static str,
    },
    /// Two entries have the same id.
    #[error("capability table has two entries with id {id:?}")]
    DuplicateId {
        /// The id.
        id: String,
    },
    /// Two entries of the same precision can match the same GPU and
    /// connector.
    #[error("capability table entries {first:?} and {second:?} can match the same GPU")]
    Ambiguous {
        /// The id of the first entry.
        first: String,
        /// The id of the second entry.
        second: String,
    },
}

impl Matrix {
    /// The table that ships in `data/hardware/matrix.toml`.
    ///
    /// # Errors
    ///
    /// Returns a [`MatrixError`] if the shipped file breaks a rule. A test
    /// guards against it.
    pub fn builtin() -> Result<Self, MatrixError> {
        Self::parse(BUILTIN_TOML)
    }

    /// Parses and checks a table.
    ///
    /// # Errors
    ///
    /// Returns a [`MatrixError`] when the text does not parse, has another
    /// version, or an entry breaks a rule: a bad id or tier, a PCI vendor
    /// without device ids or the reverse, a lowest pixel clock above the
    /// highest, a zero size, a quirk that contradicts a value, or two
    /// entries that can match the same GPU and connector.
    pub fn parse(text: &str) -> Result<Self, MatrixError> {
        let probe: VersionProbe =
            toml::from_str(text).map_err(|source| MatrixError::Parse { source })?;
        if probe.version != MATRIX_VERSION {
            return Err(MatrixError::Version {
                found: probe.version,
                supported: MATRIX_VERSION,
            });
        }
        let file: MatrixFile =
            toml::from_str(text).map_err(|source| MatrixError::Parse { source })?;
        let entries = file
            .gpu
            .into_iter()
            .map(GpuEntry::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        check_ids(&entries)?;
        check_ambiguity(&entries)?;
        Ok(Self { entries })
    }

    /// The entries, in the order of the file.
    pub fn entries(&self) -> &[GpuEntry] {
        &self.entries
    }

    /// The most precise entry that matches `gpu` on a connector of type
    /// `connector_kind`, or `None`.
    ///
    /// An entry that names PCI device ids or compatible strings is more
    /// precise than one that names only a driver, and an entry that names
    /// connectors is more precise than one that does not, at the same
    /// level.
    pub fn lookup(&self, gpu: &Gpu, connector_kind: &str) -> Option<&GpuEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.matches(gpu, connector_kind))
            .max_by_key(|entry| entry.precision())
    }

    /// The capabilities of `gpu` on a connector of type `connector_kind`
    /// for an output of kind `output`. An analog output gets
    /// [`Matrix::caps_for`], unchanged. A digital output gets the same with
    /// the highest pixel clock and the largest mode of the entry's digital
    /// limits for that connector, when it has them.
    pub fn caps_for_output(&self, gpu: &Gpu, connector_kind: &str, output: OutputKind) -> GpuCaps {
        let Some(entry) = self.lookup(gpu, connector_kind) else {
            return UNKNOWN_GPU_CAPS;
        };
        if output == OutputKind::Analog {
            return entry.caps;
        }
        match entry
            .digital
            .iter()
            .find(|p| p.connectors.iter().any(|c| c == connector_kind))
        {
            Some(limits) => GpuCaps {
                pclock_max_hz: limits.pclock_max_hz,
                max_width: limits.max_width,
                max_height: limits.max_height,
                ..entry.caps
            },
            None => entry.caps,
        }
    }

    /// The capabilities of `gpu` on a connector of type `connector_kind`:
    /// those of its entry, or [`UNKNOWN_GPU_CAPS`].
    pub fn caps_for(&self, gpu: &Gpu, connector_kind: &str) -> GpuCaps {
        self.lookup(gpu, connector_kind)
            .map_or(UNKNOWN_GPU_CAPS, |entry| entry.caps)
    }
}

impl GpuEntry {
    /// The entry matches `gpu` on a connector of type `connector_kind`.
    pub fn matches(&self, gpu: &Gpu, connector_kind: &str) -> bool {
        let driver = gpu.driver.as_deref() == Some(self.driver.as_str());
        let pci = match (self.pci_vendor, gpu.pci_id) {
            (None, _) => true,
            (Some(vendor), Some(id)) => {
                id.vendor == vendor && self.pci_devices.contains(&id.device)
            }
            (Some(_), None) => false,
        };
        let of = self.of_compatible.is_empty()
            || gpu
                .of_compatible
                .iter()
                .any(|compatible| self.of_compatible.contains(compatible));
        let connector =
            self.connectors.is_empty() || self.connectors.iter().any(|kind| kind == connector_kind);
        driver && pci && of && connector
    }

    fn precision(&self) -> (bool, bool) {
        let identity = self.pci_vendor.is_some() || !self.of_compatible.is_empty();
        (identity, !self.connectors.is_empty())
    }

    /// No GPU and connector can match both entries.
    fn disjoint(&self, other: &GpuEntry) -> bool {
        let pci = match (self.pci_vendor, other.pci_vendor) {
            (Some(a), Some(b)) => a != b || !intersects(&self.pci_devices, &other.pci_devices),
            _ => false,
        };
        let both = |a: &[String], b: &[String]| !a.is_empty() && !b.is_empty() && !intersects(a, b);
        self.driver != other.driver
            || pci
            || both(&self.of_compatible, &other.of_compatible)
            || both(&self.connectors, &other.connectors)
    }
}

const BUILTIN_TOML: &str = include_str!("../../../data/hardware/matrix.toml");
const TIERS: std::ops::RangeInclusive<u8> = 1..=3;

#[derive(Deserialize)]
struct VersionProbe {
    version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MatrixFile {
    #[serde(rename = "version")]
    _version: u32,
    #[serde(default)]
    gpu: Vec<EntryFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFile {
    id: String,
    name: String,
    tier: u8,
    driver: String,
    pci_vendor: Option<u16>,
    #[serde(default)]
    pci_devices: Vec<u16>,
    #[serde(default)]
    of_compatible: Vec<String>,
    #[serde(default)]
    connectors: Vec<String>,
    pclock_min_hz: u32,
    pclock_max_hz: u32,
    interlace: bool,
    flip_per_field: bool,
    max_width: u16,
    max_height: u16,
    quirks: Vec<Quirk>,
    /// Added after version 1 was written; optional, so older files read.
    #[serde(default)]
    digital: Vec<DigitalFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DigitalFile {
    connectors: Vec<String>,
    pclock_max_hz: u32,
    max_width: u16,
    max_height: u16,
}

impl TryFrom<EntryFile> for GpuEntry {
    type Error = MatrixError;

    fn try_from(file: EntryFile) -> Result<Self, MatrixError> {
        let entry = GpuEntry {
            caps: GpuCaps {
                pclock_min_hz: file.pclock_min_hz,
                pclock_max_hz: file.pclock_max_hz,
                interlace: file.interlace,
                flip_per_field: file.flip_per_field,
                max_width: file.max_width,
                max_height: file.max_height,
            },
            id: file.id,
            name: file.name,
            tier: file.tier,
            driver: file.driver,
            pci_vendor: file.pci_vendor,
            pci_devices: file.pci_devices,
            of_compatible: file.of_compatible,
            connectors: file.connectors,
            quirks: file.quirks,
            digital: file
                .digital
                .into_iter()
                .map(|p| DigitalLimits {
                    connectors: p.connectors,
                    pclock_max_hz: p.pclock_max_hz,
                    max_width: p.max_width,
                    max_height: p.max_height,
                })
                .collect(),
        };
        if let Some(rule) = broken_rule(&entry) {
            return Err(MatrixError::Entry { id: entry.id, rule });
        }
        Ok(entry)
    }
}

/// The first rule of the format that `entry` breaks, if any.
fn broken_rule(entry: &GpuEntry) -> Option<&'static str> {
    let caps = &entry.caps;
    let has = |quirk| entry.quirks.contains(&quirk);
    let valid_id = !entry.id.is_empty()
        && entry
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let rules = [
        (!valid_id, "id is not lower-case letters, digits and '_'"),
        (!TIERS.contains(&entry.tier), "tier is not 1, 2 or 3"),
        (entry.driver.is_empty(), "driver is empty"),
        (
            entry.pci_vendor.is_some() == entry.pci_devices.is_empty(),
            "pci_vendor and pci_devices must come together",
        ),
        (
            caps.pclock_min_hz > caps.pclock_max_hz || caps.pclock_max_hz == 0,
            "pclock_min_hz is above pclock_max_hz, or pclock_max_hz is 0",
        ),
        (
            caps.max_width == 0 || caps.max_height == 0,
            "max_width or max_height is 0",
        ),
        (
            has(Quirk::InterlaceUnproven) && caps.interlace,
            "interlace_unproven with interlace on",
        ),
        (
            has(Quirk::InterlaceNeedsKernelPatch) && caps.interlace,
            "interlace_needs_kernel_patch with interlace on",
        ),
        (
            has(Quirk::LowPixelClockUnproven) && caps.pclock_min_hz < UNKNOWN_GPU_PCLOCK_MIN_HZ,
            "low_pixel_clock_unproven with a lowest pixel clock below 25 MHz",
        ),
        (
            has(Quirk::FixedPixelClock) && caps.pclock_min_hz != caps.pclock_max_hz,
            "fixed_pixel_clock with two pixel clock limits",
        ),
    ];
    let digital_connector = entry.digital.iter().any(|p| {
        p.connectors.is_empty()
            || p.connectors
                .iter()
                .any(|c| ANALOG_CONNECTOR_KINDS.contains(&c.as_str()))
    });
    let digital_values = entry.digital.iter().any(|p| {
        p.pclock_max_hz < caps.pclock_min_hz.max(1) || p.max_width == 0 || p.max_height == 0
    });
    let digital_rules = [
        (
            digital_connector,
            "a digital limit names no connector, or an analog connector",
        ),
        (
            digital_values,
            "a digital limit has a pixel clock below pclock_min_hz, or a size of 0",
        ),
    ];
    rules
        .into_iter()
        .chain(digital_rules)
        .find_map(|(broken, rule)| broken.then_some(rule))
}

fn check_ids(entries: &[GpuEntry]) -> Result<(), MatrixError> {
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index].iter().any(|seen| seen.id == entry.id) {
            return Err(MatrixError::DuplicateId {
                id: entry.id.clone(),
            });
        }
    }
    Ok(())
}

/// Refuses two entries of the same precision that can match the same GPU
/// and connector, so that a lookup never depends on the order of the file.
fn check_ambiguity(entries: &[GpuEntry]) -> Result<(), MatrixError> {
    for (index, entry) in entries.iter().enumerate() {
        for other in &entries[index + 1..] {
            if entry.precision() == other.precision() && !entry.disjoint(other) {
                return Err(MatrixError::Ambiguous {
                    first: entry.id.clone(),
                    second: other.id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn intersects<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    a.iter().any(|item| b.contains(item))
}
