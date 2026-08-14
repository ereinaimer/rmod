//! Rendering of help and version output.
//!
//! Each function returns the complete text for one page; the dispatcher in
//! `main` prints it and exits.

/// Top-level help page: usage, profiles, and examples.
pub fn help() -> String {
    format!(
        "rmod {}
Resolution modifier

Usage:
  rmod list [--caps] (alias: ls)
  rmod main N [-y]
  rmod set [options]

  N = monitor number from 'rmod list'; omit = primary display.
  all = every monitor.

Profiles:
  720       1280x720
  1080      1920x1080
  1440      2560x1440
  4k        3840x2160
  8k        7680x4320

Examples:
  rmod list --caps
  rmod list --caps -m 2
  rmod main 2
  rmod set --max
  rmod set -p 1080
  rmod set -w 1920 -h 1080 -m 2 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y

Options:
  --help                  print help
  --version               print version
  -y, --yes               skip the confirmation prompt",
        env!("CARGO_PKG_VERSION")
    )
}

/// Help page for the `list` command (alias: `ls`).
pub fn ls() -> String {
    "rmod list
List all connected displays and their current settings (alias: ls)

Usage:
  rmod list [--caps] [-m N|all]

  Prints each display's monitor number, resolution, and refresh rate.
  Use the monitor number with the N suffix on other commands.
  With --caps, prints every resolution and refresh rate supported by
  the display, marking the active mode; -m targets a specific monitor
  (number or 'all'; requires --caps).

Examples:
  rmod list
  rmod list --caps
  rmod list --caps -m 2

Options:
  --caps                  list supported modes instead of current settings
  -m, --monitor M         monitor target (number or 'all'; requires --caps)
  -h, --help              print help"
        .to_string()
}

/// Help page for the `set` command.
pub fn set() -> String {
    "rmod set
Apply a resolution, refresh rate, and orientation to a display

Usage:
  rmod set [options]

Options:
  -w, --width W           resolution width (requires height)
  -h, --height H          resolution height (requires width)
  -r, --refresh R         refresh rate (fixed number or 'max')
  -p, --profile P         resolution profile preset (720, 1080, 1440, 4k, 8k)
  -m, --monitor M         monitor target (number or 'all', default: primary)
  -o, --orientation O     display rotation: 0 | 90 | 180 | 270 | l | lf | p | portrait | pf | landscape
  -y, --yes               skip the confirmation prompt
  --max                   shortcut to set highest resolution and refresh rate
  --help                  print help

Examples:
  rmod set --max
  rmod set -p 1080
  rmod set -w 1920 -h 1080 -m 2 -o 90
  rmod set -r 60 -m all
  rmod set -p 1440 -y"
        .to_string()
}

/// Help page for the `main` command.
pub fn main_help() -> String {
    "rmod main
make monitor N the main display

Usage:
  rmod main N [-y]

  N = monitor number from `rmod list` (required, no default).

Examples:
  rmod main 2
  rmod main 2 -y

Options:
  --help            print help
  -y, --yes         skip confirmation"
        .to_string()
}

/// Version string, e.g. `rmod 0.1.0`.
pub fn version() -> String {
    format!("rmod {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

#[test]
    fn top_help_lists_all_commands() {
        let h = help();
        assert!(h.contains("Resolution modifier"));
        assert!(h.contains("rmod list [--caps]"));
        assert!(h.contains("rmod main N [-y]"));
        assert!(h.contains("rmod set [options]"));
    }

    #[test]
    fn top_help_notes_ls_alias() {
        assert!(help().contains("(alias: ls)"));
    }

    #[test]
    fn top_help_documents_profiles() {
        let h = help();
        for (name, width, height) in super::super::parser::PROFILES {
            assert!(
                h.contains(&format!("{name:<10}{width}x{height}")),
                "profile '{name}'"
            );
        }
    }

#[test]
    fn top_help_has_examples() {
        let h = help();
        assert!(h.contains("rmod list --caps"));
        assert!(h.contains("rmod list --caps -m 2"));
        assert!(h.contains("rmod main 2"));
        assert!(h.contains("rmod set --max"));
        assert!(h.contains("rmod set -p 1080"));
        assert!(h.contains("rmod set -w 1920 -h 1080 -m 2 -o 90"));
    }

    #[test]
    fn top_help_has_options() {
        let h = help();
        assert!(h.contains("--help"));
        assert!(h.contains("--version"));
    }

#[test]
    fn ls_help() {
        let h = ls();
        assert!(h.contains("rmod list"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("(alias: ls)"));
        assert!(h.contains("--caps"));
        assert!(h.contains("-m, --monitor"));
        assert!(h.contains("--help"));
    }

    #[test]
    fn set_help() {
        let h = set();
        assert!(h.contains("rmod set"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("-w, --width"));
        assert!(h.contains("-h, --height"));
        assert!(h.contains("-r, --refresh"));
        assert!(h.contains("-p, --profile"));
        assert!(h.contains("-m, --monitor"));
        assert!(h.contains("-o, --orientation"));
        assert!(h.contains("-y, --yes"));
        assert!(h.contains("--max"));
        assert!(h.contains("--help"));
    }

    #[test]
    fn main_help_page() {
        let h = main_help();
        assert!(h.contains("rmod main"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("rmod main N [-y]"));
        assert!(h.contains("rmod main 2"));
        assert!(h.contains("--help"));
        assert!(h.contains("-y, --yes"));
    }

    #[test]
    fn top_help_lists_main_command() {
        let h = help();
        assert!(h.contains("rmod main N [-y]"));
        assert!(h.contains("rmod main 2"));
    }

    #[test]
    fn version_matches_package_version() {
        assert_eq!(version(), format!("rmod {}", env!("CARGO_PKG_VERSION")));
    }
}
