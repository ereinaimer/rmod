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
    Brightness {
        /// The value parsed so far; the page itself is static.
        value: BrightnessValue,
        /// The backend parsed so far, or `None`.
        via: Option<BrightnessBackend>,
    },
    Contrast {
        /// The value parsed so far; the page itself is static.
        value: u32,
        /// The backend parsed so far, or `None`.
        via: Option<ContrastBackend>,
        /// Whether the reset keyword was parsed.
        reset: bool,
    },
    Attach,
    Detach,
    Sleep,
    Wake,
    Mirror,
    Extend,
    Project,
    Single {
        /// The target parsed so far; the page itself is static.
        monitor: MonitorTarget,
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
        all: bool,
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
    Temp {
        action: TempAction,
        monitor: MonitorTarget,
    },
    Brightness {
        /// Backlight level 0-100, or a composite mode: min, max, or boost.
        value: BrightnessValue,
        /// Forced backend, or `None` for auto-detect.
        via: Option<BrightnessBackend>,
        /// The display(s) to target (default: primary).
        monitor: MonitorTarget,
    },
    Contrast {
        /// Contrast level 0-130, 100 = neutral.
        value: u32,
        /// Forced backend, or `None` for auto-detect.
        via: Option<ContrastBackend>,
        /// The display(s) to target (default: primary).
        monitor: MonitorTarget,
    },
    ContrastReset {
        /// The display(s) to target (default: primary).
        monitor: MonitorTarget,
    },
    Attach {
        /// The display(s) to re-attach (required).
        monitor: MonitorTarget,
        /// Skip the confirmation prompt.
        yes: bool,
    },
    Detach {
        /// The display(s) to detach (required).
        monitor: MonitorTarget,
        /// Skip the confirmation prompt.
        yes: bool,
    },
    Sleep,
    Wake,
    Mirror {
        /// Skip the confirmation prompt.
        yes: bool,
    },
    Extend {
        /// Skip the confirmation prompt.
        yes: bool,
    },
    Project {
        /// Skip the confirmation prompt.
        yes: bool,
    },
    Single {
        /// The monitor to keep (default: primary).
        monitor: MonitorTarget,
        /// Skip the confirmation prompt.
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
        "--version" | "-V" => Ok(Command::Version),
        "ls" | "list" => crate::cli::commands::ls::parse_ls(cmd_str, args),
        "layout" => crate::cli::commands::layout::parse_layout(args),
        "set" => crate::cli::commands::set::parse_set(args),
        "temp" => crate::cli::commands::temp::parse_temp(args),
        "completions" => crate::cli::commands::completions::parse_completions("completions", args),
        "brightness" => crate::cli::commands::brightness::parse_brightness(args, "brightness"),
        "contrast" => crate::cli::commands::contrast::parse_contrast(args, "contrast"),
        "attach" => crate::cli::commands::attach::parse_attach(args, "attach"),
        "detach" => crate::cli::commands::attach::parse_detach(args, "detach"),
        "sleep" => crate::cli::commands::sleep::parse_sleep(args, "sleep"),
        "wake" => crate::cli::commands::sleep::parse_wake(args, "wake"),
        "mirror" => crate::cli::commands::mirror::parse_mirror(args, "mirror"),
        "extend" => crate::cli::commands::extend::parse_extend(args, "extend"),
        "project" => crate::cli::commands::project::parse_project(args, "project"),
        "single" => crate::cli::commands::single::parse_single(args, "single"),
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
        assert_eq!(parse(&["-V"]), Ok(Command::Version));
        assert_eq!(parse(&["--version"]), Ok(Command::Version));
        assert_eq!(parse(&["--version", "x"]), Ok(Command::Version));
    }

    #[test]
    fn ls_command() {
        assert_eq!(
            parse(&["ls"]),
            Ok(Command::List {
                short: false,
                all: false
            })
        );
    }

    #[test]
    fn list_command() {
        assert_eq!(
            parse(&["list"]),
            Ok(Command::List {
                short: false,
                all: false
            })
        );
    }

    #[test]
    fn main_command_now_errors_with_generic_message() {
        assert_eq!(
            parse(&["main"]),
            Err("unknown command main. run rmod --help to list commands".to_string())
        );
        assert_eq!(
            parse(&["main", "2", "-y"]),
            Err("unknown command main. run rmod --help to list commands".to_string())
        );
    }

    #[test]
    fn legacy_commands_error_with_generic_unknown_command() {
        for args in [
            &["monitor"][..],
            &["monitor", "brightness", "60"][..],
            &["monitor", "-m", "2", "detach"][..],
            &["view"][..],
            &["view", "mirror"][..],
            &["main"][..],
            &["disable", "-m", "2"][..],
            &["off"][..],
            &["enable", "-m", "2"][..],
            &["on"][..],
        ] {
            let word = args[0];
            assert_eq!(
                parse(args),
                Err(format!(
                    "unknown command {word}. run rmod --help to list commands"
                )),
                "args: {:?}",
                args
            );
        }
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
    fn brightness_primary_default() {
        assert_eq!(
            parse(&["brightness", "60"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(60),
                via: None,
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn brightness_with_monitor_and_backend() {
        assert_eq!(
            parse(&["brightness", "40", "-m", "2", "--via", "ddc"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(40),
                via: Some(BrightnessBackend::Ddc),
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["brightness", "min", "-m", "all"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Min,
                via: None,
                monitor: MonitorTarget::All,
            })
        );
    }

    #[test]
    fn brightness_out_of_range_is_error() {
        assert_eq!(
            parse(&["brightness", "150"]),
            Err("invalid brightness 150. use a number between 0 and 100".to_string())
        );
    }

    #[test]
    fn brightness_missing_value_is_error() {
        assert_eq!(
            parse(&["brightness"]),
            Err(
                "brightness needs a value. a number between 0 and 100\ne.g. rmod brightness 60"
                    .to_string()
            )
        );
    }

    #[test]
    fn brightness_unknown_backend_is_error() {
        assert_eq!(
            parse(&["brightness", "60", "--via", "gamma2"]),
            Err("unknown backend gamma2. use ddc, slider, or gamma".to_string())
        );
    }

    #[test]
    fn brightness_rejects_yes_flag() {
        assert_eq!(
            parse(&["brightness", "60", "-y"]),
            Err(
                "-y, --yes is not valid for brightness. brightness does not prompt for confirmation"
                    .to_string()
            )
        );
    }

    #[test]
    fn contrast_primary_default() {
        assert_eq!(
            parse(&["contrast", "60"]),
            Ok(Command::Contrast {
                value: 60,
                via: None,
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn contrast_with_monitor_and_backend() {
        assert_eq!(
            parse(&["contrast", "40", "-m", "all", "--via", "gamma"]),
            Ok(Command::Contrast {
                value: 40,
                via: Some(ContrastBackend::Gamma),
                monitor: MonitorTarget::All,
            })
        );
    }

    #[test]
    fn contrast_reset_with_monitor() {
        assert_eq!(
            parse(&["contrast", "reset", "-m", "2"]),
            Ok(Command::ContrastReset {
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["contrast", "-m", "2", "reset"]),
            Ok(Command::ContrastReset {
                monitor: MonitorTarget::Index(2),
            })
        );
    }

    #[test]
    fn contrast_out_of_range_is_error() {
        assert_eq!(
            parse(&["contrast", "131"]),
            Err("invalid contrast 131. use a number between 0 and 130".to_string())
        );
    }

    #[test]
    fn contrast_missing_value_is_error() {
        assert_eq!(
            parse(&["contrast"]),
            Err(
                "contrast needs a value. a number between 0 and 130\ne.g. rmod contrast 60"
                    .to_string()
            )
        );
    }

    #[test]
    fn contrast_rejects_yes_flag() {
        assert_eq!(
            parse(&["contrast", "60", "-y"]),
            Err(
                "-y, --yes is not valid for contrast. contrast does not prompt for confirmation"
                    .to_string()
            )
        );
    }

    #[test]
    fn attach_requires_monitor_flag() {
        assert_eq!(
            parse(&["attach"]),
            Err(
                "attach needs -m, --monitor. a monitor ID or all\ne.g. rmod attach -m a1b2c3d4"
                    .to_string()
            )
        );
        assert_eq!(
            parse(&["attach", "2"]),
            Err("unexpected argument 2 for attach. use --monitor or --yes".to_string())
        );
    }

    #[test]
    fn detach_requires_monitor_flag() {
        assert_eq!(
            parse(&["detach"]),
            Err(
                "detach needs -m, --monitor. a monitor ID or all\ne.g. rmod detach -m a1b2c3d4"
                    .to_string()
            )
        );
    }

    #[test]
    fn attach_with_monitor_and_yes() {
        assert_eq!(
            parse(&["attach", "-m", "2", "-y"]),
            Ok(Command::Attach {
                monitor: MonitorTarget::Index(2),
                yes: true,
            })
        );
        assert_eq!(
            parse(&["detach", "-m", "all"]),
            Ok(Command::Detach {
                monitor: MonitorTarget::All,
                yes: false,
            })
        );
    }

    #[test]
    fn attach_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["detach", "-m"]),
            Err("-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn sleep_command() {
        assert_eq!(parse(&["sleep"]), Ok(Command::Sleep));
    }

    #[test]
    fn wake_command() {
        assert_eq!(parse(&["wake"]), Ok(Command::Wake));
    }

    #[test]
    fn sleep_rejects_monitor_flag() {
        assert_eq!(
            parse(&["sleep", "-m", "2"]),
            Err("-m, --monitor is not valid for sleep. sleep applies to all monitors".to_string())
        );
    }

    #[test]
    fn sleep_rejects_yes_flag() {
        assert_eq!(
            parse(&["wake", "-y"]),
            Err("-y, --yes is not valid for wake. wake applies to all monitors".to_string())
        );
    }

    #[test]
    fn mirror_command() {
        assert_eq!(parse(&["mirror"]), Ok(Command::Mirror { yes: false }));
        assert_eq!(parse(&["mirror", "-y"]), Ok(Command::Mirror { yes: true }));
    }

    #[test]
    fn extend_command() {
        assert_eq!(parse(&["extend"]), Ok(Command::Extend { yes: false }));
    }

    #[test]
    fn project_command() {
        assert_eq!(
            parse(&["project", "-y"]),
            Ok(Command::Project { yes: true })
        );
    }

    #[test]
    fn mode_commands_reject_monitor_flag() {
        for args in [
            &["mirror", "-m", "2"][..],
            &["extend", "-m", "2"][..],
            &["project", "--monitor", "2"][..],
        ] {
            assert_eq!(
                parse(args),
                Err(format!(
                    "unexpected argument {} for {}. use -y or --help",
                    args[1], args[0]
                )),
                "args: {:?}",
                args
            );
        }
    }

    #[test]
    fn single_with_monitor() {
        assert_eq!(
            parse(&["single", "-m", "2"]),
            Ok(Command::Single {
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["single", "-y"]),
            Ok(Command::Single {
                monitor: MonitorTarget::Primary,
                yes: true,
            })
        );
    }

    #[test]
    fn single_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["single", "-m"]),
            Err("-m, --monitor needs a value. a monitor ID or number\ne.g. -m 2".to_string())
        );
    }

    #[test]
    fn single_unexpected_argument_is_error() {
        assert_eq!(
            parse(&["single", "foo"]),
            Err("unexpected argument foo for single. use -m, --monitor, -y, or --help".to_string())
        );
    }

    #[test]
    fn all_parser_errors_are_actionable() {
        // add a row when you add an error message
        let cases: &[(&[&str], &str)] = &[
            (&["frobnicate"], "parse_from unknown command"),
            (&["main"], "parse_from unknown command"),
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
                "parse_from unknown command",
            ),
            (&["brightness"], "parse_brightness missing value"),
            (&["brightness", "150"], "parse_brightness out of range"),
            (
                &["brightness", "60", "foo"],
                "parse_brightness unexpected argument",
            ),
            (
                &["brightness", "60", "-m"],
                "parse_brightness -m missing value",
            ),
            (
                &["brightness", "60", "-v"],
                "parse_brightness -v missing value",
            ),
            (
                &["brightness", "60", "-v", "x"],
                "parse_brightness unknown backend",
            ),
            (
                &["brightness", "min", "-v", "ddc"],
                "parse_brightness keyword plus backend",
            ),
            (&["contrast"], "parse_contrast missing value"),
            (&["contrast", "131"], "parse_contrast out of range"),
            (
                &["contrast", "60", "foo"],
                "parse_contrast unexpected argument",
            ),
            (
                &["contrast", "reset", "-v", "ddc"],
                "parse_contrast via with reset",
            ),
            (&["attach"], "parse_attach missing monitor"),
            (&["detach"], "parse_detach missing monitor"),
            (&["attach", "foo"], "parse_attach unexpected argument"),
            (&["detach", "-m"], "parse_detach -m missing value"),
            (&["sleep", "foo"], "parse_sleep unexpected argument"),
            (&["mirror", "-m", "2"], "parse_mirror -m rejected"),
            (&["extend", "foo"], "parse_extend unexpected argument"),
            (&["project", "-m", "2"], "parse_project -m rejected"),
            (&["single", "-m"], "parse_single -m missing value"),
            (&["single", "foo"], "parse_single unexpected argument"),
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
