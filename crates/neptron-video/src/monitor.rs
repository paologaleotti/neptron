use neptron_platform::EdidProfile;
use neptron_types::{GpuCaps, MonitorProfile};

/// The monitor the engine drives: a profile, and where it comes from.
///
/// A profile built from EDID carries the monitor's highest pixel clock
/// beside it. The engine lowers the GPU caps to it before every call of the
/// policy and of the validator, so no caller can forget it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Monitor {
    /// A preset of `data/monitors` or a custom profile.
    Profile(MonitorProfile),
    /// A profile built from the EDID of a PC CRT.
    Edid(EdidProfile),
}

impl Monitor {
    /// The monitor profile.
    pub fn profile(&self) -> &MonitorProfile {
        match self {
            Self::Profile(profile) => profile,
            Self::Edid(edid) => &edid.profile,
        }
    }

    /// The caps the policy and the validator get: `gpu` with the EDID
    /// pixel clock limit applied, or `gpu` unchanged for a profile.
    pub fn caps_for(&self, gpu: &GpuCaps) -> GpuCaps {
        match self {
            Self::Profile(_) => *gpu,
            Self::Edid(edid) => edid.limit_caps(gpu),
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_platform::TimingFormula;
    use neptron_types::MonitorKind;

    use super::*;

    const GPU: GpuCaps = GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 400_000_000,
        interlace: false,
        flip_per_field: false,
        max_width: 4_096,
        max_height: 4_096,
    };

    fn profile() -> MonitorProfile {
        MonitorProfile {
            id: "edid_npt_0071".to_owned(),
            kind: MonitorKind::Crt { ranges: Vec::new() },
        }
    }

    #[test]
    fn an_edid_monitor_lowers_the_highest_pixel_clock() {
        let monitor = Monitor::Edid(EdidProfile {
            profile: profile(),
            pclock_max_hz: 140_000_000,
            formula: TimingFormula::DefaultGtf,
        });
        let caps = monitor.caps_for(&GPU);
        assert_eq!(caps.pclock_max_hz, 140_000_000);
        assert_eq!(caps.pclock_min_hz, GPU.pclock_min_hz);
        assert_eq!(monitor.profile(), &profile());
    }

    #[test]
    fn an_edid_limit_above_the_gpu_keeps_the_gpu_limit() {
        let monitor = Monitor::Edid(EdidProfile {
            profile: profile(),
            pclock_max_hz: 500_000_000,
            formula: TimingFormula::DefaultGtf,
        });
        assert_eq!(monitor.caps_for(&GPU), GPU);
    }

    #[test]
    fn a_profile_keeps_the_gpu_caps() {
        assert_eq!(Monitor::Profile(profile()).caps_for(&GPU), GPU);
    }
}
