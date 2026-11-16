//! The parts of the session that arcade games need: the zip listing for
//! the set check.

use std::io::Cursor;

use neptron_session::{ZipError, zip_entries};

/// The smallest archive: an end record and no entry.
fn empty_zip() -> Vec<u8> {
    let mut zip = b"PK\x05\x06".to_vec();
    zip.extend_from_slice(&[0; 18]);
    zip
}

#[test]
fn the_zip_listing_is_reachable_through_the_session() {
    assert_eq!(zip_entries(&mut Cursor::new(empty_zip())).unwrap(), []);
    assert!(matches!(
        zip_entries(&mut Cursor::new(b"not a zip".to_vec())),
        Err(ZipError::NoEndOfCentralDirectory)
    ));
}
