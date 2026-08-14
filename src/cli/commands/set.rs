//! `set` command: applies a resolution, refresh and orientation policy to
//! the targeted display(s).
//!
//! [`run_set`] reports the outcome per display, then runs the shared
//! keep-or-revert confirmation flow for the changes it applied.

use crate::cli::{MonitorTarget, SetSpec};
use crate::sys::windows::{self, ApplyOutcome, Refresh};

use super::{confirm_or_revert, confirm_or_revert_all, describe_outcome, monitor_of};

/// Resolves a SetSpec to width, height, and refresh using current display state.
fn resolve_spec(
    spec: &SetSpec,
    _current_width: u32,
    _current_height: u32,
) -> (Option<u32>, Option<u32>, Refresh) {
    use crate::cli::parser::PROFILES;
    match spec {
        SetSpec::Profile(name) => {
            let (_, w, h) = PROFILES.iter().find(|(n, _, _)| *n == name).unwrap();
            (Some(*w), Some(*h), Refresh::Keep)
        }
        SetSpec::ProfileWithRefresh(name, refresh) => {
            let (_, w, h) = PROFILES.iter().find(|(n, _, _)| *n == name).unwrap();
            (Some(*w), Some(*h), *refresh)
        }
        SetSpec::Explicit {
            width,
            height,
            refresh,
        } => (Some(*width), Some(*height), *refresh),
        SetSpec::RefreshOnly(refresh) => (None, None, *refresh),
        SetSpec::Keep => (None, None, Refresh::Keep),
        SetSpec::Max => unreachable!(),
    }
}

/// Applies a resolution, refresh and orientation policy to the targeted
/// display(s).
pub(super) fn run_set(
    spec: SetSpec,
    monitor: MonitorTarget,
    orientation: Option<u32>,
    yes: bool,
) -> i32 {
    if spec == SetSpec::Max {
        match monitor {
            MonitorTarget::Primary | MonitorTarget::Index(_) => {
                let monitor_idx = monitor_of(monitor);
                match windows::max(monitor_idx, orientation) {
                    Ok(ApplyOutcome::Unchanged(change)) => {
                        println!("{}", describe_outcome(&change, None, true));
                        0
                    }
                    Ok(ApplyOutcome::Applied(change)) => {
                        println!("{}", describe_outcome(&change, None, true));
                        confirm_or_revert(monitor_idx, change, yes)
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        2
                    }
                }
            }
            MonitorTarget::All => match windows::max_all(orientation) {
                Ok(outcomes) => {
                    let mut applied = Vec::new();
                    for outcome in outcomes {
                        match outcome {
                            ApplyOutcome::Unchanged(change) => {
                                println!(
                                    "{}",
                                    describe_outcome(&change, Some(&change.display), true)
                                )
                            }
                            ApplyOutcome::Applied(change) => {
                                println!(
                                    "{}",
                                    describe_outcome(&change, Some(&change.display), true)
                                );
                                applied.push(change);
                            }
                        }
                    }
                    confirm_or_revert_all(applied, yes)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            },
        }
    } else {
        match monitor {
            MonitorTarget::Primary | MonitorTarget::Index(_) => {
                let monitor_idx = monitor_of(monitor);
                // Get current display state for resolving spec
                let (width, height, refresh) = if let Some(idx) = monitor_idx {
                    match windows::get_current_mode(idx) {
                        Ok(mode) => resolve_spec(&spec, mode.width, mode.height),
                        Err(e) => {
                            eprintln!("error: {e}");
                            return 2;
                        }
                    }
                } else {
                    match windows::get_primary_mode() {
                        Ok(mode) => resolve_spec(&spec, mode.width, mode.height),
                        Err(e) => {
                            eprintln!("error: {e}");
                            return 2;
                        }
                    }
                };
                let mode_requested =
                    width.is_some() || height.is_some() || refresh != Refresh::Keep;
                match windows::set(monitor_idx, width, height, refresh, orientation) {
                    Ok(ApplyOutcome::Unchanged(change)) => {
                        println!("{}", describe_outcome(&change, None, mode_requested));
                        0
                    }
                    Ok(ApplyOutcome::Applied(change)) => {
                        println!("{}", describe_outcome(&change, None, mode_requested));
                        confirm_or_revert(monitor_idx, change, yes)
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        2
                    }
                }
            }
            MonitorTarget::All => {
                // For all monitors, we need to resolve spec for each monitor
                let devices = windows::enumerate_devices();
                let mut applied = Vec::new();
                let mut any_error = false;
                for (idx, _name) in devices.iter().enumerate() {
                    let monitor_num = (idx + 1) as u32;
                    let current = match windows::get_current_mode(monitor_num) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("error: {e}");
                            any_error = true;
                            continue;
                        }
                    };
                    let (width, height, refresh) =
                        resolve_spec(&spec, current.width, current.height);
                    let mode_requested =
                        width.is_some() || height.is_some() || refresh != Refresh::Keep;
                    match windows::set(Some(monitor_num), width, height, refresh, orientation) {
                        Ok(ApplyOutcome::Unchanged(change)) => println!(
                            "{}",
                            describe_outcome(&change, Some(&change.display), mode_requested)
                        ),
                        Ok(ApplyOutcome::Applied(change)) => {
                            println!(
                                "{}",
                                describe_outcome(&change, Some(&change.display), mode_requested)
                            );
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
                    confirm_or_revert_all(applied, yes)
                }
            }
        }
    }
}
