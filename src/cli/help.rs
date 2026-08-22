//! Rendering of help and version output.
//!
//! Each function returns the complete text for one page; the dispatcher in
//! `main` prints it and exits. All content rows come from the registries in
//! `flags`; this module only renders them.

use crate::cli::flags::{
    ATTACH_FLAGS, BRIGHTNESS_FLAGS, COMPLETIONS_FLAGS, CONTRAST_FLAGS, EXTEND_FLAGS, Flag,
    LAYOUT_FLAGS, LS_FLAGS, MIRROR_FLAGS, ORIENTATIONS, PROJECT_FLAGS, SET_FLAGS, SINGLE_FLAGS,
    SLEEP_FLAGS, TEMP_FLAGS, TEMP_PRESETS, TOP_FLAGS, WAKE_FLAGS,
};

/// Top-level help page: command index, global options, and examples.
pub fn help() -> String {
    format!(
        "rmod {}
Resolution modifier

{usage}
  rmod [COMMAND] [OPTIONS]

{commands}
{command_rows}

{options}
{option_rows}

{examples}
  rmod list
  rmod set -p 1080
  rmod layout -m a1b2c3d4 --primary
  rmod temp 3400",
        env!("CARGO_PKG_VERSION"),
        usage = section("Usage:"),
        commands = section("Commands:"),
        command_rows = flat_commands(),
        options = section("Options:"),
        option_rows = options(TOP_FLAGS),
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
  rmod list",
        usage = section("Usage:"),
        alias = section("Alias:"),
        options = section("Options:"),
        option_rows = options(LS_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `set` command.
pub fn set() -> String {
    let orientation_rows = ORIENTATIONS
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
  rmod set -w 1920 -h 1080 -m a1b2c3d4 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(SET_FLAGS),
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
  rmod layout -m a1b2c3d4 --left-of b2c3d4e5
  rmod layout -m a1b2c3d4 --below b2c3d4e5
  rmod layout -m a1b2c3d4 --primary",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(LAYOUT_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `brightness` command.
pub fn brightness() -> String {
    format!(
        "rmod brightness
Set the display backlight level (0-100, or min, max, boost)

{usage}
  rmod brightness <VALUE> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod brightness 60
  rmod brightness 40 -m 2 -v gamma
  rmod brightness 75 -m all
  rmod brightness min -m 2
  rmod brightness boost",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(BRIGHTNESS_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `contrast` command.
pub fn contrast() -> String {
    format!(
        "rmod contrast
Set the display contrast (0-130, 100 = neutral)

{usage}
  rmod contrast <VALUE> [OPTIONS]
  rmod contrast reset [OPTIONS]

{options}
{option_rows}

{examples}
  rmod contrast 60
  rmod contrast 130 -v gamma
  rmod contrast 75 -m all
  rmod contrast reset",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(CONTRAST_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `attach` command.
pub fn attach() -> String {
    format!(
        "rmod attach
Re-attach a monitor to the desktop

{usage}
  rmod attach -m <MONITOR> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod attach -m a1b2c3d4
  rmod attach -m a1b2c3d4 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(ATTACH_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `detach` command.
pub fn detach() -> String {
    format!(
        "rmod detach
Detach a monitor from the desktop

{usage}
  rmod detach -m <MONITOR> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod detach -m a1b2c3d4
  rmod detach -m a1b2c3d4 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(ATTACH_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `sleep` command.
pub fn sleep() -> String {
    format!(
        "rmod sleep
Put every monitor to sleep

{usage}
  rmod sleep

{options}
{option_rows}

{examples}
  rmod sleep",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(SLEEP_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `wake` command.
pub fn wake() -> String {
    format!(
        "rmod wake
Wake every monitor

{usage}
  rmod wake

{options}
{option_rows}

{examples}
  rmod wake",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(WAKE_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `mirror` command.
pub fn mirror() -> String {
    format!(
        "rmod mirror
Clone all displays: same position (0,0), same resolution (lowest common denominator)

{usage}
  rmod mirror [OPTIONS]

{options}
{option_rows}

{examples}
  rmod mirror
  rmod mirror -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(MIRROR_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `extend` command.
pub fn extend() -> String {
    format!(
        "rmod extend
Restore extended desktop: side-by-side positions (auto-arrange left-to-right by monitor number)

{usage}
  rmod extend [OPTIONS]

{options}
{option_rows}

{examples}
  rmod extend
  rmod extend -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(EXTEND_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `project` command.
pub fn project() -> String {
    format!(
        "rmod project
Second screen only: disable primary (laptop), keep external monitor(s) enabled

{usage}
  rmod project [OPTIONS]

{options}
{option_rows}

{examples}
  rmod project
  rmod project -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(PROJECT_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `single` command.
pub fn single() -> String {
    format!(
        "rmod single
PC screen only: enable only one monitor, disable all others

{usage}
  rmod single -m <MONITOR> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod single -m 2
  rmod single -m a1b2c3d4 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(SINGLE_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `completions` command.
pub fn completions() -> String {
    format!(
        "rmod completions
Output PowerShell tab-completion script

{usage}
  rmod completions [OPTIONS]

{options}
{option_rows}

{examples}
  rmod completions
  rmod completions >> $PROFILE
  rmod completions | Out-String | Invoke-Expression",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(COMPLETIONS_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for the `temp` command.
pub fn temp() -> String {
    format!(
        "rmod temp
Set or show the display color temperature

{usage}
  rmod temp [TEMPERATURE] [OPTIONS]

{options}
{option_rows}

{presets}
{preset_rows}

{examples}
  rmod temp
  rmod temp 3400
  rmod temp warm
  rmod temp reset
  rmod temp -m a1b2c3d4 4000",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(TEMP_FLAGS),
        presets = section("Presets:"),
        preset_rows = temp_presets_table(),
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
pub(crate) fn options(rows: &[Flag]) -> String {
    let max_width = rows.iter().map(|f| f.flag.len()).max().unwrap_or(0);
    rows.iter()
        .map(|f| format!("  {:<max_width$}  {}", f.flag, f.doc))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render flat command rows: `    {name:<max}  {doc}` rows, joined by `\n`.
pub(crate) fn flat_commands() -> String {
    let cmds: &[(&str, &str)] = &[
        ("list", "List displays"),
        ("set", "Set resolution/refresh/orientation"),
        ("layout", "Show/arrange monitor layout"),
        ("brightness", "Set backlight (0-100, min/max/boost)"),
        ("contrast", "Set contrast (0-130, 100=neutral)"),
        ("temp", "Set/show color temperature"),
        ("attach", "Attach a monitor"),
        ("detach", "Detach a monitor"),
        ("sleep", "Put monitors to sleep"),
        ("wake", "Wake monitors"),
        ("mirror", "Mirror displays"),
        ("extend", "Extend desktop (auto-arrange)"),
        ("project", "Project to external (disable primary)"),
        ("single", "Single display only"),
        ("completions", "Output PowerShell completions"),
    ];
    let max_width = cmds.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    cmds.iter()
        .map(|(name, doc)| format!("    {name:<max_width$}  {doc}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the profiles table from `flags::PROFILES`, joined by `\n`.
pub(crate) fn profiles_table() -> String {
    crate::cli::flags::PROFILES
        .iter()
        .map(|(name, width, height)| format!("  {name:<4}  {width}x{height}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the temperature presets table from `flags::TEMP_PRESETS`, joined
/// by `\n`.
pub(crate) fn temp_presets_table() -> String {
    TEMP_PRESETS
        .iter()
        .map(|(name, alias, kelvin)| format!("  {name:<9}{alias:<13}{kelvin}K"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::flags::EXAMPLES;
    use crate::cli::parser::parse_from;

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
  rmod [COMMAND] [OPTIONS]

Commands:
    list         List displays
    set          Set resolution/refresh/orientation
    layout       Show/arrange monitor layout
    brightness   Set backlight (0-100, min/max/boost)
    contrast     Set contrast (0-130, 100=neutral)
    temp         Set/show color temperature
    attach       Attach a monitor
    detach       Detach a monitor
    sleep        Put monitors to sleep
    wake         Wake monitors
    mirror       Mirror displays
    extend       Extend desktop (auto-arrange)
    project      Project to external (disable primary)
    single       Single display only
    completions  Output PowerShell completions

Options:
  -h, --help     Print help
  -V, --version  Print version

Examples:
  rmod list
  rmod set -p 1080
  rmod layout -m a1b2c3d4 --primary
  rmod temp 3400",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(strip_ansi(&help()), expected);
    }

    #[test]
    fn brightness_help_matches_spec_mockup() {
        let expected = "rmod brightness
Set the display backlight level (0-100, or min, max, boost)

Usage:
  rmod brightness <VALUE> [OPTIONS]

Options:
  -m, --monitor    Monitor number or all (default: primary)
  -v, --via        Backend: ddc, slider, or gamma (default: auto; not valid with min, max, boost)
  min, max, boost  Composite modes: min (barely lit), max (hardware 100 + gamma 100), boost (hardware 100 + overdriven gamma)
  -h, --help       Print help

Examples:
  rmod brightness 60
  rmod brightness 40 -m 2 -v gamma
  rmod brightness 75 -m all
  rmod brightness min -m 2
  rmod brightness boost";
        assert_eq!(strip_ansi(&brightness()), expected);
    }

    #[test]
    fn contrast_help_matches_spec_mockup() {
        let expected = "rmod contrast
Set the display contrast (0-130, 100 = neutral)

Usage:
  rmod contrast <VALUE> [OPTIONS]
  rmod contrast reset [OPTIONS]

Options:
  -m, --monitor  Monitor number or all (default: primary)
  -v, --via      Backend: ddc or gamma (default: auto)
  -h, --help     Print help

Examples:
  rmod contrast 60
  rmod contrast 130 -v gamma
  rmod contrast 75 -m all
  rmod contrast reset";
        assert_eq!(strip_ansi(&contrast()), expected);
    }

    #[test]
    fn attach_help_matches_spec_mockup() {
        let expected = "rmod attach
Re-attach a monitor to the desktop

Usage:
  rmod attach -m <MONITOR> [OPTIONS]

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (required)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod attach -m a1b2c3d4
  rmod attach -m a1b2c3d4 -y";
        assert_eq!(strip_ansi(&attach()), expected);
    }

    #[test]
    fn attach_help_has_no_aliases_section() {
        let h = strip_ansi(&attach());
        assert!(
            !h.contains("Aliases"),
            "attach page must not carry the old aliases section, got: {h}"
        );
        assert!(!h.contains("enable, on"));
    }

    #[test]
    fn detach_help_matches_spec_mockup() {
        let expected = "rmod detach
Detach a monitor from the desktop

Usage:
  rmod detach -m <MONITOR> [OPTIONS]

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (required)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod detach -m a1b2c3d4
  rmod detach -m a1b2c3d4 -y";
        assert_eq!(strip_ansi(&detach()), expected);
    }

    #[test]
    fn sleep_help_matches_spec_mockup() {
        let expected = "rmod sleep
Put every monitor to sleep

Usage:
  rmod sleep

Options:
  -h, --help  Print help

Examples:
  rmod sleep";
        assert_eq!(strip_ansi(&sleep()), expected);
    }

    #[test]
    fn wake_help_matches_spec_mockup() {
        let expected = "rmod wake
Wake every monitor

Usage:
  rmod wake

Options:
  -h, --help  Print help

Examples:
  rmod wake";
        assert_eq!(strip_ansi(&wake()), expected);
    }

    #[test]
    fn mirror_help_matches_spec_mockup() {
        let expected = "rmod mirror
Clone all displays: same position (0,0), same resolution (lowest common denominator)

Usage:
  rmod mirror [OPTIONS]

Options:
  -y, --yes   Skip the confirmation prompt
  -h, --help  Print help

Examples:
  rmod mirror
  rmod mirror -y";
        assert_eq!(strip_ansi(&mirror()), expected);
    }

    #[test]
    fn extend_help_matches_spec_mockup() {
        let expected = "rmod extend
Restore extended desktop: side-by-side positions (auto-arrange left-to-right by monitor number)

Usage:
  rmod extend [OPTIONS]

Options:
  -y, --yes   Skip the confirmation prompt
  -h, --help  Print help

Examples:
  rmod extend
  rmod extend -y";
        assert_eq!(strip_ansi(&extend()), expected);
    }

    #[test]
    fn project_help_matches_spec_mockup() {
        let expected = "rmod project
Second screen only: disable primary (laptop), keep external monitor(s) enabled

Usage:
  rmod project [OPTIONS]

Options:
  -y, --yes   Skip the confirmation prompt
  -h, --help  Print help

Examples:
  rmod project
  rmod project -y";
        assert_eq!(strip_ansi(&project()), expected);
    }

    #[test]
    fn single_help_matches_spec_mockup() {
        let expected = "rmod single
PC screen only: enable only one monitor, disable all others

Usage:
  rmod single -m <MONITOR> [OPTIONS]

Options:
  -m, --monitor  Monitor ID or number (default: primary)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod single -m 2
  rmod single -m a1b2c3d4 -y";
        assert_eq!(strip_ansi(&single()), expected);
    }

    #[test]
    fn top_help_omits_profiles_alias_and_yes() {
        let h = strip_ansi(&help());
        assert!(
            !h.contains("Profiles"),
            "profiles table must not appear at top level"
        );
        assert!(
            !h.contains("1280x720"),
            "profiles table must not appear at top level"
        );
        assert!(
            !h.contains("(alias: ls)"),
            "ls alias must not appear at top level"
        );
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
  --short     Compact one-line output
  --all       Show all monitors including detached
  -h, --help  Print help

Examples:
  rmod list";
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
  -r, --refresh      Refresh rate in Hz, or max
  -p, --profile      Resolution preset (see Profiles below)
  -m, --monitor      Monitor ID, 'primary', or 'all' (default: primary)
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
  rmod set -w 1920 -h 1080 -m a1b2c3d4 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y";
        assert_eq!(strip_ansi(&set()), expected);
    }

    #[test]
    fn set_help_renders_profiles_table() {
        let h = strip_ansi(&set());
        assert!(
            h.contains(&profiles_table()),
            "set page must embed the profiles table"
        );
    }

    #[test]
    fn layout_help_matches_spec_mockup() {
        let expected = "rmod layout
Show the monitor arrangement, place monitors, or set the primary display

Usage:
  rmod layout [OPTIONS]

Options:
  -m, --monitor  Monitor ID or 'primary' to move or promote
  --left-of      Place the monitor left of the reference
  --right-of     Place the monitor right of the reference
  --above        Place the monitor above the reference
  --below        Place the monitor below the reference
  --primary      Make the monitor the main display
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod layout
  rmod layout -m a1b2c3d4 --left-of b2c3d4e5
  rmod layout -m a1b2c3d4 --below b2c3d4e5
  rmod layout -m a1b2c3d4 --primary";
        assert_eq!(strip_ansi(&layout()), expected);
    }

    #[test]
    fn temp_help_matches_spec_mockup() {
        let expected = "rmod temp
Set or show the display color temperature

Usage:
  rmod temp [TEMPERATURE] [OPTIONS]

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (default: primary)
  -h, --help     Print help

Presets:
  candle   ember        1900K
  warm     incandescent 2700K
  neutral  halogen      3400K
  cool     fluorescent  4500K
  daylight sunlight     6500K

Examples:
  rmod temp
  rmod temp 3400
  rmod temp warm
  rmod temp reset
  rmod temp -m a1b2c3d4 4000";
        assert_eq!(strip_ansi(&temp()), expected);
    }

    #[test]
    fn temp_help_renders_presets_table() {
        assert!(
            strip_ansi(&temp()).contains(&temp_presets_table()),
            "temp page must embed the presets table"
        );
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
    fn top_help_renders_registry_rows() {
        let h = strip_ansi(&help());
        assert!(
            h.contains(&flat_commands()),
            "top page must render its command rows from the flat list"
        );
        assert!(
            h.contains(&options(TOP_FLAGS)),
            "top page must render its option rows from TOP_FLAGS"
        );
    }

    #[test]
    fn ls_help_renders_registry_rows() {
        assert!(
            strip_ansi(&ls()).contains(&options(LS_FLAGS)),
            "ls page must render its option rows from LS_FLAGS"
        );
    }

    #[test]
    fn set_help_renders_registry_rows() {
        assert!(
            strip_ansi(&set()).contains(&options(SET_FLAGS)),
            "set page must render its option rows from SET_FLAGS"
        );
    }

    #[test]
    fn set_help_renders_orientations_from_registry() {
        let expected = ORIENTATIONS
            .iter()
            .map(|(angle, name, alias)| format!("  {angle:<3}  {name:<17}   {alias}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            strip_ansi(&set()).contains(&expected),
            "set page must render the Orientations table from ORIENTATIONS"
        );
    }

    #[test]
    fn layout_help_renders_registry_rows() {
        assert!(
            strip_ansi(&layout()).contains(&options(LAYOUT_FLAGS)),
            "layout page must render its option rows from LAYOUT_FLAGS"
        );
    }

    #[test]
    fn help_pages_render_every_registry_flag() {
        for (page, registry, name) in [
            (help(), TOP_FLAGS, "help"),
            (ls(), LS_FLAGS, "ls"),
            (set(), SET_FLAGS, "set"),
            (layout(), LAYOUT_FLAGS, "layout"),
            (brightness(), BRIGHTNESS_FLAGS, "brightness"),
            (contrast(), CONTRAST_FLAGS, "contrast"),
            (attach(), ATTACH_FLAGS, "attach"),
            (detach(), ATTACH_FLAGS, "detach"),
            (sleep(), SLEEP_FLAGS, "sleep"),
            (wake(), WAKE_FLAGS, "wake"),
            (mirror(), MIRROR_FLAGS, "mirror"),
            (extend(), EXTEND_FLAGS, "extend"),
            (project(), PROJECT_FLAGS, "project"),
            (single(), SINGLE_FLAGS, "single"),
            (temp(), TEMP_FLAGS, "temp"),
        ] {
            let rendered = strip_ansi(&page);
            for f in registry {
                assert!(
                    rendered.contains(f.flag),
                    "{name}() must document {}",
                    f.flag
                );
            }
        }
    }

    #[test]
    fn registry_flags_parse_successfully() {
        // parse_from skips argv[0], so each example is prefixed with the program name.
        for (flag, example) in EXAMPLES {
            let mut argv = vec!["rmod"];
            argv.extend_from_slice(example);
            assert!(
                parse_from(&argv).is_ok(),
                "flag {flag} should parse: {example:?}"
            );
        }
    }

    #[test]
    fn registry_commands_parse_successfully() {
        assert!(
            parse_from(&["rmod", "list"]).is_ok(),
            "command 'list' should parse"
        );
        assert!(
            parse_from(&["rmod", "ls"]).is_ok(),
            "command 'ls' should parse"
        );
        // 'set' alone errors by design ('set' needs something to change);
        // assert it parses with its minimal spec.
        assert!(
            parse_from(&["rmod", "set", "--max"]).is_ok(),
            "command 'set' should parse"
        );
        assert!(
            parse_from(&["rmod", "layout"]).is_ok(),
            "command 'layout' should parse"
        );
        assert!(
            parse_from(&["rmod", "temp", "3400"]).is_ok(),
            "command 'temp' should parse"
        );
    }

    #[test]
    fn options_aligns_rows_to_widest_left() {
        let rows = &[
            Flag {
                flag: "--caps",
                doc: "List supported modes",
            },
            Flag {
                flag: "-m, --monitor <MONITOR>",
                doc: "Monitor number",
            },
        ];
        assert_eq!(
            options(rows),
            "  --caps                   List supported modes\n  -m, --monitor <MONITOR>  Monitor number"
        );
    }

    #[test]
    fn options_single_row_has_no_trailing_newline() {
        assert_eq!(
            options(&[Flag {
                flag: "--max",
                doc: "Use the highest supported mode",
            }]),
            "  --max  Use the highest supported mode"
        );
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
