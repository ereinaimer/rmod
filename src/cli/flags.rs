//! Command-line flag registries: documented flags, command tables,
//! orientations, temperature presets and resolution profiles.
//!
//! Pure data, consumed by `parser` and `help`; nothing here parses.

/// Named resolution presets (`720`, `1080`, `1440`, `4k`, `8k`).
pub(crate) const PROFILES: &[(&str, u32, u32)] = &[
    ("720", 1280, 720),
    ("1080", 1920, 1080),
    ("1440", 2560, 1440),
    ("4k", 3840, 2160),
    ("8k", 7680, 4320),
];

/// A documented flag: display text, description, and argv proving it parses.
pub(crate) struct Flag {
    pub(crate) flag: &'static str, // e.g. "-w, --width"
    pub(crate) doc: &'static str,  // e.g. "Resolution width (requires --height)"
    // Read only by the cfg(test) registry test; the bin build never reads it,
    // so without this allow `cargo clippy`/`cargo build` warn "never read".
    #[allow(dead_code)]
    pub(crate) example: &'static [&'static str], // argv proving the flag parses
}

pub(crate) const TOP_COMMANDS: &[(&str, &str)] = &[
    ("list", "List displays and their current settings"),
    ("set", "Apply resolution, refresh rate, and orientation"),
    ("layout", "Show the monitor arrangement or move monitors"),
    ("monitor", "Attach, detach, sleep, or wake monitors"),
    ("temp", "Set or show the display color temperature"),
    (
        "view",
        "Switch between mirror, extend, project, and single display modes",
    ),
    ("completions", "Output PowerShell tab-completion script"),
];

pub(crate) const TOP_FLAGS: &[Flag] = &[
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["--help"],
    },
    Flag {
        flag: "--version",
        doc: "Print version",
        example: &["--version"],
    },
];

pub(crate) const LS_FLAGS: &[Flag] = &[
    Flag {
        flag: "--short",
        doc: "Compact one-line output",
        example: &["list", "--short"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["list", "--help"],
    },
];

pub(crate) const SET_FLAGS: &[Flag] = &[
    Flag {
        flag: "-w, --width",
        doc: "Resolution width (requires --height)",
        example: &["set", "-w", "1920", "-h", "1080"],
    },
    Flag {
        flag: "-h, --height",
        doc: "Resolution height (requires --width)",
        example: &["set", "-w", "1920", "-h", "1080"],
    },
    Flag {
        flag: "-r, --refresh",
        doc: "Refresh rate in Hz, or max",
        example: &["set", "-r", "60"],
    },
    Flag {
        flag: "-p, --profile",
        doc: "Resolution preset (see Profiles below)",
        example: &["set", "-p", "1080"],
    },
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (default: primary)",
        example: &["set", "-m", "a1b2c3d4", "-r", "60"],
    },
    Flag {
        flag: "-o, --orientation",
        doc: "Rotation angle (see Orientations below)",
        example: &["set", "-o", "90"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["set", "-p", "1080", "-y"],
    },
    Flag {
        flag: "--max",
        doc: "Use the display's highest supported mode",
        example: &["set", "--max"],
    },
    Flag {
        flag: "--help",
        doc: "Print help",
        example: &["set", "--help"],
    },
];

pub(crate) const LAYOUT_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID or 'primary' to move or promote",
        example: &["layout", "-m", "a1b2c3d4", "--left-of", "b2c3d4e5"],
    },
    Flag {
        flag: "--left-of",
        doc: "Place the monitor left of the reference",
        example: &["layout", "-m", "a1b2c3d4", "--left-of", "b2c3d4e5"],
    },
    Flag {
        flag: "--right-of",
        doc: "Place the monitor right of the reference",
        example: &["layout", "-m", "a1b2c3d4", "--right-of", "b2c3d4e5"],
    },
    Flag {
        flag: "--above",
        doc: "Place the monitor above the reference",
        example: &["layout", "-m", "a1b2c3d4", "--above", "b2c3d4e5"],
    },
    Flag {
        flag: "--below",
        doc: "Place the monitor below the reference",
        example: &["layout", "-m", "a1b2c3d4", "--below", "b2c3d4e5"],
    },
    Flag {
        flag: "--primary",
        doc: "Make the monitor the main display",
        example: &["layout", "-m", "a1b2c3d4", "--primary"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["layout", "-m", "a1b2c3d4", "--primary", "-y"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["layout", "--help"],
    },
];

pub(crate) const VIEW_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID or number for single mode (default: primary)",
        example: &["view", "single", "-m", "2"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["view", "mirror", "-y"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["view", "--help"],
    },
];

pub(crate) const COMPLETIONS_FLAGS: &[Flag] = &[Flag {
    flag: "-h, --help",
    doc: "Print help",
    example: &["completions", "--help"],
}];

pub(crate) const MONITOR_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (required)",
        example: &["monitor", "detach", "-m", "a1b2c3d4"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["monitor", "detach", "-m", "a1b2c3d4", "-y"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["monitor", "--help"],
    },
];

pub(crate) const BRIGHTNESS_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (default: primary)",
        example: &["monitor", "brightness", "60", "-m", "2"],
    },
    Flag {
        flag: "-v, --via",
        doc: "Backend: ddc, slider, or gamma (default: auto; not valid with min, max, boost)",
        example: &["monitor", "brightness", "60", "-v", "ddc"],
    },
    Flag {
        flag: "min, max, boost",
        doc: "Composite modes: min (barely lit), max (hardware 100 + gamma 100), boost (hardware 100 + overdriven gamma)",
        example: &["monitor", "brightness", "min", "-m", "2"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["monitor", "brightness", "--help"],
    },
];

pub(crate) const CONTRAST_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (default: primary)",
        example: &["monitor", "contrast", "60", "-m", "2"],
    },
    Flag {
        flag: "-v, --via",
        doc: "Backend: ddc or gamma (default: auto)",
        example: &["monitor", "contrast", "60", "-v", "ddc"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["monitor", "contrast", "--help"],
    },
];

pub(crate) const TEMP_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (default: primary)",
        example: &["temp", "-m", "a1b2c3d4", "4000"],
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
        example: &["temp", "--help"],
    },
];

/// Named temperature presets (name, alias, Kelvin).
pub(crate) const TEMP_PRESETS: &[(&str, &str, u32)] = &[
    ("candle", "ember", 1900),
    ("warm", "incandescent", 2700),
    ("neutral", "halogen", 3400),
    ("cool", "fluorescent", 4500),
    ("daylight", "sunlight", 6500),
];

/// Lowest accepted Kelvin value for `temp`; mirrors the backend bounds.
pub(crate) const TEMP_MIN_KELVIN: u32 = 1000;
/// Highest accepted Kelvin value for `temp`; mirrors the backend bounds.
pub(crate) const TEMP_MAX_KELVIN: u32 = 6500;

pub(crate) const ORIENTATIONS: &[(u32, &str, &str)] = &[
    (0, "landscape", "l"),
    (90, "portrait", "p"),
    (180, "landscape-flipped", "lf"),
    (270, "portrait-flipped", "pf"),
];
