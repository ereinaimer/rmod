//! Rendering of help and version output.
//!
//! Each function returns the complete text for one page; the dispatcher in
//! `main` prints it and exits. All content rows come from the registries in
//! `flags`; this module only renders them.

use crate::cli::flags::{
    BRIGHTNESS_FLAGS, COMPLETIONS_FLAGS, CONTRAST_FLAGS, Flag, LAYOUT_FLAGS, LS_FLAGS,
    MONITOR_FLAGS, ORIENTATIONS, SET_FLAGS, TEMP_FLAGS, TEMP_PRESETS, TOP_COMMANDS, TOP_FLAGS,
    VIEW_FLAGS,
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
        command_rows = commands(TOP_COMMANDS),
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

/// Help page for the `view` command.
pub fn view() -> String {
    format!(
        "rmod view
Switch between mirror, extend, project, and single display modes

{usage}
  rmod view <SUBCOMMAND> [OPTIONS]

{subcommands}
  mirror   Clone all displays: same position (0,0), same resolution (lowest common)
  extend   Restore extended desktop: side-by-side, auto-arranged left-to-right
  project  Second screen only: disable primary, keep external monitor(s) enabled
  single   PC screen only: enable only one monitor, disable all others

{options}
{option_rows}

{examples}
  rmod view mirror
  rmod view extend
  rmod view project
  rmod view single -m 2",
        usage = section("Usage:"),
        subcommands = section("Subcommands:"),
        options = section("Options:"),
        option_rows = options(VIEW_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod view mirror`.
pub fn view_mirror_help() -> String {
    format!(
        "rmod view mirror
Clone all displays: same position (0,0), same resolution (lowest common denominator)

{usage}
  rmod view mirror [OPTIONS]

{options}
{option_rows}

{examples}
  rmod view mirror
  rmod view mirror -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(VIEW_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod view extend`.
pub fn view_extend_help() -> String {
    format!(
        "rmod view extend
Restore extended desktop: side-by-side positions (auto-arrange left-to-right by monitor number)

{usage}
  rmod view extend [OPTIONS]

{options}
{option_rows}

{examples}
  rmod view extend
  rmod view extend -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(VIEW_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod view project`.
pub fn view_project_help() -> String {
    format!(
        "rmod view project
Second screen only: disable primary (laptop), keep external monitor(s) enabled

{usage}
  rmod view project [OPTIONS]

{options}
{option_rows}

{examples}
  rmod view project
  rmod view project -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(VIEW_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod view single`.
pub fn view_single_help() -> String {
    format!(
        "rmod view single
PC screen only: enable only one monitor, disable all others

{usage}
  rmod view single -m <MONITOR> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod view single -m 2
  rmod view single -m a1b2c3d4 -y",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(VIEW_FLAGS),
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

/// Help page for the `monitor` command.
pub fn monitor() -> String {
    format!(
        "rmod monitor
Attach, detach, sleep, or wake monitors

{usage}
  rmod monitor <ACTION> [OPTIONS]

{actions}
  detach     Detach a monitor from the desktop
  attach     Re-attach a monitor to the desktop
  brightness Set the display backlight level (0-100, or min, max, boost)
  contrast   Set the display contrast (0-130, 100 = neutral)
  sleep      Put every monitor to sleep
  wake       Wake every monitor

{options}
{option_rows}

{examples}
  rmod monitor detach -m 2
  rmod monitor disable -m a1b2c3d4 -y
  rmod monitor attach -m 2
  rmod monitor brightness 60
  rmod monitor contrast 60
  rmod monitor sleep
  rmod monitor wake",
        usage = section("Usage:"),
        actions = section("Actions:"),
        options = section("Options:"),
        option_rows = options(MONITOR_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod monitor detach` (aliases: `disable`, `off`).
pub fn monitor_detach() -> String {
    format!(
        "rmod monitor detach
Detach a monitor from the desktop

{usage}
  rmod monitor detach -m <MONITOR> [OPTIONS]

{aliases}
  disable, off

{options}
{option_rows}

{examples}
  rmod monitor detach -m a1b2c3d4
  rmod monitor detach -m a1b2c3d4 -y",
        usage = section("Usage:"),
        aliases = section("Aliases:"),
        options = section("Options:"),
        option_rows = options(MONITOR_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod monitor attach` (aliases: `enable`, `on`).
pub fn monitor_attach() -> String {
    format!(
        "rmod monitor attach
Re-attach a monitor to the desktop

{usage}
  rmod monitor attach -m <MONITOR> [OPTIONS]

{aliases}
  enable, on

{options}
{option_rows}

{examples}
  rmod monitor attach -m a1b2c3d4
  rmod monitor attach -m a1b2c3d4 -y",
        usage = section("Usage:"),
        aliases = section("Aliases:"),
        options = section("Options:"),
        option_rows = options(MONITOR_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod monitor brightness`.
pub fn monitor_brightness() -> String {
    format!(
        "rmod monitor brightness
Set the display backlight level (0-100, or min, max, boost)

{usage}
  rmod monitor brightness <VALUE> [OPTIONS]

{options}
{option_rows}

{examples}
  rmod monitor brightness 60
  rmod monitor brightness 40 -m 2 -v gamma
  rmod monitor brightness 75 -m all
  rmod monitor brightness min -m 2
  rmod monitor brightness boost",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(BRIGHTNESS_FLAGS),
        examples = section("Examples:"),
    )
}

/// Help page for `rmod monitor contrast`.
pub fn monitor_contrast() -> String {
    format!(
        "rmod monitor contrast
Set the display contrast (0-130, 100 = neutral)

{usage}
  rmod monitor contrast <VALUE> [OPTIONS]
  rmod monitor contrast reset [OPTIONS]

{options}
{option_rows}

{examples}
  rmod monitor contrast 60
  rmod monitor contrast 130 -v gamma
  rmod monitor contrast 75 -m all
  rmod monitor contrast reset",
        usage = section("Usage:"),
        options = section("Options:"),
        option_rows = options(CONTRAST_FLAGS),
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

/// Render aligned command rows from `(command, doc)` pairs, joined by `\n`.
pub(crate) fn commands(rows: &[(&'static str, &'static str)]) -> String {
    let flags: Vec<Flag> = rows
        .iter()
        .map(|(name, doc)| Flag {
            flag: name,
            doc,
            example: &[],
        })
        .collect();
    options(&flags)
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
  list         List displays and their current settings
  set          Apply resolution, refresh rate, and orientation
  layout       Show the monitor arrangement or move monitors
  monitor      Attach, detach, sleep, or wake monitors
  temp         Set or show the display color temperature
  view         Switch between mirror, extend, project, and single display modes
  completions  Output PowerShell tab-completion script

Options:
  -h, --help  Print help
  --version   Print version

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
    fn monitor_help_matches_spec_mockup() {
        let expected = "rmod monitor
Attach, detach, sleep, or wake monitors

Usage:
  rmod monitor <ACTION> [OPTIONS]

Actions:
  detach     Detach a monitor from the desktop
  attach     Re-attach a monitor to the desktop
  brightness Set the display backlight level (0-100, or min, max, boost)
  contrast   Set the display contrast (0-130, 100 = neutral)
  sleep      Put every monitor to sleep
  wake       Wake every monitor

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (required)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod monitor detach -m 2
  rmod monitor disable -m a1b2c3d4 -y
  rmod monitor attach -m 2
  rmod monitor brightness 60
  rmod monitor contrast 60
  rmod monitor sleep
  rmod monitor wake";
        assert_eq!(strip_ansi(&monitor()), expected);
    }

    #[test]
    fn monitor_help_hides_aliases() {
        let h = strip_ansi(&monitor());
        assert!(
            !h.contains("aliases"),
            "aliases must only appear on action pages, got: {h}"
        );
        assert!(!h.contains("disable, off"));
        assert!(!h.contains("enable, on"));
    }

    #[test]
    fn monitor_detach_help_shows_aliases() {
        let expected = "rmod monitor detach
Detach a monitor from the desktop

Usage:
  rmod monitor detach -m <MONITOR> [OPTIONS]

Aliases:
  disable, off

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (required)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod monitor detach -m a1b2c3d4
  rmod monitor detach -m a1b2c3d4 -y";
        assert_eq!(strip_ansi(&monitor_detach()), expected);
    }

    #[test]
    fn monitor_attach_help_shows_aliases() {
        let expected = "rmod monitor attach
Re-attach a monitor to the desktop

Usage:
  rmod monitor attach -m <MONITOR> [OPTIONS]

Aliases:
  enable, on

Options:
  -m, --monitor  Monitor ID, 'primary', or 'all' (required)
  -y, --yes      Skip the confirmation prompt
  -h, --help     Print help

Examples:
  rmod monitor attach -m a1b2c3d4
  rmod monitor attach -m a1b2c3d4 -y";
        assert_eq!(strip_ansi(&monitor_attach()), expected);
    }

    #[test]
    fn monitor_brightness_help_matches_spec_mockup() {
        let expected = "rmod monitor brightness
Set the display backlight level (0-100, or min, max, boost)

Usage:
  rmod monitor brightness <VALUE> [OPTIONS]

Options:
  -m, --monitor    Monitor number or all (default: primary)
  -v, --via        Backend: ddc, slider, or gamma (default: auto; not valid with min, max, boost)
  min, max, boost  Composite modes: min (barely lit), max (hardware 100 + gamma 100), boost (hardware 100 + overdriven gamma)
  -h, --help       Print help

Examples:
  rmod monitor brightness 60
  rmod monitor brightness 40 -m 2 -v gamma
  rmod monitor brightness 75 -m all
  rmod monitor brightness min -m 2
  rmod monitor brightness boost";
        assert_eq!(strip_ansi(&monitor_brightness()), expected);
    }

    #[test]
    fn monitor_contrast_help_matches_spec_mockup() {
        let expected = "rmod monitor contrast
Set the display contrast (0-130, 100 = neutral)

Usage:
  rmod monitor contrast <VALUE> [OPTIONS]
  rmod monitor contrast reset [OPTIONS]

Options:
  -m, --monitor  Monitor number or all (default: primary)
  -v, --via      Backend: ddc or gamma (default: auto)
  -h, --help     Print help

Examples:
  rmod monitor contrast 60
  rmod monitor contrast 130 -v gamma
  rmod monitor contrast 75 -m all
  rmod monitor contrast reset";
        assert_eq!(strip_ansi(&monitor_contrast()), expected);
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
            h.contains(&commands(TOP_COMMANDS)),
            "top page must render its command rows from TOP_COMMANDS"
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
    fn monitor_help_renders_registry_rows() {
        assert!(
            strip_ansi(&monitor()).contains(&options(MONITOR_FLAGS)),
            "monitor page must render its option rows from MONITOR_FLAGS"
        );
    }

    #[test]
    fn help_pages_render_every_registry_flag() {
        for (page, registry, name) in [
            (help(), TOP_FLAGS, "help"),
            (ls(), LS_FLAGS, "ls"),
            (set(), SET_FLAGS, "set"),
            (layout(), LAYOUT_FLAGS, "layout"),
            (monitor(), MONITOR_FLAGS, "monitor"),
            (monitor_brightness(), BRIGHTNESS_FLAGS, "monitor_brightness"),
            (monitor_contrast(), CONTRAST_FLAGS, "monitor_contrast"),
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
        let registries = [
            TOP_FLAGS,
            LS_FLAGS,
            SET_FLAGS,
            LAYOUT_FLAGS,
            MONITOR_FLAGS,
            BRIGHTNESS_FLAGS,
            CONTRAST_FLAGS,
            TEMP_FLAGS,
        ];
        for flags in registries {
            for f in flags {
                let mut argv = vec!["rmod"];
                argv.extend_from_slice(f.example);
                assert!(
                    parse_from(&argv).is_ok(),
                    "flag {} should parse: {:?}",
                    f.flag,
                    f.example
                );
            }
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
            parse_from(&["rmod", "monitor", "disable", "-m", "2"]).is_ok(),
            "command 'monitor' should parse"
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
                example: &[],
            },
            Flag {
                flag: "-m, --monitor <MONITOR>",
                doc: "Monitor number",
                example: &[],
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
                example: &[]
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
