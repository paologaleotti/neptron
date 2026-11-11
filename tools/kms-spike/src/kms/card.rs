//! The DRM device and the choice of connector, CRTC and primary plane.

use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use drm::control::{
    Device as ControlDevice, ResourceHandle, ResourceHandles, connector, crtc, encoder, plane,
    property,
};
use drm::{ClientCapability, Device};

use super::KmsError;
use crate::output::{self, Connector, State};

/// Lists the connectors of `device` with their state and CRTC. It sets no
/// mode.
pub fn list_connectors(device: &Path) -> Result<String, KmsError> {
    let card = Card::open(device)?;
    let res = card
        .resource_handles()
        .map_err(KmsError::io("read the DRM resources"))?;
    let (infos, list) = connectors(&card, &res)?;
    let mut out = format!("connectors of {}:\n", device.display());
    for (info, c) in infos.iter().zip(&list) {
        let crtc = current_crtc(&card, info)?
            .map_or_else(|| "none".to_owned(), |h| u32::from(h).to_string());
        out.push_str(&format!(
            "  {} (id {}): {}, CRTC {}, {} modes listed\n",
            c.name,
            u32::from(info.handle()),
            c.state.name(),
            crtc,
            info.modes().len()
        ));
    }
    Ok(out)
}

/// An open DRM device.
pub(super) struct Card(File);

/// The display objects of a run and their property ids.
pub(super) struct Objects {
    pub connector: connector::Handle,
    pub connector_info: Connector,
    pub encoder: Option<encoder::Handle>,
    pub crtc: crtc::Handle,
    pub crtc_index: u32,
    pub plane: plane::Handle,
    pub props: Props,
}

/// The atomic property ids the spike sets.
pub(super) struct Props {
    pub connector_crtc_id: property::Handle,
    pub crtc_mode_id: property::Handle,
    pub crtc_active: property::Handle,
    /// `FB_ID`, `CRTC_ID`, `SRC_X`, `SRC_Y`, `SRC_W`, `SRC_H`, `CRTC_X`,
    /// `CRTC_Y`, `CRTC_W`, `CRTC_H`, in [`PLANE_PROPS`] order.
    pub plane: [property::Handle; PLANE_PROPS.len()],
}

/// The plane properties the spike sets and restores.
pub(super) const PLANE_PROPS: [&str; 10] = [
    "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W", "CRTC_H",
];

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl Device for Card {}
impl ControlDevice for Card {}

impl Card {
    /// Opens the device read-write and non-blocking.
    pub(super) fn open(path: &Path) -> Result<Card, KmsError> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(path)
            .map(Card)
            .map_err(KmsError::io(format!("open {}", path.display())))
    }

    /// Turns on universal planes and the atomic API.
    pub(super) fn enable_atomic(&self) -> Result<(), KmsError> {
        self.set_client_capability(ClientCapability::UniversalPlanes, true)
            .map_err(KmsError::io("turn on universal planes"))?;
        self.set_client_capability(ClientCapability::Atomic, true)
            .map_err(KmsError::io(
                "turn on the atomic API (the driver may not support atomic modesetting)",
            ))
    }

    /// Every property of `handle` with its name, id and value.
    pub(super) fn properties<H: ResourceHandle>(
        &self,
        handle: H,
    ) -> Result<Vec<(String, property::Handle, u64)>, KmsError> {
        let set = self
            .get_properties(handle)
            .map_err(KmsError::io("read the properties of a DRM object"))?;
        let (ids, values) = set.as_props_and_values();
        let mut out = Vec::with_capacity(ids.len());
        for (&id, &value) in ids.iter().zip(values) {
            let info = self
                .get_property(id)
                .map_err(KmsError::io("read a property description"))?;
            out.push((info.name().to_string_lossy().into_owned(), id, value));
        }
        Ok(out)
    }
}

/// Selects the connector, the CRTC and the primary plane on `card`.
pub(super) fn select(card: &Card, name: Option<&str>) -> Result<Objects, KmsError> {
    let res = card
        .resource_handles()
        .map_err(KmsError::io("read the DRM resources"))?;
    let (infos, list) = connectors(card, &res)?;
    let index = output::choose(&list, name).map_err(KmsError::Choice)?;
    let info = &infos[index];
    let chosen = &list[index];
    let (encoder, crtc) = pick_crtc(card, &res, info, &infos, &chosen.name)?;
    for (other, other_info) in infos.iter().zip(&list) {
        if other.handle() != info.handle() && current_crtc(card, other)? == Some(crtc) {
            return Err(KmsError::SharedCrtc {
                connector: chosen.name.clone(),
                other: other_info.name.clone(),
            });
        }
    }
    let crtc_index = res
        .crtcs()
        .iter()
        .position(|&c| c == crtc)
        .and_then(|i| u32::try_from(i).ok())
        .ok_or(KmsError::NoCrtc {
            connector: chosen.name.clone(),
        })?;
    let plane = primary_plane(card, &res, crtc)?;
    let props = Props {
        connector_crtc_id: prop(card, info.handle(), "connector", "CRTC_ID")?,
        crtc_mode_id: prop(card, crtc, "CRTC", "MODE_ID")?,
        crtc_active: prop(card, crtc, "CRTC", "ACTIVE")?,
        plane: plane_props(card, plane)?,
    };
    Ok(Objects {
        connector: info.handle(),
        connector_info: chosen.clone(),
        encoder,
        crtc,
        crtc_index,
        plane,
        props,
    })
}

/// `O_NONBLOCK` on Linux for x86, x86_64, arm and aarch64. A read of a DRM
/// device opened with it returns `WouldBlock` when no event is pending, so
/// the event drain never blocks. The standard library has no constant for
/// it, and the dependency rules allow no `libc`.
const O_NONBLOCK: i32 = 0o4000;
/// `DRM_PLANE_TYPE_PRIMARY`
const PLANE_TYPE_PRIMARY: u64 = 1;

fn connectors(
    card: &Card,
    res: &ResourceHandles,
) -> Result<(Vec<connector::Info>, Vec<Connector>), KmsError> {
    let mut infos = Vec::new();
    let mut list = Vec::new();
    for &handle in res.connectors() {
        let info = card
            .get_connector(handle, true)
            .map_err(KmsError::io("probe a connector"))?;
        list.push(Connector {
            name: format!("{}-{}", info.interface().as_str(), info.interface_id()),
            state: match info.state() {
                connector::State::Connected => State::Connected,
                connector::State::Disconnected => State::Disconnected,
                connector::State::Unknown => State::Unknown,
            },
        });
        infos.push(info);
    }
    Ok((infos, list))
}

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
) -> Result<(Option<encoder::Handle>, crtc::Handle), KmsError> {
    if let Some(crtc) = current_crtc(card, info)? {
        return Ok((info.current_encoder(), crtc));
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
            return Ok((Some(enc), crtc));
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
    free.ok_or(KmsError::NoPrimaryPlane {
        crtc_id: crtc.into(),
    })
}

fn prop<H: ResourceHandle>(
    card: &Card,
    handle: H,
    object: &'static str,
    name: &'static str,
) -> Result<property::Handle, KmsError> {
    card.properties(handle)?
        .into_iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, id, _)| id)
        .ok_or(KmsError::MissingProperty { object, name })
}

fn plane_props(
    card: &Card,
    plane: plane::Handle,
) -> Result<[property::Handle; PLANE_PROPS.len()], KmsError> {
    let all = card.properties(plane)?;
    let mut out = Vec::with_capacity(PLANE_PROPS.len());
    for name in PLANE_PROPS {
        let id = all
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, id, _)| *id)
            .ok_or(KmsError::MissingProperty {
                object: "plane",
                name,
            })?;
        out.push(id);
    }
    out.try_into().map_err(|_| KmsError::MissingProperty {
        object: "plane",
        name: "FB_ID",
    })
}
