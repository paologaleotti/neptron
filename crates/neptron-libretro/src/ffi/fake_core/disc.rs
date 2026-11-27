//! The disc control interface through the fake core: the answers to the
//! three environment calls, and the safe calls of the host.

use std::ffi::{CStr, c_uint};
use std::sync::Mutex;

use super::*;
use crate::{DiscError, DiscState};

/// The discs of the fake: the paths, the index in the drive, the tray.
struct Drive {
    paths: Vec<Option<String>>,
    index: c_uint,
    open: bool,
    refuse_index: bool,
}

static DRIVE: Mutex<Drive> = Mutex::new(Drive {
    paths: Vec::new(),
    index: 0,
    open: false,
    refuse_index: false,
});

fn drive() -> std::sync::MutexGuard<'static, Drive> {
    DRIVE.lock().unwrap_or_else(|e| e.into_inner())
}

unsafe extern "C" fn set_eject_state(open: bool) -> bool {
    record(&format!("tray {}", if open { "open" } else { "closed" }));
    drive().open = open;
    true
}
unsafe extern "C" fn get_eject_state() -> bool {
    drive().open
}
unsafe extern "C" fn get_image_index() -> c_uint {
    drive().index
}
unsafe extern "C" fn set_image_index(index: c_uint) -> bool {
    let mut d = drive();
    if d.refuse_index || !d.open {
        return false;
    }
    d.index = index;
    drop(d);
    record(&format!("disc {index}"));
    true
}
unsafe extern "C" fn get_num_images() -> c_uint {
    c_uint::try_from(drive().paths.len()).unwrap_or(0)
}
unsafe extern "C" fn replace_image_index(index: c_uint, info: *const retro_game_info) -> bool {
    // SAFETY: the host passes a valid `retro_game_info` with a path.
    let path = unsafe { CStr::from_ptr((*info).path) }
        .to_string_lossy()
        .into_owned();
    let mut d = drive();
    match d.paths.get_mut(index as usize) {
        Some(slot) => {
            *slot = Some(path);
            true
        }
        None => false,
    }
}
unsafe extern "C" fn add_image_index() -> bool {
    drive().paths.push(None);
    true
}

fn basic() -> retro_disk_control_callback {
    retro_disk_control_callback {
        set_eject_state: Some(set_eject_state),
        get_eject_state: Some(get_eject_state),
        get_image_index: Some(get_image_index),
        set_image_index: Some(set_image_index),
        get_num_images: Some(get_num_images),
        replace_image_index: Some(replace_image_index),
        add_image_index: Some(add_image_index),
    }
}

fn with_discs(on_load: Hook) -> Host {
    drive().paths = vec![Some("first.cue".to_owned())];
    script(|s| {
        s.on_set_environment = Some(|cb| {
            let mut yes = true;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &raw mut yes));
        });
        s.on_load = Some(on_load);
    });
    let mut host = open().unwrap();
    host.load_game(None).unwrap();
    host
}

#[test]
fn the_host_takes_the_extended_interface() {
    let mut host = with_discs(|cb| {
        let mut version = 0u32;
        assert!(cb.env(
            RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION,
            &raw mut version
        ));
        assert_eq!(version, 1);
        let b = basic();
        let mut ext = retro_disk_control_ext_callback {
            set_eject_state: b.set_eject_state,
            get_eject_state: b.get_eject_state,
            get_image_index: b.get_image_index,
            set_image_index: b.set_image_index,
            get_num_images: b.get_num_images,
            replace_image_index: b.replace_image_index,
            add_image_index: b.add_image_index,
            set_initial_image: None,
            get_image_path: None,
            get_image_label: None,
        };
        assert!(cb.env(
            RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE,
            &raw mut ext
        ));
        assert!(!cb.env(
            RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE,
            std::ptr::null_mut::<u8>()
        ));
    });
    assert_eq!(
        host.disc_state(),
        Some(DiscState {
            count: 1,
            index: 0,
            tray_open: false
        })
    );
}

#[test]
fn discs_are_added_and_changed() {
    let mut host = with_discs(|cb| {
        let mut b = basic();
        assert!(cb.env(RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE, &raw mut b));
    });
    assert_eq!(host.add_disc(Path::new("/roms/two.cue")), Ok(1));
    assert_eq!(host.add_disc(Path::new("/roms/three.cue")), Ok(2));
    assert_eq!(
        drive().paths,
        [
            Some("first.cue".to_owned()),
            Some("/roms/two.cue".to_owned()),
            Some("/roms/three.cue".to_owned())
        ]
    );
    assert_eq!(
        host.set_disc_index(1),
        Err(DiscError::Refused {
            call: "set_image_index"
        }),
        "the fake wants the tray open"
    );
    host.set_disc_tray(true).unwrap();
    host.set_disc_index(2).unwrap();
    assert_eq!(
        host.set_disc_index(3),
        Err(DiscError::NoSuchDisc { index: 3, count: 3 })
    );
    host.set_disc_tray(false).unwrap();
    assert_eq!(
        host.disc_state(),
        Some(DiscState {
            count: 3,
            index: 2,
            tray_open: false
        })
    );
    let calls = seen().calls;
    let tail: Vec<&str> = calls
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(tail, ["tray open", "disc 2", "tray closed"]);
    drive().refuse_index = true;
    host.set_disc_tray(true).unwrap();
    assert_eq!(
        host.set_disc_index(0),
        Err(DiscError::Refused {
            call: "set_image_index"
        })
    );
}

#[test]
fn a_core_with_no_interface_or_a_null_callback_gives_an_error() {
    let mut host = with_discs(|_| {});
    assert_eq!(host.disc_state(), None);
    assert_eq!(host.set_disc_tray(true), Err(DiscError::NoInterface));
    script(|s| {
        s.on_run = Some(|cb| {
            let mut b = basic();
            b.add_image_index = None;
            b.get_eject_state = None;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE, &raw mut b));
        });
    });
    host.run_frame().unwrap();
    assert_eq!(host.disc_state(), None, "no get_eject_state");
    assert_eq!(
        host.add_disc(Path::new("/x.cue")),
        Err(DiscError::MissingCall {
            call: "add_image_index"
        })
    );
    assert_eq!(
        host.add_disc(Path::new("/bad\0.cue")),
        Err(DiscError::InvalidPath {
            path: "/bad\0.cue".into()
        })
    );
}

#[test]
fn no_game_no_disc_call() {
    let mut host = open().unwrap();
    assert_eq!(host.set_disc_tray(true), Err(DiscError::NoGame));
    assert_eq!(host.disc_state(), None);
}
