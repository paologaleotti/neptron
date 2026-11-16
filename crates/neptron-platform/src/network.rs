//! The network status file of the image, `/run/neptron/network.toml`: the
//! host name and the IPv4 address of each wired port with a lease.

use std::net::Ipv4Addr;

use serde::Deserialize;
use thiserror::Error;

/// The version of the network file that this build reads.
pub const NETWORK_FILE_VERSION: u32 = 1;

/// The longest host name, in characters, as a DNS name allows it.
pub const HOST_NAME_MAX_CHARS: usize = 253;

/// The longest label of a host name (the text between two dots), in
/// characters.
pub const HOST_LABEL_MAX_CHARS: usize = 63;

/// The largest network file, in bytes. The file of a machine with every
/// port of [`ADDRESSES_MAX`] is far smaller; a larger file is damaged.
pub const NETWORK_FILE_MAX_BYTES: usize = 64 * 1024;

/// The most address tables in one file. A machine has a few wired ports.
pub const ADDRESSES_MAX: usize = 16;

/// The longest port name, in characters: `IFNAMSIZ` less its final zero.
pub const PORT_NAME_MAX_CHARS: usize = 15;

/// What the network file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// The system host name. `None` when the file holds an empty name or
    /// one that is not a host name: the addresses stay usable.
    pub host_name: Option<String>,
    /// One address for each wired port with a lease, in the order of the
    /// file. Empty when no port has an address.
    pub addresses: Vec<PortAddress>,
}

/// The address of one wired port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortAddress {
    /// The port, for example `eth0`.
    pub port: String,
    /// Its IPv4 address.
    pub ipv4: Ipv4Addr,
}

/// Why the network file cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkError {
    /// The text is not TOML, or a field has the wrong type.
    #[error("the network file does not parse: {reason}")]
    Parse {
        /// The parser's message.
        reason: String,
    },
    /// The file has no `version`.
    #[error("the network file has no version")]
    NoVersion,
    /// `version` is not an integer.
    #[error("the version of the network file is a {found}, not an integer")]
    VersionType {
        /// The TOML type of the value.
        found: &'static str,
    },
    /// The file has a version that this build does not read.
    #[error("the network file has version {found}; this build reads version {supported}")]
    Version {
        /// The version of the file.
        found: i64,
        /// The version this build reads.
        supported: u32,
    },
    /// The text is larger than [`NETWORK_FILE_MAX_BYTES`].
    #[error("the network file has {bytes} bytes, more than {NETWORK_FILE_MAX_BYTES}")]
    TooLarge {
        /// The size of the text, in bytes.
        bytes: usize,
    },
    /// The file has more than [`ADDRESSES_MAX`] address tables.
    #[error("the network file has {count} addresses, more than {ADDRESSES_MAX}")]
    TooManyAddresses {
        /// The number of address tables.
        count: usize,
    },
    /// Two address tables name the same port.
    #[error("the network file has two addresses for the port {port}")]
    DuplicatePort {
        /// The port.
        port: String,
    },
    /// `port` is not the name of a wired port.
    #[error(
        "the port {value:?} is not a wired port name (eth or en, then at most {PORT_NAME_MAX_CHARS} letters, digits, _ and - in all)"
    )]
    Port {
        /// The value of the file.
        value: String,
    },
    /// `ipv4` is not a dotted IPv4 address.
    #[error("the address {value:?} of the port {port} is not a dotted IPv4 address")]
    Address {
        /// The port of the address.
        port: String,
        /// The value of the file.
        value: String,
    },
}

/// Parses the text of the network file. The version comes first: a file of
/// another version is refused as a whole. Unknown fields are ignored, at
/// the top and in each address table. An empty or invalid host name gives
/// no host name, not an error.
///
/// # Errors
///
/// [`NetworkError`] for a file that is too large, does not parse, has no
/// version or another version, has too many addresses or two for one port,
/// or holds a port or an address that its writer never writes.
pub fn parse_network(text: &str) -> Result<NetworkStatus, NetworkError> {
    if text.len() > NETWORK_FILE_MAX_BYTES {
        return Err(NetworkError::TooLarge { bytes: text.len() });
    }
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| NetworkError::Parse {
            reason: e.message().to_owned(),
        })?;
    match table.get("version") {
        None => return Err(NetworkError::NoVersion),
        Some(toml::Value::Integer(v)) if *v == i64::from(NETWORK_FILE_VERSION) => {}
        Some(toml::Value::Integer(v)) => {
            return Err(NetworkError::Version {
                found: *v,
                supported: NETWORK_FILE_VERSION,
            });
        }
        Some(other) => {
            return Err(NetworkError::VersionType {
                found: other.type_str(),
            });
        }
    }
    let file: FileV1 = toml::Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| NetworkError::Parse {
            reason: e.message().to_owned(),
        })?;
    if file.address.len() > ADDRESSES_MAX {
        return Err(NetworkError::TooManyAddresses {
            count: file.address.len(),
        });
    }
    let addresses: Vec<PortAddress> = file
        .address
        .into_iter()
        .map(|entry| {
            if !is_port_name(&entry.port) {
                return Err(NetworkError::Port { value: entry.port });
            }
            let ipv4 = entry
                .ipv4
                .parse::<Ipv4Addr>()
                .map_err(|_| NetworkError::Address {
                    port: entry.port.clone(),
                    value: entry.ipv4.clone(),
                })?;
            Ok(PortAddress {
                port: entry.port,
                ipv4,
            })
        })
        .collect::<Result<_, _>>()?;
    for (i, a) in addresses.iter().enumerate() {
        if addresses[..i].iter().any(|b| b.port == a.port) {
            return Err(NetworkError::DuplicatePort {
                port: a.port.clone(),
            });
        }
    }
    Ok(NetworkStatus {
        host_name: is_host_name(&file.host_name).then_some(file.host_name),
        addresses,
    })
}

#[derive(Deserialize)]
struct FileV1 {
    host_name: String,
    #[serde(default)]
    address: Vec<AddressV1>,
}

#[derive(Deserialize)]
struct AddressV1 {
    port: String,
    ipv4: String,
}

/// Dot-separated labels of letters, digits and `-`, each 1 to 63
/// characters, at most 253 in all.
fn is_host_name(name: &str) -> bool {
    (1..=HOST_NAME_MAX_CHARS).contains(&name.len())
        && name.split('.').all(|label| {
            (1..=HOST_LABEL_MAX_CHARS).contains(&label.len())
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

fn is_port_name(name: &str) -> bool {
    name.len() <= PORT_NAME_MAX_CHARS
        && (name.starts_with("eth") || name.starts_with("en"))
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
