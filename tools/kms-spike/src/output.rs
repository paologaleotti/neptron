//! The connector choice. Pure: the KMS side lists the connectors, this
//! module decides.

use std::fmt;

/// A connector as the kernel lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// The name the kernel gives it, for example `VGA-1` or `DP-1`.
    pub name: String,
    /// The connection state.
    pub state: State,
}

/// The connection state of a connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The kernel sees a display.
    Connected,
    /// The kernel sees no display.
    Disconnected,
    /// The kernel cannot tell.
    Unknown,
}

/// Why no connector is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceError {
    /// The named connector does not exist.
    NotFound {
        /// The name asked for.
        name: String,
        /// Every connector name.
        available: Vec<String>,
    },
    /// No connector is named and none is connected.
    NoneConnected {
        /// Every connector name.
        available: Vec<String>,
    },
    /// No connector is named and more than one is connected.
    Several {
        /// The connected connector names.
        connected: Vec<String>,
    },
}

/// The index of the connector to use. A named connector wins, whatever its
/// state, because a VGA monitor often reports nothing. With no name, the
/// one connected connector is used; with none or several, the user must
/// name one.
pub fn choose(connectors: &[Connector], name: Option<&str>) -> Result<usize, ChoiceError> {
    let names = || connectors.iter().map(|c| c.name.clone()).collect();
    if let Some(name) = name {
        return connectors
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| ChoiceError::NotFound {
                name: name.to_owned(),
                available: names(),
            });
    }
    let connected: Vec<usize> = connectors
        .iter()
        .enumerate()
        .filter(|(_, c)| c.state == State::Connected)
        .map(|(i, _)| i)
        .collect();
    match connected.as_slice() {
        [one] => Ok(*one),
        [] => Err(ChoiceError::NoneConnected { available: names() }),
        _ => Err(ChoiceError::Several {
            connected: connected
                .iter()
                .map(|&i| connectors[i].name.clone())
                .collect(),
        }),
    }
}

/// A note for the chosen connector, when it needs one.
pub fn note(connector: &Connector) -> Option<String> {
    let mut notes = Vec::new();
    if connector.state != State::Connected {
        notes.push(format!(
            "{} reports {}: the kernel sees no display, so check the cable and the monitor",
            connector.name,
            connector.state.name()
        ));
    }
    if connector.name.starts_with("DP-") {
        notes.push(format!(
            "{} is a DisplayPort connector; on boards with a DP-to-VGA bridge the VGA port \
             shows up as DisplayPort, and the bridge may limit the pixel clock",
            connector.name
        ));
    }
    (!notes.is_empty()).then(|| notes.join("\n"))
}

impl State {
    /// The name in the report.
    pub fn name(self) -> &'static str {
        match self {
            State::Connected => "connected",
            State::Disconnected => "disconnected",
            State::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ChoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChoiceError::NotFound { name, available } => write!(
                f,
                "no connector {name:?}; the device has: {}",
                available.join(", ")
            ),
            ChoiceError::NoneConnected { available } => write!(
                f,
                "no connector reports a display; name one with --connector (the device has: {})",
                available.join(", ")
            ),
            ChoiceError::Several { connected } => write!(
                f,
                "several connectors report a display ({}); name one with --connector",
                connected.join(", ")
            ),
        }
    }
}

impl std::error::Error for ChoiceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, state: State) -> Connector {
        Connector {
            name: name.to_owned(),
            state,
        }
    }

    #[test]
    fn a_named_connector_wins_in_any_state() {
        let list = [c("VGA-1", State::Unknown), c("HDMI-A-1", State::Connected)];
        assert_eq!(choose(&list, Some("VGA-1")), Ok(0));
        assert_eq!(choose(&list, Some("HDMI-A-1")), Ok(1));
    }

    #[test]
    fn an_unknown_name_lists_the_connectors() {
        let list = [c("VGA-1", State::Connected)];
        assert_eq!(
            choose(&list, Some("vga-1")),
            Err(ChoiceError::NotFound {
                name: "vga-1".to_owned(),
                available: vec!["VGA-1".to_owned()]
            })
        );
    }

    #[test]
    fn the_one_connected_connector_is_used() {
        let list = [
            c("DP-1", State::Disconnected),
            c("VGA-1", State::Connected),
            c("HDMI-A-1", State::Unknown),
        ];
        assert_eq!(choose(&list, None), Ok(1));
    }

    #[test]
    fn the_tool_never_guesses_between_several() {
        let list = [c("DP-1", State::Connected), c("VGA-1", State::Connected)];
        let err = choose(&list, None).unwrap_err();
        assert_eq!(
            err,
            ChoiceError::Several {
                connected: vec!["DP-1".to_owned(), "VGA-1".to_owned()]
            }
        );
        assert!(err.to_string().contains("--connector"));
    }

    #[test]
    fn no_connected_connector_needs_a_name() {
        let list = [c("VGA-1", State::Unknown)];
        assert!(matches!(
            choose(&list, None),
            Err(ChoiceError::NoneConnected { .. })
        ));
        assert!(matches!(
            choose(&[], None),
            Err(ChoiceError::NoneConnected { .. })
        ));
    }

    #[test]
    fn notes_cover_the_bridge_and_an_unconnected_port() {
        assert_eq!(note(&c("VGA-1", State::Connected)), None);
        let dp = note(&c("DP-1", State::Connected)).unwrap();
        assert!(dp.contains("DP-to-VGA"), "{dp}");
        let vga = note(&c("VGA-1", State::Unknown)).unwrap();
        assert!(vga.contains("reports unknown"), "{vga}");
    }
}
