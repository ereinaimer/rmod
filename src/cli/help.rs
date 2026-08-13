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
  rmod ls                 list displays
  rmod max[:N]            max resolution (primary display, or monitor N)
  rmod caps[:N]           list supported resolutions (primary, or monitor N)
  rmod WxH@R[:N]          set resolution and refresh rate (primary, or monitor N)

  :N = monitor number from 'rmod ls'; omit = primary display.

Profiles:
  720       1280x720
  1080      1920x1080
  1440      2560x1440
  4k        3840x2160
  8k        7680x4320

Examples:
  rmod max
  rmod max:2
  rmod caps
  rmod 4k
  rmod 1920x1080@144
  rmod 1920x1080@60:2

Options:
  -h, --help              print help
  -V, --version           print version",
        env!("CARGO_PKG_VERSION")
    )
}

/// Help page for the `ls` command.
pub fn ls() -> String {
    format!(
        "rmod ls
List all connected displays and their current settings

Usage:
  rmod ls

  Prints each display's monitor number, resolution, and refresh rate.
  Use the monitor number with the :N suffix on other commands.

Examples:
  rmod ls

Options:
  -h, --help              print help"
    )
}

/// Help page for the `max` command.
pub fn max() -> String {
    format!(
        "rmod max
Apply the highest supported resolution and refresh rate

Usage:
  rmod max[:N]

  Sets the highest resolution and refresh rate supported by the
  display. Without :N, applies to the primary display.

  :N = monitor number from `rmod ls`; omit = primary display.

Examples:
  rmod max
  rmod max:2

Options:
  -h, --help              print help"
    )
}

/// Help page for the `caps` command.
pub fn caps() -> String {
    format!(
        "rmod caps
List all supported resolutions and refresh rates

Usage:
  rmod caps[:N]

  Prints every resolution and refresh rate supported by the display.
  Without :N, applies to the primary display.

  :N = monitor number from `rmod ls`; omit = primary display.

Examples:
  rmod caps
  rmod caps:2

Options:
  -h, --help              print help"
    )
}

/// Help page for the `set` command (`WxH@R`).
pub fn set() -> String {
    format!(
        "rmod WxH@R
Apply a resolution and refresh rate to a display

Usage:
  rmod WxH@R[:N]          set resolution and refresh rate (primary, or monitor N)

  R options:
    @60                   fixed refresh rate
    @max                  highest refresh rate at that resolution
    omit @R               keep the current refresh rate

  :N = monitor number from `rmod ls`; omit = primary display.

Profiles:
  720       1280x720
  1080      1920x1080
  1440      2560x1440
  4k        3840x2160
  8k        7680x4320

Examples:
  rmod 4k
  rmod 1920x1080@144
  rmod 1920x1080@60:2
  rmod 1440@max

Options:
  -h, --help              print help"
    )
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
        assert!(h.contains("rmod ls"));
        assert!(h.contains("rmod max[:N]"));
        assert!(h.contains("rmod caps[:N]"));
        assert!(h.contains("rmod WxH@R[:N]"));
    }

    #[test]
    fn top_help_documents_profiles() {
        let h = help();
        for (name, width, height) in super::super::parser::PROFILES {
            assert!(h.contains(&format!("{name:<10}{width}x{height}")), "profile '{name}'");
        }
    }

    #[test]
    fn top_help_has_examples() {
        let h = help();
        assert!(h.contains("rmod max:2"));
        assert!(h.contains("rmod caps"));
        assert!(h.contains("rmod 4k"));
        assert!(h.contains("rmod 1920x1080@60:2"));
    }

    #[test]
    fn top_help_has_options() {
        let h = help();
        assert!(h.contains("-h, --help"));
        assert!(h.contains("-V, --version"));
    }

    #[test]
    fn ls_help() {
        let h = ls();
        assert!(h.contains("rmod ls"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("rmod ls"));
        assert!(h.contains("-h, --help"));
    }

    #[test]
    fn max_help() {
        let h = max();
        assert!(h.contains("rmod max"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("rmod max:2"));
        assert!(h.contains("-h, --help"));
    }

    #[test]
    fn caps_help() {
        let h = caps();
        assert!(h.contains("rmod caps"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("rmod caps:2"));
        assert!(h.contains("-h, --help"));
    }

    #[test]
    fn set_help() {
        let h = set();
        assert!(h.contains("rmod WxH@R"));
        assert!(!h.contains("rmod set"));
        assert!(h.contains("Usage:"));
        assert!(h.contains("@max"));
        assert!(h.contains("rmod 1920x1080@60:2"));
        assert!(h.contains("-h, --help"));
    }

    #[test]
    fn version_matches_package_version() {
        assert_eq!(version(), format!("rmod {}", env!("CARGO_PKG_VERSION")));
    }
}