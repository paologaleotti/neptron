//! Scenes that show each widget template and the font with fixture data.

use crate::canvas::{UI_HEIGHT, UI_WIDTH};
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::snapshot::{
    ActionId, ChoiceOption, Level, MenuItem, PageId, Scopes, SettingKey, SettingValue,
};
use crate::text::{self, Align};
use crate::theme::{SAFE_AREA, Theme};
use crate::tree::{TreeRow, WidgetKind};
use crate::widgets::{
    Dialog, DialogContent, DialogTone, Grid, GridCell, List, ListItem, SettingsList, Slider,
    SliderValue,
};

use super::SceneDef;

pub(super) static SCENES: &[SceneDef] = &[
    SceneDef {
        name: "widgets/list",
        build: list,
    },
    SceneDef {
        name: "widgets/settings-list",
        build: settings_list,
    },
    SceneDef {
        name: "widgets/dialog",
        build: dialog,
    },
    SceneDef {
        name: "widgets/grid",
        build: grid,
    },
    SceneDef {
        name: "widgets/slider",
        build: slider,
    },
    SceneDef {
        name: "font/ascii",
        build: font,
    },
];

const HEADER_HEIGHT_PX: u32 = 16;

const GAMES: &[(&str, Option<&str>)] = &[
    ("Astro Courier", Some("1989")),
    ("Bit Racer", Some("1990")),
    ("Castle of Glass", None),
    ("Deep Sea Patrol", Some("1991")),
    (
        "The Extremely Long Adventure of the Pixel Knight",
        Some("1992"),
    ),
    ("Falcon Grid", Some("1990")),
    ("Garden Maze", None),
    ("Harbour Lights", Some("1993")),
    ("Iron Orbit", Some("1994")),
    ("Jungle Relay", Some("1991")),
    ("Kite Fighter", None),
    ("Lunar Tiles", Some("1992")),
    ("Metro Sprint", Some("1995")),
    ("Night Owl Express", Some("1990")),
    ("Orchard Panic", None),
    ("Pocket Comet", Some("1993")),
    ("Quartz Valley", Some("1994")),
    ("Rocket Bakery", Some("1991")),
    ("Silent Signal", None),
    ("Tundra Rally", Some("1996")),
];

fn content() -> Rect {
    let top = SAFE_AREA.y + HEADER_HEIGHT_PX as i32;
    Rect::new(
        SAFE_AREA.x,
        top,
        SAFE_AREA.w,
        SAFE_AREA.h - HEADER_HEIGHT_PX,
    )
}

fn header(frame: &mut UiFrame, theme: &Theme, screen: &'static str, title: &str) {
    frame.tree.set_screen(screen);
    frame.scene.rect(
        Rect::new(0, 0, UI_WIDTH, UI_HEIGHT),
        theme.colors.background,
    );
    let title_box = Rect::new(SAFE_AREA.x, SAFE_AREA.y, SAFE_AREA.w, theme.row_height_px());
    let clipped = text::draw_in(
        &mut frame.scene,
        title_box,
        Align::Left,
        theme.colors.accent,
        title,
    );
    let line = Rect::new(
        SAFE_AREA.x,
        SAFE_AREA.y + theme.row_height_px() as i32 + 1,
        SAFE_AREA.w,
        1,
    );
    frame.scene.rect(line, theme.colors.border);
    frame.tree.push(TreeRow {
        widget: WidgetKind::Label,
        kind: None,
        label: title,
        value: None,
        source: None,
        rect: title_box,
        focused: false,
        clipped,
    });
}

fn list(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "widgets/list", "Game list");
    let mut list = List::new(content(), theme.row_height_px());
    list.select(4, GAMES.len());
    list.draw(frame, theme, GAMES.len(), |i| {
        let (label, detail) = GAMES[i % GAMES.len()];
        ListItem {
            label,
            detail,
            enabled: i != 2,
            kind: crate::tree::RowKind::Action,
        }
    });
}

fn setting(key: u32, label: &str, value: SettingValue, source: Level) -> MenuItem {
    MenuItem {
        key: SettingKey(key),
        label: label.to_owned(),
        value,
        source,
        scopes: Scopes {
            global: true,
            system: true,
            game: true,
        },
        live: true,
        unavailable: None,
        confirm: None,
    }
}

fn choice(index: usize, options: &[&str]) -> SettingValue {
    SettingValue::Choice {
        index,
        options: options.iter().map(|o| ChoiceOption::new(*o)).collect(),
    }
}

fn number(value: i64, min: i64, max: i64, unit: &str) -> SettingValue {
    SettingValue::Number {
        value,
        min,
        max,
        step: 1,
        unit: unit.to_owned(),
    }
}

fn settings_list(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "widgets/settings-list", "Display");
    let items = [
        setting(
            1,
            "Refresh",
            choice(1, &["Native", "Force 60 Hz", "Force 50 Hz"]),
            Level::System,
        ),
        setting(2, "Interlace", SettingValue::Toggle(false), Level::Default),
        setting(
            3,
            "Picture style",
            choice(0, &["Native", "2x", "3x"]),
            Level::Global,
        ),
        setting(
            4,
            "Horizontal position",
            number(-3, -16, 16, "px"),
            Level::Game,
        ),
        setting(
            5,
            "Scanline strength",
            number(40, 0, 100, "%"),
            Level::Default,
        ),
        setting(
            6,
            "A label that is far too long for its column",
            SettingValue::Toggle(true),
            Level::Global,
        ),
        setting(
            7,
            "Test patterns",
            SettingValue::Action(ActionId(1)),
            Level::Default,
        ),
        setting(8, "Advanced", SettingValue::Page(PageId(1)), Level::Default),
    ];
    let mut list = SettingsList::new(content(), theme.row_height_px());
    list.select(3, items.len());
    list.draw(frame, theme, &items);
}

fn dialog(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "widgets/dialog", "Display");
    let content = DialogContent {
        title: "Keep this video mode?",
        message: "320x240 at 59.92 Hz on VGA-1.\nThe old mode comes back in 10 seconds.",
        buttons: &["Keep", "Revert"],
        tone: DialogTone::Normal,
    };
    Dialog::new(1).draw(frame, theme, &content);
}

fn grid(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "widgets/grid", "Home");
    let cells = [
        "NES",
        "SNES",
        "Mega Drive",
        "Favourites",
        "Recents",
        "Settings",
        "Display",
    ];
    let mut grid = Grid::new(content(), 3, 66);
    grid.select(1, cells.len());
    grid.draw(frame, theme, cells.len(), |i| GridCell {
        label: cells[i % cells.len()],
        sprite: None,
    });
}

fn slider(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "widgets/slider", "Audio");
    let area = content();
    let sliders = [
        ("Volume", 70, 0, 100, "%"),
        ("Audio latency", 64, 16, 128, "ms"),
        ("Scanline strength", 0, 0, 100, "%"),
        ("Brightness", 100, 0, 100, ""),
    ];
    for (i, (label, value, min, max, unit)) in sliders.into_iter().enumerate() {
        let y = area.y + 4 + (i as i32) * (Slider::height_px(theme) as i32 + 12);
        let rect = Rect::new(area.x, y, area.w, Slider::height_px(theme));
        let value = SliderValue {
            value,
            min,
            max,
            step: 1,
        };
        Slider::new(rect).draw(frame, theme, label, value, unit, i == 0);
    }
}

fn font(frame: &mut UiFrame, theme: &Theme) {
    header(frame, theme, "font/ascii", "Font");
    let area = content();
    let lines = [
        " !\"#$%&'()*+,-./0123456789:;<=>?",
        "@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_",
        "`abcdefghijklmnopqrstuvwxyz{|}~",
        "",
        "The quick brown fox jumps over",
        "the lazy dog. 0O 1Il| 5S 8B",
        "Pack my box with five dozen",
        "liquor jugs! (Hz, ms, px, %)",
        "",
        "\u{c0}\u{c1}\u{c2}\u{c3}\u{c4}\u{c5}\u{c6}\u{c7}\u{c8}\u{c9}\u{ca}\u{cb} \u{d1}\u{d6}\u{d8}\u{dc}\u{df} \u{e0}\u{e1}\u{e2}\u{e4}\u{e7}\u{e8}\u{e9}\u{ea}\u{f1}\u{f6}\u{fc}",
        "Caf\u{e9}, M\u{f6}bius, Ni\u{f1}o \u{a1}\u{bf}\u{ab}\u{bb}\u{a9}\u{b0}",
        "Not in the font: \u{3a9}\u{2603}",
    ];
    for (i, line) in lines.iter().enumerate() {
        let y = area.y + 4 + (i as i32) * theme.row_height_px() as i32;
        let rect = Rect::new(area.x, y, area.w, theme.row_height_px());
        let clipped = text::draw_in(&mut frame.scene, rect, Align::Left, theme.colors.text, line);
        frame.tree.push(TreeRow {
            widget: WidgetKind::Label,
            kind: None,
            label: line,
            value: None,
            source: None,
            rect,
            focused: false,
            clipped,
        });
    }
}
