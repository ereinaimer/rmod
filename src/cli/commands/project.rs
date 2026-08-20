//! `project` command: move the desktop to the external monitor (second
//! screen only).

use crate::cli::{Command, HelpTopic};
use crate::sys::windows;

use super::confirm_or_revert_project;

/// Project: move desktop to external monitor (Second screen only).
pub(super) fn run_project(yes: bool) -> i32 {
    let monitors = match windows::list_detailed() {
        Ok(monitors) => monitors,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    // Find primary monitor (laptop)
    let primary = match monitors.iter().find(|m| m.is_primary) {
        Some(p) => p,
        None => {
            eprintln!("error: no primary monitor found");
            return 2;
        }
    };

    // Find external monitor(s)
    let externals: Vec<_> = monitors.iter().filter(|m| !m.is_primary).collect();
    if externals.is_empty() {
        eprintln!("error: no external monitor to enable");
        return 2;
    }

    // Pick the best external monitor (highest resolution)
    let external = externals.iter().max_by_key(|m| m.width * m.height).unwrap();

    // Promote external to primary (move to 0,0)
    // Use device names from the already-enumerated monitors instead of re-enumerating
    let names: Vec<String> = monitors.iter().map(|m| m.device_name.clone()).collect();
    let mut main_change = None;
    match windows::make_main(external.number, &names) {
        Ok(windows::MainOutcome::Unchanged(_)) => {
            println!("{} is already the main display", external.name);
        }
        Ok(windows::MainOutcome::Applied(change)) => {
            println!("{} is now the main display", change.display);
            main_change = Some(change);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    }

    // Now disable the laptop (which is no longer primary)
    let attach_change = match windows::disable(Some(primary.number)) {
        Ok(windows::attach::AttachOutcome::Unchanged(change)) => {
            println!("{} is already detached", change.display);
            // Nothing to revert: the promotion (if any) stays. If the user
            // later wants to undo, they can re-run `extend`.
            return 0;
        }
        Ok(windows::attach::AttachOutcome::Applied(change)) => {
            println!("detached {}", change.display);
            change
        }
        Err(e) => {
            eprintln!("error: {e}");
            // The promotion was applied but the detach failed: roll back the
            // promotion so the desktop isn't left half-migrated.
            if let Some(change) = main_change.take()
                && let Err(er) = windows::revert_main(&change)
            {
                eprintln!("error: {er}");
            }
            return 2;
        }
    };
    confirm_or_revert_project(vec![attach_change], main_change, yes)
}

/// Parses `rmod project`: only `-y` is accepted; `-m` and any other token
/// are rejected. `name` is the command word embedded in error messages
/// (`project` at root, `view` through the old shim).
pub(crate) fn parse_project(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
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
                    topic: Some(HelpTopic::Project),
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
    Ok(Command::Project { yes })
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
    fn project_command() {
        assert_eq!(parse(&["project"]), Ok(Command::Project { yes: false }));
        assert_eq!(
            parse(&["project", "-y"]),
            Ok(Command::Project { yes: true })
        );
    }

    #[test]
    fn project_rejects_monitor_flag() {
        assert_eq!(
            parse(&["project", "-m", "2"]),
            Err("unexpected argument -m for project. use -y or --help".to_string())
        );
    }

    #[test]
    fn project_rejects_other_arguments() {
        assert_eq!(
            parse(&["project", "foo"]),
            Err("unexpected argument foo for project. use -y or --help".to_string())
        );
    }

    #[test]
    fn project_help_flag() {
        assert_eq!(
            parse(&["project", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Project)
            })
        );
    }

    #[test]
    fn project_version_flag() {
        assert_eq!(parse(&["project", "--version"]), Ok(Command::Version));
    }
}
