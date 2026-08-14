//! `monitor` command: disable, enable, sleep, or wake monitors.
//!
//! Disabling detaches a monitor from the desktop and enabling re-attaches
//! it, reporting the outcome and running the shared keep-or-revert
//! confirmation flow. Sleeping and waking are global broadcasts with no
//! confirmation and no revert.

use crate::cli::{MonitorAction, MonitorTarget};
use crate::sys::windows::{self, AttachOutcome};

use super::{confirm_or_revert_attach, confirm_or_revert_attach_all, describe_attach, monitor_of};

/// Runs the `monitor` command with the parsed action and target.
pub(super) fn run_monitor(action: MonitorAction, monitor: MonitorTarget, yes: bool) -> i32 {
    match action {
        MonitorAction::Sleep => match windows::sleep_monitor() {
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
        },
        MonitorAction::Wake => match windows::wake_monitor() {
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
        },
        MonitorAction::Disable | MonitorAction::Enable => run_attach(action, monitor, yes),
    }
}

/// Runs a disable/enable action against the targeted display(s).
fn run_attach(action: MonitorAction, monitor: MonitorTarget, yes: bool) -> i32 {
    match monitor {
        MonitorTarget::Primary | MonitorTarget::Index(_) => {
            let monitor_idx = monitor_of(monitor);
            let outcome = if action == MonitorAction::Disable {
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
fn report_all(action: MonitorAction, yes: bool) -> i32 {
    let count = match action {
        MonitorAction::Disable => windows::enumerate_devices().len(),
        MonitorAction::Enable => windows::enumerate_all_devices().len(),
        _ => unreachable!(),
    };
    let mut applied = Vec::new();
    let mut any_error = false;
    for monitor in 1..=count as u32 {
        let outcome = if action == MonitorAction::Disable {
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
