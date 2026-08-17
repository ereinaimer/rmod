//! `monitor` command: disable, enable, sleep, or wake monitors.
//!
//! Disabling detaches a monitor from the desktop and enabling re-attaches
//! it, reporting the outcome and running the shared keep-or-revert
//! confirmation flow. Sleeping and waking are global broadcasts with no
//! confirmation and no revert.

use crate::cli::{BrightnessBackend, MonitorAction, MonitorTarget};
use crate::sys::windows::{
    self, AttachOutcome, BrightnessLayer, BrightnessOutcome, BrightnessValue, brightness::mode_word,
};

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
        if action == MonitorAction::Disable {
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
fn run_brightness(
    value: BrightnessValue,
    via: Option<BrightnessBackend>,
    monitor: MonitorTarget,
) -> i32 {
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
                    Ok(outcome) => print_brightness(&outcome),
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
            print_brightness(&outcome);
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
    match outcome.kind {
        BrightnessValue::Percent(value) => {
            if outcome.unchanged {
                format!("{} is already at {}%", outcome.display, value)
            } else {
                format!(
                    "set {} brightness to {}% via {}",
                    outcome.display,
                    value,
                    layer_backend(outcome)
                )
            }
        }
        BrightnessValue::Min | BrightnessValue::Max | BrightnessValue::Boost => {
            if outcome.unchanged {
                format!(
                    "{} is already at {}",
                    outcome.display,
                    mode_word(outcome.kind)
                )
            } else {
                format!(
                    "set {} brightness to {} ({})",
                    outcome.display,
                    mode_word(outcome.kind),
                    describe_layers(outcome)
                )
            }
        }
    }
}

/// The joined layer descriptions of a mode outcome, e.g. `slider 5 + gamma 50%`.
fn describe_layers(outcome: &BrightnessOutcome) -> String {
    outcome
        .layers
        .iter()
        .map(describe_layer)
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Describes one write of a brightness change: a backend word with its
/// level, or the gamma ramp as a percentage.
fn describe_layer(layer: &BrightnessLayer) -> String {
    match layer {
        BrightnessLayer::Hardware { backend, level } => format!("{} {}", backend.name(), level),
        BrightnessLayer::Gamma { level } => format!("gamma {level}%"),
    }
}

/// The clip warning printed after an applied boost, or `None` otherwise.
fn clip_warning(outcome: &BrightnessOutcome) -> Option<&'static str> {
    if outcome.clipped && !outcome.unchanged {
        Some("boost clips highlights above ~77%")
    } else {
        None
    }
}

/// Prints a brightness outcome's report lines: the describe line, then the
/// clip warning when the applied boost ramp was clipped.
fn print_brightness(outcome: &BrightnessOutcome) {
    println!("{}", describe_brightness(outcome));
    if let Some(warning) = clip_warning(outcome) {
        println!("{warning}");
    }
}

/// The backend word of the outcome's hardware layer, or `gamma` for a
/// gamma layer.
fn layer_backend(outcome: &BrightnessOutcome) -> &str {
    match outcome.layers.first() {
        Some(BrightnessLayer::Hardware { backend, .. }) => backend.name(),
        Some(BrightnessLayer::Gamma { .. }) => "gamma",
        None => unreachable!("outcomes always carry at least one layer"),
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
        let layer = match backend {
            BrightnessBackend::Gamma => BrightnessLayer::Gamma { level: value },
            backend => BrightnessLayer::Hardware {
                backend,
                level: value,
            },
        };
        BrightnessOutcome {
            display: "RMOD Fake Monitor 1 [:1]".to_string(),
            kind: BrightnessValue::Percent(value),
            unchanged,
            layers: vec![layer],
            clipped: false,
        }
    }

    fn mode_outcome(
        display: &str,
        kind: BrightnessValue,
        layers: Vec<BrightnessLayer>,
        unchanged: bool,
        clipped: bool,
    ) -> BrightnessOutcome {
        BrightnessOutcome {
            display: display.to_string(),
            kind,
            unchanged,
            layers,
            clipped,
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

    #[test]
    fn describe_brightness_mode_min_joins_layers() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Min,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 5,
                },
                BrightnessLayer::Gamma { level: 50 },
            ],
            false,
            false,
        );
        assert_eq!(
            describe_brightness(&out),
            "set RMOD Fake Monitor 1 [:1] brightness to min (slider 5 + gamma 50%)"
        );
    }

    #[test]
    fn describe_brightness_mode_max_joins_layers() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Max,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Ddc,
                    level: 100,
                },
                BrightnessLayer::Gamma { level: 100 },
            ],
            false,
            false,
        );
        assert_eq!(
            describe_brightness(&out),
            "set RMOD Fake Monitor 1 [:1] brightness to max (ddc 100 + gamma 100%)"
        );
    }

    #[test]
    fn describe_brightness_mode_boost_joins_layers() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Boost,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 100,
                },
                BrightnessLayer::Gamma { level: 130 },
            ],
            false,
            true,
        );
        assert_eq!(
            describe_brightness(&out),
            "set RMOD Fake Monitor 1 [:1] brightness to boost (slider 100 + gamma 130%)"
        );
    }

    #[test]
    fn describe_brightness_mode_gamma_only() {
        let out = mode_outcome(
            "RMOD Fake Monitor 2 [:2]",
            BrightnessValue::Min,
            vec![BrightnessLayer::Gamma { level: 50 }],
            false,
            false,
        );
        assert_eq!(
            describe_brightness(&out),
            "set RMOD Fake Monitor 2 [:2] brightness to min (gamma 50%)"
        );
    }

    #[test]
    fn describe_brightness_mode_unchanged() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Min,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 5,
                },
                BrightnessLayer::Gamma { level: 50 },
            ],
            true,
            false,
        );
        assert_eq!(
            describe_brightness(&out),
            "RMOD Fake Monitor 1 [:1] is already at min"
        );
    }

    #[test]
    fn clip_warning_present_for_applied_boost() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Boost,
            vec![BrightnessLayer::Gamma { level: 130 }],
            false,
            true,
        );
        assert_eq!(
            clip_warning(&out),
            Some("boost clips highlights above ~77%")
        );
    }

    #[test]
    fn clip_warning_omitted_for_unchanged_boost() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Boost,
            vec![BrightnessLayer::Gamma { level: 130 }],
            true,
            true,
        );
        assert_eq!(clip_warning(&out), None);
    }

    #[test]
    fn clip_warning_omitted_when_not_clipped() {
        let out = mode_outcome(
            "RMOD Fake Monitor 1 [:1]",
            BrightnessValue::Boost,
            vec![BrightnessLayer::Gamma { level: 130 }],
            false,
            false,
        );
        assert_eq!(clip_warning(&out), None);
    }
}
