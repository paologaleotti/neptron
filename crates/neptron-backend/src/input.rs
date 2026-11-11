use neptron_types::InputEvent;

/// A source of input events, already mapped to the RetroPad.
pub trait InputSource {
    /// Appends to `out` every event since the last call, oldest first. It
    /// never blocks and does not clear `out`.
    fn poll(&mut self, out: &mut Vec<InputEvent>);
}
