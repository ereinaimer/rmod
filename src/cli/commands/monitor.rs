//! `monitor` command: disable, enable, sleep, or wake monitors.
//!
//! Disabling detaches a monitor from the desktop and enabling re-attaches
//! it, reporting the outcome and running the shared keep-or-revert
//! confirmation flow. Sleeping and waking are global broadcasts with no
//! confirmation and no revert.

use crate::cli::{BrightnessBackend, MonitorAction, MonitorTarget};
use crate::sys::windows::{self, AttachOutcome, BrightnessOutcome};

use super::{confirm_or_revert_attach, confirm_or_revert_attach_all, describe_attach, resolve_target};

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
        MonitorAction::Brightness { value, via } => run_brightness(value, via, monitor),
    }
}

/// Runs a disable/enable action against the targeted display(s).
fn run_attach(action: MonitorAction, monitor: MonitorTarget, yes: bool) -> i32 {
    match monitor {
        MonitorTarget::Id(_) | MonitorTarget::Primary | MonitorTarget::Index(_) => {
            let monitor_idx = match resolve_target(&monitor) {
                Ok(idx) => idx,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
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

/// Runs the brightness command against the targeted display(s).
fn run_brightness(value: u32, via: Option<BrightnessBackend>, monitor: MonitorTarget) -> i32 {
    match monitor {
        MonitorTarget::Primary => report_brightness(windows::set_brightness(None, value, via)),
        MonitorTarget::Index(n) => report_brightness(windows::set_brightness(Some(n), value, via)),
        MonitorTarget::Id(_) => match resolve_target(&monitor) {
            Ok(idx) => report_brightness(windows::set_brightness(idx, value, via)),
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        MonitorTarget::All => {
            let count = windows::enumerate_devices().len();
            let mut any_error = false;
            for n in 1..=count as u32 {
                match windows::set_brightness(Some(n), value, via) {
                    Ok(outcome) => println!("{}", describe_brightness(&outcome)),
                    Err(e) => {
                        eprintln!("error: {e}");
                        any_error = true;
                    }
                }
            }
            if any_error { 2 } else { 0 }
        }
    }
}

/// Reports a single-display brightness outcome.
fn report_brightness(outcome: Result<BrightnessOutcome, String>) -> i32 {
    match outcome {
        Ok(outcome) => {
            println!("{}", describe_brightness(&outcome));
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Describes a brightness outcome: the applied line with its backend, or
/// the already-at line.
fn describe_brightness(outcome: &BrightnessOutcome) -> String {
    if outcome.unchanged {
        format!("{} is already at {}%", outcome.display, outcome.value)
    } else {
        format!(
            "set {} brightness to {}% via {}",
            outcome.display,
            outcome.value,
            outcome.backend.name()
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::BrightnessBackend;

    fn outcome(value: u32, backend: BrightnessBackend, unchanged: bool) -> BrightnessOutcome {
        BrightnessOutcome {
            display: "RMOD Fake Monitor 1 [:1]".to_string(),
            value,
            backend,
            unchanged,
        }
    }

    #[test]
    fn describe_brightness_applied_mentions_backend() {
        assert_eq!(
            describe_brightness(&outcome(30, BrightnessBackend::Gamma, false)),
            "set RMOD Fake Monitor 1 [:1] brightness to 30% via gamma"
        );
    }

    #[test]
    fn describe_brightness_unchanged_omits_backend() {
        assert_eq!(
            describe_brightness(&outcome(60, BrightnessBackend::Ddc, true)),
            "RMOD Fake Monitor 1 [:1] is already at 60%"
        );
    }
}
