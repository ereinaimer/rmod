//! `extend` command: restore the extended desktop, auto-arranging monitors
//! left-to-right by monitor number.

use crate::cli::{Command, HelpTopic};
use crate::sys::windows;
use crate::sys::windows::{Direction, PlacementOutcome, apply_placement};

use super::flow::confirm_or_revert_placements;

/// Extend: restore extended desktop, auto-arrange left-to-right by monitor
/// number. Real placement changes ask a keep-or-revert question; `yes`
/// skips the prompt.
pub(super) fn run_extend(yes: bool) -> i32 {
    match windows::list_detailed() {
        Ok(mut monitors) => {
            if monitors.len() <= 1 {
                println!("already extended (only one monitor)");
                return 0;
            }

            // Sort by monitor number
            monitors.sort_by_key(|m| m.number);

            // Auto-arrange left-to-right
            let mut applied = Vec::new();

            for monitor in &monitors {
                if monitor.number == 1 {
                    // First monitor at origin
                    if monitor.x != 0 || monitor.y != 0 {
                        match apply_placement(monitor.number, Direction::Left, 0) {
                            Ok(PlacementOutcome::Unchanged { display, .. }) => {
                                println!("{} already at origin", display);
                            }
                            Ok(PlacementOutcome::Applied(change)) => {
                                println!("placed {} at origin", change.display);
                                applied.push(change);
                            }
                            Err(e) => {
                                eprintln!("error: {e}");
                                return 2;
                            }
                        }
                    }
                } else {
                    // Place to the right of previous monitor
                    let prev_num = monitor.number - 1;
                    match apply_placement(monitor.number, Direction::Right, prev_num) {
                        Ok(PlacementOutcome::Unchanged {
                            display,
                            reference_display,
                        }) => {
                            println!("{} already right of {}", display, reference_display);
                        }
                        Ok(PlacementOutcome::Applied(change)) => {
                            println!(
                                "placed {} right of {}",
                                change.display, change.reference_display
                            );
                            applied.push(change);
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            return 2;
                        }
                    }
                }
            }

            if applied.is_empty() {
                println!("already extended");
                0
            } else {
                confirm_or_revert_placements(applied, yes)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Parses `rmod extend`: only `-y` is accepted; `-m` and any other token
/// are rejected. `name` is the command word embedded in error messages
/// (`extend` at root, `view` through the old shim).
pub(crate) fn parse_extend(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
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
                    topic: Some(HelpTopic::Extend),
                });
            }
            "--version" => return Ok(Command::Version),
            other => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use -y or --help"
                ));
            }
        }
    }
    Ok(Command::Extend { yes })
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
    fn extend_command() {
        assert_eq!(parse(&["extend"]), Ok(Command::Extend { yes: false }));
        assert_eq!(parse(&["extend", "-y"]), Ok(Command::Extend { yes: true }));
    }

    #[test]
    fn extend_rejects_monitor_flag() {
        assert_eq!(
            parse(&["extend", "-m", "2"]),
            Err("unexpected argument -m for extend. use -y or --help".to_string())
        );
    }

    #[test]
    fn extend_rejects_other_arguments() {
        assert_eq!(
            parse(&["extend", "foo"]),
            Err("unexpected argument foo for extend. use -y or --help".to_string())
        );
    }

    #[test]
    fn extend_help_flag() {
        assert_eq!(
            parse(&["extend", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Extend)
            })
        );
    }

    #[test]
    fn extend_version_flag() {
        assert_eq!(parse(&["extend", "--version"]), Ok(Command::Version));
    }
}
