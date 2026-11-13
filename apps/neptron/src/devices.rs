//! Devices that may be missing on a machine: a run goes on without them.

use std::fmt::Write as _;

use neptron_audio::{AudioOutputs, OutputDevice, open_output};
use neptron_backend::AudioSink;
use neptron_settings::{AUDIO_OUTPUT, Schema, Storage, Target};

use crate::Paths;

/// The rate the silent sink reports, in hertz.
pub const NULL_SINK_RATE_HZ: u32 = 48_000;

/// An audio sink, or none: a machine with no usable sound device still
/// shows the picture and reads the input, so a run does not fail on it.
#[derive(Debug)]
pub enum MaybeSink<S> {
    /// The sound device.
    Device(S),
    /// No sound device: every sample is dropped, and the queue stays empty.
    Silent,
}

/// The sound device of a run, opened once at the start of the process.
#[derive(Debug)]
pub struct OpenedAudio<S> {
    /// The sink, or the silent sink when no device opened.
    pub sink: MaybeSink<S>,
    /// The options of the audio output row after `auto`.
    pub outputs: Vec<OutputDevice>,
    /// The notices for the player, in order.
    pub notices: Vec<String>,
    /// One line for the log: the driver, the devices, the choice of `auto`
    /// and the device that opened.
    pub log: String,
}

/// The `audio.output` setting of the global file: `auto` when the file
/// does not set it or cannot be read.
pub fn audio_output_setting(paths: &Paths) -> String {
    let schema = Schema::builtin();
    let global = Storage::new(paths.config())
        .load(&schema, Target::Global)
        .ok();
    global
        .as_ref()
        .and_then(|l| l.parsed.layer.get(AUDIO_OUTPUT).map(ToString::to_string))
        .unwrap_or_else(|| neptron_audio::AUTO_OUTPUT.to_owned())
}

/// Opens the sound device that `setting` names, through `outputs`, the
/// backend that started or the reason it did not. A device that is absent
/// or does not open gives way to the default device; no device at all
/// gives the silent sink. The setting is never changed.
pub fn open_audio<O: AudioOutputs>(
    outputs: Result<O, String>,
    setting: &str,
) -> OpenedAudio<O::Sink> {
    let mut outputs = match outputs {
        Ok(outputs) => outputs,
        Err(reason) => {
            return OpenedAudio {
                sink: MaybeSink::Silent,
                outputs: Vec::new(),
                notices: vec![format!(
                    "No sound: the audio device did not open ({reason})."
                )],
                log: format!("audio: the backend did not start: {reason}"),
            };
        }
    };
    let opened = open_output(&mut outputs, setting);
    let mut log = format!("audio: driver {}; devices", outputs.driver());
    if opened.devices.is_empty() {
        log.push_str(" none");
    }
    for (i, device) in opened.devices.iter().enumerate() {
        let sep = if i == 0 { " " } else { ", " };
        // Writing to a String cannot fail.
        let _ = write!(log, "{sep}\"{}\"", device.name);
    }
    if let Some(error) = &opened.list_error {
        let _ = write!(log, " ({error})");
    }
    if let Some(auto) = &opened.auto {
        let _ = write!(log, "; auto: {}", auto.reason);
    }
    for (device, error) in &opened.failures {
        let _ = write!(log, "; failed \"{device}\" ({error})");
    }
    match &opened.opened {
        Some(device) => {
            let _ = write!(log, "; opened {device}");
        }
        None => log.push_str("; none opened"),
    }
    OpenedAudio {
        sink: opened.sink.map_or(MaybeSink::Silent, MaybeSink::Device),
        outputs: opened.devices,
        notices: opened.notices.iter().map(ToString::to_string).collect(),
        log,
    }
}

impl<S: AudioSink> AudioSink for MaybeSink<S> {
    fn rate_hz(&self) -> u32 {
        match self {
            Self::Device(sink) => sink.rate_hz(),
            Self::Silent => NULL_SINK_RATE_HZ,
        }
    }

    fn queued_frames(&self) -> usize {
        match self {
            Self::Device(sink) => sink.queued_frames(),
            Self::Silent => 0,
        }
    }

    fn write(&mut self, stereo: &[i16]) -> usize {
        match self {
            Self::Device(sink) => sink.write(stereo),
            Self::Silent => stereo.len() / 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_audio::{ALSA_DRIVER, AlsaText, OutputError, alsa_outputs};
    use neptron_testkit::{FakeAudio, SimClock};

    use super::*;

    /// A backend with the analog output and one HDMI output of one card,
    /// where `refuse` names the devices that do not open.
    struct Outputs {
        refuse: Vec<Option<&'static str>>,
    }

    impl AudioOutputs for Outputs {
        type Sink = FakeAudio;

        fn driver(&self) -> &str {
            ALSA_DRIVER
        }

        fn devices(&mut self) -> Result<Vec<OutputDevice>, OutputError> {
            Ok(alsa_outputs(&AlsaText {
                cards: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n".to_owned(),
                pcm: "00-00: ALC221 Analog : ALC221 Analog : playback 1\n00-03: HDMI 0 : HDMI 0 : playback 1\n".to_owned(),
                elds: Vec::new(),
            }))
        }

        fn open(&mut self, device: Option<&str>) -> Result<FakeAudio, OutputError> {
            if self.refuse.contains(&device) {
                return Err(OutputError::Open {
                    device: device.unwrap_or("default").to_owned(),
                    reason: "busy".to_owned(),
                });
            }
            Ok(FakeAudio::new(SimClock::new(), 48_000, 6_144))
        }
    }

    #[test]
    fn the_named_device_opens_and_the_log_names_it() {
        let outputs = Outputs { refuse: Vec::new() };
        let audio = open_audio(Ok(outputs), "hw:CARD=PCH,DEV=3");
        assert!(matches!(audio.sink, MaybeSink::Device(_)));
        assert!(audio.notices.is_empty());
        assert_eq!(audio.outputs.len(), 2);
        assert_eq!(
            audio.log,
            "audio: driver alsa; devices \"hw:CARD=PCH,DEV=0\", \"hw:CARD=PCH,DEV=3\"; opened \"hw:CARD=PCH,DEV=3\""
        );
    }

    #[test]
    fn an_absent_device_plays_on_the_first_analog_output_with_a_notice() {
        let outputs = Outputs { refuse: Vec::new() };
        let audio = open_audio(Ok(outputs), "hw:CARD=Device,DEV=0");
        assert!(matches!(audio.sink, MaybeSink::Device(_)));
        assert_eq!(
            audio.notices,
            ["The audio output \"hw:CARD=Device,DEV=0\" is not connected. Another output plays."]
        );
        assert!(
            audio.log.ends_with(
                "; auto: the first analog output \"Analog (HDA Intel PCH)\", because no HDMI/DP output has a monitor that takes audio; opened \"hw:CARD=PCH,DEV=0\""
            ),
            "{}",
            audio.log
        );
    }

    #[test]
    fn no_device_and_no_backend_give_the_silent_sink() {
        let outputs = Outputs {
            refuse: vec![Some("hw:CARD=PCH,DEV=0"), None],
        };
        let audio = open_audio(Ok(outputs), "auto");
        assert!(matches!(audio.sink, MaybeSink::Silent));
        assert_eq!(
            audio.notices,
            [
                "The audio output \"Analog (HDA Intel PCH)\" did not open (busy). Another output plays.",
                "No sound: the audio device did not open (busy; busy).",
            ]
        );
        assert!(
            audio.log.ends_with(
                "; failed \"hw:CARD=PCH,DEV=0\" (busy); failed \"default\" (busy); none opened"
            ),
            "{}",
            audio.log
        );
        let audio = open_audio::<Outputs>(Err("no ALSA".to_owned()), "auto");
        assert!(matches!(audio.sink, MaybeSink::Silent));
        assert_eq!(
            audio.notices,
            ["No sound: the audio device did not open (no ALSA)."]
        );
    }

    struct Fixed;

    impl AudioSink for Fixed {
        fn rate_hz(&self) -> u32 {
            44_100
        }
        fn queued_frames(&self) -> usize {
            7
        }
        fn write(&mut self, _stereo: &[i16]) -> usize {
            1
        }
    }

    #[test]
    fn the_silent_sink_takes_every_frame_and_queues_none() {
        let mut sink = MaybeSink::<Fixed>::Silent;
        assert_eq!(sink.rate_hz(), NULL_SINK_RATE_HZ);
        assert_eq!(
            sink.write(&[1, 2, 3, 4, 5]),
            2,
            "an odd last sample is not taken"
        );
        assert_eq!(sink.queued_frames(), 0);
    }

    #[test]
    fn a_device_sink_passes_through() {
        let mut sink = MaybeSink::Device(Fixed);
        assert_eq!(sink.rate_hz(), 44_100);
        assert_eq!(sink.queued_frames(), 7);
        assert_eq!(sink.write(&[0; 8]), 1);
    }
}
