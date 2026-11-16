//! The input descriptors of a core (`SET_INPUT_DESCRIPTORS`): what each
//! RetroPad input does in the game, for the remapping screen.

/// The most input descriptors that the host keeps.
pub const MAX_INPUT_DESCRIPTORS: usize = 256;

/// One input of the core and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDescriptor {
    /// The port, from 0.
    pub port: u32,
    /// The libretro device (`RETRO_DEVICE_JOYPAD` and others).
    pub device: u32,
    /// The index on the device: the stick for an analog input.
    pub index: u32,
    /// The input id on the device.
    pub id: u32,
    /// What the input does, as the core names it.
    pub description: String,
}

/// The fields of one `retro_input_descriptor`, as plain values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawDescriptor<'a> {
    pub port: u32,
    pub device: u32,
    pub index: u32,
    pub id: u32,
    pub description: &'a [u8],
}

impl InputDescriptor {
    pub(crate) fn from_raw(raw: &RawDescriptor<'_>) -> Self {
        Self {
            port: raw.port,
            device: raw.device,
            index: raw.index,
            id: raw.id,
            description: String::from_utf8_lossy(raw.description).into_owned(),
        }
    }
}
