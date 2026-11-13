//! The playback device: the trait that lists and opens devices, the rule
//! of `auto`, and the open with its fallback chain.
//!
//! The decision is pure: it reads the trait's answers and returns the sink,
//! the list, the choice of `auto` and the notices. It never writes the
//! setting.

use std::fmt;

use neptron_backend::AudioSink;

/// The setting value of the automatic choice.
pub const AUTO_OUTPUT: &str = "auto";

/// The driver name of SDL's ALSA backend. For it the list comes from
/// `/proc/asound`, not from SDL.
pub const ALSA_DRIVER: &str = "alsa";

/// The playback devices of an audio backend, and the open of one of them.
pub trait AudioOutputs {
    /// The sink of an open device.
    type Sink: AudioSink;

    /// The backend's driver, for example [`ALSA_DRIVER`].
    fn driver(&self) -> &str;

    /// The playback devices, in menu order. The default device is not in
    /// the list.
    ///
    /// # Errors
    ///
    /// [`OutputError::List`] when the backend cannot list them.
    fn devices(&mut self) -> Result<Vec<OutputDevice>, OutputError>;

    /// Opens the default device (`None`) or the device with this name.
    ///
    /// # Errors
    ///
    /// [`OutputError::Open`] or [`OutputError::NotFound`] with the reason.
    fn open(&mut self, device: Option<&str>) -> Result<Self::Sink, OutputError>;
}

/// An error of an [`AudioOutputs`] backend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OutputError {
    /// The backend cannot list its playback devices.
    #[error("the playback devices cannot be listed: {reason}")]
    List {
        /// The backend's message.
        reason: String,
    },
    /// No playback device has this name.
    #[error("no playback device is named \"{name}\"")]
    NotFound {
        /// The name asked for.
        name: String,
    },
    /// The device did not open.
    #[error("{reason}")]
    Open {
        /// The name of the device, or `default`.
        device: String,
        /// The backend's message.
        reason: String,
    },
}

/// What is known of a monitor on a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Monitor {
    /// Not an HDMI or DisplayPort output.
    NotDigital,
    /// A monitor that takes audio is on this output; its name when known.
    Present(Option<String>),
    /// No monitor that takes audio is on this output.
    Absent,
    /// A monitor may be on this output; the backend cannot tell.
    Unknown,
}

/// One playback device, as the menu shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    /// The name the setting stores, such as `hw:CARD=PCH,DEV=3`.
    pub name: String,
    /// The short label of the menu, such as `HDA Intel PCH HDMI/DP 1`.
    pub label: String,
    /// The detail of the option, such as `HDMI 0` or `Nothing connected`.
    pub detail: String,
    /// What is known of a monitor on it.
    pub monitor: Monitor,
}

/// The device that opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenedDevice {
    /// The system default device.
    Default,
    /// The device with this name.
    Named(String),
}

/// The device that `auto` takes, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoChoice {
    /// The HDMI or DisplayPort device, or `None` for the default device.
    pub device: Option<String>,
    /// Why, in words for the log.
    pub reason: String,
}

/// What the player must know after the open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputNotice {
    /// The named device is not in the list; `auto`'s choice plays.
    Missing {
        /// The label of the device.
        label: String,
    },
    /// The named device did not open; `auto`'s choice plays.
    Failed {
        /// The label of the device.
        label: String,
        /// Why it did not open.
        reason: String,
    },
    /// No device opened: the run has no sound.
    NoSound {
        /// Why the last device did not open.
        reason: String,
    },
}

/// The result of [`open_output`].
#[derive(Debug)]
pub struct OpenedOutput<S> {
    /// The sink, or `None` when no device opened.
    pub sink: Option<S>,
    /// The device that opened.
    pub opened: Option<OpenedDevice>,
    /// The devices of the backend, in menu order.
    pub devices: Vec<OutputDevice>,
    /// Why the list is empty, when the backend could not list.
    pub list_error: Option<OutputError>,
    /// The choice of `auto`, when the setting is `auto` or the fallback
    /// came to it.
    pub auto: Option<AutoChoice>,
    /// The notices for the player, in order.
    pub notices: Vec<OutputNotice>,
}

impl fmt::Display for OutputNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { label } => write!(
                f,
                "The audio output \"{label}\" is not connected. The default output plays."
            ),
            Self::Failed { label, reason } => write!(
                f,
                "The audio output \"{label}\" did not open ({reason}). The default output plays."
            ),
            Self::NoSound { reason } => {
                write!(f, "No sound: the audio device did not open ({reason}).")
            }
        }
    }
}

impl fmt::Display for OpenedDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::Named(name) => write!(f, "\"{name}\""),
        }
    }
}

/// The label of `name`: its device's label, or the name itself.
pub fn output_label(devices: &[OutputDevice], name: &str) -> String {
    devices
        .iter()
        .find(|d| d.name == name)
        .map_or_else(|| name.to_owned(), |d| d.label.clone())
}

/// The rule of `auto`: the HDMI or DisplayPort device when exactly one
/// device has a monitor that takes audio, else the default device. A CRT
/// behind a DAC gives no ELD, so it gets the default device.
pub fn auto_choice(devices: &[OutputDevice]) -> AutoChoice {
    let present: Vec<&OutputDevice> = devices
        .iter()
        .filter(|d| matches!(d.monitor, Monitor::Present(_)))
        .collect();
    if let [device] = present[..] {
        return AutoChoice {
            device: Some(device.name.clone()),
            reason: format!(
                "the only HDMI/DP output with a monitor that takes audio ({})",
                device.label
            ),
        };
    }
    let reason = if present.len() > 1 {
        format!(
            "default output, {} HDMI/DP outputs have a monitor",
            present.len()
        )
    } else if devices.iter().any(|d| d.monitor == Monitor::Unknown) {
        "default output, an HDMI/DP output may have a monitor but /proc does not show which one"
            .to_owned()
    } else {
        "default output, no HDMI/DP output has a monitor that takes audio".to_owned()
    };
    AutoChoice {
        device: None,
        reason,
    }
}

/// Opens the device that `setting` names (`auto` or a device name), along
/// the fallback chain: the named device when it is listed, else or when it
/// fails the device `auto` takes, then the default device, then no sound.
/// The setting is never changed, so the next start tries the named device
/// again.
pub fn open_output<O: AudioOutputs>(outputs: &mut O, setting: &str) -> OpenedOutput<O::Sink> {
    let (devices, list_error) = match outputs.devices() {
        Ok(devices) => (devices, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let mut notices = Vec::new();
    if setting != AUTO_OUTPUT {
        let label = output_label(&devices, setting);
        if devices.iter().any(|d| d.name == setting) {
            match outputs.open(Some(setting)) {
                Ok(sink) => {
                    return OpenedOutput {
                        sink: Some(sink),
                        opened: Some(OpenedDevice::Named(setting.to_owned())),
                        devices,
                        list_error,
                        auto: None,
                        notices,
                    };
                }
                Err(e) => notices.push(OutputNotice::Failed {
                    label,
                    reason: e.to_string(),
                }),
            }
        } else {
            notices.push(OutputNotice::Missing { label });
        }
    }
    let auto = auto_choice(&devices);
    let mut last_error = None;
    if let Some(device) = auto.device.as_deref().filter(|d| *d != setting) {
        match outputs.open(Some(device)) {
            Ok(sink) => {
                return OpenedOutput {
                    sink: Some(sink),
                    opened: Some(OpenedDevice::Named(device.to_owned())),
                    devices,
                    list_error,
                    auto: Some(auto),
                    notices,
                };
            }
            Err(e) => last_error = Some(e),
        }
    }
    let (sink, opened) = match outputs.open(None) {
        Ok(sink) => (Some(sink), Some(OpenedDevice::Default)),
        Err(e) => {
            let reason = last_error.map_or_else(|| e.to_string(), |first| format!("{first}; {e}"));
            notices.push(OutputNotice::NoSound { reason });
            (None, None)
        }
    };
    OpenedOutput {
        sink,
        opened,
        devices,
        list_error,
        auto: Some(auto),
        notices,
    }
}

#[cfg(test)]
mod tests {
    use neptron_testkit::{FakeAudio, SimClock};

    use super::*;

    /// A backend on the fakes: a list of devices, the ones that refuse to
    /// open, and a record of every open.
    struct FakeOutputs {
        devices: Result<Vec<OutputDevice>, OutputError>,
        refused: Vec<Option<String>>,
        opens: Vec<Option<String>>,
        clock: SimClock,
    }

    impl FakeOutputs {
        fn new(devices: Vec<OutputDevice>) -> Self {
            Self {
                devices: Ok(devices),
                refused: Vec::new(),
                opens: Vec::new(),
                clock: SimClock::new(),
            }
        }

        fn refuse(mut self, device: Option<&str>) -> Self {
            self.refused.push(device.map(str::to_owned));
            self
        }
    }

    impl AudioOutputs for FakeOutputs {
        type Sink = FakeAudio;

        fn driver(&self) -> &str {
            ALSA_DRIVER
        }

        fn devices(&mut self) -> Result<Vec<OutputDevice>, OutputError> {
            self.devices.clone()
        }

        fn open(&mut self, device: Option<&str>) -> Result<FakeAudio, OutputError> {
            let device = device.map(str::to_owned);
            self.opens.push(device.clone());
            if self.refused.contains(&device) {
                return Err(OutputError::Open {
                    device: device.unwrap_or_else(|| "default".to_owned()),
                    reason: "device busy".to_owned(),
                });
            }
            Ok(FakeAudio::new(self.clock.clone(), 48_000, 6_144))
        }
    }

    const ANALOG: &str = "hw:CARD=PCH,DEV=0";
    const DP1: &str = "hw:CARD=PCH,DEV=3";
    const DP2: &str = "hw:CARD=PCH,DEV=7";

    fn device(name: &str, label: &str, monitor: Monitor) -> OutputDevice {
        OutputDevice {
            name: name.to_owned(),
            label: label.to_owned(),
            detail: String::new(),
            monitor,
        }
    }

    /// An analog output and two HDMI outputs with the given monitors.
    fn card(first: Monitor, second: Monitor) -> Vec<OutputDevice> {
        vec![
            device(ANALOG, "PCH analog", Monitor::NotDigital),
            device(DP1, "PCH HDMI/DP 1", first),
            device(DP2, "PCH HDMI/DP 2", second),
        ]
    }

    fn named(name: &str) -> Option<String> {
        Some(name.to_owned())
    }

    #[test]
    fn auto_takes_the_hdmi_output_only_when_exactly_one_has_a_monitor() {
        let none = auto_choice(&card(Monitor::Absent, Monitor::Absent));
        assert_eq!(none.device, None);
        assert_eq!(
            none.reason,
            "default output, no HDMI/DP output has a monitor that takes audio"
        );
        let one = auto_choice(&card(Monitor::Present(named("DELL")), Monitor::Absent));
        assert_eq!(one.device.as_deref(), Some(DP1));
        assert_eq!(
            one.reason,
            "the only HDMI/DP output with a monitor that takes audio (PCH HDMI/DP 1)"
        );
        let two = auto_choice(&card(Monitor::Present(None), Monitor::Present(None)));
        assert_eq!(two.device, None);
        assert_eq!(
            two.reason,
            "default output, 2 HDMI/DP outputs have a monitor"
        );
        let unknown = auto_choice(&card(Monitor::Unknown, Monitor::Unknown));
        assert_eq!(unknown.device, None);
        assert!(unknown.reason.contains("/proc does not show"));
        assert_eq!(auto_choice(&[]).device, None);
    }

    #[test]
    fn auto_on_a_valid_eld_follows_the_alsa_list() {
        use crate::{AlsaText, alsa_outputs};

        let text = |valid: u8| AlsaText {
            cards: " 0 [NVidia         ]: HDA-Intel - HDA NVidia\n".to_owned(),
            pcm: "00-03: HDMI 0 : HDMI 0 : playback 1\n".to_owned(),
            elds: vec![(
                0,
                format!("monitor_present\t1\neld_valid\t{valid}\nmonitor_name\tLG TV\n"),
            )],
        };
        let valid = auto_choice(&alsa_outputs(&text(1)));
        assert_eq!(valid.device.as_deref(), Some("hw:CARD=NVidia,DEV=3"));
        let no_audio = auto_choice(&alsa_outputs(&text(0)));
        assert_eq!(
            no_audio.device, None,
            "eld_valid 0 is no monitor with audio"
        );
    }

    #[test]
    fn auto_opens_its_hdmi_choice_with_no_notice() {
        let mut outputs = FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent));
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert!(opened.sink.is_some());
        assert_eq!(opened.opened, Some(OpenedDevice::Named(DP1.to_owned())));
        assert_eq!(opened.notices, []);
        assert_eq!(outputs.opens, [named(DP1)]);
    }

    #[test]
    fn auto_with_no_monitor_opens_the_default_device() {
        let mut outputs = FakeOutputs::new(card(Monitor::Unknown, Monitor::Unknown));
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(opened.notices, []);
        assert_eq!(opened.auto.unwrap().device, None);
        assert_eq!(outputs.opens, [None]);
    }

    #[test]
    fn a_listed_device_opens_by_its_name() {
        let mut outputs = FakeOutputs::new(card(Monitor::Unknown, Monitor::Unknown));
        let opened = open_output(&mut outputs, DP2);
        assert_eq!(opened.opened, Some(OpenedDevice::Named(DP2.to_owned())));
        assert_eq!(opened.notices, []);
        assert_eq!(opened.auto, None);
        assert_eq!(outputs.opens, [named(DP2)]);
    }

    #[test]
    fn an_absent_device_falls_back_to_autos_choice_with_a_notice() {
        let mut outputs = FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent));
        let setting = "hw:CARD=Device,DEV=0".to_owned();
        let opened = open_output(&mut outputs, &setting);
        assert_eq!(opened.opened, Some(OpenedDevice::Named(DP1.to_owned())));
        assert_eq!(
            opened.notices[0].to_string(),
            "The audio output \"hw:CARD=Device,DEV=0\" is not connected. The default output plays."
        );
        assert_eq!(
            outputs.opens,
            [named(DP1)],
            "the absent device is not tried"
        );
        assert_eq!(setting, "hw:CARD=Device,DEV=0");
    }

    #[test]
    fn a_device_that_does_not_open_falls_back_with_its_reason() {
        let mut outputs =
            FakeOutputs::new(card(Monitor::Absent, Monitor::Absent)).refuse(Some(DP2));
        let opened = open_output(&mut outputs, DP2);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(
            opened.notices[0].to_string(),
            "The audio output \"PCH HDMI/DP 2\" did not open (device busy). The default output plays."
        );
        assert_eq!(outputs.opens, [named(DP2), None]);
    }

    #[test]
    fn autos_hdmi_choice_that_fails_gives_way_to_the_default_device() {
        let mut outputs =
            FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent)).refuse(Some(DP1));
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(opened.notices, []);
        assert_eq!(outputs.opens, [named(DP1), None]);
    }

    #[test]
    fn with_no_device_at_all_the_run_has_no_sound() {
        let mut outputs = FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent))
            .refuse(Some(DP1))
            .refuse(None);
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert!(opened.sink.is_none());
        assert_eq!(opened.opened, None);
        assert_eq!(
            opened.notices[0].to_string(),
            "No sound: the audio device did not open (device busy; device busy)."
        );
        let mut empty = FakeOutputs::new(Vec::new()).refuse(None);
        let opened = open_output(&mut empty, AUTO_OUTPUT);
        assert_eq!(
            opened.notices[0].to_string(),
            "No sound: the audio device did not open (device busy)."
        );
    }

    #[test]
    fn a_list_that_fails_falls_back_to_the_default() {
        let mut outputs = FakeOutputs::new(Vec::new());
        outputs.devices = Err(OutputError::List {
            reason: "no backend".to_owned(),
        });
        let opened = open_output(&mut outputs, DP1);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert!(opened.devices.is_empty());
        assert!(opened.list_error.is_some());
        assert!(matches!(opened.notices[..], [OutputNotice::Missing { .. }]));
    }

    #[test]
    fn a_lost_device_plays_on_without_blocking_the_pipeline() {
        use crate::{AudioPipeline, CoreTiming, Resampling};

        let mut outputs = FakeOutputs::new(card(Monitor::Absent, Monitor::Absent));
        let clock = outputs.clock.clone();
        let opened = open_output(&mut outputs, ANALOG);
        let sink = opened.sink.expect("the sink");
        let core = CoreTiming {
            sample_rate_mhz: 48_000_000,
            frame_rate_mhz: 60_000,
        };
        let mut pipeline =
            AudioPipeline::new(sink.clone(), core, 60_000, Resampling::Polynomial).unwrap();
        for _ in 0..120 {
            pipeline.push_samples(&[1_000; 800 * 2]);
            pipeline.end_frame();
            clock.advance(16_666_667);
        }
        // The device is gone: nothing reads the queue any more, and the
        // queue fills up. The frames go on and count what they lose.
        let level = sink.queued_frames();
        for _ in 0..600 {
            pipeline.push_samples(&[1_000; 800 * 2]);
            pipeline.end_frame();
        }
        assert!(level > 0);
        assert_eq!(pipeline.stats().frames, 720);
        assert!(pipeline.stats().overrun_frames > 0);
        assert_eq!(sink.queued_frames(), 6_144);
    }
}
