//! The table of every TATE case in docs/spec/tate.md, section 4, against
//! `Orientation::turns`.

use std::path::Path;

use neptron_types::{Orientation, PictureSize, Rotation};

/// One row of the spec table, as written.
struct Row {
    screen: Rotation,
    vertical: bool,
    game_override: Option<Rotation>,
    game_orientation: Rotation,
    turn: Rotation,
    size: String,
    seen: String,
}

fn rotation_of_degrees(cell: &str) -> Rotation {
    let degrees = cell.parse().unwrap_or_else(|e| panic!("{cell:?}: {e}"));
    Rotation::from_degrees(degrees).unwrap_or_else(|| panic!("{cell:?} is not a quarter turn"))
}

fn rotation_of_name(cell: &str) -> Rotation {
    let degrees = cell
        .strip_prefix("`R")
        .and_then(|c| c.strip_suffix('`'))
        .unwrap_or_else(|| panic!("{cell:?} is not a turn"));
    rotation_of_degrees(degrees)
}

fn spec_rows() -> Vec<Row> {
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/tate.md");
    let text = std::fs::read_to_string(&spec).unwrap_or_else(|e| panic!("{}: {e}", spec.display()));
    let begin = "<!-- tate-table:begin -->\n";
    let start = text.find(begin).expect("the begin marker") + begin.len();
    let stop = text
        .find("<!-- tate-table:end -->")
        .expect("the end marker");
    let mut lines = text[start..stop].lines();
    assert_eq!(
        lines.next(),
        Some("| Screen `S` | Game | Override | `G'` | Turn `R` | Size | The player sees |"),
        "the header of the table"
    );
    assert_eq!(
        lines.next(),
        Some("| --- | --- | --- | --- | --- | --- | --- |")
    );
    lines
        .map(|line| {
            let cells: Vec<&str> = line
                .strip_prefix('|')
                .and_then(|l| l.strip_suffix('|'))
                .unwrap_or_else(|| panic!("{line:?} is not a table row"))
                .split('|')
                .map(str::trim)
                .collect();
            let [screen, game, game_override, g, r, size, seen] = cells[..] else {
                panic!("{line:?} has {} cells, not 7", cells.len());
            };
            Row {
                screen: rotation_of_degrees(screen),
                vertical: match game {
                    "horizontal" => false,
                    "vertical" => true,
                    other => panic!("{other:?} is not a game kind"),
                },
                game_override: match game_override {
                    "Auto" => None,
                    degrees => Some(rotation_of_degrees(degrees)),
                },
                game_orientation: rotation_of_name(g),
                turn: rotation_of_name(r),
                size: size.to_owned(),
                seen: seen.to_owned(),
            }
        })
        .collect()
}

/// The size column, from the function.
fn size_cell(size: PictureSize, notice: bool) -> &'static str {
    match (size, notice) {
        (PictureSize::OwnLinesFull, false) => "own lines, full",
        (PictureSize::AcrossSmall, false) => "across, small",
        (PictureSize::AcrossSmall, true) => "across, small, notice",
        (PictureSize::OwnLinesFull, true) => "own lines, full, notice",
    }
}

/// How the player sees the picture: the game orientation in use against
/// the true orientation of the game, which is the core's turn.
fn seen_cell(game_orientation: Rotation, truth: Rotation) -> &'static str {
    match game_orientation.minus(truth) {
        Rotation::R0 => "upright",
        Rotation::R90 => "top at the left",
        Rotation::R180 => "upside down",
        Rotation::R270 => "top at the right",
    }
}

/// The table holds 4 screen orientations x 2 game kinds x 5 overrides,
/// each case once, and every row is the output of `Orientation::turns`
/// for a horizontal game at `R0` and a vertical game at `R90`.
#[test]
fn the_spec_table_of_turns_is_the_output_of_the_function() {
    let rows = spec_rows();
    assert_eq!(rows.len(), 40);
    let mut cases: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.screen.degrees(),
                r.vertical,
                r.game_override.map(Rotation::degrees),
            )
        })
        .collect();
    cases.sort_unstable();
    cases.dedup();
    assert_eq!(cases.len(), 40, "a case is in the table twice");
    for row in &rows {
        let core = if row.vertical {
            Rotation::R90
        } else {
            Rotation::R0
        };
        let orientation = Orientation {
            screen: row.screen,
            game_override: row.game_override,
        };
        let turns = orientation.turns(core);
        let case = format!(
            "S {} {} override {:?}",
            row.screen.degrees(),
            if row.vertical {
                "vertical"
            } else {
                "horizontal"
            },
            row.game_override.map(Rotation::degrees)
        );
        assert_eq!(turns.game_orientation, row.game_orientation, "G' of {case}");
        assert_eq!(turns.game, row.turn, "R of {case}");
        assert_eq!(
            size_cell(turns.size, turns.horizontal_on_turned_screen),
            row.size,
            "size of {case}"
        );
        assert_eq!(
            seen_cell(turns.game_orientation, core),
            row.seen,
            "what the player sees of {case}"
        );
        assert_eq!(turns.ui, row.screen.inverse(), "U of {case}");
    }
}

/// A flipped board (`R180` horizontal, `R270` vertical) follows the same
/// rule: with its override turned by 180 as well, each row of the table
/// gives the turn of the row plus 180, the same size, the same notice and
/// the same picture for the player.
#[test]
fn a_flipped_board_follows_the_rows_of_the_table() {
    for row in spec_rows() {
        let core = if row.vertical {
            Rotation::R270
        } else {
            Rotation::R180
        };
        let orientation = Orientation {
            screen: row.screen,
            game_override: row.game_override.map(|o| o.plus(Rotation::R180)),
        };
        let turns = orientation.turns(core);
        let case = format!(
            "flipped S {} vertical {} override {:?}",
            row.screen.degrees(),
            row.vertical,
            row.game_override.map(Rotation::degrees)
        );
        assert_eq!(
            turns.game_orientation,
            row.game_orientation.plus(Rotation::R180),
            "{case}"
        );
        assert_eq!(turns.game, row.turn.plus(Rotation::R180), "{case}");
        assert_eq!(
            size_cell(turns.size, turns.horizontal_on_turned_screen),
            row.size,
            "{case}"
        );
        assert_eq!(seen_cell(turns.game_orientation, core), row.seen, "{case}");
    }
}
