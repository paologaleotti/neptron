//! The second reference of the shape: the width that the real console
//! gives its picture. A core says "this frame is 4:3", but a console does
//! not fill the nominal 4:3 line of the TV standard: its dot clock sets
//! the width of each pixel on the line. The table holds the dot clock of
//! each width of the systems that NEPTRON runs or plans. Pure.
//!
//! The table lives in the simulator, not in `data/systems`: the product
//! reads no such key, and the owner has not decided whether the policy
//! follows it (OP-122). A key that nothing in the product reads would be
//! data with no reader.

/// The line rate of the 525-line system, in millihertz: 4.5 MHz / 286.
pub const LINE_525_MHZ: u64 = 15_734_266;
/// The line rate of the 625-line system, in millihertz.
pub const LINE_625_MHZ: u64 = 15_625_000;

/// Which clock set of a console a case uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// The console of the line system of the screen: the PAL console on a
    /// 625-line TV, the NTSC console on every other screen.
    OfScreen,
    /// Always the PAL console: a PAL game.
    Pal,
}

/// A console: its name and the dot clock of each width it draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Console {
    /// The name in the output.
    pub name: &'static str,
    /// Width in pixels, then the dot clock of the NTSC console and of the
    /// PAL console, in hertz.
    pub clocks: &'static [(u16, u32, u32)],
    /// The line rate of the console when it is not that of the standard,
    /// in millihertz.
    pub line_mhz: Option<u64>,
    /// Which clocks a case uses.
    pub region: Region,
    /// Where the clocks come from, and what is not verified.
    pub source: &'static str,
}

/// The NES: the PPU draws one dot per 4 master clocks of 21.477272 MHz
/// (NTSC) and per 5 of 26.601712 MHz (PAL).
pub const NES: Console = Console {
    name: "NES",
    clocks: &[(256, 5_369_318, 5_320_342)],
    line_mhz: None,
    region: Region::OfScreen,
    source: "nesdev.org, Cycle reference chart: master clocks and clocks per dot",
};

/// The PAL NES of the reference source `nes_pal`.
pub const NES_PAL: Console = Console {
    region: Region::Pal,
    ..NES
};

/// The SNES: one dot per 4 master clocks of 21.477272 MHz (NTSC) and
/// 21.281370 MHz (PAL); the 512 mode draws two dots in that time.
pub const SNES: Console = Console {
    name: "SNES",
    clocks: &[(256, 5_369_318, 5_320_343), (512, 10_738_636, 10_640_685)],
    line_mhz: None,
    region: Region::OfScreen,
    source: "snes.nesdev.org Timing; pineight.com Dot clock rates and 50 Hz dot clock rates",
};

/// The Mega Drive: MCLK 53.693175 MHz (NTSC) and 53.203424 MHz (PAL),
/// divided by 8 in H40 (320 pixels) and by 10 in H32 (256 pixels).
pub const MEGA_DRIVE: Console = Console {
    name: "Mega Drive",
    clocks: &[(320, 6_711_647, 6_650_428), (256, 5_369_318, 5_320_342)],
    line_mhz: None,
    region: Region::OfScreen,
    source: "pineight.com Dot clock rates (NTSC); the PAL MCLK is from forum measures \
             (gendev.spritesmind.net), not verified from a primary source",
};

/// The PlayStation: the video clock of 53.693175 MHz (NTSC) and
/// 53.203425 MHz (PAL), divided by 10, 8, 7, 5 and 4 for 256, 320, 368,
/// 512 and 640 pixels. Each width spans 2560 video clocks.
pub const PLAYSTATION: Console = Console {
    name: "PlayStation",
    clocks: &[
        (256, 5_369_318, 5_320_343),
        (320, 6_711_647, 6_650_428),
        (368, 7_670_454, 7_600_489),
        (512, 10_738_635, 10_640_685),
        (640, 13_423_294, 13_300_856),
    ],
    line_mhz: None,
    region: Region::OfScreen,
    source: "psx-spx (nocash), GPU: video clocks, dot clock dividers, 2560 clocks of \
             display range for every width",
};

/// The Neo Geo: one pixel per 4 clocks of the 24 MHz master clock, 1536
/// clocks per line.
pub const NEO_GEO: Console = Console {
    name: "Neo Geo",
    clocks: &[(320, 6_000_000, 6_000_000)],
    line_mhz: Some(15_625_000),
    region: Region::OfScreen,
    source: "wiki.neogeodev.org, Display timing (MVS); a PAL clock is not verified",
};

/// The console of the simulator source `name`, if the table has it.
pub fn console_of(name: &str) -> Option<&'static Console> {
    let console = match name {
        "nes_pal" => &NES_PAL,
        "snes" | "snes_256" | "snes_512" => &SNES,
        "md_320" | "md_256" => &MEGA_DRIVE,
        "psx" | "psx_480i" => &PLAYSTATION,
        "neogeo" => &NEO_GEO,
        n if n.starts_with("ps1_") => &PLAYSTATION,
        _ => return None,
    };
    Some(console)
}

impl Console {
    /// The dot clock for frames `width` pixels wide, in hertz, on a screen
    /// of the 625-line system when `pal_screen`.
    pub fn dot_clock_hz(&self, width: u16, pal_screen: bool) -> Option<u32> {
        let pal = pal_screen || self.region == Region::Pal;
        self.clocks
            .iter()
            .find(|c| c.0 == width)
            .map(|&(_, ntsc, pal_hz)| if pal { pal_hz } else { ntsc })
    }

    /// The line rate of the console, in millihertz, as for
    /// [`Self::dot_clock_hz`].
    pub fn line_rate_mhz(&self, pal_screen: bool) -> u64 {
        let pal = pal_screen || self.region == Region::Pal;
        self.line_mhz
            .unwrap_or(if pal { LINE_625_MHZ } else { LINE_525_MHZ })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The square pixel clock of a 240-line picture on the 525-line
    /// system, 135/22 MHz, over the dot clock is the pixel aspect that
    /// the sources name: 8:7 for 5.37 MHz, 32:35 for 6.71 MHz.
    #[test]
    fn the_clocks_give_the_known_pixel_aspects() {
        let square_hz = 135e6 / 22.0;
        let par = |hz: u32| square_hz / f64::from(hz);
        assert!((par(NES.dot_clock_hz(256, false).unwrap()) - 8.0 / 7.0).abs() < 1e-5);
        let md = MEGA_DRIVE.dot_clock_hz(320, false).unwrap();
        assert!((par(md) - 32.0 / 35.0).abs() < 1e-5);
        let psx = PLAYSTATION.dot_clock_hz(640, false).unwrap();
        assert!((par(psx) - 16.0 / 35.0).abs() < 1e-5);
    }

    /// Every PlayStation clock is the video clock over the divider of its
    /// width.
    #[test]
    fn every_playstation_clock_is_the_video_clock_over_its_divider() {
        let dividers = [(256, 10), (320, 8), (368, 7), (512, 5), (640, 4)];
        for (&(width, ntsc, pal), (w, divider)) in PLAYSTATION.clocks.iter().zip(dividers) {
            assert_eq!(width, w);
            assert!(ntsc.abs_diff(53_693_175 / divider) <= 1, "{width}");
            assert!(pal.abs_diff(53_203_425 / divider) <= 1, "{width}");
        }
    }

    #[test]
    fn the_sources_name_their_console() {
        assert_eq!(console_of("ps1_480i").map(|c| c.name), Some("PlayStation"));
        assert_eq!(console_of("md_256").map(|c| c.name), Some("Mega Drive"));
        assert_eq!(console_of("rtype"), None);
        assert_eq!(
            console_of("nes_pal").and_then(|c| c.dot_clock_hz(256, false)),
            Some(5_320_342)
        );
        assert_eq!(SNES.dot_clock_hz(320, false), None);
        assert_eq!(NEO_GEO.line_rate_mhz(true), 15_625_000);
    }
}
