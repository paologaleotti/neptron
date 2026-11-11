//! The DRM/KMS side of the spike: Linux only, behind the `kms` feature.
//!
//! It decides nothing: the plan says which modes are valid, `output` picks
//! the connector, and this module executes, measures and restores.

mod card;
mod restore;
mod show;

use std::fmt;
use std::io;

pub use card::list_connectors;
pub use show::{Inputs, run};

use crate::output::ChoiceError;
use crate::supervisor::SupervisorGone;

/// An error of the KMS side, with what it was doing.
#[derive(Debug)]
pub enum KmsError {
    /// A DRM call failed.
    Io {
        /// What the tool was doing.
        what: String,
        /// The kernel error.
        source: io::Error,
    },
    /// No connector can be chosen.
    Choice(ChoiceError),
    /// No CRTC can drive the connector.
    NoCrtc {
        /// The connector name.
        connector: String,
    },
    /// The CRTC of the connector drives another connector too, which would
    /// get the test modes as well.
    SharedCrtc {
        /// The chosen connector.
        connector: String,
        /// The other connector on the same CRTC.
        other: String,
    },
    /// No free primary plane can feed the CRTC.
    NoPrimaryPlane {
        /// The CRTC object id.
        crtc_id: u32,
    },
    /// A DRM object lacks an atomic property the spike needs.
    MissingProperty {
        /// The kind of object.
        object: &'static str,
        /// The property name.
        name: &'static str,
    },
    /// The supervisor process is gone.
    Supervisor(SupervisorGone),
}

impl KmsError {
    fn io(what: impl Into<String>) -> impl FnOnce(io::Error) -> KmsError {
        let what = what.into();
        move |source| KmsError::Io { what, source }
    }
}

impl fmt::Display for KmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KmsError::Io { what, source } => write!(f, "cannot {what}: {source}"),
            KmsError::Choice(e) => e.fmt(f),
            KmsError::NoCrtc { connector } => write!(f, "no free CRTC can drive {connector}"),
            KmsError::SharedCrtc { connector, other } => write!(
                f,
                "the CRTC of {connector} also drives {other}; switch {other} off first, \
                 because it would get the test modes too"
            ),
            KmsError::NoPrimaryPlane { crtc_id } => {
                write!(f, "no free primary plane can feed CRTC {crtc_id}")
            }
            KmsError::MissingProperty { object, name } => {
                write!(
                    f,
                    "the {object} has no {name} property; atomic modesetting needs it"
                )
            }
            KmsError::Supervisor(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for KmsError {}

impl From<SupervisorGone> for KmsError {
    fn from(e: SupervisorGone) -> KmsError {
        KmsError::Supervisor(e)
    }
}
