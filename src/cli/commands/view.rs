//! `view` command: switches between mirror, extend, project, and single display modes.

use crate::cli::parser::{Command, HelpTopic, MonitorTarget, ViewAction};
use crate::sys::windows;
use crate::sys::windows::attach::AttachOutcome;
use crate::sys::windows::Monitor;
use crate::sys::windows::{
    Direction, PlacementOutcome, apply_placement, caps_all_modes_for_device,
};

use super::{
    confirm_or_revert_all, confirm_or_revert_attach_all, confirm_or_revert_project,
};

/// Runs a parsed view action and returns the process exit code.
pub(super) fn run_view(action: ViewAction, yes: bool) -> i32 {
    match action {
        ViewAction::Mirror => run_mirror(yes),
        ViewAction::Extend => run_extend(yes),
        ViewAction::Project => run_project(yes),
        ViewAction::Single { monitor } => run_single(monitor, yes),
    }
}

/// Mirror: clone all displays at same position (0,0) with lowest common resolution.
fn run_mirror(yes: bool) -> i32 {
    match windows::list_detailed() {
        Ok(monitors) => {
            if monitors.len() <= 1 {
                println!("already mirrored (only one monitor)");
                return 0;
            }

            // Find common resolution across all monitors
            let common_mode = find_common_mode(&monitors);
            let common_mode = match common_mode {
                Some(m) => m,
                None => {
                    eprintln!(
                        "error: no common resolution with a common refresh rate across all monitors"
                    );
                    return 2;
                }
            };

            let mut applied = Vec::new();
            for monitor in &monitors {
                match windows::set(
                    Some(monitor.number),
                    Some(common_mode.width),
                    Some(common_mode.height),
                    crate::sys::windows::apply::Refresh::Fixed(common_mode.refresh),
                    None,
                ) {
                    Ok(windows::ApplyOutcome::Unchanged(change)) => {
                        println!(
                            "{} is already at {}x{} @ {}Hz",
                            change.display,
                            change.mode.width,
                            change.mode.height,
                            change.mode.refresh
                        );
                    }
                    Ok(windows::ApplyOutcome::Applied(change)) => {
                        println!(
                            "applied {}x{} @ {}Hz to {}",
                            change.mode.width,
                            change.mode.height,
                            change.mode.refresh,
                            change.display
                        );
                        applied.push(change);
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            }

            if applied.is_empty() {
                println!("already mirrored");
                0
            } else {
                confirm_or_revert_all(applied, yes)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Extend: restore extended desktop, auto-arrange left-to-right by monitor number.
fn run_extend(_yes: bool) -> i32 {
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
                // For layout changes, we need a different revert flow
                // For now, just return success
                0
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Project: move desktop to external monitor (Second screen only).
fn run_project(yes: bool) -> i32 {
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
    let names = windows::enumerate_devices();
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
            // later wants to undo, they can re-run `view extend`.
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

/// Single: enable only monitor N, disable all others.
fn run_single(monitor_target: MonitorTarget, yes: bool) -> i32 {
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

/// Resolves a view target to a monitor number.
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

/// Finds a common resolution supported by all monitors.
fn find_common_mode(monitors: &[Monitor]) -> Option<crate::sys::windows::Mode> {
    use crate::sys::windows::Mode;

    let mut common_resolutions: Option<std::collections::HashSet<(u32, u32)>> = None;

    for monitor in monitors {
        let modes = caps_all_modes_for_device(&monitor.device_name);
        let res_set: std::collections::HashSet<(u32, u32)> =
            modes.into_iter().map(|m| (m.width, m.height)).collect();

        if let Some(ref mut common) = common_resolutions {
            common.retain(|(w, h)| res_set.contains(&(*w, *h)));
            if common.is_empty() {
                return None;
            }
        } else {
            common_resolutions = Some(res_set);
        }
    }

    // Pick the highest resolution (by pixel count)
    common_resolutions.and_then(|resolutions| {
        let best_res = resolutions.into_iter().max_by_key(|(w, h)| w * h)?;
        let (width, height) = best_res;

        // Find a common refresh rate for this resolution across all monitors
        let mut common_refresh: Option<std::collections::HashSet<u32>> = None;
        for monitor in monitors {
            let modes = caps_all_modes_for_device(&monitor.device_name);
            let refresh_set: std::collections::HashSet<u32> = modes
                .into_iter()
                .filter(|m| m.width == width && m.height == height)
                .map(|m| m.refresh)
                .collect();

            if let Some(ref mut common) = common_refresh {
                common.retain(|r| refresh_set.contains(r));
                if common.is_empty() {
                    return None;
                }
            } else {
                common_refresh = Some(refresh_set);
            }
        }

        // Pick a sensible refresh rate: prefer 60Hz if available, otherwise highest common
        common_refresh.and_then(|refreshes| {
            let refresh = if refreshes.contains(&60) {
                60
            } else {
                refreshes.into_iter().max()?
            };
            Some(Mode {
                width,
                height,
                refresh,
            })
        })
    })
}

/// Parses the `view` command.
///
/// Two phases: first scan for the subcommand (flags may precede it), then
/// validate every remaining token against the subcommand's own flag rules.
pub(crate) fn parse_view(_cmd: &str, args: &[impl AsRef<str>]) -> Result<Command, String> {
    // Phase 1: find the subcommand. Flags may appear before it.
    let mut i = 1;
    let mut yes = false;
    let subcmd_idx = loop {
        let Some(arg) = args.get(i) else {
            return Err(
                "view needs a subcommand: mirror, extend, project, or single\ne.g. rmod view mirror"
                    .to_string(),
            );
        };
        match arg.as_ref() {
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::View { action: None }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                // Skip the value too; phase 2 re-validates the flag.
                i += 2;
            }
            _ => break i,
        }
    };

    let subcmd = args[subcmd_idx].as_ref();
    let action = match subcmd {
        "mirror" => ViewAction::Mirror,
        "extend" => ViewAction::Extend,
        "project" => ViewAction::Project,
        "single" => ViewAction::Single {
            monitor: MonitorTarget::Primary,
        },
        other => {
            return Err(format!(
                "unknown view subcommand {}. use mirror, extend, project, or single",
                other
            ));
        }
    };
    let is_single = matches!(subcmd, "single");

    // Phase 2: validate every remaining token with the subcommand's rules.
    let mut monitor = MonitorTarget::Primary;
    let mut i = 1;
    while i < args.len() {
        if i == subcmd_idx {
            i += 1;
            continue;
        }
        let arg = args[i].as_ref();
        match arg {
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "--version" => return Ok(Command::Version),
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::View {
                        action: Some(action.clone()),
                    }),
                });
            }
            "-m" | "--monitor" if is_single => {
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
                monitor = crate::cli::parser::parse_monitor_target(val)?;
                i += 1;
            }
            other => {
                return Err(if is_single {
                    format!(
                        "unexpected argument {} for view single. use -m, --monitor, -y, or --help",
                        other
                    )
                } else {
                    format!("unexpected argument {} for view. use -y or --help", other)
                });
            }
        }
    }

    let action = match action {
        ViewAction::Single { .. } => ViewAction::Single { monitor },
        other => other,
    };
    Ok(Command::View { action, yes })
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
    fn flags_before_subcommand() {
        assert_eq!(
            parse(&["view", "-y", "mirror"]),
            Ok(Command::View {
                action: ViewAction::Mirror,
                yes: true
            })
        );
    }

    #[test]
    fn monitor_flag_before_subcommand() {
        assert_eq!(
            parse(&["view", "-m", "2", "single"]),
            Ok(Command::View {
                action: ViewAction::Single {
                    monitor: MonitorTarget::Index(2)
                },
                yes: false
            })
        );
    }

    #[test]
    fn flags_before_and_after_subcommand() {
        assert_eq!(
            parse(&["view", "-y", "single", "-m", "2"]),
            Ok(Command::View {
                action: ViewAction::Single {
                    monitor: MonitorTarget::Index(2)
                },
                yes: true
            })
        );
    }

    #[test]
    fn monitor_flag_before_subcommand_rejected_by_mirror() {
        assert_eq!(
            parse(&["view", "-m", "2", "mirror"]),
            Err("unexpected argument -m for view. use -y or --help".to_string())
        );
    }

    #[test]
    fn monitor_flag_after_subcommand_rejected_by_mirror() {
        assert_eq!(
            parse(&["view", "mirror", "-m", "2"]),
            Err("unexpected argument -m for view. use -y or --help".to_string())
        );
    }

    #[test]
    fn help_flags() {
        assert_eq!(
            parse(&["view", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::View { action: None })
            })
        );
        assert_eq!(
            parse(&["view", "mirror", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::View {
                    action: Some(ViewAction::Mirror)
                })
            })
        );
        assert_eq!(
            parse(&["view", "single", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::View {
                    action: Some(ViewAction::Single {
                        monitor: MonitorTarget::Primary
                    })
                })
            })
        );
    }

    #[test]
    fn version_flags() {
        assert_eq!(parse(&["view", "--version"]), Ok(Command::Version));
        assert_eq!(
            parse(&["view", "mirror", "--version"]),
            Ok(Command::Version)
        );
        assert_eq!(
            parse(&["view", "single", "-m", "2", "--version"]),
            Ok(Command::Version)
        );
    }

    #[test]
    fn no_subcommand_is_error() {
        assert_eq!(
            parse(&["view"]),
            Err(
                "view needs a subcommand: mirror, extend, project, or single\ne.g. rmod view mirror"
                    .to_string()
            )
        );
    }

    #[test]
    fn unknown_subcommand_is_error() {
        assert_eq!(
            parse(&["view", "foo"]),
            Err("unknown view subcommand foo. use mirror, extend, project, or single".to_string())
        );
    }

    #[test]
    fn unexpected_argument_is_error() {
        assert_eq!(
            parse(&["view", "mirror", "extra"]),
            Err("unexpected argument extra for view. use -y or --help".to_string())
        );
        assert_eq!(
            parse(&["view", "single", "extra"]),
            Err(
                "unexpected argument extra for view single. use -m, --monitor, -y, or --help"
                    .to_string()
            )
        );
    }

    #[test]
    fn long_form_yes_matches_short_form() {
        let expected = Ok(Command::View {
            action: ViewAction::Mirror,
            yes: true,
        });
        assert_eq!(parse(&["view", "-y", "mirror"]), expected);
        assert_eq!(parse(&["view", "--yes", "mirror"]), expected);
    }

    #[test]
    fn long_form_monitor_matches_short_form() {
        let expected = Ok(Command::View {
            action: ViewAction::Single {
                monitor: MonitorTarget::Index(2),
            },
            yes: false,
        });
        assert_eq!(parse(&["view", "-m", "2", "single"]), expected);
        assert_eq!(parse(&["view", "--monitor", "2", "single"]), expected);
    }

    #[test]
    fn single_with_monitor_and_help_pins_topic() {
        assert_eq!(
            parse(&["view", "single", "-m", "2", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::View {
                    action: Some(ViewAction::Single {
                        monitor: MonitorTarget::Primary
                    })
                })
            })
        );
    }

    #[test]
    fn monitor_flag_without_subcommand_is_error() {
        assert_eq!(
            parse(&["view", "-m", "2"]),
            Err(
                "view needs a subcommand: mirror, extend, project, or single\ne.g. rmod view mirror"
                    .to_string()
            )
        );
    }

    #[test]
    fn yes_flag_without_subcommand_is_error() {
        assert_eq!(
            parse(&["view", "-y"]),
            Err(
                "view needs a subcommand: mirror, extend, project, or single\ne.g. rmod view mirror"
                    .to_string()
            )
        );
    }

    #[test]
    fn duplicate_monitor_flag_last_wins() {
        assert_eq!(
            parse(&["view", "-m", "2", "-m", "3", "single"]),
            Ok(Command::View {
                action: ViewAction::Single {
                    monitor: MonitorTarget::Index(3)
                },
                yes: false
            })
        );
    }
}
