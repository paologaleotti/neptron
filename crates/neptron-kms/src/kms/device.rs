//! The DRM device and the choice of connector, CRTC and primary plane. It
//! only reads: nothing here changes the state of the display.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::linux::fs::MetadataExt as _;
use std::os::unix::fs::FileTypeExt as _;

use drm::control::{
    Device as ControlDevice, ResourceHandle, ResourceHandles, connector, crtc, plane, property,
};
use drm::{ClientCapability, Device};

use super::KmsError;
use crate::{ConnectorClass, ConnectorFacts, DRM_MAJOR, choose, is_drm_node};

/// An open DRM device. It owns its descriptor.
pub(super) struct Card(OwnedFd);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl Device for Card {}
impl ControlDevice for Card {}

/// `DRM_PLANE_TYPE_PRIMARY`
const PLANE_TYPE_PRIMARY: u64 = 1;

impl Card {
    /// Checks that `fd` is a DRM device node and turns on the atomic API.
    pub(super) fn new(fd: OwnedFd, writeback: bool) -> Result<Card, KmsError> {
        let file = std::fs::File::from(fd);
        let meta = file
            .metadata()
            .map_err(KmsError::io("read the descriptor"))?;
        if !is_drm_node(meta.file_type().is_char_device(), meta.st_rdev()) {
            return Err(KmsError::NotDrm {
                reason: format!("not a character device of major {DRM_MAJOR}"),
            });
        }
        let card = Card(OwnedFd::from(file));
        card.set_client_capability(ClientCapability::UniversalPlanes, true)
            .map_err(KmsError::io("turn on universal planes"))?;
        card.set_client_capability(ClientCapability::Atomic, true)
            .map_err(KmsError::io("turn on the atomic API"))?;
        if writeback {
            card.set_client_capability(ClientCapability::WritebackConnectors, true)
                .map_err(KmsError::io("turn on writeback connectors"))?;
        }
        Ok(card)
    }

    /// A second descriptor on the same open file description.
    pub(super) fn duplicate(&self) -> Result<OwnedFd, KmsError> {
        self.0
            .try_clone()
            .map_err(KmsError::io("duplicate the descriptor"))
    }

    /// The driver name, such as `i915` or `vkms`.
    pub(super) fn driver_name(&self) -> String {
        self.get_driver().map_or_else(
            |e| format!("unknown ({e})"),
            |d| d.name().to_string_lossy().into_owned(),
        )
    }

    fn properties<H: ResourceHandle>(
        &self,
        handle: H,
    ) -> Result<Vec<(String, u32, u64)>, KmsError> {
        let set = self
            .get_properties(handle)
            .map_err(KmsError::io("read the properties of a DRM object"))?;
        let (ids, values) = set.as_props_and_values();
        let mut out = Vec::with_capacity(ids.len());
        for (&id, &value) in ids.iter().zip(values) {
            let info = self
                .get_property(id)
                .map_err(KmsError::io("read a property description"))?;
            out.push((
                info.name().to_string_lossy().into_owned(),
                u32::from(id),
                value,
            ));
        }
        Ok(out)
    }
}

/// The objects the display drives, by kernel id, and their property ids.
#[derive(Debug, Clone)]
pub(super) struct Objects {
    pub connector: u32,
    pub connector_name: String,
    /// The `subconnector` property of the connector, by its enum name.
    pub subconnector: Option<String>,
    pub crtc: u32,
    pub plane: u32,
    pub props: Props,
    pub writeback: Option<Writeback>,
}

/// The atomic property ids the display sets.
#[derive(Debug, Clone, Copy)]
pub(super) struct Props {
    pub connector_crtc_id: u32,
    pub crtc_mode_id: u32,
    pub crtc_active: u32,
    /// In [`PLANE_PROPS`] order.
    pub plane: [u32; PLANE_PROPS.len()],
}

/// A writeback connector and its property ids.
#[derive(Debug, Clone, Copy)]
pub(super) struct Writeback {
    pub connector: u32,
    pub crtc_id: u32,
    pub fb_id: u32,
    pub out_fence_ptr: u32,
}

/// The plane properties, in the order of [`Props::plane`].
pub(super) const PLANE_PROPS: [&str; 10] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
];
/// Index of `FB_ID` in [`PLANE_PROPS`].
pub(super) const PLANE_FB_ID: usize = 0;
/// Index of `CRTC_ID` in [`PLANE_PROPS`].
pub(super) const PLANE_CRTC_ID: usize = 1;

/// Chooses the connector with the rule of the Definition, then its CRTC
/// and its primary plane.
pub(super) fn select(
    card: &Card,
    named: Option<&str>,
    crt: bool,
    writeback: bool,
) -> Result<Objects, KmsError> {
    let res = card
        .resource_handles()
        .map_err(KmsError::io("read the DRM resources"))?;
    let mut infos = Vec::new();
    let mut facts = Vec::new();
    for &handle in res.connectors() {
        let info = card
            .get_connector(handle, true)
            .map_err(KmsError::io("probe a connector"))?;
        let has_edid = card
            .properties(handle)?
            .iter()
            .any(|(name, _, value)| name == "EDID" && *value != 0);
        facts.push(ConnectorFacts {
            name: format!("{}-{}", info.interface().as_str(), info.interface_id()),
            class: ConnectorClass::of_type(u32::from(info.interface())),
            connected: info.state() == connector::State::Connected,
            has_edid,
        });
        infos.push(info);
    }
    let index = choose(&facts, named, crt)?;
    let info = &infos[index];
    let name = facts[index].name.clone();
    let crtc = pick_crtc(card, &res, info, &infos, &name)?;
    for (other, other_facts) in infos.iter().zip(&facts) {
        if other.handle() != info.handle()
            && other_facts.class != ConnectorClass::Writeback
            && current_crtc(card, other)? == Some(crtc)
        {
            return Err(KmsError::SharedCrtc {
                connector: name,
                other: other_facts.name.clone(),
            });
        }
    }
    let plane = primary_plane(card, &res, crtc)?;
    let writeback = if writeback {
        let wb = infos
            .iter()
            .zip(&facts)
            .find(|(_, f)| f.class == ConnectorClass::Writeback)
            .ok_or(KmsError::NoWriteback)?;
        let handle = wb.0.handle();
        Some(Writeback {
            connector: handle.into(),
            crtc_id: prop(card, handle, "writeback connector", "CRTC_ID")?,
            fb_id: prop(card, handle, "writeback connector", "WRITEBACK_FB_ID")?,
            out_fence_ptr: prop(
                card,
                handle,
                "writeback connector",
                "WRITEBACK_OUT_FENCE_PTR",
            )?,
        })
    } else {
        None
    };
    let mut plane_props = [0; PLANE_PROPS.len()];
    for (slot, name) in plane_props.iter_mut().zip(PLANE_PROPS) {
        *slot = prop(card, plane, "plane", name)?;
    }
    Ok(Objects {
        connector: info.handle().into(),
        connector_name: name,
        subconnector: subconnector(card, info.handle()),
        crtc: crtc.into(),
        plane: plane.into(),
        props: Props {
            connector_crtc_id: prop(card, info.handle(), "connector", "CRTC_ID")?,
            crtc_mode_id: prop(card, crtc, "CRTC", "MODE_ID")?,
            crtc_active: prop(card, crtc, "CRTC", "ACTIVE")?,
            plane: plane_props,
        },
        writeback,
    })
}

/// The enum name of the `subconnector` property: the downstream port type
/// of a DisplayPort connector (`VGA` behind a DAC) or the signal of a DVI-I
/// connector. `None` when the connector has no such property or a read
/// fails: the property is a hint for the caller's class, and the EDID
/// still decides without it.
fn subconnector(card: &Card, handle: connector::Handle) -> Option<String> {
    let set = card.get_properties(handle).ok()?;
    let (ids, values) = set.as_props_and_values();
    ids.iter().zip(values).find_map(|(&id, &value)| {
        let info = card.get_property(id).ok()?;
        if info.name().to_bytes() != SUBCONNECTOR.as_bytes() {
            return None;
        }
        let property::ValueType::Enum(names) = info.value_type() else {
            return None;
        };
        names
            .get_value_from_raw_value(value)
            .map(|v| v.name().to_string_lossy().into_owned())
    })
}

/// The name of the connector property that gives the downstream port type.
const SUBCONNECTOR: &str = "subconnector";

fn current_crtc(card: &Card, info: &connector::Info) -> Result<Option<crtc::Handle>, KmsError> {
    let Some(enc) = info.current_encoder() else {
        return Ok(None);
    };
    Ok(card
        .get_encoder(enc)
        .map_err(KmsError::io("read an encoder"))?
        .crtc())
}

/// The CRTC the connector uses now, or else a CRTC that no connector uses
/// and that one of the connector's encoders can drive.
fn pick_crtc(
    card: &Card,
    res: &ResourceHandles,
    info: &connector::Info,
    all: &[connector::Info],
    name: &str,
) -> Result<crtc::Handle, KmsError> {
    if let Some(crtc) = current_crtc(card, info)? {
        return Ok(crtc);
    }
    let mut busy = Vec::new();
    for other in all {
        if let Some(c) = current_crtc(card, other)? {
            busy.push(c);
        }
    }
    for &enc in info.encoders() {
        let e = card
            .get_encoder(enc)
            .map_err(KmsError::io("read an encoder"))?;
        if let Some(crtc) = res
            .filter_crtcs(e.possible_crtcs())
            .into_iter()
            .find(|c| !busy.contains(c))
        {
            return Ok(crtc);
        }
    }
    Err(KmsError::NoCrtc {
        connector: name.to_owned(),
    })
}

/// The primary plane on the CRTC, or else a primary plane that can feed it
/// and feeds no CRTC now.
fn primary_plane(
    card: &Card,
    res: &ResourceHandles,
    crtc: crtc::Handle,
) -> Result<plane::Handle, KmsError> {
    let mut free = None;
    for p in card
        .plane_handles()
        .map_err(KmsError::io("list the planes"))?
    {
        let info = card.get_plane(p).map_err(KmsError::io("read a plane"))?;
        if !res.filter_crtcs(info.possible_crtcs()).contains(&crtc) {
            continue;
        }
        let primary = card
            .properties(p)?
            .iter()
            .any(|(name, _, value)| name == "type" && *value == PLANE_TYPE_PRIMARY);
        if !primary {
            continue;
        }
        match info.crtc() {
            Some(c) if c == crtc => return Ok(p),
            Some(_) => {}
            None => {
                free.get_or_insert(p);
            }
        }
    }
    free.ok_or(KmsError::NoPrimaryPlane { crtc: crtc.into() })
}

fn prop<H: ResourceHandle>(
    card: &Card,
    handle: H,
    object: &'static str,
    name: &'static str,
) -> Result<u32, KmsError> {
    card.properties(handle)?
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, id, _)| id)
        .ok_or(KmsError::MissingProperty { object, name })
}
