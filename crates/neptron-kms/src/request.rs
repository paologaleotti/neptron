//! An atomic request in fixed arrays, so that a page flip allocates
//! nothing.

/// The most objects of one request: connector, writeback connector, CRTC
/// and plane.
const OBJECTS_MAX: usize = 4;
/// The most properties of one request: 1 + 3 + 2 + 10 for a modeset with
/// writeback.
const PROPS_MAX: usize = 16;

/// `DRM_MODE_PAGE_FLIP_EVENT`
pub(crate) const PAGE_FLIP_EVENT: u32 = 0x01;
/// `DRM_MODE_ATOMIC_TEST_ONLY`
pub(crate) const TEST_ONLY: u32 = 0x0100;
/// `DRM_MODE_ATOMIC_NONBLOCK`
pub(crate) const NONBLOCK: u32 = 0x0200;
/// `DRM_MODE_ATOMIC_ALLOW_MODESET`
pub(crate) const ALLOW_MODESET: u32 = 0x0400;

/// Object, property and value ids of one atomic commit. The properties of
/// one object are added one after the other.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Request {
    objects: [u32; OBJECTS_MAX],
    counts: [u32; OBJECTS_MAX],
    props: [u32; PROPS_MAX],
    values: [u64; PROPS_MAX],
    n_objects: usize,
    n_props: usize,
    overflow: bool,
}

impl Request {
    /// An empty request.
    pub(crate) fn new() -> Request {
        Request {
            objects: [0; OBJECTS_MAX],
            counts: [0; OBJECTS_MAX],
            props: [0; PROPS_MAX],
            values: [0; PROPS_MAX],
            n_objects: 0,
            n_props: 0,
            overflow: false,
        }
    }

    /// Sets `prop` of `object` to `value`.
    pub(crate) fn add(&mut self, object: u32, prop: u32, value: u64) {
        let same = self.n_objects > 0 && self.objects[self.n_objects - 1] == object;
        if self.n_props == PROPS_MAX || (!same && self.n_objects == OBJECTS_MAX) {
            self.overflow = true;
            return;
        }
        if !same {
            self.objects[self.n_objects] = object;
            self.counts[self.n_objects] = 0;
            self.n_objects += 1;
        }
        self.counts[self.n_objects - 1] += 1;
        self.props[self.n_props] = prop;
        self.values[self.n_props] = value;
        self.n_props += 1;
    }

    /// The object ids, the property count of each object, the property
    /// ids and the values, as the atomic ioctl takes them. `None` when
    /// [`add`](Self::add) ran out of room.
    pub(crate) fn parts(&self) -> Option<Parts> {
        if self.overflow {
            return None;
        }
        Some(Parts {
            objects: self.objects,
            counts: self.counts,
            props: self.props,
            values: self.values,
            n_objects: self.n_objects,
            n_props: self.n_props,
        })
    }
}

/// A copy of the arrays of a [`Request`], which the ioctl may write.
pub(crate) struct Parts {
    pub objects: [u32; OBJECTS_MAX],
    pub counts: [u32; OBJECTS_MAX],
    pub props: [u32; PROPS_MAX],
    pub values: [u64; PROPS_MAX],
    pub n_objects: usize,
    pub n_props: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_group_by_object() {
        let mut r = Request::new();
        r.add(10, 1, 100);
        r.add(10, 2, 200);
        r.add(20, 3, 300);
        assert_eq!(&r.objects[..r.n_objects], &[10, 20]);
        assert_eq!(&r.counts[..r.n_objects], &[2, 1]);
        assert_eq!(&r.props[..r.n_props], &[1, 2, 3]);
        assert_eq!(&r.values[..r.n_props], &[100, 200, 300]);
        assert!(!r.overflow);
    }

    #[test]
    fn parts_hold_the_used_entries() {
        let mut r = Request::new();
        r.add(10, 1, 100);
        let p = r.parts().unwrap();
        assert_eq!((p.n_objects, p.n_props), (1, 1));
        assert_eq!(
            (p.objects[0], p.counts[0], p.props[0], p.values[0]),
            (10, 1, 1, 100)
        );
    }

    #[test]
    fn too_many_entries_mark_the_request() {
        let mut r = Request::new();
        for i in 0..=PROPS_MAX as u32 {
            r.add(1, i, 0);
        }
        assert!(r.overflow);
        assert!(r.parts().is_none());
        let mut r = Request::new();
        for i in 0..=OBJECTS_MAX as u32 {
            r.add(i, 1, 0);
        }
        assert!(r.overflow);
    }
}
