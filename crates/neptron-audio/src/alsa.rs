//! The ALSA playback PCMs, from the text of `/proc/asound`: the cards, the
//! playback PCMs, and the ELD of each HDMI or DisplayPort pin.
//!
//! Parsing is pure. [`AlsaText::read`] is the only file access: it runs
//! once at start, off the frame loop, and a file it cannot read is empty
//! text.

use std::path::Path;

use crate::{Monitor, OutputDevice};

/// The prefix of a stored ALSA name: `hw:CARD=<id>,DEV=<n>`.
pub const ALSA_NAME_PREFIX: &str = "hw:";

/// The text of `/proc/asound` that the list needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlsaText {
    /// `/proc/asound/cards`.
    pub cards: String,
    /// `/proc/asound/pcm`.
    pub pcm: String,
    /// The ELD files of each card: the card index and the text of one
    /// `card<N>/eld#*` file.
    pub elds: Vec<(u32, String)>,
}

impl AlsaText {
    /// Reads the text under `root` (`/proc/asound` on a device). A missing
    /// or unreadable file is empty text, so the list is empty or shorter,
    /// and nothing stops the start.
    pub fn read(root: &Path) -> Self {
        let read = |path: &Path| std::fs::read_to_string(path).unwrap_or_default();
        let cards = read(&root.join("cards"));
        let pcm = read(&root.join("pcm"));
        let mut elds = Vec::new();
        for card in parse_cards(&cards) {
            let dir = root.join(format!("card{}", card.index));
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut names: Vec<_> = entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("eld#"))
                .collect();
            names.sort();
            for name in names {
                elds.push((card.index, read(&dir.join(name))));
            }
        }
        Self { cards, pcm, elds }
    }
}

/// One sound card of `/proc/asound/cards`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaCard {
    /// The card index.
    pub index: u32,
    /// The card id, such as `PCH`: stable when another card is plugged in.
    pub id: String,
    /// The card name, such as `HDA Intel PCH`.
    pub name: String,
}

/// One playback PCM of `/proc/asound/pcm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlsaPcm {
    /// The card index.
    pub card: u32,
    /// The PCM device number.
    pub device: u32,
    /// The PCM id, such as `HDMI 0`.
    pub id: String,
    /// The PCM name, such as `ALC221 Analog`.
    pub name: String,
}

/// What one ELD file says about the monitor on its pin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Eld {
    /// `monitor_present 1`.
    pub present: bool,
    /// `eld_valid 1`: the monitor takes audio.
    pub valid: bool,
    /// `monitor_name`, when the file has one.
    pub name: Option<String>,
}

/// The cards of the text of `/proc/asound/cards`. Lines of another form
/// are skipped.
pub fn parse_cards(text: &str) -> Vec<AlsaCard> {
    text.lines()
        .filter_map(|line| {
            let (index, rest) = line.trim_start().split_once(' ')?;
            let index = index.parse().ok()?;
            let (id, rest) = rest.trim_start().strip_prefix('[')?.split_once(']')?;
            let rest = rest.strip_prefix(':')?;
            let name = rest.split_once(" - ").map_or(rest, |(_, name)| name).trim();
            Some(AlsaCard {
                index,
                id: id.trim().to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// The playback PCMs of the text of `/proc/asound/pcm`. A capture-only
/// PCM and a line of another form are skipped.
pub fn parse_pcms(text: &str) -> Vec<AlsaPcm> {
    text.lines()
        .filter_map(|line| {
            let (address, rest) = line.split_once(": ")?;
            let (card, device) = address.trim().split_once('-')?;
            let mut parts = rest.split(" : ");
            let id = parts.next()?.trim();
            let name = parts.next()?.trim();
            let playback = parts.any(|p| p.trim().starts_with("playback"));
            playback.then(|| AlsaPcm {
                card: card.parse().unwrap_or(u32::MAX),
                device: device.parse().unwrap_or(u32::MAX),
                id: id.to_owned(),
                name: name.to_owned(),
            })
        })
        .filter(|p| p.card != u32::MAX && p.device != u32::MAX)
        .collect()
}

/// The monitor state of the text of one ELD file.
pub fn parse_eld(text: &str) -> Eld {
    let mut eld = Eld::default();
    for line in text.lines() {
        let line = line.trim();
        let (key, value) = line
            .split_once(char::is_whitespace)
            .map_or((line, ""), |(k, v)| (k, v.trim()));
        match key {
            "monitor_present" => eld.present = value == "1",
            "eld_valid" => eld.valid = value == "1",
            "monitor_name" if !value.is_empty() => eld.name = Some(value.to_owned()),
            _ => {}
        }
    }
    eld
}

/// True for an HDMI or DisplayPort PCM, from its id.
pub fn is_digital(pcm: &AlsaPcm) -> bool {
    pcm.id.starts_with("HDMI") || pcm.id.starts_with("DP") || pcm.id.contains("DisplayPort")
}

/// The stored name of a PCM: `hw:CARD=<id>,DEV=<n>`.
pub fn alsa_name(card_id: &str, device: u32) -> String {
    format!("{ALSA_NAME_PREFIX}CARD={card_id},DEV={device}")
}

/// The PCM that ALSA opens for a stored name: `plughw:` in place of `hw:`,
/// so that ALSA converts the format and the rate when the hardware needs
/// it. Another name is opened as it is.
pub fn open_name(name: &str) -> String {
    match name.strip_prefix(ALSA_NAME_PREFIX) {
        Some(rest) => format!("plughw:{rest}"),
        None => name.to_owned(),
    }
}

/// The playback devices of the text, in menu order: the cards in index
/// order with their PCMs in device order, and the HDMI or DisplayPort PCMs
/// with no monitor last. A PCM of a card that `cards` does not list is
/// left out.
///
/// The monitor of an HDMI PCM comes from the counts of its card: `/proc`
/// does not say which PCM a pin uses. With `v` ELDs that are present and
/// valid and `h` HDMI PCMs, `v = 0` means no monitor on any of them, and
/// `v = h` means a monitor on each, named when `h = 1`. Any other count
/// leaves the monitor of each PCM unknown.
pub fn alsa_outputs(text: &AlsaText) -> Vec<OutputDevice> {
    let cards = parse_cards(&text.cards);
    let pcms = parse_pcms(&text.pcm);
    let mut listed = Vec::new();
    let mut last = Vec::new();
    for card in &cards {
        let elds: Vec<Eld> = text
            .elds
            .iter()
            .filter(|(index, _)| *index == card.index)
            .map(|(_, t)| parse_eld(t))
            .filter(|e| e.present && e.valid)
            .collect();
        let digital = pcms
            .iter()
            .filter(|p| p.card == card.index && is_digital(p))
            .count();
        let mut k = 0;
        for pcm in pcms.iter().filter(|p| p.card == card.index) {
            let name = alsa_name(&card.id, pcm.device);
            if !is_digital(pcm) {
                listed.push(OutputDevice {
                    name,
                    label: format!("{} analog", card.name),
                    detail: pcm.name.clone(),
                    monitor: Monitor::NotDigital,
                });
                continue;
            }
            k += 1;
            let numbered = format!("{} HDMI/DP {k}", card.name);
            let device = match (elds.len(), digital) {
                (0, _) => OutputDevice {
                    name,
                    label: numbered,
                    detail: "Nothing connected".to_owned(),
                    monitor: Monitor::Absent,
                },
                (1, 1) => {
                    let monitor = elds[0].name.clone();
                    OutputDevice {
                        name,
                        label: match &monitor {
                            Some(m) => format!("{m} HDMI/DP"),
                            None => numbered,
                        },
                        detail: pcm.name.clone(),
                        monitor: Monitor::Present(monitor),
                    }
                }
                (v, h) if v == h => OutputDevice {
                    name,
                    label: numbered,
                    detail: "Monitor connected".to_owned(),
                    monitor: Monitor::Present(None),
                },
                _ => OutputDevice {
                    name,
                    label: numbered,
                    detail: pcm.name.clone(),
                    monitor: Monitor::Unknown,
                },
            };
            if device.monitor == Monitor::Absent {
                last.push(device);
            } else {
                listed.push(device);
            }
        }
    }
    listed.extend(last);
    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of the bench PC on 2026-11-13: one HDA card, a DELL
    /// monitor on DisplayPort, three HDMI PCMs.
    pub(crate) fn bench() -> AlsaText {
        let present = "monitor_present\t\t1\neld_valid\t\t1\ncodec_pin_nid\t\t0x5\nmonitor_name\t\tDELL U2724D\nconnection_type\t\tDisplayPort\n";
        let absent = "monitor_present\t\t0\neld_valid\t\t0\ncodec_pin_nid\t\t0x5\n";
        let mut elds = vec![(0, present.to_owned())];
        elds.extend((1..9).map(|_| (0, absent.to_owned())));
        AlsaText {
            cards: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xe1224000 irq 128\n".to_owned(),
            pcm: "00-00: ALC221 Analog : ALC221 Analog : playback 1 : capture 1\n00-03: HDMI 0 : HDMI 0 : playback 1\n00-07: HDMI 1 : HDMI 1 : playback 1\n00-08: HDMI 2 : HDMI 2 : playback 1\n".to_owned(),
            elds,
        }
    }

    fn eld(present: bool, valid: bool, name: &str) -> String {
        format!(
            "monitor_present\t\t{}\neld_valid\t\t{}\nmonitor_name\t\t{name}\n",
            u8::from(present),
            u8::from(valid)
        )
    }

    /// A card with one HDMI PCM (an AMD or NVIDIA style codec) and one USB
    /// card, with the ELDs of the HDMI card.
    fn one_hdmi(elds: &[String]) -> AlsaText {
        AlsaText {
            cards: " 0 [HDMI           ]: HDA-Intel - HDA ATI HDMI\n                      HDA ATI HDMI at 0x1 irq 1\n 1 [Device         ]: USB-Audio - USB Audio Device\n                      Generic USB Audio at usb-1\n".to_owned(),
            pcm: "00-03: HDMI 0 : HDMI 0 : playback 1\n01-00: USB Audio : USB Audio : playback 1 : capture 1\n01-01: USB Audio #1 : USB Audio #1 : capture 1\n".to_owned(),
            elds: elds.iter().map(|t| (0, t.clone())).collect(),
        }
    }

    fn view(devices: &[OutputDevice]) -> Vec<(&str, &str, &str)> {
        devices
            .iter()
            .map(|d| (d.name.as_str(), d.label.as_str(), d.detail.as_str()))
            .collect()
    }

    #[test]
    fn cards_pcms_and_elds_parse_from_the_proc_text() {
        let text = bench();
        assert_eq!(
            parse_cards(&text.cards),
            [AlsaCard {
                index: 0,
                id: "PCH".to_owned(),
                name: "HDA Intel PCH".to_owned()
            }]
        );
        let pcms = parse_pcms(&text.pcm);
        let devices: Vec<_> = pcms.iter().map(|p| (p.device, p.id.as_str())).collect();
        assert_eq!(
            devices,
            [
                (0, "ALC221 Analog"),
                (3, "HDMI 0"),
                (7, "HDMI 1"),
                (8, "HDMI 2")
            ]
        );
        assert_eq!(pcms[0].name, "ALC221 Analog");
        assert_eq!(
            parse_eld(&text.elds[0].1),
            Eld {
                present: true,
                valid: true,
                name: Some("DELL U2724D".to_owned())
            }
        );
        assert_eq!(parse_eld(&text.elds[1].1), Eld::default());
        assert_eq!(parse_eld(""), Eld::default());
        assert!(parse_cards("garbage\n[x]\n").is_empty());
        assert!(parse_pcms("00-00: Only : capture 1\nbad line\n").is_empty());
    }

    #[test]
    fn the_bench_lists_the_analog_output_and_three_hdmi_outputs_of_unknown_monitor() {
        let devices = alsa_outputs(&bench());
        assert_eq!(
            view(&devices),
            [
                ("hw:CARD=PCH,DEV=0", "HDA Intel PCH analog", "ALC221 Analog"),
                ("hw:CARD=PCH,DEV=3", "HDA Intel PCH HDMI/DP 1", "HDMI 0"),
                ("hw:CARD=PCH,DEV=7", "HDA Intel PCH HDMI/DP 2", "HDMI 1"),
                ("hw:CARD=PCH,DEV=8", "HDA Intel PCH HDMI/DP 3", "HDMI 2"),
            ]
        );
        assert_eq!(devices[0].monitor, Monitor::NotDigital);
        assert!(devices[1..].iter().all(|d| d.monitor == Monitor::Unknown));
    }

    #[test]
    fn a_single_hdmi_pcm_with_a_valid_eld_carries_the_monitor_name() {
        let devices = alsa_outputs(&one_hdmi(&[eld(true, true, "SONY TV")]));
        assert_eq!(
            view(&devices),
            [
                ("hw:CARD=HDMI,DEV=3", "SONY TV HDMI/DP", "HDMI 0"),
                (
                    "hw:CARD=Device,DEV=0",
                    "USB Audio Device analog",
                    "USB Audio"
                ),
            ]
        );
        assert_eq!(
            devices[0].monitor,
            Monitor::Present(Some("SONY TV".to_owned()))
        );
    }

    #[test]
    fn hdmi_pcms_with_no_valid_eld_have_nothing_connected_and_come_last() {
        for elds in [
            vec![],
            vec![eld(false, false, "")],
            vec![eld(true, false, "CRT DAC")],
        ] {
            let devices = alsa_outputs(&one_hdmi(&elds));
            assert_eq!(
                view(&devices),
                [
                    (
                        "hw:CARD=Device,DEV=0",
                        "USB Audio Device analog",
                        "USB Audio"
                    ),
                    (
                        "hw:CARD=HDMI,DEV=3",
                        "HDA ATI HDMI HDMI/DP 1",
                        "Nothing connected"
                    ),
                ],
                "{elds:?}"
            );
            assert_eq!(devices[1].monitor, Monitor::Absent);
        }
    }

    #[test]
    fn more_valid_elds_than_pcms_leave_the_monitor_unknown() {
        let devices = alsa_outputs(&one_hdmi(&[eld(true, true, "A"), eld(true, true, "B")]));
        assert_eq!(devices[0].monitor, Monitor::Unknown);
        assert_eq!(devices[0].label, "HDA ATI HDMI HDMI/DP 1");
    }

    #[test]
    fn every_hdmi_pcm_with_a_monitor_is_present_with_no_name() {
        let mut text = bench();
        text.elds = (0..3)
            .map(|i| (0, eld(true, true, &format!("M{i}"))))
            .collect();
        let devices = alsa_outputs(&text);
        assert!(
            devices[1..]
                .iter()
                .all(|d| d.monitor == Monitor::Present(None))
        );
        assert_eq!(devices[1].detail, "Monitor connected");
    }

    #[test]
    fn missing_text_gives_an_empty_list() {
        assert!(alsa_outputs(&AlsaText::default()).is_empty());
        let mut text = bench();
        text.cards.clear();
        assert!(alsa_outputs(&text).is_empty(), "a PCM of no listed card");
        let dir = std::env::temp_dir().join(format!("neptron-alsa-none-{}", std::process::id()));
        assert_eq!(AlsaText::read(&dir), AlsaText::default());
    }

    #[test]
    fn the_reader_takes_the_files_of_a_proc_tree() {
        let dir = std::env::temp_dir().join(format!("neptron-alsa-{}", std::process::id()));
        let card = dir.join("card0");
        std::fs::create_dir_all(&card).unwrap();
        let text = bench();
        std::fs::write(dir.join("cards"), &text.cards).unwrap();
        std::fs::write(dir.join("pcm"), &text.pcm).unwrap();
        std::fs::write(card.join("eld#2.0"), &text.elds[0].1).unwrap();
        std::fs::write(card.join("eld#2.1"), &text.elds[1].1).unwrap();
        std::fs::write(card.join("codec#0"), "Codec: Realtek ALC221\n").unwrap();
        let read = AlsaText::read(&dir);
        assert_eq!(read.cards, text.cards);
        assert_eq!(read.pcm, text.pcm);
        assert_eq!(read.elds, text.elds[..2]);
        // A leftover folder in the temp dir does no harm.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_name_opens_through_plughw() {
        assert_eq!(alsa_name("PCH", 3), "hw:CARD=PCH,DEV=3");
        assert_eq!(open_name("hw:CARD=PCH,DEV=3"), "plughw:CARD=PCH,DEV=3");
        assert_eq!(open_name("default"), "default");
    }
}
