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

/// A documented flag: display text and description.
pub(crate) struct Flag {
    pub(crate) flag: &'static str, // e.g. "-w, --width"
    pub(crate) doc: &'static str,  // e.g. "Resolution width (requires --height)"
}

pub(crate) const TOP_FLAGS: &[Flag] = &[
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
    Flag {
        flag: "-V, --version",
        doc: "Print version",
    },
];

pub(crate) const LS_FLAGS: &[Flag] = &[
    Flag {
        flag: "--short",
        doc: "Compact one-line output",
    },
    Flag {
        flag: "--all",
        doc: "Show all monitors including detached",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const SET_FLAGS: &[Flag] = &[
    Flag {
        flag: "-w, --width",
        doc: "Resolution width (requires --height)",
    },
    Flag {
        flag: "-h, --height",
        doc: "Resolution height (requires --width)",
    },
    Flag {
        flag: "-r, --refresh",
        doc: "Refresh rate in Hz, or max",
    },
    Flag {
        flag: "-p, --profile",
        doc: "Resolution preset (see Profiles below)",
    },
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (default: primary)",
    },
    Flag {
        flag: "-o, --orientation",
        doc: "Rotation angle (see Orientations below)",
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "--max",
        doc: "Use the display's highest supported mode",
    },
    Flag {
        flag: "--help",
        doc: "Print help",
    },
];

pub(crate) const LAYOUT_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID or 'primary' to move or promote",
    },
    Flag {
        flag: "--left-of",
        doc: "Place the monitor left of the reference",
    },
    Flag {
        flag: "--right-of",
        doc: "Place the monitor right of the reference",
    },
    Flag {
        flag: "--above",
        doc: "Place the monitor above the reference",
    },
    Flag {
        flag: "--below",
        doc: "Place the monitor below the reference",
    },
    Flag {
        flag: "--primary",
        doc: "Make the monitor the main display",
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const ATTACH_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (required)",
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const SLEEP_FLAGS: &[Flag] = &[Flag {
    flag: "-h, --help",
    doc: "Print help",
}];

pub(crate) const WAKE_FLAGS: &[Flag] = &[Flag {
    flag: "-h, --help",
    doc: "Print help",
}];

pub(crate) const MIRROR_FLAGS: &[Flag] = &[
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const EXTEND_FLAGS: &[Flag] = &[
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const PROJECT_FLAGS: &[Flag] = &[
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const SINGLE_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID or number (default: primary)",
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const COMPLETIONS_FLAGS: &[Flag] = &[Flag {
    flag: "-h, --help",
    doc: "Print help",
}];

pub(crate) const BRIGHTNESS_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (default: primary)",
    },
    Flag {
        flag: "-v, --via",
        doc: "Backend: ddc, slider, or gamma (default: auto; not valid with min, max, boost)",
    },
    Flag {
        flag: "min, max, boost",
        doc: "Composite modes: min (barely lit), max (hardware 100 + gamma 100), boost (hardware 100 + overdriven gamma)",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const CONTRAST_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (default: primary)",
    },
    Flag {
        flag: "-v, --via",
        doc: "Backend: ddc or gamma (default: auto)",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
    },
];

pub(crate) const TEMP_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor ID, 'primary', or 'all' (default: primary)",
    },
    Flag {
        flag: "-h, --help",
        doc: "Print help",
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

/// Test-only argv proving each documented flag parses, keyed by the
/// flag's display text. Lives in its own table so release builds
/// exclude the data entirely.
#[cfg(test)]
pub(crate) const EXAMPLES: &[(&str, &[&str])] = &[
    ("-h, --help", &["--help"]),
    ("--version", &["--version"]),
    ("--short", &["list", "--short"]),
    ("--all", &["list", "--all"]),
    ("-h, --help", &["list", "--help"]),
    ("-w, --width", &["set", "-w", "1920", "-h", "1080"]),
    ("-h, --height", &["set", "-w", "1920", "-h", "1080"]),
    ("-r, --refresh", &["set", "-r", "60"]),
    ("-p, --profile", &["set", "-p", "1080"]),
    ("-m, --monitor", &["set", "-m", "a1b2c3d4", "-r", "60"]),
    ("-o, --orientation", &["set", "-o", "90"]),
    ("-y, --yes", &["set", "-p", "1080", "-y"]),
    ("--max", &["set", "--max"]),
    ("--help", &["set", "--help"]),
    (
        "-m, --monitor",
        &["layout", "-m", "a1b2c3d4", "--left-of", "b2c3d4e5"],
    ),
    (
        "--left-of",
        &["layout", "-m", "a1b2c3d4", "--left-of", "b2c3d4e5"],
    ),
    (
        "--right-of",
        &["layout", "-m", "a1b2c3d4", "--right-of", "b2c3d4e5"],
    ),
    (
        "--above",
        &["layout", "-m", "a1b2c3d4", "--above", "b2c3d4e5"],
    ),
    (
        "--below",
        &["layout", "-m", "a1b2c3d4", "--below", "b2c3d4e5"],
    ),
    ("--primary", &["layout", "-m", "a1b2c3d4", "--primary"]),
    (
        "-y, --yes",
        &["layout", "-m", "a1b2c3d4", "--primary", "-y"],
    ),
    ("-h, --help", &["layout", "--help"]),
    ("-h, --help", &["completions", "--help"]),
    ("-m, --monitor", &["temp", "-m", "a1b2c3d4", "4000"]),
    ("-h, --help", &["temp", "--help"]),
    ("-m, --monitor", &["brightness", "60", "-m", "2"]),
    ("-v, --via", &["brightness", "60", "-v", "ddc"]),
    ("min, max, boost", &["brightness", "min", "-m", "2"]),
    ("-h, --help", &["brightness", "--help"]),
    ("-m, --monitor", &["contrast", "60", "-m", "2"]),
    ("-v, --via", &["contrast", "60", "-v", "ddc"]),
    ("-h, --help", &["contrast", "--help"]),
    ("-m, --monitor", &["attach", "-m", "a1b2c3d4"]),
    ("-y, --yes", &["attach", "-m", "a1b2c3d4", "-y"]),
    ("-h, --help", &["attach", "--help"]),
    ("-m, --monitor", &["detach", "-m", "a1b2c3d4"]),
    ("-y, --yes", &["detach", "-m", "a1b2c3d4", "-y"]),
    ("-h, --help", &["detach", "--help"]),
    ("-h, --help", &["sleep", "--help"]),
    ("-h, --help", &["wake", "--help"]),
    ("-y, --yes", &["mirror", "-y"]),
    ("-h, --help", &["mirror", "--help"]),
    ("-y, --yes", &["extend", "-y"]),
    ("-h, --help", &["extend", "--help"]),
    ("-y, --yes", &["project", "-y"]),
    ("-h, --help", &["project", "--help"]),
    ("-m, --monitor", &["single", "-m", "2"]),
    ("-y, --yes", &["single", "-m", "a1b2c3d4", "-y"]),
    ("-h, --help", &["single", "--help"]),
    ("-V, --version", &["-V"]),
];
