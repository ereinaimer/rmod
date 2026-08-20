//! `single` command: enable only one monitor, disabling all others.

use crate::cli::parser::parse_monitor_target;
use crate::cli::{Command, HelpTopic, MonitorTarget};
use crate::sys::windows;
use crate::sys::windows::attach::AttachOutcome;

use super::confirm_or_revert_attach_all;

/// Single: enable only monitor N, disable all others.
pub(super) fn run_single(monitor_target: MonitorTarget, yes: bool) -> i32 {
    let monitor_num = match resolve_view_target(&monitor_target) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };

    match windows::list_detailed() {
        Ok(monitors) => {
            if !monitors.iter().any(|m| m.number == monitor_num) {
                eprintln!("error: monitor {} not found", monitor_num);
                return 2;
            }

            let mut changes = Vec::new();

            for monitor in &monitors {
                if monitor.number == monitor_num {
                    // Enable target
                    if monitor.width == 0 {
                        match windows::enable(Some(monitor.number)) {
                            Ok(AttachOutcome::Unchanged(change)) => {
                                println!("{} is already attached", change.display);
                            }
                            Ok(AttachOutcome::Applied(change)) => {
                                println!("attached {}", change.display);
                                changes.push(change);
                            }
                            Err(e) => {
                                eprintln!("error: {e}");
                                return 2;
                            }
                        }
                    }
                } else if !monitor.is_primary {
                    // Disable others (but not primary)
                    if monitor.width > 0 {
                        match windows::disable(Some(monitor.number)) {
                            Ok(AttachOutcome::Unchanged(change)) => {
                                println!("{} is already detached", change.display);
                            }
                            Ok(AttachOutcome::Applied(change)) => {
                                println!("detached {}", change.display);
                                changes.push(change);
                            }
                            Err(e) => {
                                eprintln!("error: {e}");
                                return 2;
                            }
                        }
                    }
                }
            }

            if changes.is_empty() {
                println!("already in single mode");
                0
            } else {
                confirm_or_revert_attach_all(changes, yes)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Resolves a single-mode target to a monitor number.
fn resolve_view_target(target: &MonitorTarget) -> Result<u32, String> {
    match target {
        MonitorTarget::Primary => crate::sys::windows::get_primary_mode().map(|m| m.number),
        MonitorTarget::Index(n) => Ok(*n),
        MonitorTarget::Id(id) => crate::sys::windows::resolve_by_id(id).ok_or_else(|| {
            format!(
                "monitor with id '{}' not found. connected: {}",
                id,
                crate::sys::windows::connected_displays_list()
            )
        }),
        MonitorTarget::All => Err("single mode requires a specific monitor, not 'all'".to_string()),
    }
}

/// Parses `rmod single [OPTIONS]`: `-m` selects the monitor to keep
/// (default: primary), `-y` is accepted. `name` is the command word
/// embedded in error messages (`single` at root, `view single` through the
/// old shim).
pub(crate) fn parse_single(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    let mut monitor = MonitorTarget::Primary;
    let mut yes = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Single { monitor }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or number\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or number\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use -m, --monitor, -y, or --help"
                ));
            }
        }
    }
    Ok(Command::Single { monitor, yes })
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
    fn single_defaults_to_primary() {
        assert_eq!(
            parse(&["single"]),
            Ok(Command::Single {
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
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
            parse(&["single", "--monitor", "2", "-y"]),
            Ok(Command::Single {
                monitor: MonitorTarget::Index(2),
                yes: true,
            })
        );
    }

    #[test]
    fn single_with_yes() {
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
    fn single_flag_like_monitor_value_is_error() {
        assert_eq!(
            parse(&["single", "-m", "-y"]),
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
    fn single_help_flag() {
        assert_eq!(
            parse(&["single", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Single {
                    monitor: MonitorTarget::Primary
                })
            })
        );
        assert_eq!(
            parse(&["single", "-m", "2", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Single {
                    monitor: MonitorTarget::Index(2)
                })
            })
        );
    }

    #[test]
    fn single_version_flag() {
        assert_eq!(parse(&["single", "--version"]), Ok(Command::Version));
        assert_eq!(
            parse(&["single", "-m", "2", "--version"]),
            Ok(Command::Version)
        );
    }

    #[test]
    fn duplicate_monitor_flag_last_wins() {
        assert_eq!(
            parse(&["single", "-m", "2", "-m", "3"]),
            Ok(Command::Single {
                monitor: MonitorTarget::Index(3),
                yes: false,
            })
        );
    }
}
