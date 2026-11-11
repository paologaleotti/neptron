//! Smoke test of the SDL3 gamepad source, for a machine with a gamepad.
//!
//! It polls the source 60 times a second for 10 seconds and prints every
//! event and notice. Connect, press, move and disconnect a gamepad while it
//! runs. Run it with:
//!
//! ```sh
//! cargo run -p neptron-input --features sdl3 --example gamepads
//! ```

#![forbid(unsafe_code)]

use std::error::Error;
use std::time::{Duration, Instant};

use neptron_backend::InputSource;
use neptron_input::{InputNotice, SdlGamepads, port_of};
use neptron_types::InputEvent;

const RUN_FOR: Duration = Duration::from_secs(10);
const FRAME: Duration = Duration::from_micros(16_667);

fn main() -> Result<(), Box<dyn Error>> {
    let mut pads = SdlGamepads::open()?;
    let mut events: Vec<InputEvent> = Vec::with_capacity(256);
    let mut notices: Vec<InputNotice> = Vec::new();
    let start = Instant::now();
    let mut count = 0usize;
    println!("Reading gamepads for {} seconds.", RUN_FOR.as_secs());
    while start.elapsed() < RUN_FOR {
        events.clear();
        pads.poll(&mut events);
        pads.take_notices(&mut notices);
        let ms = start.elapsed().as_millis();
        for notice in notices.drain(..) {
            println!("{ms:>6} ms  notice  {notice:?}");
        }
        for event in &events {
            let port = port_of(event.device).map_or("-".to_owned(), |p| p.to_string());
            println!(
                "{ms:>6} ms  device {:#x} port {port}  {:?} = {}",
                event.device, event.control, event.value
            );
        }
        count += events.len();
        std::thread::sleep(FRAME);
    }
    println!("{count} events.");
    Ok(())
}
