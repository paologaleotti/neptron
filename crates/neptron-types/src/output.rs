use serde::{Deserialize, Serialize};

/// The kind of link from an output to its display, decided once at start
/// from the class rule of the detection.
///
/// The limits of the GPU that bound a mode depend on it: an analog output
/// keeps the careful CRT limits, a digital link to a flat panel has the
/// limits of the link and the scanout. With no certainty, an output is
/// analog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// An analog signal: a VGA, DVI-A or TV output, a DAC on a digital
    /// connector, or any output that is not proven digital to a flat
    /// panel.
    #[default]
    Analog,
    /// A digital link (HDMI, DisplayPort, DVI-D) to a detected flat panel.
    Digital,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_output_is_analog() {
        assert_eq!(OutputKind::default(), OutputKind::Analog);
    }
}
