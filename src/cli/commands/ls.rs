//! `ls` command: lists every display with full EDID information.
//!
//! [`run_list`] prints detailed blocks for each monitor including
//! manufacturer, current/native resolution, manufacture date, and supported
//! modes grouped by resolution. Each block's heading is the display name
//! with the EDID fingerprint suffix (e.g. `Lenovo 9059 [a1b2c3d4]`).

use crate::cli::parser::{Command, HelpTopic};
use crate::sys::windows;
use crate::sys::windows::edid::GamutCoverage;
use crate::sys::windows::hdr::hdr_label;

/// The `Physical:` value: diagonal size in inches plus the cm dimensions.
fn physical_line(size: Option<(f32, f32)>) -> String {
    match size {
        Some((w, h)) if w > 0.0 && h > 0.0 => {
            let diag = (w * w + h * h).sqrt() / 2.54;
            format!("{diag:.1}\" ({w:.1} cm × {h:.1} cm)")
        }
        _ => "Unknown".to_string(),
    }
}

/// The `DPI:` value: physical `HxV` and logical DPI, whichever is known.
fn dpi_line(dpi: Option<(u32, u32)>, logical: u32) -> String {
    let physical = dpi.map(|(h, v)| format!("{h}×{v} physical"));
    let logical = (logical > 0).then(|| format!("{logical} logical"));
    match (physical, logical) {
        (Some(p), Some(l)) => format!("{p} / {l}"),
        (Some(p), None) => p,
        (None, Some(l)) => l,
        (None, None) => "Unknown".to_string(),
    }
}

/// The `Color Depth:` value from the mode's bits-per-pel.
fn color_depth(bits: u32) -> String {
    match bits {
        32 => "32-bit (RGB 8:8:8)".to_string(),
        30 => "30-bit (RGB 10:10:10)".to_string(),
        24 => "24-bit (RGB 8:8:8)".to_string(),
        16 => "16-bit (RGB 5:6:5)".to_string(),
        0 => "Unknown".to_string(),
        n => format!("{n}-bit"),
    }
}

/// The `Orientation:` value from the display orientation angle.
fn orientation(angle: u32) -> String {
    match angle {
        0 => "Landscape".to_string(),
        1 => "Portrait".to_string(),
        2 => "Landscape (flipped)".to_string(),
        3 => "Portrait (flipped)".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// The `Gamma:` value.
fn gamma_line(g: Option<f32>) -> String {
    match g {
        Some(v) => format!("{v:.1}"),
        None => "Unknown".to_string(),
    }
}

/// The `Color Gamut:` value from the EDID chromaticity coverage.
fn gamut_line(g: Option<GamutCoverage>) -> String {
    match g {
        Some(c) => format!("sRGB {}% / DCI-P3 {}%", c.srgb, c.p3),
        None => "Unknown".to_string(),
    }
}

/// A supported-mode line with the `@` aligned to the longest resolution
/// in the list (`res_width` is the padded width of the resolution column).
fn mode_line(width: u32, height: u32, rates: &str, res_width: usize) -> String {
    format!("{:<res_width$} @ {}", format!("{width}x{height}"), rates)
}

/// Lists every display with full EDID information and supported modes.
pub(super) fn run_list() -> i32 {
    match windows::list_detailed() {
        Ok(mut monitors) => {
            monitors.sort_by(|a, b| {
                // Primary first, then by fingerprint
                match (a.is_primary, b.is_primary) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.fingerprint.cmp(&b.fingerprint),
                }
            });
            for (i, m) in monitors.iter().enumerate() {
                if i > 0 {
                    println!(); // blank line between monitors
                }
                print_monitor(m);
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Lists every display in compact one-line format.
pub(super) fn run_list_short() -> i32 {
    match windows::list_detailed() {
        Ok(mut monitors) => {
            monitors.sort_by(|a, b| {
                // Primary first, then by fingerprint
                match (a.is_primary, b.is_primary) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.fingerprint.cmp(&b.fingerprint),
                }
            });
            for m in monitors {
                let primary = if m.is_primary { "  (primary)" } else { "" };
                println!(
                    "# {}: {} [{}]  {}x{}@{}Hz{}",
                    m.number, m.name, m.fingerprint, m.width, m.height, m.refresh, primary
                );
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Prints a single monitor's detailed information.
fn print_monitor(m: &crate::sys::windows::Monitor) {
    println!("{}", m.name);
    println!(
        "  Primary:         {}",
        if m.is_primary { "true" } else { "false" }
    );
    println!("  Manufacturer:    {}", m.manufacturer);
    println!(
        "  Current:         {}x{} @ {}Hz",
        m.width, m.height, m.refresh
    );
    println!(
        "  Native:          {}x{} @ {}Hz",
        m.native_width, m.native_height, m.native_refresh
    );
    println!("  Physical:        {}", physical_line(m.physical_size_cm));
    println!(
        "  DPI:             {}",
        dpi_line(m.dpi_physical, m.log_pixels)
    );
    println!("  Color Depth:     {}", color_depth(m.bits_per_pel));
    println!("  Orientation:     {}", orientation(m.orientation));
    println!("  Connector:       {}", m.connector.unwrap_or("Unknown"));
    println!(
        "  Manufactured:    Week {}, {}",
        m.manufactured_week, m.manufactured_year
    );
    println!("  Gamma:           {}", gamma_line(m.gamma));
    println!("  HDR:             {}", hdr_label(m.hdr.as_ref()));
    println!("  Color Gamut:     {}", gamut_line(m.gamut));
    println!("  Supported:");

    // Group modes by resolution
    let mut modes_by_res: std::collections::HashMap<(u32, u32), Vec<u32>> =
        std::collections::HashMap::new();
    for mode in windows::caps_all_modes_for_device(&m.device_name) {
        modes_by_res
            .entry((mode.width, mode.height))
            .or_default()
            .push(mode.refresh);
    }

    // Sort resolutions by width, then height
    let mut resolutions: Vec<_> = modes_by_res.keys().cloned().collect();
    resolutions.sort_by_key(|&(w, h)| (w, h));

    let res_width = resolutions
        .iter()
        .map(|&(w, h)| format!("{w}x{h}").len())
        .max()
        .unwrap_or(0);

    for (width, height) in resolutions {
        if let Some(refresh_rates) = modes_by_res.get(&(width, height)) {
            let mut rates = refresh_rates.clone();
            rates.sort_unstable();
            rates.dedup();
            let rates_str = rates
                .iter()
                .map(|r| format!("{}Hz", r))
                .collect::<Vec<_>>()
                .join(", ");
            println!("    {}", mode_line(width, height, &rates_str, res_width));
        }
    }
}

pub(crate) fn parse_ls(_cmd: &str, args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut i = 1;
    let mut short = false;

    while let Some(arg) = args.get(i) {
        match arg.as_ref() {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::List),
                });
            }
            "--version" => return Ok(Command::Version),
            "--short" => {
                short = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for list. use --help",
                    other
                ));
            }
        }
    }

    Ok(Command::List { short })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_line_formats_inches_and_cm() {
        assert_eq!(
            physical_line(Some((59.8, 33.6))),
            "27.0\" (59.8 cm × 33.6 cm)"
        );
        assert_eq!(
            physical_line(Some((53.1, 29.9))),
            "24.0\" (53.1 cm × 29.9 cm)"
        );
    }

    #[test]
    fn physical_line_unknown_for_missing_or_non_positive_size() {
        assert_eq!(physical_line(None), "Unknown");
        assert_eq!(physical_line(Some((0.0, 33.6))), "Unknown");
        assert_eq!(physical_line(Some((59.8, 0.0))), "Unknown");
    }

    #[test]
    fn dpi_line_joins_physical_and_logical() {
        assert_eq!(dpi_line(Some((82, 82)), 96), "82×82 physical / 96 logical");
        assert_eq!(
            dpi_line(Some((92, 92)), 144),
            "92×92 physical / 144 logical"
        );
    }

    #[test]
    fn dpi_line_omits_unknown_parts() {
        assert_eq!(dpi_line(Some((82, 82)), 0), "82×82 physical");
        assert_eq!(dpi_line(None, 96), "96 logical");
        assert_eq!(dpi_line(None, 0), "Unknown");
    }

    #[test]
    fn color_depth_maps_common_bits_per_pel() {
        assert_eq!(color_depth(32), "32-bit (RGB 8:8:8)");
        assert_eq!(color_depth(30), "30-bit (RGB 10:10:10)");
        assert_eq!(color_depth(24), "24-bit (RGB 8:8:8)");
        assert_eq!(color_depth(16), "16-bit (RGB 5:6:5)");
        assert_eq!(color_depth(0), "Unknown");
        assert_eq!(color_depth(10), "10-bit");
    }

    #[test]
    fn orientation_maps_angles() {
        assert_eq!(orientation(0), "Landscape");
        assert_eq!(orientation(1), "Portrait");
        assert_eq!(orientation(2), "Landscape (flipped)");
        assert_eq!(orientation(3), "Portrait (flipped)");
        assert_eq!(orientation(90), "Unknown");
    }

    #[test]
    fn gamma_line_formats_one_decimal() {
        assert_eq!(gamma_line(Some(2.2)), "2.2");
        assert_eq!(gamma_line(Some(2.4)), "2.4");
        assert_eq!(gamma_line(None), "Unknown");
    }

    #[test]
    fn gamut_line_formats_coverage() {
        let g = GamutCoverage { srgb: 100, p3: 95 };
        assert_eq!(gamut_line(Some(g)), "sRGB 100% / DCI-P3 95%");
        assert_eq!(gamut_line(None), "Unknown");
    }

    #[test]
    fn mode_line_aligns_at_to_longest_resolution() {
        assert_eq!(
            mode_line(1920, 1080, "60Hz, 144Hz", 9),
            "1920x1080 @ 60Hz, 144Hz"
        );
        assert_eq!(mode_line(1280, 720, "60Hz", 9), "1280x720  @ 60Hz");
        assert_eq!(mode_line(640, 480, "120Hz", 9), "640x480   @ 120Hz");
    }

    const SERIAL_A: &str = "ABC12345678";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn ls_help_flags() {
        assert_eq!(
            parse(&["ls", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::List)
            })
        );
        assert_eq!(
            parse(&["ls", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::List)
            })
        );
    }

    #[test]
    fn ls_version_flag() {
        assert_eq!(parse(&["ls", "--version"]), Ok(Command::Version));
    }

    #[test]
    fn ls_unknown_argument_is_error() {
        assert_eq!(
            parse(&["ls", "foo"]),
            Err("unexpected argument foo for list. use --help".to_string())
        );
    }

    #[test]
    fn list_unknown_argument_is_error() {
        assert_eq!(
            parse(&["list", "foo"]),
            Err("unexpected argument foo for list. use --help".to_string())
        );
    }

    #[test]
    fn list_help_flag() {
        assert_eq!(
            parse(&["list", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::List)
            })
        );
    }

    #[test]
    fn ls_rejects_caps_flag() {
        assert_eq!(
            parse(&["ls", "--caps"]),
            Err("unexpected argument --caps for list. use --help".to_string())
        );
    }

    #[test]
    fn ls_rejects_monitor_flag() {
        assert_eq!(
            parse(&["ls", "-m", SERIAL_A]),
            Err("unexpected argument -m for list. use --help".to_string())
        );
    }

    #[test]
    fn ls_rejects_all_old_flags() {
        for args in [
            &["ls", "--caps", "-m", SERIAL_A][..],
            &["ls", "-m", SERIAL_A, "--caps"][..],
            &["ls", "--caps", "-m", "all"][..],
            &["ls", "--caps", "--help"][..],
            &["ls", "-m", "2", "--caps"][..],
        ] {
            assert!(parse(args).is_err(), "args: {:?}", args);
        }
    }
}
