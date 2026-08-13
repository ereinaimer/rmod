use std::env;

#[derive(Debug, PartialEq)]
pub enum Refresh {
    Keep,
    Max,
    Fixed(u32),
}

#[derive(Debug, PartialEq)]
pub enum HelpTopic {
    List,
    Max,
    Caps,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    List,
    Max { monitor: Option<u32> },
    Caps { monitor: Option<u32> },
    Set { width: u32, height: u32, refresh: Refresh, monitor: Option<u32> },
    Help { topic: Option<HelpTopic> },
    Version,
}

pub(crate) const PROFILES: &[(&str, u32, u32)] = &[
    ("720", 1280, 720),
    ("1080", 1920, 1080),
    ("1440", 2560, 1440),
    ("4k", 3840, 2160),
    ("8k", 7680, 4320),
];

pub fn parse() -> Result<Command, String> {
    parse_from(env::args())
}

pub fn parse_from<I: Iterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut args = args.skip(1);
    let Some(cmd) = args.next() else {
        return Ok(Command::Help { topic: None });
    };
    match cmd.as_str() {
        "-h" | "--help" => Ok(Command::Help { topic: None }),
        "-V" | "--version" => Ok(Command::Version),
        "ls" => parse_tail("ls", args.next().as_deref(), Command::List, HelpTopic::List),
        "max" => parse_tail("max", args.next().as_deref(), Command::Max { monitor: None }, HelpTopic::Max),
        "caps" => parse_tail("caps", args.next().as_deref(), Command::Caps { monitor: None }, HelpTopic::Caps),
        _ if cmd.starts_with("max:") => {
            let monitor = parse_monitor(&cmd[4..], &cmd)?;
            Ok(Command::Max { monitor: Some(monitor) })
        }
        _ if cmd.starts_with("caps:") => {
            let monitor = parse_monitor(&cmd[5..], &cmd)?;
            Ok(Command::Caps { monitor: Some(monitor) })
        }
        _ => parse_set(&cmd),
    }
}

fn parse_tail(
    name: &str,
    tail: Option<&str>,
    cmd: Command,
    topic: HelpTopic,
) -> Result<Command, String> {
    match tail {
        Some("-h" | "--help") => Ok(Command::Help { topic: Some(topic) }),
        Some(other) => Err(format!("unknown argument '{other}' for '{name}'")),
        None => Ok(cmd),
    }
}

fn parse_set(cmd: &str) -> Result<Command, String> {
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
    Ok(Command::Set {
        width,
        height,
        refresh: refresh.unwrap_or(Refresh::Keep),
        monitor,
    })
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
        Command::Set { width, height, refresh, monitor }
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
    fn max_command() {
        assert_eq!(parse(&["max"]), Ok(Command::Max { monitor: None }));
    }

    #[test]
    fn max_with_monitor() {
        assert_eq!(parse(&["max:2"]), Ok(Command::Max { monitor: Some(2) }));
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
}