//! Notices: what a device backend tells the frontend to log.

/// Something a device backend saw that the frontend logs. Notices come
/// from hotplug and from device errors, never from the steady state, so
/// their text may allocate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputNotice {
    /// A device was connected and got a device id.
    Connected {
        /// The device id.
        device: u32,
        /// What the backend knows of the device: an SDL GUID or a path.
        what: String,
    },
    /// A device went away. Its controls were released.
    Disconnected {
        /// The device id it had.
        device: u32,
    },
    /// A device was found and is not read.
    Ignored {
        /// What the backend knows of the device.
        what: String,
        /// Why it is not read.
        reason: String,
    },
    /// A keyboard is read without a grab, so its keys can also reach
    /// another reader, such as a console.
    NotGrabbed {
        /// The device id.
        device: u32,
        /// The error of the grab.
        reason: String,
    },
    /// The queue was full and dropped this many notices.
    Dropped {
        /// The number of notices dropped.
        count: u32,
    },
}

/// The most notices a backend keeps between two reads.
pub const MAX_NOTICES: usize = 32;

/// A bounded queue of notices. A notice that comes when the queue is full
/// is counted and dropped, so a device that fails again and again cannot
/// grow the memory of the frontend.
#[derive(Debug, Default)]
pub(crate) struct NoticeQueue {
    queue: Vec<InputNotice>,
    dropped: u32,
}

impl NoticeQueue {
    /// Adds `notice`, or counts it as dropped when the queue is full.
    pub(crate) fn push(&mut self, notice: InputNotice) {
        if self.queue.len() < MAX_NOTICES {
            self.queue.push(notice);
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    /// Moves every notice to `out`, oldest first, then a
    /// [`InputNotice::Dropped`] when some were dropped.
    pub(crate) fn drain_into(&mut self, out: &mut Vec<InputNotice>) {
        out.append(&mut self.queue);
        if self.dropped > 0 {
            out.push(InputNotice::Dropped {
                count: self.dropped,
            });
            self.dropped = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_come_out_in_order() {
        let mut queue = NoticeQueue::default();
        queue.push(InputNotice::Disconnected { device: 1 });
        queue.push(InputNotice::Disconnected { device: 2 });
        let mut out = Vec::new();
        queue.drain_into(&mut out);
        assert_eq!(
            out,
            [
                InputNotice::Disconnected { device: 1 },
                InputNotice::Disconnected { device: 2 }
            ]
        );
        out.clear();
        queue.drain_into(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn a_full_queue_counts_what_it_drops() {
        let mut queue = NoticeQueue::default();
        for device in 0..100 {
            queue.push(InputNotice::Disconnected { device });
        }
        let mut out = Vec::new();
        queue.drain_into(&mut out);
        assert_eq!(out.len(), MAX_NOTICES + 1);
        assert_eq!(
            out[MAX_NOTICES - 1],
            InputNotice::Disconnected { device: 31 }
        );
        assert_eq!(out[MAX_NOTICES], InputNotice::Dropped { count: 100 - 32 });
        out.clear();
        queue.push(InputNotice::Disconnected { device: 0 });
        queue.drain_into(&mut out);
        assert_eq!(out, [InputNotice::Disconnected { device: 0 }]);
    }
}
