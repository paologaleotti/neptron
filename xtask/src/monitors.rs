//! `cargo xtask monitors`: converts the Switchres preset table into the
//! TOML files of `data/monitors`.
//!
//! The input is a local copy of Switchres `monitor.cpp`. The converter reads
//! the `crt_range` strings of `monitor_set_preset` and converts each value
//! from its decimal text with integer maths, so no value goes through a
//! float.

use std::io;
use std::path::PathBuf;

use crate::repo::Repo;

/// An error of the monitor preset converter.
#[derive(Debug, thiserror::Error)]
pub enum MonitorsError {
    /// The arguments are wrong.
    #[error("usage: cargo xtask monitors <path to Switchres monitor.cpp> [output directory]")]
    Usage,
    /// A file cannot be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the operating system.
        source: io::Error,
    },
    /// The preset table does not have the expected form.
    #[error("monitor.cpp line {line}: {reason}")]
    Table {
        /// The line number, from 1.
        line: usize,
        /// What is wrong.
        reason: String,
    },
    /// A `crt_range` string cannot be converted.
    #[error("crt_range {spec:?}: {reason}")]
    Spec {
        /// The `crt_range` string.
        spec: String,
        /// What is wrong.
        reason: String,
    },
    /// The repository cannot be found.
    #[error(transparent)]
    Repo(#[from] crate::repo::RepoError),
}

/// Runs `cargo xtask monitors <monitor.cpp> [output directory]`. The default
/// output directory is `data/monitors` of the checkout.
pub fn run(args: &[&str]) -> Result<(), MonitorsError> {
    let (source, out) = match args {
        [source] => (
            PathBuf::from(source),
            Repo::discover()?.root.join(DEFAULT_OUT),
        ),
        [source, out] => (PathBuf::from(source), PathBuf::from(out)),
        _ => return Err(MonitorsError::Usage),
    };
    let text = std::fs::read_to_string(&source).map_err(|e| MonitorsError::Io {
        path: source.clone(),
        source: e,
    })?;
    let table = extract(&text)?;
    for preset in &table.presets {
        let path = out.join(format!("{}.toml", preset.id));
        std::fs::write(&path, render(preset)?)
            .map_err(|e| MonitorsError::Io { path, source: e })?;
    }
    println!(
        "wrote {} presets to {}; not converted: {}",
        table.presets.len(),
        out.display(),
        table.skipped.join(", ")
    );
    Ok(())
}

const DEFAULT_OUT: &str = "data/monitors";

/// A preset of the Switchres table.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Preset {
    id: String,
    /// The `crt_range` strings, as in the source.
    specs: Vec<String>,
}

/// The presets of the table, and the names that have no `crt_range` strings
/// (the VESA GTF presets, which Switchres computes at run time).
#[derive(Debug, Default, PartialEq, Eq)]
struct Table {
    presets: Vec<Preset>,
    skipped: Vec<String>,
}

/// One `crt_range` in integer units, with the field names of `CrtRange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    h_min_mhz: u64,
    h_max_mhz: u64,
    v_min_mhz: u64,
    v_max_mhz: u64,
    h_front_porch_ns: u32,
    h_sync_ns: u32,
    h_back_porch_ns: u32,
    v_front_porch_ns: u32,
    v_sync_ns: u32,
    v_back_porch_ns: u32,
    hsync_positive: bool,
    vsync_positive: bool,
    progressive_lines_min: u16,
    progressive_lines_max: u16,
    interlaced_lines_min: u16,
    interlaced_lines_max: u16,
}

const NAME_MARK: &str = "strcmp(type, \"";
const RANGE_MARK: &str = "monitor_fill_range(&range[";
const VESA_MARK: &str = "monitor_fill_vesa_gtf(";

/// Reads the preset names and their `crt_range` strings. A condition line
/// with several names (`d9800` or `d9400`) gives one preset per name.
fn extract(source: &str) -> Result<Table, MonitorsError> {
    let mut table = Table::default();
    let mut names: Vec<String> = Vec::new();
    let mut specs: Vec<String> = Vec::new();
    let mut vesa = false;
    let mut flush = |names: &mut Vec<String>, specs: &mut Vec<String>, vesa: bool, line: usize| {
        if names.is_empty() {
            return Ok(());
        }
        if vesa {
            table.skipped.append(names);
        } else if specs.is_empty() {
            return Err(MonitorsError::Table {
                line,
                reason: format!("preset {} has no crt_range", names.join(", ")),
            });
        } else {
            for id in names.drain(..) {
                table.presets.push(Preset {
                    id,
                    specs: specs.clone(),
                });
            }
        }
        specs.clear();
        Ok(())
    };
    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        if line.contains(NAME_MARK) {
            flush(&mut names, &mut specs, vesa, number)?;
            vesa = false;
            names = names_in(line);
        } else if line.contains(RANGE_MARK) {
            if names.is_empty() {
                return Err(MonitorsError::Table {
                    line: number,
                    reason: "crt_range outside a preset".to_owned(),
                });
            }
            let Some(spec) = spec_in(line) else {
                return Err(MonitorsError::Table {
                    line: number,
                    reason: "expected a quoted crt_range".to_owned(),
                });
            };
            specs.push(spec);
        } else if line.contains(VESA_MARK) && !names.is_empty() {
            vesa = true;
        }
    }
    flush(&mut names, &mut specs, vesa, source.lines().count())?;
    if table.presets.is_empty() {
        return Err(MonitorsError::Table {
            line: 0,
            reason: "no preset found".to_owned(),
        });
    }
    Ok(table)
}

/// The names in the `strcmp(type, "...")` conditions of a line.
fn names_in(line: &str) -> Vec<String> {
    line.split(NAME_MARK)
        .skip(1)
        .filter_map(|part| part.split_once('"').map(|(name, _)| name.to_owned()))
        .collect()
}

/// The quoted string after `monitor_fill_range(&range[N], `.
fn spec_in(line: &str) -> Option<String> {
    let (_, after) = line.split_once(RANGE_MARK)?;
    let (_, quoted) = after.split_once('"')?;
    let (spec, _) = quoted.split_once('"')?;
    Some(spec.to_owned())
}

/// Converts one `crt_range` string. The form is the one of the Switchres
/// documentation: `HfreqMin-HfreqMax, VfreqMin-VfreqMax, HFrontPorch,
/// HSyncPulse, HBackPorch, VFrontPorch, VSyncPulse, VBackPorch, HSyncPol,
/// VSyncPol, ProgressiveLinesMin, ProgressiveLinesMax, InterlacedLinesMin,
/// InterlacedLinesMax`. Rates are in hertz, horizontal times in
/// microseconds, vertical times in milliseconds.
fn parse_range(spec: &str) -> Result<Range, MonitorsError> {
    let error = |reason: String| MonitorsError::Spec {
        spec: spec.to_owned(),
        reason,
    };
    let fields: Vec<&str> = spec.split(',').map(str::trim).collect();
    let [
        h,
        v,
        hfp,
        hs,
        hbp,
        vfp,
        vs,
        vbp,
        hpol,
        vpol,
        pmin,
        pmax,
        imin,
        imax,
    ] = fields.as_slice()
    else {
        return Err(error(format!("{} fields, expected 14", fields.len())));
    };
    let not_a_pair = |text: &str| error(format!("{text:?} is not a min-max pair"));
    let (h_min, h_max) = h.split_once('-').ok_or_else(|| not_a_pair(h))?;
    let (v_min, v_max) = v.split_once('-').ok_or_else(|| not_a_pair(v))?;
    let rate = |text: &str| scaled(text, HZ_TO_MHZ_DIGITS).map_err(&error);
    let ns = |text: &str, digits: u32| {
        let value = scaled(text, digits).map_err(&error)?;
        u32::try_from(value).map_err(|_| error(format!("{text} is too long for u32 nanoseconds")))
    };
    let lines = |text: &str| {
        text.parse::<u16>()
            .map_err(|e| error(format!("line count {text:?}: {e}")))
    };
    let polarity = |text: &str| match text {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(error(format!("sync polarity {text:?} is not 0 or 1"))),
    };
    Ok(Range {
        h_min_mhz: rate(h_min.trim())?,
        h_max_mhz: rate(h_max.trim())?,
        v_min_mhz: rate(v_min.trim())?,
        v_max_mhz: rate(v_max.trim())?,
        h_front_porch_ns: ns(hfp, US_TO_NS_DIGITS)?,
        h_sync_ns: ns(hs, US_TO_NS_DIGITS)?,
        h_back_porch_ns: ns(hbp, US_TO_NS_DIGITS)?,
        v_front_porch_ns: ns(vfp, MS_TO_NS_DIGITS)?,
        v_sync_ns: ns(vs, MS_TO_NS_DIGITS)?,
        v_back_porch_ns: ns(vbp, MS_TO_NS_DIGITS)?,
        hsync_positive: polarity(hpol)?,
        vsync_positive: polarity(vpol)?,
        progressive_lines_min: lines(pmin)?,
        progressive_lines_max: lines(pmax)?,
        interlaced_lines_min: lines(imin)?,
        interlaced_lines_max: lines(imax)?,
    })
}

/// Decimal digits between hertz and millihertz.
const HZ_TO_MHZ_DIGITS: u32 = 3;
/// Decimal digits between microseconds and nanoseconds.
const US_TO_NS_DIGITS: u32 = 3;
/// Decimal digits between milliseconds and nanoseconds.
const MS_TO_NS_DIGITS: u32 = 6;

/// The unsigned decimal `text` times `10^digits`, exactly. A value with
/// more fraction digits than `digits` is an error unless the extra digits
/// are zero, so no conversion rounds.
fn scaled(text: &str, digits: u32) -> Result<u64, String> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let is_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !is_digits(whole) || !is_digits(fraction) {
        return Err(format!("{text:?} is not an unsigned decimal number"));
    }
    let width = usize::try_from(digits).map_err(|e| e.to_string())?;
    let (kept, dropped) = fraction.split_at(fraction.len().min(width));
    if dropped.bytes().any(|b| b != b'0') {
        return Err(format!("{text} has more than {digits} fraction digits"));
    }
    let overflow = || format!("{text} is too large");
    let mut value: u64 = 0;
    for byte in whole.bytes().chain(kept.bytes()) {
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(byte - b'0')))
            .ok_or_else(overflow)?;
    }
    let missing = u32::try_from(width - kept.len()).map_err(|e| e.to_string())?;
    value.checked_mul(10u64.pow(missing)).ok_or_else(overflow)
}

/// The TOML text of one preset file.
fn render(preset: &Preset) -> Result<String, MonitorsError> {
    let mut out = String::new();
    out.push_str(&format!(
        "# {}: converted from the Switchres preset table by `cargo xtask monitors`.\n",
        preset.id
    ));
    out.push_str("# Do not edit. See data/monitors/README.md for the source and the units.\n");
    out.push_str("# Source crt_range lines:\n");
    for spec in &preset.specs {
        out.push_str(&format!("#   {spec}\n"));
    }
    out.push_str(&format!("\nid = \"{}\"\n", preset.id));
    for spec in &preset.specs {
        let r = parse_range(spec)?;
        out.push_str("\n[[kind.crt.ranges]]\n");
        let fields: [(&str, String); 16] = [
            ("h_min_mhz", r.h_min_mhz.to_string()),
            ("h_max_mhz", r.h_max_mhz.to_string()),
            ("v_min_mhz", r.v_min_mhz.to_string()),
            ("v_max_mhz", r.v_max_mhz.to_string()),
            ("h_front_porch_ns", r.h_front_porch_ns.to_string()),
            ("h_sync_ns", r.h_sync_ns.to_string()),
            ("h_back_porch_ns", r.h_back_porch_ns.to_string()),
            ("v_front_porch_ns", r.v_front_porch_ns.to_string()),
            ("v_sync_ns", r.v_sync_ns.to_string()),
            ("v_back_porch_ns", r.v_back_porch_ns.to_string()),
            ("hsync_positive", r.hsync_positive.to_string()),
            ("vsync_positive", r.vsync_positive.to_string()),
            ("progressive_lines_min", r.progressive_lines_min.to_string()),
            ("progressive_lines_max", r.progressive_lines_max.to_string()),
            ("interlaced_lines_min", r.interlaced_lines_min.to_string()),
            ("interlaced_lines_max", r.interlaced_lines_max.to_string()),
        ];
        for (key, value) in fields {
            out.push_str(&format!("{key} = {value}\n"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNIPPET: &str = r#"
int monitor_fill_range(monitor_range *range, const char *specs_line)
{
	if (strcmp(specs_line, "auto")) {}
}
int monitor_set_preset(const char *type, monitor_range *range)
{
	// NTSC TV - 60 Hz/525
	if (!strcmp(type, "ntsc"))
	{
		monitor_fill_range(&range[0], "15734.26-15734.26, 59.94-59.94, 1.500, 4.700, 4.700, 0.191, 0.191, 0.953, 0, 0, 192, 240, 448, 480");
		return 1;
	}
	else if (!strcmp(type, "d9800") || !strcmp(type, "d9400"))
	{
		monitor_fill_range(&range[0], "15250-18000, 40-80, 2.187, 4.688, 6.719, 0.190, 0.191, 1.018, 0, 0, 224, 288, 448, 576");
		monitor_fill_range(&range[1], "18001-19000, 40-80, 2.187, 4.688, 6.719, 0.140, 0.191, 0.950, 1, 1, 288, 320, 0, 0");
		return 2;
	}
	else if (!strcmp(type, "vesa_480") || !strcmp(type, "vesa_600"))
	{
		return monitor_fill_vesa_gtf(&range[0], type);
	}
	return 0;
}
"#;

    #[test]
    fn scaled_converts_decimal_text_exactly() {
        assert_eq!(scaled("15734.26", 3), Ok(15_734_260));
        assert_eq!(scaled("49.5", 3), Ok(49_500));
        assert_eq!(scaled("40", 3), Ok(40_000));
        assert_eq!(scaled("4.700", 3), Ok(4_700));
        assert_eq!(scaled("0.064", 6), Ok(64_000));
        assert_eq!(scaled("1.0560", 3), Ok(1_056));
        assert_eq!(scaled("7.", 3), Ok(7_000));
    }

    #[test]
    fn scaled_refuses_what_it_cannot_convert_exactly() {
        for bad in ["", ".5", "-1", "1e3", "1.2.3", " 1", "0x10", "1.2345"] {
            assert!(scaled(bad, 3).is_err(), "{bad:?}");
        }
        assert!(scaled("18446744073709551615", 0).is_ok());
        assert!(scaled("18446744073709551616", 0).is_err());
        assert!(scaled("18446744073709551615", 1).is_err());
    }

    #[test]
    fn parse_range_converts_each_unit() {
        let r = parse_range(
            "15734.26-15734.26, 59.94-59.94, 1.500, 4.700, 4.700, 0.191, 0.191, 0.953, 0, 1, 192, 240, 448, 480",
        )
        .unwrap();
        assert_eq!(
            r,
            Range {
                h_min_mhz: 15_734_260,
                h_max_mhz: 15_734_260,
                v_min_mhz: 59_940,
                v_max_mhz: 59_940,
                h_front_porch_ns: 1_500,
                h_sync_ns: 4_700,
                h_back_porch_ns: 4_700,
                v_front_porch_ns: 191_000,
                v_sync_ns: 191_000,
                v_back_porch_ns: 953_000,
                hsync_positive: false,
                vsync_positive: true,
                progressive_lines_min: 192,
                progressive_lines_max: 240,
                interlaced_lines_min: 448,
                interlaced_lines_max: 480,
            }
        );
    }

    #[test]
    fn parse_range_errors_name_the_problem() {
        let good = "15625-15750, 49.50-65.00, 2.000, 4.700, 8.000, 0.064, 0.192, 1.024, 0, 0, 192, 288, 448, 576";
        let cases = [
            (good.replace(", 576", ""), "13 fields, expected 14"),
            (
                good.replace("15625-15750", "15625"),
                "is not a min-max pair",
            ),
            (
                good.replace("0, 0, 192", "2, 0, 192"),
                "sync polarity \"2\"",
            ),
            (
                good.replace("192, 288", "192, 70000"),
                "line count \"70000\"",
            ),
            (
                good.replace("4.700", "4.7001"),
                "more than 3 fraction digits",
            ),
            (
                good.replace("1.024", "5000"),
                "too long for u32 nanoseconds",
            ),
        ];
        for (spec, expected) in cases {
            let err = parse_range(&spec).unwrap_err().to_string();
            assert!(err.contains(expected), "{err}");
            assert!(err.contains(&spec), "{err}");
        }
    }

    #[test]
    fn extract_reads_presets_aliases_and_skips_vesa() {
        let table = extract(SNIPPET).unwrap();
        let ids: Vec<&str> = table.presets.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["ntsc", "d9800", "d9400"]);
        assert_eq!(table.presets[0].specs.len(), 1);
        assert_eq!(table.presets[1].specs, table.presets[2].specs);
        assert_eq!(table.presets[1].specs.len(), 2);
        assert_eq!(table.skipped, ["vesa_480", "vesa_600"]);
    }

    #[test]
    fn extract_refuses_a_preset_without_ranges() {
        let broken = SNIPPET.replace("monitor_fill_range(&range[0], \"15734.26", "x(\"");
        let err = extract(&broken).unwrap_err().to_string();
        assert!(err.contains("preset ntsc has no crt_range"), "{err}");
        assert!(extract("int main() {}").is_err());
    }

    #[test]
    fn render_writes_the_profile_shape() {
        let preset = Preset {
            id: "ntsc".to_owned(),
            specs: vec![
                "15734.26-15734.26, 59.94-59.94, 1.500, 4.700, 4.700, 0.191, 0.191, 0.953, 0, 0, 192, 240, 448, 480".to_owned(),
            ],
        };
        let text = render(&preset).unwrap();
        assert!(text.contains("\nid = \"ntsc\"\n"));
        assert!(text.contains("#   15734.26-15734.26, 59.94-59.94,"));
        assert!(text.contains("\n[[kind.crt.ranges]]\nh_min_mhz = 15734260\n"));
        assert!(text.ends_with("interlaced_lines_max = 480\n"));
        assert_eq!(text.matches(" = ").count(), 17);
    }
}
