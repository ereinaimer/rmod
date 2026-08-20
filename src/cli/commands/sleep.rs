//! `sleep` and `wake` commands: global display power broadcasts.
//!
//! Sleeping and waking are global broadcasts with no confirmation and no
//! revert, so the parsers accept only `-h`/`--help`/`--version`.

use crate::cli::{Command, HelpTopic};
use crate::sys::windows;

/// Puts every monitor to sleep, printing one line per display.
pub(super) fn run_sleep() -> i32 {
    match windows::sleep_monitor() {
        Ok(labels) => {
            for label in labels {
                println!("slept {label}");
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Wakes every monitor, printing one line per display.
pub(super) fn run_wake() -> i32 {
    match windows::wake_monitor() {
        Ok(labels) => {
            for label in labels {
                println!("woke {label}");
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Parses `rmod sleep`: no flags other than `-h`/`--help`/`--version`.
pub(crate) fn parse_sleep(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    parse_sleep_shared(args, name, "sleep", false)
}

/// Parses `rmod wake`: no flags other than `-h`/`--help`/`--version`.
pub(crate) fn parse_wake(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    parse_sleep_shared(args, name, "wake", true)
}

/// Shared body of [`parse_sleep`] and [`parse_wake`]; `-m`/`-y` and any
/// other token are rejected. `name` is the command word embedded in error
/// messages (`sleep`/`wake` at root, `monitor sleep`/`monitor wake` through
/// the old shim).
fn parse_sleep_shared(
    args: &[impl AsRef<str>],
    name: &str,
    verb: &str,
    wake: bool,
) -> Result<Command, String> {
    if let Some(arg) = args.get(1) {
        match arg.as_ref() {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(if wake {
                        HelpTopic::Wake
                    } else {
                        HelpTopic::Sleep
                    }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                return Err(format!(
                    "-m, --monitor is not valid for {name}. {verb} applies to all monitors"
                ));
            }
            "-y" | "--yes" => {
                return Err(format!(
                    "-y, --yes is not valid for {name}. {verb} applies to all monitors"
                ));
            }
            other => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use --help"
                ));
            }
        }
    }
    Ok(if wake { Command::Wake } else { Command::Sleep })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
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
        assert_eq!(
            parse(&["wake", "-m", "2"]),
            Err("-m, --monitor is not valid for wake. wake applies to all monitors".to_string())
        );
    }

    #[test]
    fn sleep_rejects_yes_flag() {
        assert_eq!(
            parse(&["sleep", "-y"]),
            Err("-y, --yes is not valid for sleep. sleep applies to all monitors".to_string())
        );
        assert_eq!(
            parse(&["wake", "--yes"]),
            Err("-y, --yes is not valid for wake. wake applies to all monitors".to_string())
        );
    }

    #[test]
    fn sleep_rejects_other_arguments() {
        assert_eq!(
            parse(&["sleep", "foo"]),
            Err("unexpected argument foo for sleep. use --help".to_string())
        );
        assert_eq!(
            parse(&["wake", "1"]),
            Err("unexpected argument 1 for wake. use --help".to_string())
        );
    }

    #[test]
    fn sleep_help_flag() {
        assert_eq!(
            parse(&["sleep", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Sleep)
            })
        );
        assert_eq!(
            parse(&["wake", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Wake)
            })
        );
    }

    #[test]
    fn sleep_version_flag() {
        assert_eq!(parse(&["sleep", "--version"]), Ok(Command::Version));
        assert_eq!(parse(&["wake", "--version"]), Ok(Command::Version));
    }
}
