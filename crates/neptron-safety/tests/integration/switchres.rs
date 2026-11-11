//! Modelines that upstream Switchres generates for real systems. NEPTRON
//! trusts Switchres, so each one must validate against the preset it was
//! made for.

use neptron_safety::validate;
use neptron_types::{GpuCaps, Modeline, MonitorProfile};

use crate::synth::{presets, ranges};

/// (system, preset, modeline) as the standalone `switchres` program of
/// commit 09272ac24640daee45f5be15b1b856ecd220dda2 prints them, with
/// doublescan off unless the modeline says so. The modeline is the pixel
/// clock in MHz and the timings of its "Modeline" line.
const GOLDENS: [(&str, &str, &str); 61] = [
    (
        "nes_ntsc",
        "generic_15",
        "5.172119 256 266 290 331 240 241 244 260 -hsync -vsync",
    ),
    (
        "nes_ntsc",
        "pc_31_120",
        "10.203367 256 263 290 324 240 243 246 262 -hsync -vsync",
    ),
    (
        "nes_ntsc",
        "pc_70_120",
        "10.463596 256 279 282 331 240 242 243 263 -hsync -vsync",
    ),
    (
        "nes_ntsc",
        "generic_15",
        "41.533216 2048 2131 2326 2658 240 241 244 260 -hsync -vsync",
    ),
    (
        "nes_pal",
        "pal",
        "4.898400 256 263 286 314 240 267 269 312 -hsync -vsync",
    ),
    (
        "nes_pal",
        "generic_15",
        "5.180875 256 266 290 331 240 268 271 313 -hsync -vsync",
    ),
    (
        "snes_ntsc",
        "ntsc",
        "4.852622 256 263 286 309 224 235 238 262 -hsync -vsync",
    ),
    (
        "snes_ntsc",
        "arcade_31",
        "20.273495 512 531 607 645 448 474 476 523 -hsync -vsync",
    ),
    (
        "snes_ntsc",
        "k7131",
        "5.172119 256 266 290 331 224 233 236 260 -hsync -vsync",
    ),
    (
        "snes_pal",
        "pal",
        "4.898400 256 263 286 314 239 266 268 312 -hsync -vsync",
    ),
    (
        "md_ntsc_h40",
        "generic_15",
        "6.506199 320 333 364 416 224 234 237 261 -hsync -vsync",
    ),
    (
        "md_ntsc_h40",
        "arcade_15_25_31",
        "6.506199 320 333 364 416 224 234 237 261 -hsync -vsync",
    ),
    (
        "md_ntsc_h40",
        "k7000",
        "6.506199 320 333 364 416 224 234 237 261 -hsync -vsync",
    ),
    (
        "md_ntsc_h40",
        "arcade_31",
        "12.709668 320 332 380 404 224 237 238 262 doublescan -hsync -vsync",
    ),
    (
        "md_ntsc_h32",
        "arcade_15",
        "5.176807 256 266 290 331 224 234 237 261 -hsync -vsync",
    ),
    (
        "md_pal_h40",
        "pal",
        "6.146400 320 329 358 394 240 267 269 312 -hsync -vsync",
    ),
    (
        "sms_ntsc",
        "ntsc",
        "4.852622 256 263 286 309 192 219 222 262 -hsync -vsync",
    ),
    (
        "game_gear",
        "generic_15",
        "6.551999 320 333 364 416 288 289 292 308 -hsync -vsync",
    ),
    (
        "game_boy",
        "arcade_15",
        "6.804000 320 334 366 420 288 289 292 309 -hsync -vsync",
    ),
    (
        "game_boy",
        "pc_31_120",
        "19.101492 480 493 544 608 432 471 474 526 -hsync -vsync",
    ),
    (
        "gba",
        "generic_15",
        "14.659491 720 749 818 935 480 485 491 525 interlace -hsync -vsync",
    ),
    (
        "pc_engine",
        "generic_15",
        "5.203904 256 266 290 332 240 242 245 262 -hsync -vsync",
    ),
    (
        "neogeo",
        "arcade_15",
        "6.484418 320 333 363 415 224 235 238 264 -hsync -vsync",
    ),
    (
        "neogeo",
        "pc_31_120",
        "12.783702 320 329 363 406 224 237 240 266 -hsync -vsync",
    ),
    (
        "neogeo",
        "arcade_15_31",
        "6.484418 320 333 363 415 224 235 238 264 -hsync -vsync",
    ),
    (
        "cps1",
        "arcade_15",
        "7.842265 384 400 437 500 224 235 238 263 -hsync -vsync",
    ),
    (
        "cps1",
        "d9800",
        "7.404530 384 400 435 485 224 232 235 256 -hsync -vsync",
    ),
    (
        "cps1",
        "m2929",
        "28.073283 768 785 855 934 448 468 471 504 -hsync -vsync",
    ),
    (
        "pacman",
        "arcade_15",
        "8.472600 398 415 455 523 288 289 292 309 -hsync -vsync",
    ),
    (
        "pacman",
        "d9400",
        "10.116353 398 420 467 535 288 291 295 312 -hsync -vsync",
    ),
    (
        "donkey_kong",
        "generic_15",
        "8.142750 398 414 452 517 256 257 260 276 -hsync -vsync",
    ),
    (
        "donkey_kong",
        "polo",
        "8.785090 398 416 457 527 256 257 260 277 -hsync -vsync",
    ),
    (
        "space_invaders",
        "arcade_15",
        "8.472600 398 415 455 523 256 257 260 277 -hsync -vsync",
    ),
    (
        "space_invaders",
        "h9110",
        "8.658895 398 415 456 525 256 257 260 277 -hsync -vsync",
    ),
    (
        "gradius",
        "arcade_15ex",
        "5.175631 256 266 290 331 224 232 235 258 -hsync -vsync",
    ),
    (
        "gradius",
        "r666b",
        "5.084540 256 272 296 329 224 230 233 255 -hsync -vsync",
    ),
    (
        "system16",
        "arcade_15",
        "6.520423 320 333 364 416 224 234 237 261 -hsync -vsync",
    ),
    (
        "system16",
        "m3129",
        "6.147247 320 333 362 403 224 231 234 254 +hsync +vsync",
    ),
    (
        "system16",
        "pstar",
        "5.947508 320 331 334 378 224 234 237 262 -hsync -vsync",
    ),
    (
        "rtype",
        "arcade_15",
        "7.796931 384 400 437 499 256 261 264 284 -hsync -vsync",
    ),
    (
        "rtype",
        "arcade_15_25_31",
        "7.796931 384 400 437 499 256 261 264 284 -hsync -vsync",
    ),
    (
        "rtype",
        "d9200",
        "7.418077 384 400 435 485 256 259 262 278 -hsync -vsync",
    ),
    (
        "system24",
        "arcade_15_25_31",
        "15.417984 496 508 570 619 384 394 399 433 -hsync -vsync",
    ),
    (
        "system24",
        "arcade_25",
        "15.417984 496 508 570 619 384 394 399 433 -hsync -vsync",
    ),
    (
        "system24",
        "arcade_15_25",
        "15.417984 496 508 570 619 384 394 399 433 -hsync -vsync",
    ),
    (
        "vga_mode13h",
        "arcade_31",
        "25.419680 640 664 760 808 400 430 432 484 -hsync -vsync",
    ),
    (
        "vga_mode13h",
        "pc_31_120",
        "12.748400 320 329 363 406 200 249 252 314 -hsync -vsync",
    ),
    (
        "vga_mode13h",
        "generic_15",
        "6.516640 320 333 364 416 200 212 215 241 -hsync -vsync",
    ),
    (
        "vga_mode13h",
        "ms929",
        "6.481930 320 345 375 419 200 211 214 238 -hsync -vsync",
    ),
    (
        "vga_640x480",
        "arcade_31",
        "25.426547 640 664 760 808 480 491 493 525 -hsync -vsync",
    ),
    (
        "vga_640x480",
        "generic_15",
        "13.025351 640 666 727 831 480 483 489 523 interlace -hsync -vsync",
    ),
    (
        "vga_640x480",
        "ms2930",
        "24.654040 640 648 736 779 480 490 492 528 -hsync -vsync",
    ),
    (
        "ntsc_480i",
        "generic_15",
        "13.025351 640 666 727 831 480 483 489 523 interlace -hsync -vsync",
    ),
    (
        "ntsc_480i",
        "ntsc",
        "12.146841 640 658 715 772 480 487 493 525 interlace -hsync -vsync",
    ),
    (
        "ntsc_480i",
        "generic_15",
        "13.000446 640 666 727 831 240 242 245 261 -hsync -vsync",
    ),
    (
        "pal_576i",
        "pal",
        "13.843750 720 741 806 886 576 583 588 625 interlace -hsync -vsync",
    ),
    (
        "wonderswan",
        "generic_15",
        "9.166499 448 466 509 582 288 289 292 308 -hsync -vsync",
    ),
    (
        "atari_2600",
        "ntsc",
        "3.030926 160 165 179 193 192 219 222 262 -hsync -vsync",
    ),
    (
        "c64_pal",
        "pal",
        "6.146400 320 329 358 394 200 247 249 312 -hsync -vsync",
    ),
    (
        "c64_ntsc",
        "generic_15",
        "6.520555 320 333 364 416 200 222 225 262 -hsync -vsync",
    ),
    (
        "zx_spectrum",
        "pal",
        "4.898400 256 263 286 314 192 243 245 312 -hsync -vsync",
    ),
];

/// No pixel clock floor, interlace allowed, as the goldens were made.
const CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: 1,
    pclock_max_hz: 1_000_000_000,
    interlace: true,
    flip_per_field: false,
    max_width: 8192,
    max_height: 8192,
};

fn parse(text: &str) -> Modeline {
    let words: Vec<&str> = text.split_whitespace().collect();
    let (mhz, fraction) = words[0].split_once('.').unwrap();
    assert_eq!(fraction.len(), 6, "{text}");
    let pclock_hz = mhz.parse::<u32>().unwrap() * 1_000_000 + fraction.parse::<u32>().unwrap();
    let n = |i: usize| words[i].parse::<u16>().unwrap();
    let flags = &words[9..];
    Modeline {
        pclock_hz,
        hactive: n(1),
        hsync_start: n(2),
        hsync_end: n(3),
        htotal: n(4),
        vactive: n(5),
        vsync_start: n(6),
        vsync_end: n(7),
        vtotal: n(8),
        interlace: flags.contains(&"interlace"),
        doublescan: flags.contains(&"doublescan"),
        hsync_positive: flags.contains(&"+hsync"),
        vsync_positive: flags.contains(&"+vsync"),
    }
}

fn preset<'a>(all: &'a [MonitorProfile], id: &str) -> &'a MonitorProfile {
    all.iter().find(|p| p.id == id).unwrap()
}

#[test]
fn every_switchres_golden_validates() {
    let all = presets();
    let failed: Vec<String> = GOLDENS
        .iter()
        .filter_map(|(system, id, text)| {
            let mode = parse(text);
            match validate(&mode, preset(&all, id), &CAPS) {
                Ok(validated) if *validated.mode() == mode => None,
                other => Some(format!("{system} on {id}: {other:?}")),
            }
        })
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

/// These modes sit below the lowest line rate of a fixed range: Switchres
/// stops adding lines when one more would pass the highest line rate.
#[test]
fn modes_below_a_fixed_line_rate_validate() {
    let all = presets();
    let mut below = 0;
    for (system, id, text) in GOLDENS {
        let mode = parse(text);
        let profile = preset(&all, id);
        let line = mode.line_rate_mhz();
        if ranges(profile)
            .iter()
            .any(|r| (r.h_min_mhz..=r.h_max_mhz).contains(&line))
        {
            continue;
        }
        below += 1;
        assert!(validate(&mode, profile, &CAPS).is_ok(), "{system} on {id}");
    }
    assert_eq!(below, 12);
}
