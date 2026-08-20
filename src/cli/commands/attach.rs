//! `attach` and `detach` commands: re-attach or detach monitors.
//!
//! [`run_attach`] applies the action to the targeted display(s), reporting
//! the outcome and running the shared keep-or-revert confirmation flow.

use crate::cli::parser::parse_monitor_target;
use crate::cli::{Command, HelpTopic, MonitorTarget};
use crate::sys::windows::{self, AttachAction, AttachOutcome};

use super::{
    confirm_or_revert_attach, confirm_or_revert_attach_all, describe_attach, resolve_target,
    resolve_target_all,
};

/// Runs an attach/detach action against the targeted display(s).
pub(super) fn run_attach(action: AttachAction, monitor: MonitorTarget, yes: bool) -> i32 {
    match monitor {
        MonitorTarget::Id(_) | MonitorTarget::Primary | MonitorTarget::Index(_) => {
            let monitor_idx = match action {
                AttachAction::Enable => resolve_target_all(&monitor),
                AttachAction::Disable => resolve_target(&monitor),
            };
            let monitor_idx = match monitor_idx {
                Ok(idx) => idx,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let outcome = if action == AttachAction::Disable {
                windows::disable(monitor_idx)
            } else {
                windows::enable(monitor_idx)
            };
            report_single(outcome, yes)
        }
        MonitorTarget::All => report_all(action, yes),
    }
}

/// Applies the attach action to every display, collecting applied changes
/// for the shared confirmation flow.
fn report_all(action: AttachAction, yes: bool) -> i32 {
    let count = match action {
        AttachAction::Disable => windows::enumerate_devices().len(),
        AttachAction::Enable => windows::enumerate_all_devices().len(),
    };
    let mut applied = Vec::new();
    let mut any_error = false;
    for monitor in 1..=count as u32 {
        if action == AttachAction::Disable {
            match windows::get_current_mode(monitor) {
                Ok(mode) if mode.is_primary => {
                    println!(
                        "skipped {} [:{number}], the primary display cannot be detached",
                        mode.name,
                        number = mode.number
                    );
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("error: {e}");
                    any_error = true;
                    continue;
                }
            }
        }
        let outcome = if action == AttachAction::Disable {
            windows::disable(Some(monitor))
        } else {
            windows::enable(Some(monitor))
        };
        match outcome {
            Ok(AttachOutcome::Unchanged(change)) => {
                println!("{}", describe_attach(&change));
            }
            Ok(AttachOutcome::Applied(change)) => {
                println!("{}", describe_attach(&change));
                applied.push(change);
            }
            Err(e) => {
                eprintln!("error: {e}");
                any_error = true;
            }
        }
    }
    if any_error {
        2
    } else {
        confirm_or_revert_attach_all(applied, yes)
    }
}

/// Reports a single-display attach outcome and runs the confirmation flow
/// when the change was applied.
fn report_single(outcome: Result<AttachOutcome, String>, yes: bool) -> i32 {
    match outcome {
        Ok(AttachOutcome::Unchanged(change)) => {
            println!("{}", describe_attach(&change));
            0
        }
        Ok(AttachOutcome::Applied(change)) => {
            println!("{}", describe_attach(&change));
            confirm_or_revert_attach(change, yes)
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Parses `rmod attach [OPTIONS]`: `-m` is required, `-y` is allowed.
pub(crate) fn parse_attach(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    parse_attach_shared(args, name, false)
}

/// Parses `rmod detach [OPTIONS]`: `-m` is required, `-y` is allowed.
pub(crate) fn parse_detach(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    parse_attach_shared(args, name, true)
}

/// Shared body of [`parse_attach`] and [`parse_detach`]. `name` is the
/// command word embedded in error messages (`attach`/`detach` at root,
/// `monitor attach`/`monitor detach` through the old shim).
fn parse_attach_shared(
    args: &[impl AsRef<str>],
    name: &str,
    detach: bool,
) -> Result<Command, String> {
    let mut monitor = MonitorTarget::Primary;
    let mut monitor_explicit = false;
    let mut yes = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(if detach {
                        HelpTopic::Detach
                    } else {
                        HelpTopic::Attach
                    }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                monitor_explicit = true;
                i += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use --monitor or --yes"
                ));
            }
        }
    }
    if !monitor_explicit {
        return Err(format!(
            "{name} needs -m, --monitor. a monitor ID or all\ne.g. rmod {name} -m a1b2c3d4"
        ));
    }
    Ok(if detach {
        Command::Detach { monitor, yes }
    } else {
        Command::Attach { monitor, yes }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERIAL_A: &str = "ABC12345678";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
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
        for args in [
            &["attach", "-m", SERIAL_A, "-y"][..],
            &["attach", "-y", "-m", SERIAL_A][..],
            &["attach", "-m", "all", "-y"][..],
        ] {
            let expected = Ok(Command::Attach {
                monitor: if args.contains(&"all") {
                    MonitorTarget::All
                } else {
                    MonitorTarget::Id(SERIAL_A.to_string())
                },
                yes: true,
            });
            assert_eq!(parse(args), expected, "args: {:?}", args);
        }
    }

    #[test]
    fn detach_with_monitor() {
        assert_eq!(
            parse(&["detach", "-m", SERIAL_A, "-y"]),
            Ok(Command::Detach {
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                yes: true
            })
        );
        assert_eq!(
            parse(&["detach", "-m", "2"]),
            Ok(Command::Detach {
                monitor: MonitorTarget::Index(2),
                yes: false
            })
        );
    }

    #[test]
    fn attach_any_string_is_id() {
        assert_eq!(
            parse(&["detach", "-m", "x"]),
            Ok(Command::Detach {
                monitor: MonitorTarget::Id("x".to_string()),
                yes: false
            })
        );
        assert!(parse(&["detach", "-m", "0"]).is_err());
    }

    #[test]
    fn attach_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["detach", "-m"]),
            Err("-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn attach_unknown_argument_is_error() {
        assert_eq!(
            parse(&["detach", "foo"]),
            Err("unexpected argument foo for detach. use --monitor or --yes".to_string())
        );
    }

    #[test]
    fn attach_help_flag() {
        assert_eq!(
            parse(&["attach", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Attach)
            })
        );
        assert_eq!(
            parse(&["detach", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Detach)
            })
        );
    }

    #[test]
    fn attach_version_flag() {
        assert_eq!(parse(&["attach", "--version"]), Ok(Command::Version));
        assert_eq!(
            parse(&["detach", "-m", "2", "--version"]),
            Ok(Command::Version)
        );
    }

    #[test]
    fn attach_long_form_matches_short_form() {
        let expected = Ok(Command::Attach {
            monitor: MonitorTarget::Index(2),
            yes: true,
        });
        assert_eq!(parse(&["attach", "-m", "2", "-y"]), expected);
        assert_eq!(parse(&["attach", "--monitor", "2", "--yes"]), expected);
    }

    #[test]
    fn detach_flag_like_monitor_value_is_error() {
        assert_eq!(
            parse(&["detach", "-m", "-y"]),
            Err("-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4".to_string())
        );
    }
}
