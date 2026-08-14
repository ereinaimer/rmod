//! Rendering of help and version output.
//!
//! Each function returns the complete text for one page; the dispatcher in
//! `main` prints it and exits.

/// Top-level help page: command index, global options, and examples.
pub fn help() -> String {
    format!(
        "rmod {}
Resolution modifier

{usage}
  rmod <COMMAND> [OPTIONS]

{commands}
{command_rows}

{options}
{option_rows}

{examples}
  rmod list --caps
  rmod set -p 1080
  rmod layout -m 2 --primary",
        env!("CARGO_PKG_VERSION"),
        usage = section("Usage:"),
        commands = section("Commands:"),
        command_rows = options(&[
            ("list", "List displays and their current settings"),
            ("set", "Apply resolution, refresh rate, and orientation"),
            ("layout", "Show the monitor arrangement or move monitors"),
        ]),
        options = section("Options:"),
        option_rows = options(&[
            ("--help", "Print help"),
            ("--version", "Print version"),
        ]),
        examples = section("Examples:"),
    )
}

/// Help page for the `list` command (alias: `ls`).
pub fn ls() -> String {
    format!(
        "rmod list
List displays and their current settings

{usage}
  rmod list [OPTIONS]

{alias} ls

{options}
{option_rows}

{examples}
  rmod list
  rmod list --caps
  rmod list --caps -m 2",
        usage = section("Usage:"),
        alias = section("Alias:"),
        options = section("Options:"),
        option_rows = options(&[
            ("--caps", "List supported modes instead of current settings"),
            ("-m, --monitor", "Monitor number or 'all' (requires --caps)"),
            ("--help", "Print help"),
        ]),
        examples = section("Examples:"),
    )
}

/// Help page for the `set` command.
pub fn set() -> String {
    let orientation_rows = [
        ("0", "landscape", "l"),
        ("90", "portrait", "p"),
        ("180", "landscape-flipped", "lf"),
        ("270", "portrait-flipped", "pf"),
    ]
    .iter()
    .map(|(angle, name, alias)| format!("  {angle:<3}  {name:<17}   {alias}"))
    .collect::<Vec<_>>()
    .join("\n");

    format!(
        "rmod set
Apply resolution, refresh rate, and orientation to a display

{usage}
  rmod set [OPTIONS]

{options}
{option_rows}

{profiles}
{profile_rows}

{orientations}
{orientation_rows}

{examples}
  rmod set --max
  rmod set -p 1080
  rmod set -w 1920 -h 1080 -m 2 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(&[
            ("-w, --width", "Resolution width (requires --height)"),
            ("-h, --height", "Resolution height (requires --width)"),
            ("-r, --refresh", "Refresh rate in Hz, or 'max'"),
            ("-p, --profile", "Resolution preset (see Profiles below)"),
            ("-m, --monitor", "Monitor number or 'all' (default: primary)"),
            ("-o, --orientation", "Rotation angle (see Orientations below)"),
            ("-y, --yes", "Skip the confirmation prompt"),
            ("--max", "Use the display's highest supported mode"),
            ("--help", "Print help"),
        ]),
        profiles = section("Profiles:"),
        profile_rows = profiles_table(),
        orientations = section("Orientations:"),
        orientation_rows = orientation_rows,
        examples = section("Examples:"),
    )
}

/// Help page for the `layout` command.
pub fn layout() -> String {
    format!(
        "rmod layout
Show the monitor arrangement, place monitors, or set the primary display

{usage}
  rmod layout [OPTIONS]

{options}
{option_rows}

{examples}
  rmod layout
  rmod layout -m 2 --left-of 1
  rmod layout -m 2 --below 1
  rmod layout -m 2 --primary",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(&[
            ("-m, --monitor", "Monitor to move or promote"),
            ("--left-of", "Place the monitor left of the reference"),
            ("--right-of", "Place the monitor right of the reference"),
            ("--above", "Place the monitor above the reference"),
            ("--below", "Place the monitor below the reference"),
            ("--primary", "Make the monitor the main display"),
            ("-y, --yes", "Skip the confirmation prompt"),
            ("--help", "Print help"),
        ]),
        examples = section("Examples:"),
    )
}

/// Version string, e.g. `rmod 0.1.0`.
pub fn version() -> String {
    format!("rmod {}", env!("CARGO_PKG_VERSION"))
}

/// ANSI-underline a section header such as `Usage:`.
pub(crate) fn section(title: &str) -> String {
    format!("\x1b[4m{title}\x1b[0m")
}

/// Render aligned option rows: `"  {left:<max}  {desc}"`, joined by `\n`.
pub(crate) fn options(rows: &[(&str, &str)]) -> String {
    let max_width = rows.iter().map(|(left, _)| left.len()).max().unwrap_or(0);
    rows.iter()
        .map(|(left, desc)| format!("  {left:<max_width$}  {desc}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the profiles table from `parser::PROFILES`, joined by `\n`.
pub(crate) fn profiles_table() -> String {
    crate::cli::parser::PROFILES
        .iter()
        .map(|(name, width, height)| format!("  {name:<4}  {width}x{height}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strip ANSI escape sequences used by the pages (`\x1b[4m`, `\x1b[0m`).
    fn strip_ansi(s: &str) -> String {
        s.replace("\x1b[4m", "").replace("\x1b[0m", "")
    }

    #[test]
    fn top_help_matches_spec_mockup() {
        let expected = format!(
            "rmod {}
Resolution modifier

Usage:
  rmod <COMMAND> [OPTIONS]

Commands:
  list    List displays and their current settings
  set     Apply resolution, refresh rate, and orientation
  layout  Show the monitor arrangement or move monitors

Options:
  --help     Print help
  --version  Print version

Examples:
  rmod list --caps
  rmod set -p 1080
  rmod layout -m 2 --primary",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(strip_ansi(&help()), expected);
    }

    #[test]
    fn top_help_omits_profiles_alias_and_yes() {
        let h = strip_ansi(&help());
        assert!(!h.contains("Profiles"), "profiles table must not appear at top level");
        assert!(!h.contains("1280x720"), "profiles table must not appear at top level");
        assert!(!h.contains("(alias: ls)"), "ls alias must not appear at top level");
        assert!(!h.contains("-y"), "-y must not appear at top level");
    }

    #[test]
    fn ls_help_matches_spec_mockup() {
        let expected = "rmod list
List displays and their current settings

Usage:
  rmod list [OPTIONS]

Alias: ls

Options:
  --caps         List supported modes instead of current settings
  -m, --monitor  Monitor number or 'all' (requires --caps)
  --help         Print help

Examples:
  rmod list
  rmod list --caps
  rmod list --caps -m 2";
        assert_eq!(strip_ansi(&ls()), expected);
    }

    #[test]
    fn set_help_matches_spec_mockup() {
        let expected = "rmod set
Apply resolution, refresh rate, and orientation to a display

Usage:
  rmod set [OPTIONS]

Options:
  -w, --width        Resolution width (requires --height)
  -h, --height       Resolution height (requires --width)
  -r, --refresh      Refresh rate in Hz, or 'max'
  -p, --profile      Resolution preset (see Profiles below)
  -m, --monitor      Monitor number or 'all' (default: primary)
  -o, --orientation  Rotation angle (see Orientations below)
  -y, --yes          Skip the confirmation prompt
  --max              Use the display's highest supported mode
  --help             Print help

Profiles:
  720   1280x720
  1080  1920x1080
  1440  2560x1440
  4k    3840x2160
  8k    7680x4320

Orientations:
  0    landscape           l
  90   portrait            p
  180  landscape-flipped   lf
  270  portrait-flipped    pf

Examples:
  rmod set --max
  rmod set -p 1080
  rmod set -w 1920 -h 1080 -m 2 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y";
        assert_eq!(strip_ansi(&set()), expected);
    }

    #[test]
    fn set_help_renders_profiles_table() {
        let h = strip_ansi(&set());
        assert!(h.contains(&profiles_table()), "set page must embed the profiles table");
    }

    #[test]
    fn layout_help_matches_spec_mockup() {
        let expected = "rmod layout
Show the monitor arrangement, place monitors, or set the primary display

Usage:
  rmod layout [OPTIONS]

Options:
  -m, --monitor  Monitor to move or promote
  --left-of      Place the monitor left of the reference
  --right-of     Place the monitor right of the reference
  --above        Place the monitor above the reference
  --below        Place the monitor below the reference
  --primary      Make the monitor the main display
  -y, --yes      Skip the confirmation prompt
  --help         Print help

Examples:
  rmod layout
  rmod layout -m 2 --left-of 1
  rmod layout -m 2 --below 1
  rmod layout -m 2 --primary";
        assert_eq!(strip_ansi(&layout()), expected);
    }

    #[test]
    fn version_matches_package_version() {
        assert_eq!(version(), format!("rmod {}", env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn section_underlines_title() {
        assert_eq!(section("Usage"), "\x1b[4mUsage\x1b[0m");
    }

    #[test]
    fn section_underlines_empty_title() {
        assert_eq!(section(""), "\x1b[4m\x1b[0m");
    }

    #[test]
    fn options_aligns_rows_to_widest_left() {
        let rows = &[
            ("--caps", "List supported modes"),
            ("-m, --monitor <MONITOR>", "Monitor number"),
        ];
        assert_eq!(
            options(rows),
            "  --caps                   List supported modes\n  -m, --monitor <MONITOR>  Monitor number"
        );
    }

    #[test]
    fn options_single_row_has_no_trailing_newline() {
        assert_eq!(options(&[("--max", "Use the highest supported mode")]), "  --max  Use the highest supported mode");
    }

    #[test]
    fn options_empty_rows_returns_empty_string() {
        assert_eq!(options(&[]), "");
    }

    #[test]
    fn profiles_table_renders_all_profiles() {
        let expected = "  720   1280x720\n  1080  1920x1080\n  1440  2560x1440\n  4k    3840x2160\n  8k    7680x4320";
        assert_eq!(profiles_table(), expected);
    }

    #[test]
    fn profiles_table_has_no_trailing_newline() {
        assert!(!profiles_table().ends_with('\n'));
    }
}
