//! The disc control calls of the host: the number of discs, the disc in the
//! drive, the tray, and discs added after the load.

use std::path::Path;

use super::Host;
use crate::content::c_path;
use crate::error::DiscError;
use crate::ffi::{DiscCallbacks, Poisoned};

/// What the disc control interface of the core says now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscState {
    /// The number of discs that the core holds.
    pub count: u32,
    /// The index of the disc in the drive, from 0. It can equal `count`
    /// when the core holds no disc in the drive.
    pub index: u32,
    /// The tray is open.
    pub tray_open: bool,
}

impl Host {
    /// The state of the disc control interface, after the load. `None`
    /// when the core gave no interface, or one with no
    /// `get_num_images`, `get_image_index` or `get_eject_state`.
    pub fn disc_state(&mut self) -> Option<DiscState> {
        let callbacks = self.disc_callbacks().ok()?;
        self.call(|_| {
            Some(DiscState {
                count: callbacks.count()?,
                index: callbacks.index()?,
                tray_open: callbacks.tray_open()?,
            })
        })
        .ok()
        .flatten()
    }

    /// Opens or closes the tray (`set_eject_state`).
    pub fn set_disc_tray(&mut self, open: bool) -> Result<(), DiscError> {
        let callbacks = self.disc_callbacks()?;
        let done = self.call(|_| callbacks.set_tray(open)).map_err(poisoned)?;
        answer(done, "set_eject_state")
    }

    /// Puts disc `index` in the drive (`set_image_index`). libretro.h asks
    /// for the tray to be open first.
    pub fn set_disc_index(&mut self, index: u32) -> Result<(), DiscError> {
        let callbacks = self.disc_callbacks()?;
        let count =
            self.call(|_| callbacks.count())
                .map_err(poisoned)?
                .ok_or(DiscError::MissingCall {
                    call: "get_num_images",
                })?;
        if index >= count {
            return Err(DiscError::NoSuchDisc { index, count });
        }
        let done = self
            .call(|_| callbacks.set_index(index))
            .map_err(poisoned)?;
        answer(done, "set_image_index")
    }

    /// Adds the disc at `path` after the discs that the core holds
    /// (`add_image_index`, then `replace_image_index` with the path).
    /// Returns its index.
    pub fn add_disc(&mut self, path: &Path) -> Result<u32, DiscError> {
        let callbacks = self.disc_callbacks()?;
        let c_path = c_path(path).map_err(|_| DiscError::InvalidPath {
            path: path.to_owned(),
        })?;
        let added = self.call(|_| callbacks.add()).map_err(poisoned)?;
        answer(added, "add_image_index")?;
        let index = self
            .call(|_| callbacks.count())
            .map_err(poisoned)?
            .ok_or(DiscError::MissingCall {
                call: "get_num_images",
            })?
            .checked_sub(1)
            .ok_or(DiscError::Refused {
                call: "add_image_index",
            })?;
        let replaced = self
            .call(|_| callbacks.replace(index, &c_path))
            .map_err(poisoned)?;
        // The core may keep the pointer of the path, so it stays.
        self.disc_paths.push(c_path);
        answer(replaced, "replace_image_index")?;
        Ok(index)
    }

    fn disc_callbacks(&mut self) -> Result<DiscCallbacks, DiscError> {
        if !self.game_loaded {
            return Err(DiscError::NoGame);
        }
        self.begin_call();
        let state = self.state().ok_or(DiscError::HostPoisoned)?;
        state.disc().ok_or(DiscError::NoInterface)
    }
}

fn answer(done: Option<bool>, call: &'static str) -> Result<(), DiscError> {
    match done {
        Some(true) => Ok(()),
        Some(false) => Err(DiscError::Refused { call }),
        None => Err(DiscError::MissingCall { call }),
    }
}

fn poisoned(_: Poisoned) -> DiscError {
    DiscError::HostPoisoned
}
