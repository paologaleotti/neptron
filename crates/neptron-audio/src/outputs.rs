//! The playback device: the trait that lists and opens devices, the list
//! the menu shows, and the open with its fallback to the default device.
//!
//! The decision is pure: it reads the trait's answers and returns the sink,
//! the menu list and the notices. It never writes the setting.

use std::fmt;

use neptron_backend::AudioSink;

/// The setting value of the system default device.
pub const AUTO_OUTPUT: &str = "auto";

/// The driver name of SDL's ALSA backend. Its device names are
/// `<card name>:<PCM name>`, and every device of one card opens the same
/// PCM.
pub const ALSA_DRIVER: &str = "alsa";

/// The playback devices of an audio backend, and the open of one of them.
pub trait AudioOutputs {
    /// The sink of an open device.
    type Sink: AudioSink;

    /// The backend's driver, for example [`ALSA_DRIVER`]. The menu labels
    /// depend on it.
    fn driver(&self) -> &str;

    /// The playback devices as raw names, in the backend's order. The
    /// default device is not in the list.
    ///
    /// # Errors
    ///
    /// [`OutputError::List`] when the backend cannot list them.
    fn devices(&mut self) -> Result<Vec<String>, OutputError>;

    /// Opens the default device (`None`) or the device with this raw name.
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
        /// The raw name asked for.
        name: String,
    },
    /// The device did not open.
    #[error("{reason}")]
    Open {
        /// The raw name of the device, or `default`.
        device: String,
        /// The backend's message.
        reason: String,
    },
}

/// One option of the output menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChoice {
    /// The raw name: the value the setting stores.
    pub name: String,
    /// The short label the menu shows.
    pub label: String,
}

/// The device that opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenedDevice {
    /// The system default device.
    Default,
    /// The device with this raw name.
    Named(String),
}

/// What the player must know after the open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputNotice {
    /// The named device is not in the list; the default device plays.
    Missing {
        /// The label of the device.
        label: String,
    },
    /// The named device did not open; the default device plays.
    Failed {
        /// The label of the device.
        label: String,
        /// Why it did not open.
        reason: String,
    },
    /// No device opened: the run has no sound.
    NoSound {
        /// Why the default device did not open.
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
    /// The options of the output menu after `auto`.
    pub choices: Vec<OutputChoice>,
    /// The raw names the backend listed.
    pub devices: Vec<String>,
    /// Why the list is empty, when the backend could not list.
    pub list_error: Option<OutputError>,
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

/// The options of the output menu from the raw names of `driver`.
///
/// For [`ALSA_DRIVER`], the devices of one card are one option, the first
/// of them, labelled with the card name, because SDL opens the same PCM
/// for each of them. For another driver, each device is an option labelled
/// with its raw name. A name with no card part keeps the raw name as its
/// label.
pub fn menu_outputs(driver: &str, raw: &[String]) -> Vec<OutputChoice> {
    let mut choices: Vec<OutputChoice> = Vec::with_capacity(raw.len());
    for name in raw {
        let card = (driver == ALSA_DRIVER)
            .then(|| name.split_once(':').map(|(card, _)| card.trim()))
            .flatten()
            .filter(|card| !card.is_empty());
        let Some(card) = card else {
            choices.push(OutputChoice {
                name: name.clone(),
                label: name.clone(),
            });
            continue;
        };
        if choices.iter().any(|c| c.label == card && c.name != c.label) {
            continue;
        }
        choices.push(OutputChoice {
            name: name.clone(),
            label: card.to_owned(),
        });
    }
    choices
}

/// The label of the raw `name`: its option's label, or the name itself.
pub fn output_label(choices: &[OutputChoice], name: &str) -> String {
    choices
        .iter()
        .find(|c| c.name == name)
        .map_or_else(|| name.to_owned(), |c| c.label.clone())
}

/// Opens the device that `setting` names (`auto` or a raw name), with the
/// fallback of the spec: a named device that is not listed or does not
/// open gives way to the default device, with a notice. The setting is
/// never changed, so the next start tries the named device again.
pub fn open_output<O: AudioOutputs>(outputs: &mut O, setting: &str) -> OpenedOutput<O::Sink> {
    let (devices, list_error) = match outputs.devices() {
        Ok(devices) => (devices, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let choices = menu_outputs(outputs.driver(), &devices);
    let mut notices = Vec::new();
    if setting != AUTO_OUTPUT {
        let label = output_label(&choices, setting);
        if devices.iter().any(|d| d == setting) {
            match outputs.open(Some(setting)) {
                Ok(sink) => {
                    return OpenedOutput {
                        sink: Some(sink),
                        opened: Some(OpenedDevice::Named(setting.to_owned())),
                        choices,
                        devices,
                        list_error,
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
    let (sink, opened) = match outputs.open(None) {
        Ok(sink) => (Some(sink), Some(OpenedDevice::Default)),
        Err(e) => {
            notices.push(OutputNotice::NoSound {
                reason: e.to_string(),
            });
            (None, None)
        }
    };
    OpenedOutput {
        sink,
        opened,
        choices,
        devices,
        list_error,
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
        driver: &'static str,
        devices: Result<Vec<String>, OutputError>,
        refused: Vec<Option<String>>,
        opens: Vec<Option<String>>,
        clock: SimClock,
    }

    impl FakeOutputs {
        fn new(driver: &'static str, devices: &[&str]) -> Self {
            Self {
                driver,
                devices: Ok(devices.iter().map(|d| (*d).to_owned()).collect()),
                refused: Vec::new(),
                opens: Vec::new(),
                clock: SimClock::new(),
            }
        }
    }

    impl AudioOutputs for FakeOutputs {
        type Sink = FakeAudio;

        fn driver(&self) -> &str {
            self.driver
        }

        fn devices(&mut self) -> Result<Vec<String>, OutputError> {
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

    const BENCH: [&str; 4] = [
        "HDA Intel PCH:ALC221 Analog",
        "HDA Intel PCH:HDMI 0",
        "HDA Intel PCH:HDMI 1",
        "HDA Intel PCH:HDMI 2",
    ];

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn choice(name: &str, label: &str) -> OutputChoice {
        OutputChoice {
            name: name.to_owned(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn alsa_devices_of_one_card_are_one_option_labelled_with_the_card() {
        let mut raw = owned(&BENCH);
        raw.push("USB Audio Device:USB Audio".to_owned());
        assert_eq!(
            menu_outputs(ALSA_DRIVER, &raw),
            [
                choice("HDA Intel PCH:ALC221 Analog", "HDA Intel PCH"),
                choice("USB Audio Device:USB Audio", "USB Audio Device"),
            ]
        );
    }

    #[test]
    fn other_drivers_and_odd_names_keep_the_raw_name() {
        let raw = owned(&["MacBook Pro Speakers", "Display: Left", "Display: Right"]);
        assert_eq!(
            menu_outputs("coreaudio", &raw),
            [
                choice("MacBook Pro Speakers", "MacBook Pro Speakers"),
                choice("Display: Left", "Display: Left"),
                choice("Display: Right", "Display: Right"),
            ]
        );
        let odd = owned(&[":HDMI 0", "plain", "plain", " :x", "Card:", "Card:y"]);
        assert_eq!(
            menu_outputs(ALSA_DRIVER, &odd),
            [
                choice(":HDMI 0", ":HDMI 0"),
                choice("plain", "plain"),
                choice("plain", "plain"),
                choice(" :x", " :x"),
                choice("Card:", "Card"),
            ]
        );
        assert!(menu_outputs(ALSA_DRIVER, &[]).is_empty());
    }

    #[test]
    fn auto_opens_the_default_device_with_no_notice() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        let opened = open_output(&mut outputs, AUTO_OUTPUT);
        assert!(opened.sink.is_some());
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(opened.notices, []);
        assert_eq!(opened.choices.len(), 1);
        assert_eq!(opened.devices.len(), 4);
        assert_eq!(outputs.opens, [None]);
    }

    #[test]
    fn a_listed_device_opens_by_its_raw_name() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        let opened = open_output(&mut outputs, "HDA Intel PCH:HDMI 1");
        assert_eq!(
            opened.opened,
            Some(OpenedDevice::Named("HDA Intel PCH:HDMI 1".to_owned()))
        );
        assert_eq!(opened.notices, []);
        assert_eq!(outputs.opens, [Some("HDA Intel PCH:HDMI 1".to_owned())]);
    }

    #[test]
    fn an_absent_device_falls_back_to_the_default_with_a_notice() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        let setting = "USB Audio Device:USB Audio".to_owned();
        let opened = open_output(&mut outputs, &setting);
        assert!(opened.sink.is_some());
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(
            opened.notices,
            [OutputNotice::Missing {
                label: setting.clone()
            }]
        );
        assert_eq!(
            opened.notices[0].to_string(),
            "The audio output \"USB Audio Device:USB Audio\" is not connected. The default output plays."
        );
        assert_eq!(outputs.opens, [None], "the absent device is not tried");
        assert_eq!(setting, "USB Audio Device:USB Audio");
    }

    #[test]
    fn a_device_that_does_not_open_falls_back_with_its_reason() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        let name = "HDA Intel PCH:ALC221 Analog";
        outputs.refused.push(Some(name.to_owned()));
        let opened = open_output(&mut outputs, name);
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert_eq!(
            opened.notices[0].to_string(),
            "The audio output \"HDA Intel PCH\" did not open (device busy). The default output plays."
        );
        assert_eq!(outputs.opens, [Some(name.to_owned()), None]);
    }

    #[test]
    fn with_no_device_at_all_the_run_has_no_sound() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        outputs.refused = vec![None, Some("HDA Intel PCH:HDMI 0".to_owned())];
        let opened = open_output(&mut outputs, "HDA Intel PCH:HDMI 0");
        assert!(opened.sink.is_none());
        assert_eq!(opened.opened, None);
        assert_eq!(opened.notices.len(), 2);
        assert_eq!(
            opened.notices[1].to_string(),
            "No sound: the audio device did not open (device busy)."
        );
        let mut auto = FakeOutputs::new(ALSA_DRIVER, &[]);
        auto.refused = vec![None];
        let opened = open_output(&mut auto, AUTO_OUTPUT);
        assert!(opened.sink.is_none());
        assert_eq!(opened.notices.len(), 1);
    }

    #[test]
    fn a_list_that_fails_falls_back_to_the_default() {
        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &[]);
        outputs.devices = Err(OutputError::List {
            reason: "no backend".to_owned(),
        });
        let opened = open_output(&mut outputs, "HDA Intel PCH:ALC221 Analog");
        assert_eq!(opened.opened, Some(OpenedDevice::Default));
        assert!(opened.choices.is_empty());
        assert!(opened.list_error.is_some());
        assert!(matches!(opened.notices[..], [OutputNotice::Missing { .. }]));
    }

    #[test]
    fn a_lost_device_plays_on_without_blocking_the_pipeline() {
        use crate::{AudioPipeline, CoreTiming, Resampling};

        let mut outputs = FakeOutputs::new(ALSA_DRIVER, &BENCH);
        let clock = outputs.clock.clone();
        let opened = open_output(&mut outputs, BENCH[0]);
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
