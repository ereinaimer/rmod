//! Command-line grammar: unified verb-centric syntax.
//!
//! Every command: `rmod <verb> [arguments]`
//! Monitor targeting is always a positional argument after the verb.

use std::env;

pub use crate::sys::windows::Direction;
pub use crate::sys::windows::apply::Refresh;

/// Help topics reachable via the command-specific `-h`/`--help` flags.
#[derive(Debug, PartialEq)]
pub enum HelpTopic {
    List,
    Set,
    Layout,
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
                return Err(format!("unexpected argument '{}'", args[1].as_ref()));
            }
            Ok(Command::Help { topic: None })
        }
        "--version" => {
            if args.len() > 1 {
                return Err(format!("unexpected argument '{}'", args[1].as_ref()));
            }
            Ok(Command::Version)
        }
        "ls" | "list" => parse_ls(cmd_str, args),
        "layout" => parse_layout(args),
        "main" => Err("unknown command 'main', use 'rmod layout -m N --primary'".to_string()),
        "set" => parse_set(args),
        _ => Err(format!("unknown command '{}'", cmd_str)),
    }
}

fn parse_ls(cmd: &str, args: &[impl AsRef<str>]) -> Result<Command, String> {
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
                    return Err("missing value for -m".to_string());
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err("missing value for -m".to_string());
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
            other => return Err(format!("unexpected argument '{}' for '{}'", other, cmd)),
        }
    }

    if monitor_explicit && !caps {
        return Err("-m is only valid with --caps".to_string());
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
                    return Err("missing value for -m".to_string());
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err("missing value for -m".to_string());
                }
                monitor = Some(parse_monitor_number(val)?);
                monitor_explicit = true;
                i += 1;
            }
            "--left-of" | "--right-of" | "--above" | "--below" => {
                if placement.is_some() {
                    return Err("only one direction flag allowed for 'layout'".to_string());
                }
                let direction = match arg {
                    "--left-of" => Direction::Left,
                    "--right-of" => Direction::Right,
                    "--above" => Direction::Above,
                    _ => Direction::Below,
                };
                i += 1;
                let Some(next) = args.get(i) else {
                    return Err(format!("missing value for {arg}"));
                };
                let next = next.as_ref();
                if next.starts_with('-') {
                    return Err(format!("missing value for {arg}"));
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
            other => return Err(format!("unexpected argument '{}' for 'layout'", other)),
        }
    }

    if primary {
        if placement.is_some() {
            return Err("cannot combine --primary with a direction flag".to_string());
        }
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for 'layout', e.g. 'rmod layout -m 2 --left-of 1'".to_string(),
            );
        };
        return Ok(Command::Layout {
            action: LayoutAction::Primary { monitor },
            yes,
        });
    }

    if monitor_explicit && placement.is_none() {
        return Err("-m is only valid with a direction flag or --primary".to_string());
    }

    if let Some((direction, reference)) = placement {
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for 'layout', e.g. 'rmod layout -m 2 --left-of 1'".to_string(),
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
    let n = arg
        .parse::<u32>()
        .map_err(|_| format!("invalid monitor number '{}'", arg))?;
    if n == 0 {
        return Err("monitor number must be >= 1".to_string());
    }
    Ok(n)
}

fn parse_set(args: &[impl AsRef<str>]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("missing action for 'set'".to_string());
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
                    return Err("missing value for -w".to_string());
                };
                width = Some(
                    val.as_ref()
                        .parse::<u32>()
                        .map_err(|_| format!("invalid width '{}'", val.as_ref()))?,
                );
                i += 1;
            }
            "-h" | "--height" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("missing value for -h".to_string());
                };
                height = Some(
                    val.as_ref()
                        .parse::<u32>()
                        .map_err(|_| format!("invalid height '{}'", val.as_ref()))?,
                );
                i += 1;
            }
            "-r" | "--refresh" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("missing value for -r".to_string());
                };
                refresh = Some(parse_refresh(val.as_ref())?);
                i += 1;
            }
            "-p" | "--profile" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("missing value for -p".to_string());
                };
                if !PROFILES.iter().any(|(name, _, _)| *name == val.as_ref()) {
                    return Err(format!("unknown profile '{}'", val.as_ref()));
                }
                profile = Some(val.as_ref().to_string());
                i += 1;
            }
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("missing value for -m".to_string());
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err("missing value for -m".to_string());
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            "-o" | "--orientation" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("missing value for -o".to_string());
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
            other => return Err(format!("unexpected argument '{}' for 'set'", other)),
        }
    }

    if (width.is_some() && height.is_none()) || (width.is_none() && height.is_some()) {
        return Err("width requires height and height requires width".to_string());
    }

    if profile.is_some() && (width.is_some() || height.is_some()) {
        return Err("cannot combine profile with explicit width or height".to_string());
    }

    if max_flag && (width.is_some() || height.is_some() || refresh.is_some() || profile.is_some()) {
        return Err("cannot combine --max with width, height, refresh, or profile".to_string());
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

fn parse_monitor_target(arg: &str) -> Result<MonitorTarget, String> {
    if arg == "all" {
        Ok(MonitorTarget::All)
    } else {
        let n = arg
            .parse::<u32>()
            .map_err(|_| format!("invalid monitor target '{}'", arg))?;
        if n == 0 {
            return Err("monitor number must be >= 1".to_string());
        }
        Ok(MonitorTarget::Index(n))
    }
}

fn parse_refresh(arg: &str) -> Result<Refresh, String> {
    match arg.to_lowercase().as_str() {
        "max" => Ok(Refresh::Max),
        _ => arg
            .parse::<u32>()
            .map(Refresh::Fixed)
            .map_err(|_| format!("invalid refresh rate '{}'", arg)),
    }
}

fn parse_orientation(arg: &str) -> Result<u32, String> {
    match arg.to_lowercase().as_str() {
        "0" | "l" | "landscape" => Ok(0),
        "90" | "p" | "portrait" => Ok(90),
        "180" | "lf" | "landscape-flipped" => Ok(180),
        "270" | "pf" | "portrait-flipped" => Ok(270),
        _ => Err(format!("invalid orientation '{}'", arg)),
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
            Err("unexpected argument 'foo' for 'list'".to_string())
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
                Err(format!("missing value for {flag}")),
                "flag '{}'",
                flag
            );
            assert_eq!(
                parse(&["layout", "-m", "2", flag, "--primary"]),
                Err(format!("missing value for {flag}")),
                "flag '{}'",
                flag
            );
        }
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "0"]),
            Err("monitor number must be >= 1".to_string())
        );
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "x"]),
            Err("invalid monitor number 'x'".to_string())
        );
    }

    #[test]
    fn layout_second_direction_flag_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "1", "--right-of", "1"]),
            Err("only one direction flag allowed for 'layout'".to_string())
        );
    }

    #[test]
    fn layout_primary_with_direction_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "2", "--primary", "--left-of", "1"]),
            Err("cannot combine --primary with a direction flag".to_string())
        );
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", "1", "--primary"]),
            Err("cannot combine --primary with a direction flag".to_string())
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
            Err("missing monitor for 'layout', e.g. 'rmod layout -m 2 --left-of 1'".to_string())
        );
    }

    #[test]
    fn layout_direction_without_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "--left-of", "1"]),
            Err("missing monitor for 'layout', e.g. 'rmod layout -m 2 --left-of 1'".to_string())
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
            Err("-m is only valid with a direction flag or --primary".to_string())
        );
    }

    #[test]
    fn layout_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["layout", "-m", "--left-of", "1"]),
            Err("missing value for -m".to_string())
        );
    }

    #[test]
    fn ls_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["ls", "-m", "--caps"]),
            Err("missing value for -m".to_string())
        );
    }

    #[test]
    fn set_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["set", "-m", "--max"]),
            Err("missing value for -m".to_string())
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
            Err("unexpected argument 'foo' for 'layout'".to_string())
        );
    }

    #[test]
    fn layout_invalid_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "-m", "x"]),
            Err("invalid monitor number 'x'".to_string())
        );
        assert_eq!(
            parse(&["layout", "-m", "0"]),
            Err("monitor number must be >= 1".to_string())
        );
    }

    #[test]
    fn main_command_now_errors_with_hint() {
        assert_eq!(
            parse(&["main"]),
            Err("unknown command 'main', use 'rmod layout -m N --primary'".to_string())
        );
        assert_eq!(
            parse(&["main", "2", "-y"]),
            Err("unknown command 'main', use 'rmod layout -m N --primary'".to_string())
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
        assert_eq!(parse(&["set"]), Err("missing action for 'set'".to_string()));
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
            Err("missing value for -o".to_string())
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
            Err("-m is only valid with --caps".to_string())
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
            Err("-m is only valid with --caps".to_string())
        );
    }
}
