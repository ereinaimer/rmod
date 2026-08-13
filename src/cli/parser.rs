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
        /// Monitor number; `None` = primary display.
        monitor: Option<u32>,
        /// `-y`/`--yes` — skip the confirmation prompt.
        yes: bool,
    },
    /// `caps[:N]` — list supported modes.
    Caps {
        /// Monitor number; `None` = primary display.
        monitor: Option<u32>,
    },
    /// `WxH@R[:N]` — set resolution and refresh rate.
    Set {
        width: u32,
        height: u32,
        refresh: Refresh,
        /// Monitor number; `None` = primary display.
        monitor: Option<u32>,
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
    let mut args = args.skip(1);
    let Some(cmd) = args.next() else {
        return Ok(Command::Help { topic: None });
    };
    let command = match cmd.as_str() {
        "-h" | "--help" => Command::Help { topic: None },
        "-V" | "--version" => Command::Version,
        "ls" => parse_tail("ls", args.next(), Command::List, HelpTopic::List, false)?,
        "max" => parse_tail(
            "max",
            args.next(),
            Command::Max { monitor: None, yes: false },
            HelpTopic::Max,
            true,
        )?,
        "caps" => parse_tail("caps", args.next(), Command::Caps { monitor: None }, HelpTopic::Caps, false)?,
        _ if cmd.starts_with("max:") => {
            let monitor = parse_monitor(&cmd[4..], &cmd)?;
            parse_tail(
                "max",
                args.next(),
                Command::Max { monitor: Some(monitor), yes: false },
                HelpTopic::Max,
                true,
            )?
        }
        _ if cmd.starts_with("caps:") => {
            let monitor = parse_monitor(&cmd[5..], &cmd)?;
            parse_tail(
                "caps",
                args.next(),
                Command::Caps { monitor: Some(monitor) },
                HelpTopic::Caps,
                false,
            )?
        }
        _ => parse_set(&cmd, args.next())?,
    };
    if let Some(extra) = args.next() {
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
            Command::Max { monitor, .. } => Command::Max { monitor, yes: true },
            Command::Set { width, height, refresh, monitor, .. } => {
                Command::Set { width, height, refresh, monitor, yes: true }
            }
            other => other,
        }),
        Some(other) => Err(format!("unknown argument '{other}' for '{name}'")),
        None => Ok(cmd),
    }
}

fn parse_set(cmd: &str, tail: Option<String>) -> Result<Command, String> {
    let (spec, monitor) = match cmd.split_once(':') {
        Some((spec, m)) => (spec, Some(parse_monitor(m, cmd)?)),
        None => (cmd, None),
    };
    let (res, refresh) = match spec.split_once('@') {
        Some((res, r)) => (res, Some(parse_refresh(r, cmd)?)),
        None => (spec, None),
    };
    let (width, height) = match res.split_once('x') {
        Some((w, h)) => (
            w.parse().map_err(|_| format!("invalid width in '{cmd}'"))?,
            h.parse().map_err(|_| format!("invalid height in '{cmd}'"))?,
        ),
        None => match PROFILES.iter().find(|(name, _, _)| *name == res) {
            Some((_, w, h)) => (*w, *h),
            None => return Err(format!("unknown profile or invalid resolution '{cmd}'")),
        },
    };
    let refresh = refresh.unwrap_or(Refresh::Keep);
    match tail.as_deref() {
        Some("-h" | "--help") => Ok(Command::Help { topic: Some(HelpTopic::Set) }),
        Some("-y" | "--yes") => Ok(Command::Set { width, height, refresh, monitor, yes: true }),
        Some(other) => Err(format!("unexpected argument '{other}'")),
        None => Ok(Command::Set { width, height, refresh, monitor, yes: false }),
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

fn parse_monitor(m: &str, cmd: &str) -> Result<u32, String> {
    m.parse().map_err(|_| format!("invalid monitor id in '{cmd}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_from(
            std::iter::once("rmod".to_string()).chain(args.iter().map(|s| s.to_string())),
        )
    }

    fn set(width: u32, height: u32, refresh: Refresh, monitor: Option<u32>) -> Command {
        Command::Set { width, height, refresh, monitor, yes: false }
    }

    fn set_yes(width: u32, height: u32, refresh: Refresh, monitor: Option<u32>) -> Command {
        Command::Set { width, height, refresh, monitor, yes: true }
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
        assert_eq!(parse(&["max"]), Ok(Command::Max { monitor: None, yes: false }));
    }

    #[test]
    fn max_with_monitor() {
        assert_eq!(parse(&["max:2"]), Ok(Command::Max { monitor: Some(2), yes: false }));
    }

    #[test]
    fn max_yes_flag() {
        assert_eq!(
            parse(&["max", "-y"]),
            Ok(Command::Max { monitor: None, yes: true })
        );
    }

    #[test]
    fn max_yes_flag_with_monitor() {
        assert_eq!(
            parse(&["max:2", "-y"]),
            Ok(Command::Max { monitor: Some(2), yes: true })
        );
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
        assert_eq!(parse(&["caps"]), Ok(Command::Caps { monitor: None }));
    }

    #[test]
    fn caps_with_monitor() {
        assert_eq!(parse(&["caps:2"]), Ok(Command::Caps { monitor: Some(2) }));
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
        assert_eq!(parse(&["caps:2", "--help"]), Ok(help(Some(HelpTopic::Caps))));
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
        assert_eq!(parse(&["1920x1080@60", "-h"]), Ok(help(Some(HelpTopic::Set))));
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
            Ok(set_yes(1920, 1080, Refresh::Fixed(60), None))
        );
    }

    #[test]
    fn set_yes_flag_long() {
        assert_eq!(
            parse(&["1920x1080@60", "--yes"]),
            Ok(set_yes(1920, 1080, Refresh::Fixed(60), None))
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
            Ok(set(1920, 1080, Refresh::Fixed(144), None))
        );
    }

    #[test]
    fn set_with_monitor_suffix() {
        assert_eq!(
            parse(&["1920x1080@60:2"]),
            Ok(set(1920, 1080, Refresh::Fixed(60), Some(2)))
        );
    }

    #[test]
    fn set_without_refresh_keeps_current() {
        assert_eq!(
            parse(&["1920x1080"]),
            Ok(set(1920, 1080, Refresh::Keep, None))
        );
    }

    #[test]
    fn set_with_max_refresh() {
        assert_eq!(
            parse(&["1920x1080@max"]),
            Ok(set(1920, 1080, Refresh::Max, None))
        );
    }

    #[test]
    fn profile_resolves_to_resolution() {
        assert_eq!(parse(&["4k"]), Ok(set(3840, 2160, Refresh::Keep, None)));
    }

    #[test]
    fn profile_with_fixed_refresh() {
        assert_eq!(
            parse(&["720@60"]),
            Ok(set(1280, 720, Refresh::Fixed(60), None))
        );
    }

    #[test]
    fn profile_with_max_refresh_and_monitor() {
        assert_eq!(
            parse(&["720@max:2"]),
            Ok(set(1280, 720, Refresh::Max, Some(2)))
        );
    }

    #[test]
    fn profile_with_monitor_suffix() {
        assert_eq!(parse(&["4k:2"]), Ok(set(3840, 2160, Refresh::Keep, Some(2))));
    }

    #[test]
    fn all_profiles_resolve_to_their_resolution() {
        for (name, width, height) in PROFILES {
            assert_eq!(
                parse(&[name]),
                Ok(set(*width, *height, Refresh::Keep, None)),
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
            Ok(set(1920, 1080, Refresh::Fixed(0), None))
        );
    }

    #[test]
    fn monitor_with_leading_zeros() {
        assert_eq!(
            parse(&["1920x1080@60:02"]),
            Ok(set(1920, 1080, Refresh::Fixed(60), Some(2)))
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
}