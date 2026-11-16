//! The network status file: the form the address script of the image
//! writes, and every refusal.

use std::net::Ipv4Addr;

use neptron_platform::{
    ADDRESSES_MAX, NETWORK_FILE_MAX_BYTES, NETWORK_FILE_VERSION, NetworkError, PortAddress,
    parse_network,
};

fn address(port: &str, ipv4: [u8; 4]) -> PortAddress {
    PortAddress {
        port: port.to_owned(),
        ipv4: Ipv4Addr::from(ipv4),
    }
}

#[test]
fn one_port_with_a_lease() {
    let status = parse_network(
        "version = 1\nhost_name = \"neptron\"\n\n[[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.112\"\n",
    )
    .expect("a status");
    assert_eq!(status.host_name.as_deref(), Some("neptron"));
    assert_eq!(status.addresses, [address("eth0", [192, 168, 0, 112])]);
}

#[test]
fn two_ports_keep_the_order_of_the_file() {
    let text = "version = 1\nhost_name = \"neptron\"\n\
                [[address]]\nport = \"enx001122334455\"\nipv4 = \"10.0.0.5\"\n\
                [[address]]\nport = \"eth0\"\nipv4 = \"255.255.255.255\"\n";
    let status = parse_network(text).expect("a status");
    assert_eq!(
        status.addresses,
        [
            address("enx001122334455", [10, 0, 0, 5]),
            address("eth0", [255, 255, 255, 255]),
        ]
    );
}

#[test]
fn no_table_is_no_address() {
    let status = parse_network("version = 1\nhost_name = \"neptron\"\n").expect("a status");
    assert!(status.addresses.is_empty());
}

#[test]
fn unknown_fields_are_ignored() {
    let text = "version = 1\nhost_name = \"neptron\"\nwifi = \"later\"\n\
                [later]\nkey = 3\n\
                [[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.2\"\nipv6 = \"fe80::1\"\n";
    let status = parse_network(text).expect("a status");
    assert_eq!(status.addresses, [address("eth0", [192, 168, 0, 2])]);
}

#[test]
fn the_version_comes_first() {
    assert_eq!(
        parse_network("host_name = \"neptron\"\n"),
        Err(NetworkError::NoVersion)
    );
    assert_eq!(
        parse_network("version = \"1\"\nhost_name = \"neptron\"\n"),
        Err(NetworkError::VersionType { found: "string" })
    );
    let newer = parse_network("version = 2\nhost_name = 7\n[[address]]\nport = 1\n");
    assert_eq!(
        newer,
        Err(NetworkError::Version {
            found: 2,
            supported: NETWORK_FILE_VERSION
        }),
        "another version is refused before its fields are read"
    );
    let text = newer.map(|_| ()).unwrap_err().to_string();
    assert_eq!(
        text,
        "the network file has version 2; this build reads version 1"
    );
}

#[test]
fn a_file_that_is_not_toml_or_has_a_wrong_type_does_not_parse() {
    assert!(matches!(
        parse_network("version = 1\nhost_name = neptron\n"),
        Err(NetworkError::Parse { .. })
    ));
    assert!(matches!(
        parse_network("version = 1\n"),
        Err(NetworkError::Parse { .. })
    ));
    assert!(matches!(
        parse_network("version = 1\nhost_name = \"neptron\"\naddress = 3\n"),
        Err(NetworkError::Parse { .. })
    ));
    assert!(matches!(
        parse_network("version = 1\nhost_name = \"neptron\"\n[[address]]\nport = \"eth0\"\n"),
        Err(NetworkError::Parse { .. })
    ));
}

#[test]
fn a_host_name_with_dots_is_kept() {
    let label = "a".repeat(63);
    for name in [
        "neptron",
        "neptron.lan",
        "game-room.example.org",
        label.as_str(),
    ] {
        let text = format!("version = 1\nhost_name = {name:?}\n");
        assert_eq!(
            parse_network(&text).expect("a status").host_name.as_deref(),
            Some(name),
            "{name:?}"
        );
    }
    let longest = vec!["a".repeat(63); 4].join(".")[..253].to_owned();
    assert_eq!(longest.len(), 253);
    let text = format!("version = 1\nhost_name = {longest:?}\n");
    assert_eq!(
        parse_network(&text).expect("253 characters").host_name,
        Some(longest)
    );
}

#[test]
fn an_empty_or_invalid_host_name_gives_no_name_and_keeps_the_addresses() {
    let label = "a".repeat(64);
    let total = vec!["a".repeat(50); 6].join(".");
    assert!(total.len() > 253);
    for name in [
        "",
        "nep tron",
        ".neptron",
        "neptron.",
        "nep..tron",
        "n\u{e9}ptron",
        label.as_str(),
        total.as_str(),
    ] {
        let text = format!(
            "version = 1\nhost_name = {name:?}\n[[address]]\nport = \"eth0\"\nipv4 = \"10.0.0.1\"\n"
        );
        let status = parse_network(&text).expect("a status");
        assert_eq!(status.host_name, None, "{name:?}");
        assert_eq!(
            status.addresses,
            [address("eth0", [10, 0, 0, 1])],
            "{name:?}"
        );
    }
}

#[test]
fn a_file_over_its_bounds_is_refused() {
    let big = format!(
        "version = 1\nhost_name = \"neptron\"\n# {}\n",
        "x".repeat(NETWORK_FILE_MAX_BYTES)
    );
    assert_eq!(
        parse_network(&big),
        Err(NetworkError::TooLarge { bytes: big.len() })
    );
    let table = |i: usize| format!("[[address]]\nport = \"eth{i}\"\nipv4 = \"10.0.0.{i}\"\n");
    let at_most: String = (0..ADDRESSES_MAX).map(table).collect();
    let text = format!("version = 1\nhost_name = \"neptron\"\n{at_most}");
    assert_eq!(
        parse_network(&text).expect("16 ports").addresses.len(),
        ADDRESSES_MAX
    );
    let text = format!("{text}{}", table(ADDRESSES_MAX));
    assert_eq!(
        parse_network(&text),
        Err(NetworkError::TooManyAddresses {
            count: ADDRESSES_MAX + 1
        })
    );
}

#[test]
fn two_tables_for_one_port_are_refused() {
    let text = "version = 1\nhost_name = \"neptron\"\n\
                [[address]]\nport = \"eth0\"\nipv4 = \"10.0.0.1\"\n\
                [[address]]\nport = \"eth1\"\nipv4 = \"10.0.0.2\"\n\
                [[address]]\nport = \"eth0\"\nipv4 = \"10.0.0.3\"\n";
    let refused = parse_network(text);
    assert_eq!(
        refused,
        Err(NetworkError::DuplicatePort {
            port: "eth0".to_owned()
        })
    );
    assert_eq!(
        refused.map(|_| ()).unwrap_err().to_string(),
        "the network file has two addresses for the port eth0"
    );
}

#[test]
fn a_bad_port_is_refused() {
    for port in ["wlan0", "lo", "", "eth0 ", "eth/0", "enx0011223344556"] {
        let text = format!(
            "version = 1\nhost_name = \"neptron\"\n[[address]]\nport = {port:?}\nipv4 = \"10.0.0.1\"\n"
        );
        assert_eq!(
            parse_network(&text),
            Err(NetworkError::Port {
                value: port.to_owned()
            }),
            "{port:?}"
        );
    }
}

/// The address script also writes a group with a leading zero, such as
/// `10.0.0.01`; `Ipv4Addr` refuses it, so the whole file counts as damaged.
#[test]
fn a_bad_address_is_refused() {
    for ipv4 in [
        "10.0.0",
        "10.0.0.256",
        "10.0.0.01",
        "ten.0.0.1",
        "fe80::1",
        "",
    ] {
        let text = format!(
            "version = 1\nhost_name = \"neptron\"\n[[address]]\nport = \"eth0\"\nipv4 = {ipv4:?}\n"
        );
        assert_eq!(
            parse_network(&text),
            Err(NetworkError::Address {
                port: "eth0".to_owned(),
                value: ipv4.to_owned()
            }),
            "{ipv4:?}"
        );
    }
}
