//! An atomic request in fixed arrays, so that a page flip allocates
//! nothing.

use crate::PlaneRect;
use crate::tv::TvWrite;

/// The most objects of one request: connector, writeback connector, CRTC
/// and plane.
const OBJECTS_MAX: usize = 4;
/// The most properties of one request: a modeset with writeback on a TV
/// connector sets 1 + 1 + 4 on the connector (`CRTC_ID`, `TV mode`, the
/// four margins), 1 on the writeback connector, 2 on the CRTC and 10 on
/// the plane.
const PROPS_MAX: usize = 19;

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
    /// ids and the values, as the atomic ioctl takes them.
    ///
    /// # Errors
    ///
    /// [`RequestFull`] when [`add`](Self::add) ran out of room.
    pub(crate) fn parts(&self) -> Result<Parts, RequestFull> {
        if self.overflow {
            return Err(RequestFull);
        }
        Ok(Parts {
            objects: self.objects,
            counts: self.counts,
            props: self.props,
            values: self.values,
            n_objects: self.n_objects,
            n_props: self.n_props,
        })
    }
}

/// The object and property ids that [`output_off`] sets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutputIds {
    pub connector: u32,
    /// The connector's `CRTC_ID` property.
    pub connector_crtc_id: u32,
    /// The writeback connector and its `CRTC_ID` property, when it is
    /// routed.
    pub writeback: Option<(u32, u32)>,
    pub crtc: u32,
    /// The CRTC's `MODE_ID` property.
    pub crtc_mode_id: u32,
    /// The CRTC's `ACTIVE` property.
    pub crtc_active: u32,
    pub plane: u32,
    /// The plane's `FB_ID` property.
    pub plane_fb_id: u32,
    /// The plane's `CRTC_ID` property.
    pub plane_crtc_id: u32,
}

/// The request that turns the output off: the connectors detached, the CRTC
/// with no mode and inactive, the plane with no framebuffer and no CRTC.
/// Every value is 0, so this request can never set a mode. It needs
/// `ALLOW_MODESET`.
pub(crate) fn output_off(ids: &OutputIds) -> Request {
    let mut req = Request::new();
    req.add(ids.connector, ids.connector_crtc_id, 0);
    if let Some((connector, crtc_id)) = ids.writeback {
        req.add(connector, crtc_id, 0);
    }
    req.add(ids.crtc, ids.crtc_mode_id, 0);
    req.add(ids.crtc, ids.crtc_active, 0);
    req.add(ids.plane, ids.plane_fb_id, 0);
    req.add(ids.plane, ids.plane_crtc_id, 0);
    req
}

/// The object and property ids that [`modeset`] sets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ModesetIds {
    pub connector: u32,
    /// The connector's `CRTC_ID` property.
    pub connector_crtc_id: u32,
    /// The connector's `left margin`, `right margin`, `top margin` and
    /// `bottom margin` properties, each when it has it.
    pub margins: [Option<u32>; 4],
    /// The writeback connector and its `CRTC_ID` property, when it is
    /// routed.
    pub writeback: Option<(u32, u32)>,
    pub crtc: u32,
    /// The CRTC's `MODE_ID` property.
    pub crtc_mode_id: u32,
    /// The CRTC's `ACTIVE` property.
    pub crtc_active: u32,
    pub plane: u32,
    /// The plane's `FB_ID`, `CRTC_ID`, `SRC_X`, `SRC_Y`, `SRC_W`, `SRC_H`,
    /// `CRTC_X`, `CRTC_Y`, `CRTC_W` and `CRTC_H` properties.
    pub plane_props: [u32; 10],
}

/// The request that sets the mode blob `blob`: the connector on the CRTC
/// with its `TV mode` value `tv` and every margin at 0, the writeback
/// connector on the CRTC, the mode and `ACTIVE` 1 on the CRTC, and the
/// plane with the framebuffer `fb` in `rect`. It needs `ALLOW_MODESET`.
///
/// The margins shrink the plane in the scaler of a TV encoder, which would
/// resample the lines, so each modeset sets them to 0.
pub(crate) fn modeset(
    ids: &ModesetIds,
    tv: Option<TvWrite>,
    blob: u64,
    fb: u32,
    rect: &PlaneRect,
) -> Request {
    let crtc = u64::from(ids.crtc);
    let mut req = Request::new();
    req.add(ids.connector, ids.connector_crtc_id, crtc);
    if let Some(tv) = tv {
        req.add(ids.connector, tv.prop, tv.value);
    }
    for &margin in ids.margins.iter().flatten() {
        req.add(ids.connector, margin, 0);
    }
    if let Some((connector, crtc_id)) = ids.writeback {
        req.add(connector, crtc_id, crtc);
    }
    req.add(ids.crtc, ids.crtc_mode_id, blob);
    req.add(ids.crtc, ids.crtc_active, 1);
    let values = [
        u64::from(fb),
        crtc,
        rect.src[0],
        rect.src[1],
        rect.src[2],
        rect.src[3],
        rect.crtc[0],
        rect.crtc[1],
        rect.crtc[2],
        rect.crtc[3],
    ];
    for (&prop, value) in ids.plane_props.iter().zip(values) {
        req.add(ids.plane, prop, value);
    }
    req
}

/// A request had more objects or properties than its fixed arrays. It is
/// never sent; this is a fault of the backend, not of the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "the atomic request needs more than {OBJECTS_MAX} objects or {PROPS_MAX} properties; it was \
     not sent"
)]
pub(crate) struct RequestFull;

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
        assert!(Request::new().parts().is_ok());
        assert_eq!((p.n_objects, p.n_props), (1, 1));
        assert_eq!(
            (p.objects[0], p.counts[0], p.props[0], p.values[0]),
            (10, 1, 1, 100)
        );
    }

    fn ids(writeback: Option<(u32, u32)>) -> OutputIds {
        OutputIds {
            connector: 10,
            connector_crtc_id: 11,
            writeback,
            crtc: 30,
            crtc_mode_id: 31,
            crtc_active: 32,
            plane: 40,
            plane_fb_id: 41,
            plane_crtc_id: 42,
        }
    }

    #[test]
    fn output_off_detaches_everything_with_zeros() {
        let p = output_off(&ids(None)).parts().unwrap();
        assert_eq!(&p.objects[..p.n_objects], &[10, 30, 40]);
        assert_eq!(&p.counts[..p.n_objects], &[1, 2, 2]);
        assert_eq!(&p.props[..p.n_props], &[11, 31, 32, 41, 42]);
        assert!(p.values[..p.n_props].iter().all(|&v| v == 0));
    }

    #[test]
    fn output_off_detaches_the_writeback_connector_too() {
        let p = output_off(&ids(Some((20, 21)))).parts().unwrap();
        assert_eq!(&p.objects[..p.n_objects], &[10, 20, 30, 40]);
        assert_eq!(&p.props[..p.n_props], &[11, 21, 31, 32, 41, 42]);
        assert!(p.values[..p.n_props].iter().all(|&v| v == 0));
    }

    fn modeset_ids(writeback: Option<(u32, u32)>, margins: [Option<u32>; 4]) -> ModesetIds {
        ModesetIds {
            connector: 10,
            connector_crtc_id: 11,
            margins,
            writeback,
            crtc: 30,
            crtc_mode_id: 31,
            crtc_active: 32,
            plane: 40,
            plane_props: [41, 42, 43, 44, 45, 46, 47, 48, 49, 50],
        }
    }

    fn rect() -> PlaneRect {
        PlaneRect::for_mode(720, 240, 1920, 576).unwrap()
    }

    const TV_MODE: TvWrite = TvWrite { prop: 12, value: 3 };
    const MARGINS: [Option<u32>; 4] = [Some(13), Some(14), Some(15), Some(16)];

    #[test]
    fn a_modeset_sets_the_connector_the_crtc_and_the_plane() {
        let p = modeset(&modeset_ids(None, [None; 4]), None, 99, 7, &rect())
            .parts()
            .unwrap();
        assert_eq!(&p.objects[..p.n_objects], &[10, 30, 40]);
        assert_eq!(&p.counts[..p.n_objects], &[1, 2, 10]);
        assert_eq!(&p.props[..3], &[11, 31, 32]);
        assert_eq!(&p.values[..3], &[30, 99, 1]);
        let r = rect();
        assert_eq!(
            &p.values[3..p.n_props],
            &[
                7, 30, r.src[0], r.src[1], r.src[2], r.src[3], r.crtc[0], r.crtc[1], r.crtc[2],
                r.crtc[3]
            ]
        );
    }

    #[test]
    fn the_tv_mode_and_the_margins_go_in_the_same_request_as_the_mode() {
        let p = modeset(
            &modeset_ids(Some((20, 21)), MARGINS),
            Some(TV_MODE),
            99,
            7,
            &rect(),
        )
        .parts()
        .unwrap();
        assert_eq!(&p.objects[..p.n_objects], &[10, 20, 30, 40]);
        assert_eq!(&p.counts[..p.n_objects], &[6, 1, 2, 10]);
        assert_eq!(&p.props[..9], &[11, 12, 13, 14, 15, 16, 21, 31, 32]);
        assert_eq!(&p.values[..9], &[30, 3, 0, 0, 0, 0, 30, 99, 1]);
        assert_eq!(p.n_props, PROPS_MAX);
    }

    #[test]
    fn only_the_margins_the_connector_has_are_set() {
        let p = modeset(
            &modeset_ids(None, [None, Some(14), None, Some(16)]),
            Some(TV_MODE),
            99,
            7,
            &rect(),
        )
        .parts()
        .unwrap();
        assert_eq!(&p.counts[..p.n_objects], &[4, 2, 10]);
        assert_eq!(&p.props[..4], &[11, 12, 14, 16]);
        assert_eq!(&p.values[..4], &[30, 3, 0, 0]);
        let p = modeset(&modeset_ids(None, MARGINS), None, 99, 7, &rect())
            .parts()
            .unwrap();
        assert_eq!(&p.props[..5], &[11, 13, 14, 15, 16]);
        assert_eq!(&p.values[..5], &[30, 0, 0, 0, 0]);
    }

    #[test]
    fn too_many_entries_mark_the_request() {
        let mut r = Request::new();
        for i in 0..=PROPS_MAX as u32 {
            r.add(1, i, 0);
        }
        assert!(r.overflow);
        assert_eq!(r.parts().err(), Some(RequestFull));
        assert_eq!(
            RequestFull.to_string(),
            "the atomic request needs more than 4 objects or 19 properties; it was not sent"
        );
        let mut r = Request::new();
        for i in 0..=OBJECTS_MAX as u32 {
            r.add(i, 1, 0);
        }
        assert!(r.overflow);
    }
}
