pub fn help() -> String {
    format!(
        "rmod {}
Resolution modifier

Usage:
  rmod ls                 list displays
  rmod max[:N]            max resolution (primary display, or monitor N)
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
  rmod 4k
  rmod 1920x1080@144
  rmod 1920x1080@60:2

Options:
  -h, --help              print help
  -V, --version           print version",
        env!("CARGO_PKG_VERSION")
    )
}

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

pub fn version() -> String {
    format!("rmod {}", env!("CARGO_PKG_VERSION"))
}