//! Command-line grammar: unified verb-centric syntax.
//!
//! Every command: `rmod <verb> [arguments]`
//! Monitor targeting is always a positional argument after the verb.

use std::env;

pub use crate::sys::windows::Direction;
pub use crate::sys::windows::apply::Refresh;

/// Help topics reachable via the command-specific `--help` flags.
#[derive(Debug, PartialEq)]
pub enum HelpTopic {
    List,
    Set,
    Layout,
    Monitor {
        /// The action whose page to show; `None` is the top-level page.
        action: Option<MonitorAction>,
    },
}

/// What the `layout` command should do.
#[derive(Debug, PartialEq)]
pub enum LayoutAction {
    Show,
    Place {
        monitor: u32,
        direction: Direction,
        reference: u32,
    },
    Primary {
        monitor: u32,
    },
}

/// What the `monitor` command should do.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MonitorAction {
    /// Detach a monitor from the desktop.
    Disable,
    /// Re-attach a monitor to the desktop.
    Enable,
    /// Put every monitor to sleep.
    Sleep,
    /// Wake every monitor.
    Wake,
}

/// Every top-level command rmod accepts.
#[derive(Debug, PartialEq)]
pub enum Command {
    List {
        caps: bool,
        monitor: MonitorTarget,
    },
    Layout {
        action: LayoutAction,
        yes: bool,
    },
    Set {
        spec: SetSpec,
        monitor: MonitorTarget,
        orientation: Option<u32>,
        yes: bool,
    },
    Monitor {
        action: MonitorAction,
        monitor: MonitorTarget,
        yes: bool,
    },
    Help {
        topic: Option<HelpTopic>,
    },
    Version,
}

/// Which display(s) a command targets.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum MonitorTarget {
    Primary,
    Index(u32),
    All,
}

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
];

pub(crate) const TOP_FLAGS: &[Flag] = &[
    Flag {
        flag: "--help",
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
        flag: "--caps",
        doc: "List supported modes instead of current settings",
        example: &["list", "--caps"],
    },
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (requires --caps)",
        example: &["list", "-m", "2", "--caps"],
    },
    Flag {
        flag: "--help",
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
        doc: "Monitor number or all (default: primary)",
        example: &["set", "-m", "2", "-r", "60"],
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
        doc: "Monitor to move or promote",
        example: &["layout", "-m", "2", "--left-of", "1"],
    },
    Flag {
        flag: "--left-of",
        doc: "Place the monitor left of the reference",
        example: &["layout", "-m", "2", "--left-of", "1"],
    },
    Flag {
        flag: "--right-of",
        doc: "Place the monitor right of the reference",
        example: &["layout", "-m", "2", "--right-of", "1"],
    },
    Flag {
        flag: "--above",
        doc: "Place the monitor above the reference",
        example: &["layout", "-m", "2", "--above", "1"],
    },
    Flag {
        flag: "--below",
        doc: "Place the monitor below the reference",
        example: &["layout", "-m", "2", "--below", "1"],
    },
    Flag {
        flag: "--primary",
        doc: "Make the monitor the main display",
        example: &["layout", "-m", "2", "--primary"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["layout", "-m", "2", "--primary", "-y"],
    },
    Flag {
        flag: "--help",
        doc: "Print help",
        example: &["layout", "--help"],
    },
];

pub(crate) const MONITOR_FLAGS: &[Flag] = &[
    Flag {
        flag: "-m, --monitor",
        doc: "Monitor number or all (required)",
        example: &["monitor", "detach", "-m", "2"],
    },
    Flag {
        flag: "-y, --yes",
        doc: "Skip the confirmation prompt",
        example: &["monitor", "detach", "-m", "2", "-y"],
    },
    Flag {
        flag: "--help",
        doc: "Print help",
        example: &["monitor", "--help"],
    },
];

pub(crate) const ORIENTATIONS: &[(u32, &str, &str)] = &[
    (0, "landscape", "l"),
    (90, "portrait", "p"),
    (180, "landscape-flipped", "lf"),
    (270, "portrait-flipped", "pf"),
];

/// Set specification formats.
#[derive(Debug, PartialEq, Clone)]
pub enum SetSpec {
    Profile(String),
    ProfileWithRefresh(String, Refresh),
    Explicit {
        width: u32,
        height: u32,
        refresh: Refresh,
    },
    RefreshOnly(Refresh),
    Max,
    Keep,
}

/// Parses the process arguments into a [`Command`].
///
/// # Errors
/// Returns `Err` with a human-readable message for unknown commands,
/// invalid numbers, or unexpected trailing arguments.
pub fn parse() -> Result<Command, String> {
    let args: Vec<String> = env::args().collect();
    parse_from(&args)
}

/// Parses a command from an argument iterator; the first item is argv[0]
/// and is skipped. Split out from [`parse`] for testability.
///
/// # Errors
/// Returns `Err` with a human-readable message for unknown commands,
/// invalid numbers, or unexpected trailing arguments.
pub fn parse_from<S: AsRef<str>>(args: &[S]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Help { topic: None });
    }
    let args = &args[1..];
    let Some(cmd) = args.first() else {
        return Ok(Command::Help { topic: None });
    };
    let cmd_str = cmd.as_ref();

    match cmd_str {
        "--help" => {
            if args.len() > 1 {
                return Err(format!(
                    "unexpected argument {}. --help takes no arguments\ne.g. rmod --help",
                    args[1].as_ref()
                ));
            }
            Ok(Command::Help { topic: None })
        }
        "--version" => {
            if args.len() > 1 {
                return Err(format!(
                    "unexpected argument {}. --version takes no arguments\ne.g. rmod --version",
                    args[1].as_ref()
                ));
            }
            Ok(Command::Version)
        }
        "ls" | "list" => parse_ls(cmd_str, args),
        "layout" => parse_layout(args),
        "main" => Err("unknown command main. use rmod layout -m N --primary".to_string()),
        "set" => parse_set(args),
        "monitor" => parse_monitor(args),
        _ => Err(format!(
            "unknown command {}. run rmod --help to list commands",
            cmd_str
        )),
    }
}

fn parse_ls(_cmd: &str, args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut caps = false;
    let mut monitor = MonitorTarget::Primary;
    let mut monitor_explicit = false;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--caps" => {
                caps = true;
                i += 1;
            }
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                monitor_explicit = true;
                i += 1;
            }
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::List),
                });
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for list. use --caps or --monitor",
                    other
                ));
            }
        }
    }

    if monitor_explicit && !caps {
        return Err(
            "-m, --monitor only works with --caps\ne.g. rmod list --caps -m 2".to_string(),
        );
    }

    Ok(Command::List { caps, monitor })
}

fn parse_layout(args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut monitor: Option<u32> = None;
    let mut monitor_explicit = false;
    let mut placement: Option<(Direction, u32)> = None;
    let mut primary = false;
    let mut yes = false;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Layout),
                });
            }
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number\ne.g. -m 2".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number\ne.g. -m 2".to_string(),
                    );
                }
                monitor = Some(parse_monitor_number(val)?);
                monitor_explicit = true;
                i += 1;
            }
            "--left-of" | "--right-of" | "--above" | "--below" => {
                if placement.is_some() {
                    return Err(
                        "use only one direction flag\ne.g. rmod layout -m 2 --left-of 1"
                            .to_string(),
                    );
                }
                let direction = match arg {
                    "--left-of" => Direction::Left,
                    "--right-of" => Direction::Right,
                    "--above" => Direction::Above,
                    _ => Direction::Below,
                };
                i += 1;
                let Some(next) = args.get(i) else {
                    return Err(format!(
                        "{arg} needs a value. a monitor number\ne.g. {arg} 1"
                    ));
                };
                let next = next.as_ref();
                if next.starts_with('-') {
                    return Err(format!(
                        "{arg} needs a value. a monitor number\ne.g. {arg} 1"
                    ));
                }
                placement = Some((direction, parse_monitor_number(next)?));
                i += 1;
            }
            "--primary" => {
                primary = true;
                i += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for layout. use --left-of, --right-of, --above, --below, or --primary",
                    other
                ));
            }
        }
    }

    if primary {
        if placement.is_some() {
            return Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m 2 --primary"
                    .to_string(),
            );
        }
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for layout\ne.g. rmod layout -m 2 --primary".to_string(),
            );
        };
        return Ok(Command::Layout {
            action: LayoutAction::Primary { monitor },
            yes,
        });
    }

    if monitor_explicit && placement.is_none() {
        return Err("-m, --monitor needs a direction flag or --primary\ne.g. rmod layout -m 2 --left-of 1".to_string());
    }

    if let Some((direction, reference)) = placement {
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for layout\ne.g. rmod layout -m 2 --left-of 1".to_string(),
            );
        };
        return Ok(Command::Layout {
            action: LayoutAction::Place {
                monitor,
                direction,
                reference,
            },
            yes,
        });
    }

    Ok(Command::Layout {
        action: LayoutAction::Show,
        yes,
    })
}

fn parse_monitor_number(arg: &str) -> Result<u32, String> {
    let n = arg.parse::<u32>().map_err(|_| {
        format!(
            "invalid monitor number {}. use a number from rmod list",
            arg
        )
    })?;
    if n == 0 {
        return Err("monitor numbers start at 1. run rmod list to see them".to_string());
    }
    Ok(n)
}

fn parse_set(args: &[impl AsRef<str>]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("set needs something to change. width/height, refresh rate, profile, or --max\ne.g. rmod set -p 1080".to_string());
    }

    let mut width = None;
    let mut height = None;
    let mut refresh = None;
    let mut profile = None;
    let mut monitor = MonitorTarget::Primary;
    let mut orientation = None;
    let mut yes = false;
    let mut max_flag = false;

    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Set),
                });
            }
            "-w" | "--width" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-w, --width needs a value. a number of pixels\ne.g. -w 1920"
                            .to_string(),
                    );
                };
                width = Some(val.as_ref().parse::<u32>().map_err(|_| {
                    format!(
                        "invalid width {}. use a number of pixels\ne.g. 1920",
                        val.as_ref()
                    )
                })?);
                i += 1;
            }
            "-h" | "--height" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-h, --height needs a value. a number of pixels\ne.g. -h 1080"
                            .to_string(),
                    );
                };
                height = Some(val.as_ref().parse::<u32>().map_err(|_| {
                    format!(
                        "invalid height {}. use a number of pixels\ne.g. 1080",
                        val.as_ref()
                    )
                })?);
                i += 1;
            }
            "-r" | "--refresh" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-r, --refresh needs a value. a number in Hz or max\ne.g. -r 144"
                            .to_string(),
                    );
                };
                refresh = Some(parse_refresh(val.as_ref())?);
                i += 1;
            }
            "-p" | "--profile" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-p, --profile needs a value. 720, 1080, 1440, 4k, or 8k\ne.g. -p 1080"
                            .to_string(),
                    );
                };
                if !PROFILES.iter().any(|(name, _, _)| *name == val.as_ref()) {
                    let names = PROFILES
                        .iter()
                        .map(|(name, _, _)| *name)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!(
                        "unknown profile {}. use one of: {}",
                        val.as_ref(),
                        names
                    ));
                }
                profile = Some(val.as_ref().to_string());
                i += 1;
            }
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            "-o" | "--orientation" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-o, --orientation needs a value. 0, 90, 180, or 270\ne.g. -o 90"
                            .to_string(),
                    );
                };
                orientation = Some(parse_orientation(val.as_ref())?);
                i += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "--max" => {
                max_flag = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for set. use --width, --height, --refresh, --profile, --monitor, --orientation, or --max",
                    other
                ));
            }
        }
    }

    if (width.is_some() && height.is_none()) || (width.is_none() && height.is_some()) {
        return Err(
            "-w, --width and -h, --height must be used together\ne.g. -w 1920 -h 1080"
                .to_string(),
        );
    }

    if profile.is_some() && (width.is_some() || height.is_some()) {
        return Err("use --profile or explicit width/height, not both".to_string());
    }

    if max_flag && (width.is_some() || height.is_some() || refresh.is_some() || profile.is_some()) {
        return Err("use --max alone or one of: width/height, refresh, profile".to_string());
    }

    let spec = if max_flag {
        SetSpec::Max
    } else if let Some(p) = profile {
        if let Some(r) = refresh {
            SetSpec::ProfileWithRefresh(p, r)
        } else {
            SetSpec::Profile(p)
        }
    } else if let Some(w) = width {
        let h = height.unwrap();
        let r = refresh.unwrap_or(Refresh::Keep);
        SetSpec::Explicit {
            width: w,
            height: h,
            refresh: r,
        }
    } else if let Some(r) = refresh {
        SetSpec::RefreshOnly(r)
    } else {
        SetSpec::Keep
    };

    Ok(Command::Set {
        spec,
        monitor,
        orientation,
        yes,
    })
}

fn parse_monitor(args: &[impl AsRef<str>]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("monitor needs an action. attach, detach, sleep, or wake\ne.g. rmod monitor detach -m 2".to_string());
    }
    let action_str = args[1].as_ref();
    if action_str == "--help" {
        return Ok(Command::Help {
            topic: Some(HelpTopic::Monitor { action: None }),
        });
    }
    let action = match action_str {
        "detach" | "disable" | "off" => MonitorAction::Disable,
        "attach" | "enable" | "on" => MonitorAction::Enable,
        "sleep" => MonitorAction::Sleep,
        "wake" => MonitorAction::Wake,
        other => {
            return Err(format!(
                "unknown action {} for monitor. use attach, detach, sleep, or wake",
                other
            ));
        }
    };
    let mut monitor = MonitorTarget::Primary;
    let mut monitor_explicit = false;
    let mut yes = false;
    let mut i = 2;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Monitor { action: Some(action) }),
                });
            }
            "-m" | "--monitor" => {
                if !matches!(action, MonitorAction::Disable | MonitorAction::Enable) {
                    return Err(format!(
                        "-m, --monitor is not valid for monitor {action_str}. {action_str} applies to all monitors"
                    ));
                }
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                monitor_explicit = true;
                i += 1;
            }
            "-y" | "--yes" => {
                if !matches!(action, MonitorAction::Disable | MonitorAction::Enable) {
                    return Err(format!(
                        "-y, --yes is not valid for monitor {action_str}. {action_str} applies to all monitors"
                    ));
                }
                yes = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for monitor {action_str}. use --monitor or --yes",
                    other
                ));
            }
        }
    }

    if matches!(action, MonitorAction::Disable | MonitorAction::Enable) && !monitor_explicit {
        let verb = if action == MonitorAction::Disable {
            "detach"
        } else {
            "attach"
        };
        return Err(format!(
            "monitor {verb} needs -m, --monitor. a monitor number or all\ne.g. rmod monitor {verb} -m 2"
        ));
    }

    Ok(Command::Monitor {
        action,
        monitor,
        yes,
    })
}

fn parse_monitor_target(arg: &str) -> Result<MonitorTarget, String> {
    if arg == "all" {
        Ok(MonitorTarget::All)
    } else {
        let n = arg.parse::<u32>().map_err(|_| {
            format!(
                "invalid monitor target {}. use a monitor number or all",
                arg
            )
        })?;
        if n == 0 {
            return Err("monitor numbers start at 1. run rmod list to see them".to_string());
        }
        Ok(MonitorTarget::Index(n))
    }
}

fn parse_refresh(arg: &str) -> Result<Refresh, String> {
    match arg.to_lowercase().as_str() {
        "max" => Ok(Refresh::Max),
        _ => arg.parse::<u32>().map(Refresh::Fixed).map_err(|_| {
            format!(
                "invalid refresh rate {}. use a number in Hz or max",
                arg
            )
        }),
    }
}

fn parse_orientation(arg: &str) -> Result<u32, String> {
    match arg.to_lowercase().as_str() {
        "0" | "l" | "landscape" => Ok(0),
        "90" | "p" | "portrait" => Ok(90),
        "180" | "lf" | "landscape-flipped" => Ok(180),
        "270" | "pf" | "portrait-flipped" => Ok(270),
        _ => {
            let angles = ORIENTATIONS
                .iter()
                .map(|(angle, _, _)| angle.to_string())
                .collect::<Vec<_>>();
            let aliases = ORIENTATIONS
                .iter()
                .map(|(_, _, alias)| *alias)
                .collect::<Vec<_>>()
                .join(", ");
            let names = ORIENTATIONS
                .iter()
                .map(|(_, name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "invalid orientation {}. use {}, or {} (also: {aliases}, {names})",
                arg,
                angles[..angles.len() - 1].join(", "),
                angles[angles.len() - 1],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        parse_from(&full_args)
    }

    #[test]
    fn no_args_prints_help() {
        assert_eq!(parse(&[]), Ok(Command::Help { topic: None }));
    }

    #[test]
    fn help_flags() {
        assert!(parse(&["-h"]).is_err());
        assert_eq!(parse(&["--help"]), Ok(Command::Help { topic: None }));
    }

    #[test]
    fn version_flags() {
        assert!(parse(&["-V"]).is_err());
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
    }

    #[test]
    fn ls_command() {
        assert_eq!(
            parse(&["ls"]),
            Ok(Command::List {
                caps: false,
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn list_command() {
        assert_eq!(
            parse(&["list"]),
            Ok(Command::List {
                caps: false,
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn ls_help_flags() {
        assert!(parse(&["ls", "-h"]).is_err());
        assert_eq!(
            parse(&["ls", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::List)
            })
        );
    }

    #[test]
    fn ls_unknown_argument_is_error() {
        assert!(parse(&["ls", "foo"]).is_err());
    }

    #[test]
    fn list_unknown_argument_is_error() {
        assert_eq!(
            parse(&["list", "foo"]),
            Err("unexpected argument foo for list. use --caps or --monitor".to_string())
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
    fn set_max_command() {
        assert_eq!(
            parse(&["set", "--max"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_with_monitor() {
        assert_eq!(
            parse(&["set", "--max", "-m", "2"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_with_all() {
        assert_eq!(
            parse(&["set", "--max", "-m", "all"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::All,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_yes_flag() {
        assert_eq!(
            parse(&["set", "--max", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_yes_flag_with_monitor() {
        assert_eq!(
            parse(&["set", "--max", "-m", "2", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: true
            })
        );
        assert_eq!(
            parse(&["set", "-y", "--max", "-m", "2"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_all_with_yes() {
        assert_eq!(
            parse(&["set", "--max", "-m", "all", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::All,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_invalid_monitor_is_error() {
        assert!(parse(&["set", "--max", "-m", "x"]).is_err());
        assert!(parse(&["set", "--max", "-m", "0"]).is_err());
    }

    #[test]
    fn set_max_conflicting_spec_is_error() {
        assert!(parse(&["set", "-p", "1080", "--max"]).is_err());
        assert!(parse(&["set", "--max", "-p", "1080"]).is_err());
        assert!(parse(&["set", "-w", "1920", "-h", "1080", "--max"]).is_err());
    }

    #[test]
    fn layout_no_args_is_show() {
        assert_eq!(
            parse(&["layout"]),
            Ok(Command::Layout {
                action: LayoutAction::Show,
                yes: false
            })
        );
    }

    #[test]
    fn layout_place_left_of_with_reference() {
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "1"]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: 2,
                    direction: Direction::Left,
                    reference: 1,
                },
                yes: false,
            })
        );
    }

    #[test]
    fn layout_place_with_explicit_reference() {
        assert_eq!(
            parse(&["layout", "-m", "3", "--above", "1"]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: 3,
                    direction: Direction::Above,
                    reference: 1,
                },
                yes: false,
            })
        );
    }

    #[test]
    fn layout_direction_flags_cover_all_four() {
        for (flag, direction) in [
            ("--left-of", Direction::Left),
            ("--right-of", Direction::Right),
            ("--above", Direction::Above),
            ("--below", Direction::Below),
        ] {
            assert_eq!(
                parse(&["layout", "-m", "2", flag, "1"]),
                Ok(Command::Layout {
                    action: LayoutAction::Place {
                        monitor: 2,
                        direction,
                        reference: 1,
                    },
                    yes: false,
                }),
                "flag '{}'",
                flag
            );
        }
    }

    #[test]
    fn layout_missing_value_for_direction_is_error() {
        for flag in ["--left-of", "--right-of", "--above", "--below"] {
            assert_eq!(
                parse(&["layout", "-m", "2", flag]),
                Err(format!(
                    "{flag} needs a value. a monitor number\ne.g. {flag} 1"
                )),
                "flag '{}'",
                flag
            );
            assert_eq!(
                parse(&["layout", "-m", "2", flag, "--primary"]),
                Err(format!(
                    "{flag} needs a value. a monitor number\ne.g. {flag} 1"
                )),
                "flag '{}'",
                flag
            );
        }
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "0"]),
            Err("monitor numbers start at 1. run rmod list to see them".to_string())
        );
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "x"]),
            Err("invalid monitor number x. use a number from rmod list".to_string())
        );
    }

    #[test]
    fn layout_second_direction_flag_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "1", "--right-of", "1"]),
            Err("use only one direction flag\ne.g. rmod layout -m 2 --left-of 1".to_string())
        );
    }

    #[test]
    fn layout_primary_with_direction_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "2", "--primary", "--left-of", "1"]),
            Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m 2 --primary"
                    .to_string()
            )
        );
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "1", "--primary"]),
            Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m 2 --primary"
                    .to_string()
            )
        );
    }

    #[test]
    fn layout_primary_with_monitor() {
        for args in [
            &["layout", "-m", "2", "--primary"][..],
            &["layout", "--primary", "-m", "2"][..],
        ] {
            assert_eq!(
                parse(args),
                Ok(Command::Layout {
                    action: LayoutAction::Primary { monitor: 2 },
                    yes: false
                })
            );
        }
    }

    #[test]
    fn layout_primary_without_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "--primary"]),
            Err("missing monitor for layout\ne.g. rmod layout -m 2 --primary".to_string())
        );
    }

    #[test]
    fn layout_direction_without_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "--left-of", "1"]),
            Err("missing monitor for layout\ne.g. rmod layout -m 2 --left-of 1".to_string())
        );
    }

    #[test]
    fn layout_yes_flag() {
        for args in [
            &["layout", "-y", "--left-of", "1", "-m", "2"][..],
            &["layout", "--left-of", "1", "-y", "-m", "2"][..],
        ] {
            assert_eq!(
                parse(args),
                Ok(Command::Layout {
                    action: LayoutAction::Place {
                        monitor: 2,
                        direction: Direction::Left,
                        reference: 1,
                    },
                    yes: true,
                })
            );
        }
    }

    #[test]
    fn layout_monitor_without_action_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "2"]),
Err("-m, --monitor needs a direction flag or --primary\ne.g. rmod layout -m 2 --left-of 1".to_string())
        );
    }

    #[test]
    fn layout_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["layout", "-m", "--left-of", "1"]),
            Err("-m, --monitor needs a value. a monitor number\ne.g. -m 2".to_string())
        );
    }

    #[test]
    fn ls_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["ls", "-m", "--caps"]),
            Err(
                "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string()
            )
        );
    }

    #[test]
    fn set_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["set", "-m", "--max"]),
            Err(
                "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string()
            )
        );
    }

    #[test]
    fn layout_help_flag() {
        assert!(parse(&["layout", "-h"]).is_err());
        assert_eq!(
            parse(&["layout", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Layout)
            })
        );
    }

    #[test]
    fn layout_unknown_argument_is_error() {
        assert_eq!(
            parse(&["layout", "foo"]),
            Err("unexpected argument foo for layout. use --left-of, --right-of, --above, --below, or --primary".to_string())
        );
    }

    #[test]
    fn layout_invalid_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "x"]),
            Err("invalid monitor number x. use a number from rmod list".to_string())
        );
        assert_eq!(
            parse(&["layout", "-m", "0"]),
            Err("monitor numbers start at 1. run rmod list to see them".to_string())
        );
    }

    #[test]
    fn main_command_now_errors_with_hint() {
        assert_eq!(
            parse(&["main"]),
            Err("unknown command main. use rmod layout -m N --primary".to_string())
        );
        assert_eq!(
            parse(&["main", "2", "-y"]),
            Err("unknown command main. use rmod layout -m N --primary".to_string())
        );
    }

    #[test]
    fn monitor_detach_requires_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "detach"]),
            Err("monitor detach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor detach -m 2".to_string())
        );
        assert_eq!(
            parse(&["monitor", "disable"]),
            Err("monitor detach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor detach -m 2".to_string())
        );
        assert_eq!(
            parse(&["monitor", "off"]),
            Err("monitor detach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor detach -m 2".to_string())
        );
    }

    #[test]
    fn monitor_attach_requires_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "attach"]),
            Err("monitor attach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor attach -m 2".to_string())
        );
        assert_eq!(
            parse(&["monitor", "enable"]),
            Err("monitor attach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor attach -m 2".to_string())
        );
        assert_eq!(
            parse(&["monitor", "on"]),
            Err("monitor attach needs -m, --monitor. a monitor number or all\ne.g. rmod monitor attach -m 2".to_string())
        );
    }

    #[test]
    fn monitor_disable_and_off_are_aliases_for_detach() {
        assert_eq!(
            parse(&["monitor", "disable", "-m", "2"]),
            parse(&["monitor", "detach", "-m", "2"])
        );
        assert_eq!(
            parse(&["monitor", "off", "-m", "2"]),
            parse(&["monitor", "detach", "-m", "2"])
        );
    }

    #[test]
    fn monitor_enable_and_on_are_aliases_for_attach() {
        assert_eq!(
            parse(&["monitor", "enable", "-m", "2"]),
            parse(&["monitor", "attach", "-m", "2"])
        );
        assert_eq!(
            parse(&["monitor", "on", "-m", "2"]),
            parse(&["monitor", "attach", "-m", "2"])
        );
    }

    #[test]
    fn monitor_detach_with_monitor_and_yes() {
        for args in [
            &["monitor", "detach", "-m", "2", "-y"][..],
            &["monitor", "detach", "-y", "-m", "2"][..],
            &["monitor", "disable", "-m", "all", "-y"][..],
        ] {
            let expected = Command::Monitor {
                action: MonitorAction::Disable,
                monitor: if args.contains(&"all") {
                    MonitorTarget::All
                } else {
                    MonitorTarget::Index(2)
                },
                yes: true,
            };
            assert_eq!(parse(args), Ok(expected), "args: {:?}", args);
        }
    }

    #[test]
    fn monitor_attach_with_monitor() {
        assert_eq!(
            parse(&["monitor", "attach", "-m", "2", "-y"]),
            Ok(Command::Monitor {
                action: MonitorAction::Enable,
                monitor: MonitorTarget::Index(2),
                yes: true
            })
        );
    }

    #[test]
    fn monitor_sleep_command() {
        assert_eq!(
            parse(&["monitor", "sleep"]),
            Ok(Command::Monitor {
                action: MonitorAction::Sleep,
                monitor: MonitorTarget::Primary,
                yes: false
            })
        );
    }

    #[test]
    fn monitor_wake_command() {
        assert_eq!(
            parse(&["monitor", "wake"]),
            Ok(Command::Monitor {
                action: MonitorAction::Wake,
                monitor: MonitorTarget::Primary,
                yes: false
            })
        );
    }

    #[test]
    fn monitor_sleep_rejects_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "sleep", "-m", "2"]),
            Err("-m, --monitor is not valid for monitor sleep. sleep applies to all monitors"
                .to_string())
        );
        assert_eq!(
            parse(&["monitor", "wake", "-m", "2"]),
            Err("-m, --monitor is not valid for monitor wake. wake applies to all monitors"
                .to_string())
        );
    }

    #[test]
    fn monitor_sleep_rejects_yes_flag() {
        assert_eq!(
            parse(&["monitor", "sleep", "-y"]),
            Err("-y, --yes is not valid for monitor sleep. sleep applies to all monitors"
                .to_string())
        );
    }

    #[test]
    fn monitor_missing_action_is_error() {
        assert_eq!(
            parse(&["monitor"]),
            Err(
                "monitor needs an action. attach, detach, sleep, or wake\ne.g. rmod monitor detach -m 2"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_unknown_action_is_error() {
        assert_eq!(
            parse(&["monitor", "frobnicate"]),
            Err(
                "unknown action frobnicate for monitor. use attach, detach, sleep, or wake"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_invalid_monitor_target_is_error() {
        assert!(parse(&["monitor", "detach", "-m", "x"]).is_err());
        assert!(parse(&["monitor", "detach", "-m", "0"]).is_err());
    }

    #[test]
    fn monitor_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["monitor", "detach", "-m"]),
            Err(
                "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_unknown_argument_is_error() {
        assert_eq!(
            parse(&["monitor", "detach", "foo"]),
            Err(
                "unexpected argument foo for monitor detach. use --monitor or --yes"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_help_flag() {
        assert_eq!(
            parse(&["monitor", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor { action: None })
            })
        );
        assert_eq!(
            parse(&["monitor", "disable", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Disable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "detach", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Disable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "attach", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Enable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "sleep", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Sleep)
                })
            })
        );
    }

    #[test]
    fn set_command() {
        assert_eq!(
            parse(&["set", "-p", "1080"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1080".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_monitor() {
        assert_eq!(
            parse(&["set", "-p", "4k", "-r", "144", "-m", "2"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("4k".to_string(), Refresh::Fixed(144)),
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_all() {
        assert_eq!(
            parse(&["set", "-r", "60", "-m", "all"]),
            Ok(Command::Set {
                spec: SetSpec::RefreshOnly(Refresh::Fixed(60)),
                monitor: MonitorTarget::All,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_orientation() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-m", "2", "-o", "90"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Index(2),
                orientation: Some(90),
                yes: false
            })
        );
    }

    #[test]
    fn set_with_yes() {
        assert_eq!(
            parse(&["set", "-p", "1440", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1440".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_explicit_resolution_and_refresh() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-r", "144"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Fixed(144)
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_explicit_no_refresh() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_refresh_only() {
        assert_eq!(
            parse(&["set", "-r", "max"]),
            Ok(Command::Set {
                spec: SetSpec::RefreshOnly(Refresh::Max),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_profile_with_refresh() {
        assert_eq!(
            parse(&["set", "-p", "720", "-r", "60"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("720".to_string(), Refresh::Fixed(60)),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_profile_with_max_refresh() {
        assert_eq!(
            parse(&["set", "-p", "720", "-r", "max"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("720".to_string(), Refresh::Max),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_all_profiles() {
        for (name, _, _) in PROFILES {
            assert!(parse(&["set", "-p", name]).is_ok(), "profile '{}'", name);
        }
    }

    #[test]
    fn set_unknown_profile_is_error() {
        assert!(parse(&["set", "-p", "480"]).is_err());
        assert!(parse(&["set", "-p", "1080p"]).is_err());
    }

    #[test]
    fn set_invalid_width_is_error() {
        assert!(parse(&["set", "-w", "abc", "-h", "1080"]).is_err());
    }

    #[test]
    fn set_invalid_height_is_error() {
        assert!(parse(&["set", "-w", "1920", "-h", "xyz"]).is_err());
    }

    #[test]
    fn set_invalid_refresh_is_error() {
        assert!(parse(&["set", "-r", "fast"]).is_err());
    }

    #[test]
    fn set_missing_spec_is_error() {
        assert_eq!(parse(&["set"]), Err("set needs something to change. width/height, refresh rate, profile, or --max\ne.g. rmod set -p 1080".to_string()));
    }

    #[test]
    fn set_orientation_aliases() {
        for (token, angle) in [
            ("0", 0),
            ("l", 0),
            ("landscape", 0),
            ("90", 90),
            ("p", 90),
            ("portrait", 90),
            ("180", 180),
            ("lf", 180),
            ("landscape-flipped", 180),
            ("270", 270),
            ("pf", 270),
            ("portrait-flipped", 270),
        ] {
            assert_eq!(
                parse(&["set", "-w", "1920", "-h", "1080", "-o", token]),
                Ok(Command::Set {
                    spec: SetSpec::Explicit {
                        width: 1920,
                        height: 1080,
                        refresh: Refresh::Keep
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: Some(angle),
                    yes: false
                }),
                "angle '{}'",
                token
            );
        }
    }

    #[test]
    fn set_orientation_case_insensitive() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-o", "Portrait"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Primary,
                orientation: Some(90),
                yes: false
            })
        );
    }

    #[test]
    fn set_invalid_orientation_is_error() {
        assert!(parse(&["set", "-w", "1920", "-h", "1080", "-o", "45"]).is_err());
    }

    #[test]
    fn set_missing_orientation_value_is_error() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-o"]),
            Err("-o, --orientation needs a value. 0, 90, 180, or 270\ne.g. -o 90".to_string())
        );
    }

    #[test]
    fn set_help_flag() {
        assert!(parse(&["set", "-h"]).is_err());
        assert_eq!(
            parse(&["set", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Set)
            })
        );
    }

    #[test]
    fn set_optional_spec() {
        assert_eq!(
            parse(&["set", "-o", "portrait"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Primary,
                orientation: Some(90),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-m", "2", "-o", "90"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Index(2),
                orientation: Some(90),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-m", "all", "-o", "landscape"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::All,
                orientation: Some(0),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn unknown_command_is_error() {
        assert!(parse(&["foo"]).is_err());
    }

    #[test]
    fn commands_are_case_sensitive() {
        assert!(parse(&["LS"]).is_err());
        assert!(parse(&["Max"]).is_err());
        assert!(parse(&["CAPS"]).is_err());
        assert!(parse(&["SET"]).is_err());
        assert!(parse(&["MAIN"]).is_err());
    }

    #[test]
    fn whitespace_in_token_is_error() {
        assert!(parse(&[" max"]).is_err());
        assert!(parse(&["max "]).is_err());
    }

    #[test]
    fn empty_argument_is_error() {
        assert!(parse(&[""]).is_err());
    }

    #[test]
    fn old_syntax_max_colon_is_error() {
        assert!(parse(&["max:2"]).is_err());
        assert!(parse(&["max:*"]).is_err());
    }

    #[test]
    fn old_syntax_caps_colon_is_error() {
        assert!(parse(&["caps:2"]).is_err());
        assert!(parse(&["caps:*"]).is_err());
    }

    #[test]
    fn old_syntax_main_colon_is_error() {
        assert!(parse(&["main:2"]).is_err());
    }

    #[test]
    fn old_syntax_implicit_set_is_error() {
        assert!(parse(&["1920x1080@60"]).is_err());
        assert!(parse(&["4k"]).is_err());
        assert!(parse(&["4k:2"]).is_err());
        assert!(parse(&["1920x1080:2/90"]).is_err());
    }

    #[test]
    fn old_syntax_flag_based_is_error() {
        assert!(parse(&["-w", "1920", "-h", "1080", "-r", "60"]).is_err());
        assert!(parse(&["-r", "144"]).is_err());
        assert!(parse(&["-o", "90"]).is_err());
    }

    #[test]
    fn old_syntax_main_m_flag_is_error() {
        assert!(parse(&["main", "-m", "2"]).is_err());
    }

    #[test]
    fn ls_caps_command() {
        assert_eq!(
            parse(&["ls", "--caps"]),
            Ok(Command::List {
                caps: true,
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn list_is_alias_for_ls() {
        assert_eq!(parse(&["list"]), parse(&["ls"]));
        assert_eq!(parse(&["list", "--caps"]), parse(&["ls", "--caps"]));
        assert_eq!(
            parse(&["list", "--caps", "-m", "2"]),
            parse(&["ls", "--caps", "-m", "2"])
        );
    }

    #[test]
    fn list_monitor_without_caps_is_error() {
        assert_eq!(
            parse(&["list", "-m", "2"]),
            Err(
                "-m, --monitor only works with --caps\ne.g. rmod list --caps -m 2"
                    .to_string()
            )
        );
    }

    #[test]
    fn ls_caps_with_monitor() {
        assert_eq!(
            parse(&["ls", "--caps", "-m", "2"]),
            Ok(Command::List {
                caps: true,
                monitor: MonitorTarget::Index(2)
            })
        );
    }

    #[test]
    fn ls_caps_all() {
        assert_eq!(
            parse(&["ls", "--caps", "-m", "all"]),
            Ok(Command::List {
                caps: true,
                monitor: MonitorTarget::All
            })
        );
    }

    #[test]
    fn ls_caps_monitor_before_flag() {
        assert_eq!(
            parse(&["ls", "-m", "2", "--caps"]),
            Ok(Command::List {
                caps: true,
                monitor: MonitorTarget::Index(2)
            })
        );
    }

    #[test]
    fn ls_caps_invalid_monitor_is_error() {
        assert!(parse(&["ls", "--caps", "-m", "x"]).is_err());
        assert!(parse(&["ls", "--caps", "-m", "0"]).is_err());
    }

    #[test]
    fn ls_caps_help_flag() {
        assert_eq!(
            parse(&["ls", "--caps", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::List)
            })
        );
    }

    #[test]
    fn ls_monitor_without_caps_is_error() {
        assert_eq!(
            parse(&["ls", "-m", "2"]),
            Err(
                "-m, --monitor only works with --caps\ne.g. rmod list --caps -m 2"
                    .to_string()
            )
        );
    }

    #[test]
    fn all_parser_errors_are_actionable() {
        // add a row when you add an error message
        let cases: &[(&[&str], &str)] = &[
            (&["--help", "x"], "parse_from --help with trailing arg"),
            (
                &["--version", "x"],
                "parse_from --version with trailing arg",
            ),
            (&["frobnicate"], "parse_from unknown command"),
            (&["main"], "parse_from legacy 'main' command"),
            (&["list", "-m"], "parse_ls -m missing value"),
            (&["list", "-m", "-x"], "parse_ls -m flag-like value"),
            (&["list", "foo"], "parse_ls unexpected argument"),
            (&["list", "-m", "2"], "parse_ls -m without --caps"),
            (
                &["layout", "--left-of"],
                "parse_layout direction missing value",
            ),
            (
                &["layout", "--left-of", "-m", "2"],
                "parse_layout direction flag-like value",
            ),
            (
                &["layout", "--left-of", "1", "--right-of", "2"],
                "parse_layout two directions",
            ),
            (&["layout", "-m"], "parse_layout -m missing value"),
            (&["layout", "-m", "-x"], "parse_layout -m flag-like value"),
            (
                &["layout", "-m", "x", "--left-of", "1"],
                "parse_layout invalid monitor number",
            ),
            (
                &["layout", "-m", "0", "--left-of", "1"],
                "parse_layout monitor number zero",
            ),
            (&["layout", "foo"], "parse_layout unexpected argument"),
            (
                &["layout", "--primary", "--left-of", "1"],
                "parse_layout primary plus direction",
            ),
            (
                &["layout", "--primary"],
                "parse_layout primary without monitor",
            ),
            (
                &["layout", "--left-of", "1"],
                "parse_layout direction without monitor",
            ),
            (
                &["layout", "-m", "2"],
                "parse_layout monitor without action",
            ),
            (&["set"], "parse_set missing spec"),
            (&["set", "-w"], "parse_set -w missing value"),
            (&["set", "-w", "x"], "parse_set invalid width"),
            (&["set", "-h"], "parse_set -h missing value"),
            (&["set", "-h", "x"], "parse_set invalid height"),
            (&["set", "-r"], "parse_set -r missing value"),
            (&["set", "-r", "x"], "parse_set invalid refresh"),
            (&["set", "-p"], "parse_set -p missing value"),
            (&["set", "-p", "x"], "parse_set unknown profile"),
            (&["set", "-m"], "parse_set -m missing value"),
            (&["set", "-m", "x"], "parse_set invalid monitor target"),
            (&["set", "-o"], "parse_set -o missing value"),
            (&["set", "-o", "x"], "parse_set invalid orientation"),
            (&["set", "foo"], "parse_set unexpected argument"),
            (&["set", "-w", "1920"], "parse_set width without height"),
            (
                &["set", "-p", "1080", "-w", "1920", "-h", "1080"],
                "parse_set profile plus width/height",
            ),
            (
                &["set", "--max", "-p", "1080"],
                "parse_set --max plus profile",
            ),
            (
                &["ls", "-m", "x", "--caps"],
                "parse_monitor_target invalid target",
            ),
        ];
        for (args, label) in cases {
            let err = parse(args).unwrap_err();
            assert!(
                err.contains("e.g.")
                    || err.contains("run rmod")
                    || err.contains("use ")
                    || err.contains("connect ")
                    || err.contains("move "),
                "{label}: message not actionable: {err}"
            );
        }
    }
}
