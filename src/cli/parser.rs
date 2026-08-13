//! Command-line grammar: commands, `:N` monitor suffix, profiles.
//!
//! A single [`Command`] is produced from argv; any trailing or malformed
//! argument is rejected with an `Err(String)` describing the problem.

use std::env;

pub use crate::sys::windows::apply::Refresh;

/// Help topics reachable via the command-specific `-h`/`--help` flags.
#[derive(Debug, PartialEq)]
pub enum HelpTopic {
    /// `rmod ls -h`
    List,
    /// `rmod max -h`
    Max,
    /// `rmod caps -h`
    Caps,
    /// `rmod WxH@R -h`
    Set,
}

/// Every top-level command rmod accepts.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// `ls` — list displays and their current settings.
    List,
    /// `max[:N]` — apply the highest supported resolution/refresh.
    Max {
        /// Which display to target; `:*` = every monitor.
        target: Target,
        /// `-y`/`--yes` — skip the confirmation prompt.
        yes: bool,
    },
    /// `caps[:N]` — list supported modes.
    Caps {
        /// Which display to target; `:*` = every monitor.
        target: Target,
    },
    /// `WxH@R[:N][/angle]` — set resolution, refresh rate and rotation.
    Set {
        /// Pixel width; `None` keeps the current width.
        width: Option<u32>,
        /// Pixel height; `None` keeps the current height.
        height: Option<u32>,
        refresh: Refresh,
        /// Rotation angle in degrees; `None` keeps the current orientation.
        orientation: Option<u32>,
        /// Which display to target; `:*` = every monitor.
        target: Target,
        /// `-y`/`--yes` — skip the confirmation prompt.
        yes: bool,
    },
    /// `help [ls|max|caps|WxH@R]` or `-h`/`--help`.
    Help {
        /// Optional per-command topic.
        topic: Option<HelpTopic>,
    },
    /// `-V`/`--version` — print the version.
    Version,
}

/// Which display(s) a command targets.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Target {
    /// The primary display (no `:N` suffix).
    Primary,
    /// A numbered display from `ls` (`:N`, 1-based).
    Index(u32),
    /// Every attached display (`:*`).
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

/// Parses the process arguments into a [`Command`].
///
/// # Errors
/// Returns `Err` with a human-readable message for unknown commands,
/// invalid numbers, or unexpected trailing arguments.
pub fn parse() -> Result<Command, String> {
    parse_from(env::args())
}

/// Parses a command from an argument iterator; the first item is argv[0]
/// and is skipped. Split out from [`parse`] for testability.
///
/// # Errors
/// Returns `Err` with a human-readable message for unknown commands,
/// invalid numbers, or unexpected trailing arguments.
pub fn parse_from<I: Iterator<Item = String>>(args: I) -> Result<Command, String> {
    let args: Vec<String> = args.skip(1).collect();
    let Some(cmd) = args.first() else {
        return Ok(Command::Help { topic: None });
    };
    let command = match cmd.as_str() {
        "-h" => {
            if args.get(1).is_some_and(|t| t.parse::<u32>().is_ok()) {
                return parse_flags(&args);
            }
            return match args.get(1) {
                None => Ok(Command::Help { topic: None }),
                Some(extra) => Err(format!("unexpected argument '{extra}'")),
            };
        }
        "--help" => {
            return match args.get(1) {
                None => Ok(Command::Help { topic: None }),
                Some(extra) => Err(format!("unexpected argument '{extra}'")),
            };
        }
        "-V" | "--version" => {
            return match args.get(1) {
                None => Ok(Command::Version),
                Some(extra) => Err(format!("unexpected argument '{extra}'")),
            };
        }
        "ls" => parse_tail(
            "ls",
            args.get(1).cloned(),
            Command::List,
            HelpTopic::List,
            false,
        )?,
        "max" => parse_tail(
            "max",
            args.get(1).cloned(),
            Command::Max {
                target: Target::Primary,
                yes: false,
            },
            HelpTopic::Max,
            true,
        )?,
        "caps" => parse_tail(
            "caps",
            args.get(1).cloned(),
            Command::Caps {
                target: Target::Primary,
            },
            HelpTopic::Caps,
            false,
        )?,
        _ if cmd.starts_with("max:") => {
            let target = parse_monitor(&cmd[4..], cmd)?;
            parse_tail(
                "max",
                args.get(1).cloned(),
                Command::Max { target, yes: false },
                HelpTopic::Max,
                true,
            )?
        }
        _ if cmd.starts_with("caps:") => {
            let target = parse_monitor(&cmd[5..], cmd)?;
            parse_tail(
                "caps",
                args.get(1).cloned(),
                Command::Caps { target },
                HelpTopic::Caps,
                false,
            )?
        }
        _ if cmd.starts_with('-') => return parse_flags(&args),
        _ => parse_set(cmd, args.get(1).cloned())?,
    };
    if let Some(extra) = args.get(2) {
        return Err(format!("unexpected argument '{extra}'"));
    }
    Ok(command)
}

fn parse_tail(
    name: &str,
    tail: Option<String>,
    cmd: Command,
    topic: HelpTopic,
    allow_yes: bool,
) -> Result<Command, String> {
    match tail.as_deref() {
        Some("-h" | "--help") => Ok(Command::Help { topic: Some(topic) }),
        Some("-y" | "--yes") if allow_yes => Ok(match cmd {
            Command::Max { target, .. } => Command::Max { target, yes: true },
            Command::Set {
                width,
                height,
                refresh,
                orientation,
                target,
                ..
            } => Command::Set {
                width,
                height,
                refresh,
                orientation,
                target,
                yes: true,
            },
            other => other,
        }),
        Some(other) => Err(format!("unknown argument '{other}' for '{name}'")),
        None => Ok(cmd),
    }
}

fn parse_set(cmd: &str, tail: Option<String>) -> Result<Command, String> {
    let (spec, orientation) = match cmd.split_once('/') {
        Some((spec, angle)) => (spec, Some(parse_orientation(angle, cmd)?)),
        None => (cmd, None),
    };
    let (spec, target) = match spec.split_once(':') {
        Some((spec, m)) => (spec, parse_monitor(m, cmd)?),
        None => (spec, Target::Primary),
    };
    let (res, refresh) = match spec.split_once('@') {
        Some((res, r)) => (res, Some(parse_refresh(r, cmd)?)),
        None => (spec, None),
    };
    let (width, height) = match res.split_once('x') {
        Some((w, h)) => (
            Some(w.parse().map_err(|_| format!("invalid width in '{cmd}'"))?),
            Some(
                h.parse()
                    .map_err(|_| format!("invalid height in '{cmd}'"))?,
            ),
        ),
        None => match PROFILES.iter().find(|(name, _, _)| *name == res) {
            Some((_, w, h)) => (Some(*w), Some(*h)),
            None => return Err(format!("unknown profile or invalid resolution '{cmd}'")),
        },
    };
    let refresh = refresh.unwrap_or(Refresh::Keep);
    match tail.as_deref() {
        Some("-h" | "--help") => Ok(Command::Help {
            topic: Some(HelpTopic::Set),
        }),
        Some("-y" | "--yes") => Ok(Command::Set {
            width,
            height,
            refresh,
            orientation,
            target,
            yes: true,
        }),
        Some(other) => Err(format!("unexpected argument '{other}'")),
        None => Ok(Command::Set {
            width,
            height,
            refresh,
            orientation,
            target,
            yes: false,
        }),
    }
}

/// Parses a rotation angle token; names are matched case-insensitively.
fn parse_orientation(token: &str, cmd: &str) -> Result<u32, String> {
    match token.to_lowercase().as_str() {
        "0" | "l" | "landscape" => Ok(0),
        "90" | "p" | "portrait" => Ok(90),
        "180" | "lf" => Ok(180),
        "270" | "pf" => Ok(270),
        _ => Err(format!("invalid orientation in '{cmd}'")),
    }
}

fn parse_refresh(r: &str, cmd: &str) -> Result<Refresh, String> {
    if r == "max" {
        Ok(Refresh::Max)
    } else {
        r.parse()
            .map(Refresh::Fixed)
            .map_err(|_| format!("invalid refresh rate in '{cmd}'"))
    }
}

fn parse_monitor(m: &str, cmd: &str) -> Result<Target, String> {
    if m == "*" {
        Ok(Target::All)
    } else {
        m.parse()
            .map(Target::Index)
            .map_err(|_| format!("invalid monitor id in '{cmd}'"))
    }
}

/// Parses `-w`/`-h`/`-r`/`-o`/`-m`/`-y` flag syntax into a set command.
///
/// Flags may appear in any order; repeated flags keep the last value.
/// `-h` is contextual: a following token that parses as a height keeps it,
/// otherwise (missing, non-numeric, or flag-like) the set help page wins.
///
/// # Errors
/// Returns `Err` for missing or invalid flag values, unknown flags, or
/// when no dimension/refresh flag was given.
fn parse_flags(args: &[String]) -> Result<Command, String> {
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut refresh: Option<Refresh> = None;
    let mut orientation: Option<u32> = None;
    let mut target = Target::Primary;
    let mut yes = false;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        match flag {
            "-w" | "--width" => {
                let Some(value) = args.get(i) else {
                    return Err("missing value for -w".to_string());
                };
                i += 1;
                width = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid width in '-w {value}'"))?,
                );
            }
            "-h" | "--height" => {
                let Some(value) = args.get(i) else {
                    return Ok(Command::Help {
                        topic: Some(HelpTopic::Set),
                    });
                };
                match value.parse::<u32>() {
                    Ok(h) => {
                        i += 1;
                        height = Some(h);
                    }
                    Err(_) => {
                        return Ok(Command::Help {
                            topic: Some(HelpTopic::Set),
                        });
                    }
                }
            }
            "-r" | "--refresh" => {
                let Some(value) = args.get(i) else {
                    return Err("missing value for -r".to_string());
                };
                i += 1;
                refresh = Some(match value.as_str() {
                    "max" => Refresh::Max,
                    "keep" => Refresh::Keep,
                    _ => value
                        .parse()
                        .map(Refresh::Fixed)
                        .map_err(|_| format!("invalid refresh in '-r {value}'"))?,
                });
            }
            "-m" | "--monitor" => {
                let Some(value) = args.get(i) else {
                    return Err("missing value for -m".to_string());
                };
                i += 1;
                target = if value == "*" {
                    Target::All
                } else {
                    value
                        .parse()
                        .map(Target::Index)
                        .map_err(|_| format!("invalid monitor id in '-m {value}'"))?
                };
            }
            "-o" | "--orientation" => {
                let Some(value) = args.get(i) else {
                    return Err("missing value for -o".to_string());
                };
                i += 1;
                orientation = Some(parse_orientation(value, &format!("-o {value}"))?);
            }
            "-y" | "--yes" => yes = true,
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Set),
                });
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if width.is_none() && height.is_none() && refresh.is_none() && orientation.is_none() {
        return Err("nothing to set".to_string());
    }
    if width.is_some() != height.is_some() {
        return Err(if width.is_some() {
            "-w requires -h".to_string()
        } else {
            "-h requires -w".to_string()
        });
    }
    Ok(Command::Set {
        width,
        height,
        refresh: refresh.unwrap_or(Refresh::Keep),
        orientation,
        target,
        yes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_from(std::iter::once("rmod".to_string()).chain(args.iter().map(|s| s.to_string())))
    }

    fn set(width: u32, height: u32, refresh: Refresh, target: Target) -> Command {
        Command::Set {
            width: Some(width),
            height: Some(height),
            refresh,
            orientation: None,
            target,
            yes: false,
        }
    }

    fn set_yes(width: u32, height: u32, refresh: Refresh, target: Target) -> Command {
        Command::Set {
            width: Some(width),
            height: Some(height),
            refresh,
            orientation: None,
            target,
            yes: true,
        }
    }

    fn set_rotated(
        width: u32,
        height: u32,
        refresh: Refresh,
        orientation: u32,
        target: Target,
    ) -> Command {
        Command::Set {
            width: Some(width),
            height: Some(height),
            refresh,
            orientation: Some(orientation),
            target,
            yes: false,
        }
    }

    fn help(topic: Option<HelpTopic>) -> Command {
        Command::Help { topic }
    }

    #[test]
    fn no_args_prints_help() {
        assert_eq!(parse(&[]), Ok(help(None)));
    }

    #[test]
    fn help_flags() {
        assert_eq!(parse(&["-h"]), Ok(help(None)));
        assert_eq!(parse(&["--help"]), Ok(help(None)));
    }

    #[test]
    fn version_flags() {
        assert_eq!(parse(&["-V"]), Ok(Command::Version));
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
    }

    #[test]
    fn ls_command() {
        assert_eq!(parse(&["ls"]), Ok(Command::List));
    }

    #[test]
    fn ls_help_flags() {
        assert_eq!(parse(&["ls", "-h"]), Ok(help(Some(HelpTopic::List))));
        assert_eq!(parse(&["ls", "--help"]), Ok(help(Some(HelpTopic::List))));
    }

    #[test]
    fn ls_unknown_argument_is_error() {
        assert!(parse(&["ls", "foo"]).is_err());
    }

    #[test]
    fn ls_yes_flag_is_error() {
        let err = parse(&["ls", "-y"]).unwrap_err();
        assert!(err.contains("unknown argument '-y' for 'ls'"), "{err}");
    }

    #[test]
    fn max_command() {
        assert_eq!(
            parse(&["max"]),
            Ok(Command::Max {
                target: Target::Primary,
                yes: false
            })
        );
    }

    #[test]
    fn max_with_monitor() {
        assert_eq!(
            parse(&["max:2"]),
            Ok(Command::Max {
                target: Target::Index(2),
                yes: false
            })
        );
    }

    #[test]
    fn max_yes_flag() {
        assert_eq!(
            parse(&["max", "-y"]),
            Ok(Command::Max {
                target: Target::Primary,
                yes: true
            })
        );
    }

    #[test]
    fn max_yes_flag_with_monitor() {
        assert_eq!(
            parse(&["max:2", "-y"]),
            Ok(Command::Max {
                target: Target::Index(2),
                yes: true
            })
        );
    }

    #[test]
    fn max_all_target() {
        assert_eq!(
            parse(&["max:*"]),
            Ok(Command::Max {
                target: Target::All,
                yes: false
            })
        );
    }

    #[test]
    fn max_all_target_with_yes() {
        assert_eq!(
            parse(&["max:*", "-y"]),
            Ok(Command::Max {
                target: Target::All,
                yes: true
            })
        );
    }

    #[test]
    fn max_all_invalid_id_is_error() {
        let err = parse(&["max:*:2"]).unwrap_err();
        assert!(err.contains("invalid monitor id"), "{err}");
    }

    #[test]
    fn max_invalid_monitor_is_error() {
        assert!(parse(&["max:x"]).is_err());
        assert!(parse(&["max:"]).is_err());
    }

    #[test]
    fn max_help_flags() {
        assert_eq!(parse(&["max", "-h"]), Ok(help(Some(HelpTopic::Max))));
        assert_eq!(parse(&["max", "--help"]), Ok(help(Some(HelpTopic::Max))));
        assert_eq!(parse(&["max:2", "-h"]), Ok(help(Some(HelpTopic::Max))));
    }

    #[test]
    fn caps_command() {
        assert_eq!(
            parse(&["caps"]),
            Ok(Command::Caps {
                target: Target::Primary
            })
        );
    }

    #[test]
    fn caps_with_monitor() {
        assert_eq!(
            parse(&["caps:2"]),
            Ok(Command::Caps {
                target: Target::Index(2)
            })
        );
    }

    #[test]
    fn caps_all_target() {
        assert_eq!(
            parse(&["caps:*"]),
            Ok(Command::Caps {
                target: Target::All
            })
        );
    }

    #[test]
    fn caps_invalid_monitor_is_error() {
        assert!(parse(&["caps:x"]).is_err());
        assert!(parse(&["caps:"]).is_err());
    }

    #[test]
    fn caps_help_flags() {
        assert_eq!(parse(&["caps", "-h"]), Ok(help(Some(HelpTopic::Caps))));
        assert_eq!(parse(&["caps", "--help"]), Ok(help(Some(HelpTopic::Caps))));
    }

    #[test]
    fn caps_help_flags_with_monitor() {
        assert_eq!(parse(&["caps:2", "-h"]), Ok(help(Some(HelpTopic::Caps))));
        assert_eq!(
            parse(&["caps:2", "--help"]),
            Ok(help(Some(HelpTopic::Caps)))
        );
    }

    #[test]
    fn caps_yes_flag_is_error() {
        let err = parse(&["caps", "-y"]).unwrap_err();
        assert!(err.contains("unknown argument '-y' for 'caps'"), "{err}");
    }

    #[test]
    fn caps_yes_flag_with_monitor_is_error() {
        let err = parse(&["caps:2", "-y"]).unwrap_err();
        assert!(err.contains("unknown argument '-y' for 'caps'"), "{err}");
    }

    #[test]
    fn set_help_flags() {
        assert_eq!(
            parse(&["1920x1080@60", "-h"]),
            Ok(help(Some(HelpTopic::Set)))
        );
        assert_eq!(parse(&["4k", "--help"]), Ok(help(Some(HelpTopic::Set))));
        assert_eq!(
            parse(&["1920x1080@60:2", "-h"]),
            Ok(help(Some(HelpTopic::Set)))
        );
    }

    #[test]
    fn set_help_extra_argument_is_error() {
        assert!(parse(&["1920x1080@60", "-h", "foo"]).is_err());
    }

    #[test]
    fn set_unknown_argument_is_error() {
        assert!(parse(&["1920x1080@60", "foo"]).is_err());
    }

    #[test]
    fn set_yes_flag() {
        assert_eq!(
            parse(&["1920x1080@60", "-y"]),
            Ok(set_yes(1920, 1080, Refresh::Fixed(60), Target::Primary))
        );
    }

    #[test]
    fn set_yes_flag_long() {
        assert_eq!(
            parse(&["1920x1080@60", "--yes"]),
            Ok(set_yes(1920, 1080, Refresh::Fixed(60), Target::Primary))
        );
    }

    #[test]
    fn set_yes_extra_argument_is_error() {
        assert!(parse(&["1920x1080@60", "-y", "extra"]).is_err());
    }

    #[test]
    fn set_resolution_and_refresh() {
        assert_eq!(
            parse(&["1920x1080@144"]),
            Ok(set(1920, 1080, Refresh::Fixed(144), Target::Primary))
        );
    }

    #[test]
    fn set_with_monitor_suffix() {
        assert_eq!(
            parse(&["1920x1080@60:2"]),
            Ok(set(1920, 1080, Refresh::Fixed(60), Target::Index(2)))
        );
    }

    #[test]
    fn set_all_target() {
        assert_eq!(
            parse(&["1920x1080@60:*"]),
            Ok(set(1920, 1080, Refresh::Fixed(60), Target::All))
        );
    }

    #[test]
    fn set_all_target_help() {
        assert_eq!(
            parse(&["1920x1080@60:*", "-h"]),
            Ok(help(Some(HelpTopic::Set)))
        );
    }

    #[test]
    fn set_all_invalid_id_is_error() {
        let err = parse(&["1920x1080@60:12x"]).unwrap_err();
        assert!(err.contains("invalid monitor id"), "{err}");
    }

    #[test]
    fn set_without_refresh_keeps_current() {
        assert_eq!(
            parse(&["1920x1080"]),
            Ok(set(1920, 1080, Refresh::Keep, Target::Primary))
        );
    }

    #[test]
    fn set_with_max_refresh() {
        assert_eq!(
            parse(&["1920x1080@max"]),
            Ok(set(1920, 1080, Refresh::Max, Target::Primary))
        );
    }

    #[test]
    fn profile_resolves_to_resolution() {
        assert_eq!(
            parse(&["4k"]),
            Ok(set(3840, 2160, Refresh::Keep, Target::Primary))
        );
    }

    #[test]
    fn profile_with_fixed_refresh() {
        assert_eq!(
            parse(&["720@60"]),
            Ok(set(1280, 720, Refresh::Fixed(60), Target::Primary))
        );
    }

    #[test]
    fn profile_with_max_refresh_and_monitor() {
        assert_eq!(
            parse(&["720@max:2"]),
            Ok(set(1280, 720, Refresh::Max, Target::Index(2)))
        );
    }

    #[test]
    fn profile_with_monitor_suffix() {
        assert_eq!(
            parse(&["4k:2"]),
            Ok(set(3840, 2160, Refresh::Keep, Target::Index(2)))
        );
    }

    #[test]
    fn all_profiles_resolve_to_their_resolution() {
        for (name, width, height) in PROFILES {
            assert_eq!(
                parse(&[name]),
                Ok(set(*width, *height, Refresh::Keep, Target::Primary)),
                "profile '{name}'"
            );
        }
    }

    #[test]
    fn unknown_command_is_error() {
        assert!(parse(&["foo"]).is_err());
    }

    #[test]
    fn unknown_profile_is_error() {
        assert!(parse(&["480"]).is_err());
        assert!(parse(&["1080p"]).is_err());
    }

    #[test]
    fn invalid_width_is_error() {
        assert!(parse(&["ax1080@60"]).is_err());
        assert!(parse(&["-1x1080@60"]).is_err());
    }

    #[test]
    fn invalid_height_is_error() {
        assert!(parse(&["1920x@60"]).is_err());
    }

    #[test]
    fn invalid_refresh_is_error() {
        assert!(parse(&["1920x1080@fast"]).is_err());
        assert!(parse(&["1920x1080@"]).is_err());
    }

    #[test]
    fn empty_resolution_is_error() {
        assert!(parse(&["@60"]).is_err());
    }

    #[test]
    fn max_with_refresh_syntax_is_error() {
        assert!(parse(&["max@60"]).is_err());
    }

    #[test]
    fn trailing_argument_after_set_is_error() {
        assert!(parse(&["1920x1080@60", "extra"]).is_err());
        assert!(parse(&["4k:2", "extra"]).is_err());
    }

    #[test]
    fn trailing_argument_after_flag_is_error() {
        assert!(parse(&["-h", "extra"]).is_err());
        assert!(parse(&["--version", "extra"]).is_err());
    }

    #[test]
    fn trailing_argument_after_monitor_command_is_error() {
        assert!(parse(&["max:2", "extra"]).is_err());
        assert!(parse(&["caps:2", "extra"]).is_err());
    }

    #[test]
    fn monitor_overflow_is_error() {
        assert!(parse(&["max:4294967296"]).is_err());
        assert!(parse(&["caps:4294967296"]).is_err());
        assert!(parse(&["1920x1080@60:4294967296"]).is_err());
    }

    #[test]
    fn dimension_overflow_is_error() {
        assert!(parse(&["99999999999999x1080@60"]).is_err());
        assert!(parse(&["1920x99999999999999@60"]).is_err());
    }

    #[test]
    fn refresh_overflow_is_error() {
        assert!(parse(&["1920x1080@99999999999"]).is_err());
    }

    #[test]
    fn zero_refresh_parses_as_fixed_zero() {
        assert_eq!(
            parse(&["1920x1080@0"]),
            Ok(set(1920, 1080, Refresh::Fixed(0), Target::Primary))
        );
    }

    #[test]
    fn monitor_with_leading_zeros() {
        assert_eq!(
            parse(&["1920x1080@60:02"]),
            Ok(set(1920, 1080, Refresh::Fixed(60), Target::Index(2)))
        );
    }

    #[test]
    fn multiple_at_signs_is_error() {
        assert!(parse(&["1920x1080@60@70"]).is_err());
    }

    #[test]
    fn multiple_x_is_error() {
        assert!(parse(&["1920x1080x2@60"]).is_err());
    }

    #[test]
    fn multiple_colons_is_error() {
        assert!(parse(&["1920x1080@60:2:3"]).is_err());
        assert!(parse(&["max:2:3"]).is_err());
    }

    #[test]
    fn colon_before_at_is_error() {
        assert!(parse(&["1920x1080:2@60"]).is_err());
    }

    #[test]
    fn commands_are_case_sensitive() {
        assert!(parse(&["LS"]).is_err());
        assert!(parse(&["Max"]).is_err());
        assert!(parse(&["Caps"]).is_err());
        assert!(parse(&["4K"]).is_err());
        assert!(parse(&["-V"]).is_ok());
        assert!(parse(&["-v"]).is_err());
        assert!(parse(&["--HELP"]).is_err());
    }

    #[test]
    fn whitespace_in_token_is_error() {
        assert!(parse(&[" 720"]).is_err());
        assert!(parse(&["720 "]).is_err());
        assert!(parse(&["1920x1080 @60"]).is_err());
    }

    #[test]
    fn empty_argument_is_error() {
        assert!(parse(&[""]).is_err());
    }

    #[test]
    fn double_dash_is_error() {
        assert!(parse(&["--"]).is_err());
    }

    #[test]
    fn command_with_monitor_suffix_is_error() {
        assert!(parse(&["ls:2"]).is_err());
        assert!(parse(&["ls:"]).is_err());
    }

    #[test]
    fn flags_full_spec() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080", "-r", "144", "-m", "2", "-y"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Fixed(144),
                orientation: None,
                target: Target::Index(2),
                yes: true,
            })
        );
    }

    #[test]
    fn flags_long_names() {
        assert_eq!(
            parse(&[
                "--width",
                "1920",
                "--height",
                "1080",
                "--refresh",
                "144",
                "--monitor",
                "2",
                "--yes",
            ]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Fixed(144),
                orientation: None,
                target: Target::Index(2),
                yes: true,
            })
        );
    }

    #[test]
    fn flags_order_independent() {
        assert_eq!(
            parse(&["-m", "2", "-r", "144", "-h", "1080", "-w", "1920", "-y"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Fixed(144),
                orientation: None,
                target: Target::Index(2),
                yes: true,
            })
        );
    }

    #[test]
    fn flags_repeated_last_wins() {
        assert_eq!(
            parse(&["-w", "100", "-h", "200", "-w", "1920", "-h", "1080"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_width_and_height_keep_current_refresh() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_refresh_only() {
        assert_eq!(
            parse(&["-r", "144"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Fixed(144),
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_refresh_max() {
        assert_eq!(
            parse(&["-r", "max"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Max,
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_refresh_keep() {
        assert_eq!(
            parse(&["-r", "keep"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_monitor_all() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080", "-m", "*"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::All,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_monitor_zero_parses() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080", "-m", "0"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::Index(0),
                yes: false,
            })
        );
    }

    #[test]
    fn flags_height_first_then_width() {
        assert_eq!(
            parse(&["-h", "1080", "-w", "1920"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: None,
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_height_first_requires_width() {
        let err = parse(&["-h", "1080"]).unwrap_err();
        assert_eq!(err, "-h requires -w");
    }

    #[test]
    fn flags_width_requires_height() {
        let err = parse(&["-w", "1920"]).unwrap_err();
        assert_eq!(err, "-w requires -h");
    }

    #[test]
    fn flags_trailing_h_is_help() {
        assert_eq!(parse(&["-w", "1920", "-h"]), Ok(help(Some(HelpTopic::Set))));
    }

    #[test]
    fn flags_h_with_bad_value_is_help() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "abc"]),
            Ok(help(Some(HelpTopic::Set)))
        );
    }

    #[test]
    fn flags_h_before_other_flag_is_help() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "-r", "60"]),
            Ok(help(Some(HelpTopic::Set)))
        );
    }

    #[test]
    fn flags_help_flag_in_flag_mode() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080", "--help"]),
            Ok(help(Some(HelpTopic::Set)))
        );
    }

    #[test]
    fn flags_missing_width_value() {
        let err = parse(&["-w"]).unwrap_err();
        assert_eq!(err, "missing value for -w");
    }

    #[test]
    fn flags_missing_refresh_value() {
        let err = parse(&["-r"]).unwrap_err();
        assert_eq!(err, "missing value for -r");
    }

    #[test]
    fn flags_missing_monitor_value() {
        let err = parse(&["-m"]).unwrap_err();
        assert_eq!(err, "missing value for -m");
    }

    #[test]
    fn flags_invalid_width() {
        let err = parse(&["-w", "abc"]).unwrap_err();
        assert_eq!(err, "invalid width in '-w abc'");
    }

    #[test]
    fn flags_invalid_refresh() {
        let err = parse(&["-r", "abc"]).unwrap_err();
        assert_eq!(err, "invalid refresh in '-r abc'");
    }

    #[test]
    fn flags_invalid_monitor_id() {
        let err = parse(&["-m", "abc"]).unwrap_err();
        assert_eq!(err, "invalid monitor id in '-m abc'");
    }

    #[test]
    fn flags_yes_alone_is_nothing_to_set() {
        let err = parse(&["-y"]).unwrap_err();
        assert_eq!(err, "nothing to set");
    }

    #[test]
    fn flags_monitor_alone_is_nothing_to_set() {
        let err = parse(&["-m", "2"]).unwrap_err();
        assert_eq!(err, "nothing to set");
    }

    #[test]
    fn flags_unknown_argument() {
        let err = parse(&["-x"]).unwrap_err();
        assert_eq!(err, "unknown argument '-x'");
    }

    #[test]
    fn flags_positional_after_flags_is_error() {
        let err = parse(&["-w", "1920", "-h", "1080", "extra"]).unwrap_err();
        assert_eq!(err, "unknown argument 'extra'");
    }

    #[test]
    fn compact_angle_suffix_parses() {
        assert_eq!(
            parse(&["1920x1080/90"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 90, Target::Primary))
        );
        assert_eq!(
            parse(&["1920x1080@60:1/270"]),
            Ok(set_rotated(
                1920,
                1080,
                Refresh::Fixed(60),
                270,
                Target::Index(1)
            ))
        );
        assert_eq!(
            parse(&["1920x1080:2/portrait"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 90, Target::Index(2)))
        );
    }

    #[test]
    fn compact_angle_aliases() {
        for (token, angle) in [
            ("0", 0),
            ("l", 0),
            ("landscape", 0),
            ("90", 90),
            ("p", 90),
            ("portrait", 90),
            ("180", 180),
            ("lf", 180),
            ("270", 270),
            ("pf", 270),
        ] {
            assert_eq!(
                parse(&[&format!("1920x1080/{token}")]),
                Ok(set_rotated(
                    1920,
                    1080,
                    Refresh::Keep,
                    angle,
                    Target::Primary
                )),
                "angle '{token}'"
            );
        }
    }

    #[test]
    fn compact_angle_aliases_case_insensitive() {
        assert_eq!(
            parse(&["1920x1080/Portrait"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 90, Target::Primary))
        );
        assert_eq!(
            parse(&["1920x1080/PF"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 270, Target::Primary))
        );
        assert_eq!(
            parse(&["1920x1080/LF"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 180, Target::Primary))
        );
        assert_eq!(
            parse(&["1920x1080/L"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 0, Target::Primary))
        );
    }

    #[test]
    fn compact_angle_with_all_monitors() {
        assert_eq!(
            parse(&["1920x1080:*/pf"]),
            Ok(set_rotated(1920, 1080, Refresh::Keep, 270, Target::All))
        );
    }

    #[test]
    fn compact_angle_with_yes_flag() {
        assert_eq!(
            parse(&["1920x1080:2/90", "-y"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Index(2),
                yes: true,
            })
        );
    }

    #[test]
    fn compact_invalid_angle_is_error() {
        assert_eq!(
            parse(&["1920x1080:2/45"]),
            Err("invalid orientation in '1920x1080:2/45'".to_string())
        );
    }

    #[test]
    fn compact_multiple_slashes_is_error() {
        assert_eq!(
            parse(&["1920x1080/90/180"]),
            Err("invalid orientation in '1920x1080/90/180'".to_string())
        );
    }

    #[test]
    fn compact_empty_angle_is_error() {
        assert_eq!(
            parse(&["1920x1080:2/"]),
            Err("invalid orientation in '1920x1080:2/'".to_string())
        );
    }

    #[test]
    fn flags_orientation() {
        assert_eq!(
            parse(&["-o", "90"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Primary,
                yes: false,
            })
        );
        assert_eq!(
            parse(&["--orientation", "portrait"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_orientation_aliases() {
        for (token, angle) in [
            ("0", 0),
            ("l", 0),
            ("landscape", 0),
            ("90", 90),
            ("p", 90),
            ("portrait", 90),
            ("180", 180),
            ("lf", 180),
            ("270", 270),
            ("pf", 270),
        ] {
            assert_eq!(
                parse(&["-o", token]),
                Ok(Command::Set {
                    width: None,
                    height: None,
                    refresh: Refresh::Keep,
                    orientation: Some(angle),
                    target: Target::Primary,
                    yes: false,
                }),
                "angle '{token}'"
            );
        }
    }

    #[test]
    fn flags_orientation_repeated_last_wins() {
        assert_eq!(
            parse(&["-o", "0", "-o", "90"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_orientation_invalid_is_error() {
        assert_eq!(
            parse(&["-o", "45"]),
            Err("invalid orientation in '-o 45'".to_string())
        );
    }

    #[test]
    fn flags_orientation_missing_value_is_error() {
        assert_eq!(parse(&["-o"]), Err("missing value for -o".to_string()));
    }

    #[test]
    fn flags_orientation_with_dimensions() {
        assert_eq!(
            parse(&["-w", "1920", "-h", "1080", "-o", "90"]),
            Ok(Command::Set {
                width: Some(1920),
                height: Some(1080),
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Primary,
                yes: false,
            })
        );
        assert_eq!(
            parse(&["-o", "90", "-r", "144"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Fixed(144),
                orientation: Some(90),
                target: Target::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_orientation_with_monitor() {
        assert_eq!(
            parse(&["-o", "90", "-m", "2"]),
            Ok(Command::Set {
                width: None,
                height: None,
                refresh: Refresh::Keep,
                orientation: Some(90),
                target: Target::Index(2),
                yes: false,
            })
        );
    }
}
