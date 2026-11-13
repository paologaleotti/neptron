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

/// What kind of output a device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// An analog output of a sound card. `auto` and the fallback may take
    /// it.
    Analog,
    /// An S/PDIF output of a sound card.
    Digital,
    /// An HDMI or DisplayPort output.
    Hdmi,
    /// A device of another driver, known by its name only.
    Other,
}

/// One playback device, as the menu shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    /// The name the setting stores, such as `hw:CARD=PCH,DEV=3`.
    pub name: String,
    /// The short label of the menu, such as `HDMI/DP 1 (HDA Intel PCH)`.
    pub label: String,
    /// The detail of the option, such as `ALC221 Analog` or `Nothing
    /// connected`.
    pub detail: String,
    /// What is known of a monitor on it.
    pub monitor: Monitor,
    /// What kind of output it is.
    pub kind: OutputKind,
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
    /// The HDMI or DisplayPort device, or the first analog device, or
    /// `None` for the default device.
    pub device: Option<String>,
    /// Why, in words for the log.
    pub reason: String,
}

/// What the player must know after the open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputNotice {
    /// The named device is not in the list.
    Missing {
        /// The label of the device.
        label: String,
        /// Another device plays in its place.
        replaced: bool,
    },
    /// A listed device did not open.
    Failed {
        /// The label of the device.
        label: String,
        /// Why it did not open.
        reason: String,
        /// Another device plays in its place.
        replaced: bool,
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
    /// Every device that did not open, with its error, in the order tried:
    /// the name, or `default`.
    pub failures: Vec<(String, String)>,
}

impl fmt::Display for OutputNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { label, replaced } => {
                write!(f, "The audio output \"{label}\" is not connected.")?;
                if *replaced {
                    f.write_str(" Another output plays.")?;
                }
                Ok(())
            }
            Self::Failed {
                label,
                reason,
                replaced,
            } => {
                write!(f, "The audio output \"{label}\" did not open ({reason}).")?;
                if *replaced {
                    f.write_str(" Another output plays.")?;
                }
                Ok(())
            }
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
/// device has a monitor that takes audio, else the first analog device of
/// the list, else the default device. A CRT behind a DAC gives no ELD, so
/// it gets the analog output. The first analog device and not ALSA's
/// default: a GPU card can be card 0 with no device 0.
pub fn auto_choice(devices: &[OutputDevice]) -> AutoChoice {
    let present: Vec<&OutputDevice> = devices
        .iter()
        .filter(|d| matches!(d.monitor, Monitor::Present(_)))
        .collect();
    if let [device] = present[..] {
        return AutoChoice {
            device: Some(device.name.clone()),
            reason: format!(
                "the only HDMI/DP output with a monitor that takes audio, \"{}\"",
                device.label
            ),
        };
    }
    let why = if present.len() > 1 {
        format!("{} HDMI/DP outputs have a monitor", present.len())
    } else if devices.iter().any(|d| d.monitor == Monitor::Unknown) {
        "an HDMI/DP output may have a monitor but /proc does not show which one".to_owned()
    } else {
        "no HDMI/DP output has a monitor that takes audio".to_owned()
    };
    match first_analog(devices) {
        Some(device) => AutoChoice {
            device: Some(device.name.clone()),
            reason: format!(
                "the first analog output \"{}\", because {why}",
                device.label
            ),
        },
        None => AutoChoice {
            device: None,
            reason: format!("the default output, because {why}"),
        },
    }
}

/// The first analog device of the list: not HDMI, not S/PDIF, and not a
/// device of another driver.
fn first_analog(devices: &[OutputDevice]) -> Option<&OutputDevice> {
    devices.iter().find(|d| d.kind == OutputKind::Analog)
}

/// Opens the device that `setting` names (`auto` or a device name), along
/// the fallback chain: the named device when it is listed, else or when it
/// fails the device `auto` takes, then the first analog device, then the
/// default device, then no sound. A device already tried is not tried
/// again. Each step down gives a notice, and every error is kept. The
/// setting is never changed, so the next start tries the named device
/// again.
pub fn open_output<O: AudioOutputs>(outputs: &mut O, setting: &str) -> OpenedOutput<O::Sink> {
    let (devices, list_error) = match outputs.devices() {
        Ok(devices) => (devices, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let mut notices = Vec::new();
    let mut failures = Vec::new();
    let mut tried: Vec<String> = Vec::new();
    let mut auto = None;
    let mut candidates: Vec<String> = Vec::new();
    if setting == AUTO_OUTPUT {
        let choice = auto_choice(&devices);
        candidates.extend(choice.device.clone());
        auto = Some(choice);
    } else if devices.iter().any(|d| d.name == setting) {
        candidates.push(setting.to_owned());
    } else {
        notices.push(OutputNotice::Missing {
            label: output_label(&devices, setting),
            replaced: true,
        });
    }
    let mut index = 0;
    loop {
        if index == candidates.len() {
            // The named device is done: add auto's choice, then the first
            // analog device, once each.
            if auto.is_none() {
                let choice = auto_choice(&devices);
                candidates.extend(choice.device.clone());
                auto = Some(choice);
            }
            if let Some(analog) = first_analog(&devices)
                && !candidates.contains(&analog.name)
            {
                candidates.push(analog.name.clone());
            }
            if index == candidates.len() {
                break;
            }
        }
        let name = candidates[index].clone();
        index += 1;
        if tried.contains(&name) {
            continue;
        }
        tried.push(name.clone());
        match outputs.open(Some(&name)) {
            Ok(sink) => {
                return OpenedOutput {
                    sink: Some(sink),
                    opened: Some(OpenedDevice::Named(name)),
                    devices,
                    list_error,
                    auto,
                    notices,
                    failures,
                };
            }
            Err(e) => {
                notices.push(OutputNotice::Failed {
                    label: output_label(&devices, &name),
                    reason: e.to_string(),
                    replaced: true,
                });
                failures.push((name, e.to_string()));
            }
        }
    }
    let (sink, opened) = match outputs.open(None) {
        Ok(sink) => (Some(sink), Some(OpenedDevice::Default)),
        Err(e) => {
            failures.push(("default".to_owned(), e.to_string()));
            // Nothing plays: no earlier step was replaced, and the earlier
            // errors are in their own notices already.
            for notice in &mut notices {
                if let OutputNotice::Missing { replaced, .. }
                | OutputNotice::Failed { replaced, .. } = notice
                {
                    *replaced = false;
                }
            }
            notices.push(OutputNotice::NoSound {
                reason: e.to_string(),
            });
            (None, None)
        }
    };
    OpenedOutput {
        sink,
        opened,
        devices,
        list_error,
        auto,
        notices,
        failures,
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
        let kind = if monitor == Monitor::NotDigital {
            OutputKind::Analog
        } else {
            OutputKind::Hdmi
        };
        OutputDevice {
            name: name.to_owned(),
            label: label.to_owned(),
            detail: String::new(),
            monitor,
            kind,
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
        assert_eq!(none.device.as_deref(), Some(ANALOG));
        assert_eq!(
            none.reason,
            "the first analog output \"PCH analog\", because no HDMI/DP output has a monitor that takes audio"
        );
        let one = auto_choice(&card(Monitor::Present(named("DELL")), Monitor::Absent));
        assert_eq!(one.device.as_deref(), Some(DP1));
        assert_eq!(
            one.reason,
            "the only HDMI/DP output with a monitor that takes audio, \"PCH HDMI/DP 1\""
        );
        let two = auto_choice(&card(Monitor::Present(None), Monitor::Present(None)));
        assert_eq!(two.device.as_deref(), Some(ANALOG));
        assert_eq!(
            two.reason,
            "the first analog output \"PCH analog\", because 2 HDMI/DP outputs have a monitor"
        );
        let unknown = auto_choice(&card(Monitor::Unknown, Monitor::Unknown));
        assert_eq!(unknown.device.as_deref(), Some(ANALOG));
        assert_eq!(
            unknown.reason,
            "the first analog output \"PCH analog\", because an HDMI/DP output may have a monitor but /proc does not show which one"
        );
        let digital_only = auto_choice(&card(Monitor::Absent, Monitor::Absent)[1..]);
        assert_eq!(digital_only.device, None);
        assert_eq!(
            digital_only.reason,
            "the default output, because no HDMI/DP output has a monitor that takes audio"
        );
        assert_eq!(auto_choice(&[]).device, None);
    }

    #[test]
    fn auto_takes_the_analog_card_after_a_gpu_card_with_no_device_0() {
        use crate::{AlsaText, alsa_outputs};

        let devices = alsa_outputs(&AlsaText {
            cards: " 0 [NVidia         ]: HDA-Intel - HDA NVidia\n 1 [PCH            ]: HDA-Intel - HDA Intel PCH\n".to_owned(),
            pcm: "00-03: HDMI 0 : HDMI 0 : playback 1\n00-07: HDMI 1 : HDMI 1 : playback 1\n01-00: ALC892 Analog : ALC892 Analog : playback 1\n".to_owned(),
            elds: Vec::new(),
        });
        let mut outputs = FakeOutputs::new(devices);
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(
            opened.opened,
            Some(OpenedDevice::Named("hw:CARD=PCH,DEV=0".to_owned()))
        );
        assert_eq!(outputs.opens, [named("hw:CARD=PCH,DEV=0")]);
    }

    #[test]
    fn auto_and_the_fallback_never_take_a_digital_or_other_output() {
        let mut spdif = device("hw:CARD=PCH,DEV=1", "Digital (PCH)", Monitor::NotDigital);
        spdif.kind = OutputKind::Digital;
        let mut other = device(
            "MacBook Pro Speakers",
            "MacBook Pro Speakers",
            Monitor::NotDigital,
        );
        other.kind = OutputKind::Other;
        let choice = auto_choice(&[spdif.clone(), other.clone()]);
        assert_eq!(choice.device, None);
        let mut outputs = FakeOutputs::new(vec![spdif, other]);
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(outputs.opens, [None]);
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
    fn auto_with_no_known_monitor_opens_the_first_analog_output() {
        let mut outputs = FakeOutputs::new(card(Monitor::Unknown, Monitor::Unknown));
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(opened.opened, Some(OpenedDevice::Named(ANALOG.to_owned())));
        assert_eq!(opened.notices, []);
        assert_eq!(opened.auto.unwrap().device.as_deref(), Some(ANALOG));
        assert_eq!(outputs.opens, [named(ANALOG)]);
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
            "The audio output \"hw:CARD=Device,DEV=0\" is not connected. Another output plays."
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
        assert_eq!(opened.opened, Some(OpenedDevice::Named(ANALOG.to_owned())));
        assert_eq!(
            opened.notices[0].to_string(),
            "The audio output \"PCH HDMI/DP 2\" did not open (device busy). Another output plays."
        );
        assert_eq!(
            opened.failures,
            [(DP2.to_owned(), "device busy".to_owned())]
        );
        assert_eq!(outputs.opens, [named(DP2), named(ANALOG)]);
    }

    #[test]
    fn autos_hdmi_choice_that_fails_gives_way_to_the_analog_output_with_a_notice() {
        let mut outputs =
            FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent)).refuse(Some(DP1));
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert_eq!(opened.opened, Some(OpenedDevice::Named(ANALOG.to_owned())));
        assert_eq!(
            opened
                .notices
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            [
                "The audio output \"PCH HDMI/DP 1\" did not open (device busy). Another output plays."
            ]
        );
        assert_eq!(
            opened.failures,
            [(DP1.to_owned(), "device busy".to_owned())]
        );
        assert_eq!(outputs.opens, [named(DP1), named(ANALOG)]);
    }

    #[test]
    fn a_named_device_and_autos_choice_that_fail_report_both_errors() {
        let mut outputs = FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent))
            .refuse(Some(DP2))
            .refuse(Some(DP1));
        let opened = open_output(&mut outputs, DP2);
        assert_eq!(opened.opened, Some(OpenedDevice::Named(ANALOG.to_owned())));
        assert_eq!(opened.notices.len(), 2);
        assert_eq!(
            opened.failures,
            [
                (DP2.to_owned(), "device busy".to_owned()),
                (DP1.to_owned(), "device busy".to_owned()),
            ]
        );
        assert_eq!(outputs.opens, [named(DP2), named(DP1), named(ANALOG)]);
    }

    #[test]
    fn with_no_device_at_all_the_run_has_no_sound() {
        let mut outputs = FakeOutputs::new(card(Monitor::Present(None), Monitor::Absent))
            .refuse(Some(DP1))
            .refuse(Some(ANALOG))
            .refuse(None);
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert!(opened.sink.is_none());
        assert_eq!(opened.opened, None);
        assert_eq!(opened.failures.len(), 3);
        let notices: Vec<String> = opened.notices.iter().map(ToString::to_string).collect();
        assert_eq!(
            notices,
            [
                "The audio output \"PCH HDMI/DP 1\" did not open (device busy).",
                "The audio output \"PCH analog\" did not open (device busy).",
                "No sound: the audio device did not open (device busy).",
            ]
        );
        assert_eq!(outputs.opens, [named(DP1), named(ANALOG), None]);
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
        assert!(matches!(
            opened.notices[..],
            [OutputNotice::Missing { replaced: true, .. }]
        ));
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
