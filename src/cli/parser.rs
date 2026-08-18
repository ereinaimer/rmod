//! Command-line grammar: unified verb-centric syntax.
//!
//! Every command: `rmod <verb> [arguments]`
//! Monitor targeting is always a positional argument after the verb.

use std::env;

pub use crate::sys::windows::BrightnessBackend;
pub use crate::sys::windows::BrightnessValue;
pub use crate::sys::windows::ContrastBackend;
pub use crate::sys::windows::Direction;
pub use crate::sys::windows::apply::Refresh;

/// Help topics reachable via the command-specific `--help` flags.
#[derive(Debug, PartialEq)]
pub enum HelpTopic {
    List,
    Set,
    Layout,
    Temp,
    View {
        /// The action whose page to show; `None` is the top-level page.
        action: Option<ViewAction>,
    },
    #[allow(dead_code)]
    Completions,
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
        monitor: MonitorTarget,
        direction: Direction,
        reference: MonitorTarget,
    },
    Primary {
        monitor: MonitorTarget,
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
    /// Set the backlight level of a display.
    Brightness {
        /// Backlight level 0-100, or a composite mode: min, max, or boost.
        value: BrightnessValue,
        /// Forced backend, or `None` for auto-detect.
        via: Option<BrightnessBackend>,
    },
    /// `rmod monitor contrast <VALUE>` — set display contrast
    /// (0-130; 100 = neutral, above 100 overdrives the gamma ramp).
    Contrast {
        /// Contrast level 0-130, 100 = neutral.
        value: u32,
        /// Forced backend, or `None` for auto-detect.
        via: Option<ContrastBackend>,
    },
    /// `rmod monitor contrast reset` — reset contrast to defaults (DDC 100 + gamma identity).
    ContrastReset,
}

/// What the `view` command should do.
#[derive(Debug, PartialEq, Clone)]
pub enum ViewAction {
    Mirror,
    Extend,
    Project,
    Single { monitor: MonitorTarget },
}

/// What the `temp` command should do.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TempAction {
    /// Set the temperature to a clamped Kelvin value.
    Set(u32),
    /// Restore the identity ramp (6500K).
    Reset,
    /// Show the current approximate temperature.
    Show,
}

/// Every top-level command rmod accepts.
#[derive(Debug, PartialEq)]
pub enum Command {
    List {
        short: bool,
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
    Temp {
        action: TempAction,
        monitor: MonitorTarget,
    },
    View {
        action: ViewAction,
        yes: bool,
    },
    Completions {
        help: bool,
    },
    Help {
        topic: Option<HelpTopic>,
    },
    Version,
}

/// Which display(s) a command targets.
#[derive(Debug, PartialEq, Clone)]
pub enum MonitorTarget {
    Primary,
    Index(u32), // 1-based display number
    Id(String), // EDID serial or fingerprint
    All,
}

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
        "--help" | "-h" => Ok(Command::Help { topic: None }),
        "--version" => Ok(Command::Version),
        "ls" | "list" => crate::cli::commands::ls::parse_ls(cmd_str, args),
        "layout" => crate::cli::commands::layout::parse_layout(args),
        "main" => Err("unknown command main. use rmod layout -m a1b2c3d4 --primary".to_string()),
        "set" => crate::cli::commands::set::parse_set(args),
        "monitor" => crate::cli::commands::monitor::parse_monitor(args),
        "temp" => crate::cli::commands::temp::parse_temp(args),
        "view" => crate::cli::commands::view::parse_view(cmd_str, args),
        "completions" => crate::cli::commands::completions::parse_completions("completions", args),
        _ => Err(format!(
            "unknown command {}. run rmod --help to list commands",
            cmd_str
        )),
    }
}

pub(crate) fn parse_monitor_target(arg: &str) -> Result<MonitorTarget, String> {
    match arg.to_lowercase().as_str() {
        "primary" => Ok(MonitorTarget::Primary),
        "all" => Ok(MonitorTarget::All),
        _ if arg.bytes().all(|b| b.is_ascii_digit()) => {
            let n = arg.parse::<u32>().map_err(|_| {
                format!("invalid monitor target {arg}. use a monitor number or all")
            })?;
            if n == 0 {
                return Err("monitor numbers start at 1. run rmod list to see them".to_string());
            }
            Ok(MonitorTarget::Index(n))
        }
        // Treat any other string as a monitor id: the EDID serial when a
        // panel ships one, otherwise the EDID fingerprint from rmod list.
        _ => Ok(MonitorTarget::Id(arg.to_string())),
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
        assert_eq!(parse(&["-h"]), Ok(Command::Help { topic: None }));
        assert_eq!(parse(&["-h", "set"]), Ok(Command::Help { topic: None }));
        assert_eq!(parse(&["--help"]), Ok(Command::Help { topic: None }));
        assert_eq!(parse(&["--help", "set"]), Ok(Command::Help { topic: None }));
    }

    #[test]
    fn version_flags() {
        assert!(parse(&["-V"]).is_err());
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
        assert_eq!(parse(&["--version", "x"]), Ok(Command::Version));
    }

    #[test]
    fn ls_command() {
        assert_eq!(parse(&["ls"]), Ok(Command::List { short: false }));
    }

    #[test]
    fn list_command() {
        assert_eq!(parse(&["list"]), Ok(Command::List { short: false }));
    }

    #[test]
    fn main_command_now_errors_with_hint() {
        assert_eq!(
            parse(&["main"]),
            Err("unknown command main. use rmod layout -m a1b2c3d4 --primary".to_string())
        );
        assert_eq!(
            parse(&["main", "2", "-y"]),
            Err("unknown command main. use rmod layout -m a1b2c3d4 --primary".to_string())
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
    fn list_is_alias_for_ls() {
        assert_eq!(parse(&["list"]), parse(&["ls"]));
    }

    #[test]
    fn all_parser_errors_are_actionable() {
        // add a row when you add an error message
        let cases: &[(&[&str], &str)] = &[
            (&["frobnicate"], "parse_from unknown command"),
            (&["main"], "parse_from legacy 'main' command"),
            (&["list", "-m"], "parse_ls -m missing value"),
            (&["list", "-m", "-x"], "parse_ls -m flag-like value"),
            (&["list", "foo"], "parse_ls unexpected argument"),
            (&["list", "-m", "2"], "parse_ls -m rejected"),
            (&["ls", "--caps"], "parse_ls --caps rejected"),
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
            (&["temp", "bogus"], "parse_temp invalid value"),
            (&["temp", "9000"], "parse_temp out-of-range value"),
            (&["temp", "-m"], "parse_temp -m missing value"),
            (&["temp", "3000", "4000"], "parse_temp second positional"),
            (
                &["monitor", "brightness", "min", "-v", "ddc"],
                "parse_monitor_brightness keyword plus backend",
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
