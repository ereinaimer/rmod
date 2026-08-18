//! `monitor` command: disable, enable, sleep, or wake monitors.
//!
//! Disabling detaches a monitor from the desktop and enabling re-attaches
//! it, reporting the outcome and running the shared keep-or-revert
//! confirmation flow. Sleeping and waking are global broadcasts with no
//! confirmation and no revert.

use crate::cli::parser::parse_monitor_target;
use crate::cli::{BrightnessBackend, Command, HelpTopic, MonitorAction, MonitorTarget};
use crate::sys::windows::{
    self, AttachOutcome, BrightnessLayer, BrightnessOutcome, BrightnessValue, ContrastBackend,
    ContrastOutcome, brightness::mode_word,
};

use super::{
    confirm_or_revert_attach, confirm_or_revert_attach_all, describe_attach, resolve_target,
    resolve_target_all,
};

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
        MonitorAction::Contrast { value, via } => run_contrast(value, via, monitor),
        MonitorAction::ContrastReset => run_contrast_reset(monitor),
    }
}

/// Runs a disable/enable action against the targeted display(s).
fn run_attach(action: MonitorAction, monitor: MonitorTarget, yes: bool) -> i32 {
    match monitor {
        MonitorTarget::Id(_) | MonitorTarget::Primary | MonitorTarget::Index(_) => {
            let monitor_idx = match action {
                MonitorAction::Enable => resolve_target_all(&monitor),
                _ => resolve_target(&monitor),
            };
            let monitor_idx = match monitor_idx {
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

/// Runs the contrast command against the targeted display(s).
fn run_contrast(value: u32, via: Option<ContrastBackend>, monitor: MonitorTarget) -> i32 {
    match monitor {
        MonitorTarget::Primary => report_contrast(windows::set_contrast(None, value, via)),
        MonitorTarget::Index(n) => report_contrast(windows::set_contrast(Some(n), value, via)),
        MonitorTarget::Id(_) => match resolve_target(&monitor) {
            Ok(idx) => report_contrast(windows::set_contrast(idx, value, via)),
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        MonitorTarget::All => contrast_all(value, via),
    }
}

/// Applies contrast to all displays.
fn contrast_all(value: u32, via: Option<ContrastBackend>) -> i32 {
    let count = windows::enumerate_devices().len();
    let mut any_error = false;
    for n in 1..=count as u32 {
        match windows::set_contrast(Some(n), value, via) {
            Ok(outcome) => print_contrast(&outcome),
            Err(e) => {
                eprintln!("error: {e}");
                any_error = true;
            }
        }
    }
    if any_error { 2 } else { 0 }
}

/// Reports a single-display contrast outcome.
fn report_contrast(outcome: Result<ContrastOutcome, String>) -> i32 {
    match outcome {
        Ok(outcome) => {
            print_contrast(&outcome);
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Runs the contrast reset command against the targeted display(s).
fn run_contrast_reset(monitor: MonitorTarget) -> i32 {
    match monitor {
        MonitorTarget::Primary => report_contrast(windows::reset_contrast(None)),
        MonitorTarget::Index(n) => report_contrast(windows::reset_contrast(Some(n))),
        MonitorTarget::Id(_) => match resolve_target(&monitor) {
            Ok(idx) => report_contrast(windows::reset_contrast(idx)),
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        MonitorTarget::All => contrast_all_reset(),
    }
}

/// Applies contrast reset to all displays.
fn contrast_all_reset() -> i32 {
    let count = windows::enumerate_devices().len();
    let mut any_error = false;
    for n in 1..=count as u32 {
        match windows::reset_contrast(Some(n)) {
            Ok(outcome) => print_contrast(&outcome),
            Err(e) => {
                eprintln!("error: {e}");
                any_error = true;
            }
        }
    }
    if any_error { 2 } else { 0 }
}

/// Describes a contrast outcome: the applied line with its backend, or
/// the already-at line.
fn describe_contrast(outcome: &ContrastOutcome) -> String {
    if outcome.unchanged {
        format!("{} is already at {}%", outcome.display, outcome.value)
    } else {
        format!(
            "set {} contrast to {}% via {}",
            outcome.display,
            outcome.value,
            outcome.backend.name()
        )
    }
}

/// The clip warning printed after an applied contrast boost, or `None` otherwise.
fn contrast_clip_warning(outcome: &ContrastOutcome) -> Option<&'static str> {
    if outcome.clipped && !outcome.unchanged {
        Some("contrast boost clips shadows and highlights")
    } else {
        None
    }
}

/// Prints a contrast outcome's report lines: the describe line, then the
/// clip warning when the applied boost ramp was clipped.
fn print_contrast(outcome: &ContrastOutcome) {
    println!("{}", describe_contrast(outcome));
    if let Some(warning) = contrast_clip_warning(outcome) {
        println!("{warning}");
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

/// Parses the `monitor` command.
///
/// Two phases: first scan for the action (flags may precede it), then
/// validate every remaining token against the action's own flag rules.
pub(crate) fn parse_monitor(args: &[impl AsRef<str>]) -> Result<Command, String> {
    // Phase 1: find the action. Flags may appear before it.
    let mut i = 1;
    let action_idx = loop {
        let Some(arg) = args.get(i) else {
            return Err("monitor needs an action. attach, detach, sleep, wake, brightness, or contrast\ne.g. rmod monitor detach -m 2".to_string());
        };
        match arg.as_ref() {
            "-y" | "--yes" => {
                i += 1;
            }
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Monitor { action: None }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                // Skip the value too; phase 2 re-validates the flag.
                let Some(val) = args.get(i + 1) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4"
                            .to_string(),
                    );
                };
                if val.as_ref().starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4"
                            .to_string(),
                    );
                }
                i += 2;
            }
            _ => break i,
        }
    };

    let action_str = args[action_idx].as_ref();
    match action_str {
        "brightness" => return parse_monitor_brightness(args),
        "contrast" => return parse_monitor_contrast(args),
        _ => {}
    }
    let action = match action_str {
        "detach" | "disable" | "off" => MonitorAction::Disable,
        "attach" | "enable" | "on" => MonitorAction::Enable,
        "sleep" => MonitorAction::Sleep,
        "wake" => MonitorAction::Wake,
        other => {
            return Err(format!(
                "unknown action {} for monitor. use attach, detach, sleep, wake, brightness, or contrast",
                other
            ));
        }
    };

    // Phase 2: validate every remaining token with the action's rules.
    let mut monitor = MonitorTarget::Primary;
    let mut monitor_explicit = false;
    let mut yes = false;
    let mut i = 1;

    while i < args.len() {
        if i == action_idx {
            i += 1;
            continue;
        }
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Monitor {
                        action: Some(action),
                    }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                if !matches!(action, MonitorAction::Disable | MonitorAction::Enable) {
                    return Err(format!(
                        "-m, --monitor is not valid for monitor {action_str}. {action_str} applies to all monitors"
                    ));
                }
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
                if !matches!(action, MonitorAction::Disable | MonitorAction::Enable) {
                    return Err(format!(
                        "-y, --yes is not valid for monitor {action_str}. {action_str} applies to all monitors"
                    ));
                }
                yes = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for monitor {action_str}. use --monitor or --yes",
                    other
                ));
            }
        }
    }

    if matches!(action, MonitorAction::Disable | MonitorAction::Enable) && !monitor_explicit {
        let verb = if action == MonitorAction::Disable {
            "detach"
        } else {
            "attach"
        };
        return Err(format!(
            "monitor {verb} needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor {verb} -m a1b2c3d4"
        ));
    }

    Ok(Command::Monitor {
        action,
        monitor,
        yes,
    })
}

/// Parses `rmod monitor brightness <VALUE> [OPTIONS]`.
///
/// The value is the first positional after the action: a token that does not
/// start with `-` and is not consumed as a `-m` or `-v` value.
pub(crate) fn parse_monitor_brightness(args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut value: Option<BrightnessValue> = None;
    let mut monitor = MonitorTarget::Primary;
    let mut via = None;
    let mut seen_action = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Monitor {
                        action: Some(MonitorAction::Brightness {
                            value: value.unwrap_or(BrightnessValue::Percent(0)),
                            via,
                        }),
                    }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            "-v" | "--via" => {
                if matches!(
                    value,
                    Some(BrightnessValue::Min | BrightnessValue::Max | BrightnessValue::Boost)
                ) {
                    return Err("-v, --via is not valid with min, max, or boost. use a number to choose a backend".to_string());
                }
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string(),
                    );
                }
                via = Some(parse_backend(val)?);
                i += 1;
            }
            "-y" | "--yes" => {
                return Err(
                    "-y, --yes is not valid for monitor brightness. brightness does not prompt for confirmation"
                        .to_string(),
                );
            }
            other if other.starts_with('-') => {
                return Err(format!(
                    "unexpected argument {other} for monitor brightness. use -m/--monitor or -v/--via"
                ));
            }
            other => {
                if !seen_action {
                    seen_action = true;
                    i += 1;
                    continue;
                }
                if value.is_some() {
                    return Err(format!(
                        "unexpected argument {other} for monitor brightness. use -m/--monitor or -v/--via"
                    ));
                }
                let new_value = match other.to_lowercase().as_str() {
                    "min" => BrightnessValue::Min,
                    "max" => BrightnessValue::Max,
                    "boost" => BrightnessValue::Boost,
                    _ => BrightnessValue::Percent(other.parse::<u32>().map_err(|_| {
                        format!("invalid brightness {other}. use a number between 0 and 100")
                    })?),
                };
                if let BrightnessValue::Percent(v) = new_value
                    && v > 100
                {
                    return Err(format!(
                        "invalid brightness {other}. use a number between 0 and 100"
                    ));
                }
                if via.is_some()
                    && matches!(
                        new_value,
                        BrightnessValue::Min | BrightnessValue::Max | BrightnessValue::Boost
                    )
                {
                    return Err("-v, --via is not valid with min, max, or boost. use a number to choose a backend".to_string());
                }
                value = Some(new_value);
                i += 1;
            }
        }
    }
    let Some(value) = value else {
        return Err(
            "monitor brightness needs a value. a number between 0 and 100\ne.g. rmod monitor brightness 60"
                .to_string(),
        );
    };
    Ok(Command::Monitor {
        action: MonitorAction::Brightness { value, via },
        monitor,
        yes: false,
    })
}

/// Parses a `--via` backend name.
fn parse_backend(arg: &str) -> Result<BrightnessBackend, String> {
    match arg {
        "ddc" => Ok(BrightnessBackend::Ddc),
        "slider" => Ok(BrightnessBackend::Slider),
        "gamma" => Ok(BrightnessBackend::Gamma),
        _ => Err(format!("unknown backend {arg}. use ddc, slider, or gamma")),
    }
}

/// Parses `rmod monitor contrast <VALUE> [OPTIONS]`.
///
/// The value is the first positional after the action: a token that does not
/// start with `-` and is not consumed as a `-m` or `-v` value. A value of
/// `reset` (in either position) selects reset semantics.
pub(crate) fn parse_monitor_contrast(args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut value: Option<u32> = None;
    let mut reset = false;
    let mut monitor = MonitorTarget::Primary;
    let mut via = None;
    let mut seen_action = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Monitor {
                        action: Some(if reset {
                            MonitorAction::ContrastReset
                        } else {
                            MonitorAction::Contrast {
                                value: value.unwrap_or(0),
                                via,
                            }
                        }),
                    }),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor number or all\ne.g. -m 2"
                            .to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            "-v" | "--via" => {
                if reset {
                    return Err(format!(
                        "unexpected argument {arg} for monitor contrast reset. use -m/--monitor"
                    ));
                }
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err("-v, --via needs a value. ddc or gamma\ne.g. -v ddc".to_string());
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err("-v, --via needs a value. ddc or gamma\ne.g. -v ddc".to_string());
                }
                via = Some(parse_contrast_backend(val)?);
                i += 1;
            }
            "-y" | "--yes" => {
                return Err(if reset {
                    "-y, --yes is not valid for monitor contrast reset. reset does not prompt for confirmation".to_string()
                } else {
                    "-y, --yes is not valid for monitor contrast. contrast does not prompt for confirmation".to_string()
                });
            }
            other if other.starts_with('-') => {
                return Err(if reset {
                    format!(
                        "unexpected argument {other} for monitor contrast reset. use -m/--monitor"
                    )
                } else {
                    format!(
                        "unexpected argument {other} for monitor contrast. use -m/--monitor or -v/--via"
                    )
                });
            }
            other => {
                if !seen_action {
                    seen_action = true;
                    i += 1;
                    continue;
                }
                if reset || value.is_some() {
                    return Err(if reset {
                        format!(
                            "unexpected argument {other} for monitor contrast reset. use -m/--monitor"
                        )
                    } else {
                        format!(
                            "unexpected argument {other} for monitor contrast. use -m/--monitor or -v/--via"
                        )
                    });
                }
                if other == "reset" {
                    if via.is_some() {
                        return Err("-v, --via is not valid with reset. reset restores defaults"
                            .to_string());
                    }
                    reset = true;
                } else {
                    let parsed = other.parse::<u32>().map_err(|_| {
                        format!("invalid contrast {other}. use a number between 0 and 130")
                    })?;
                    if parsed > 130 {
                        return Err(format!(
                            "invalid contrast {other}. use a number between 0 and 130"
                        ));
                    }
                    value = Some(parsed);
                }
                i += 1;
            }
        }
    }
    let action = if reset {
        MonitorAction::ContrastReset
    } else {
        let Some(value) = value else {
            return Err(
                "monitor contrast needs a value. a number between 0 and 130\ne.g. rmod monitor contrast 60"
                    .to_string(),
            );
        };
        MonitorAction::Contrast { value, via }
    };
    Ok(Command::Monitor {
        action,
        monitor,
        yes: false,
    })
}

/// Parses a contrast `--via` backend name.
fn parse_contrast_backend(arg: &str) -> Result<ContrastBackend, String> {
    match arg {
        "ddc" => Ok(ContrastBackend::Ddc),
        "gamma" => Ok(ContrastBackend::Gamma),
        _ => Err(format!("unknown backend {arg}. use ddc or gamma")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::BrightnessBackend;
    use crate::sys::windows::ContrastBackend;
    use crate::sys::windows::ContrastOutcome;

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

    fn contrast_outcome(
        display: &str,
        value: u32,
        backend: ContrastBackend,
        unchanged: bool,
        clipped: bool,
    ) -> ContrastOutcome {
        ContrastOutcome {
            display: display.to_string(),
            value,
            unchanged,
            backend,
            clipped,
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

    #[test]
    fn describe_contrast_applied_mentions_backend() {
        let out = contrast_outcome(
            "RMOD Fake Monitor 1 [:1]",
            60,
            ContrastBackend::Ddc,
            false,
            false,
        );
        assert_eq!(
            describe_contrast(&out),
            "set RMOD Fake Monitor 1 [:1] contrast to 60% via ddc"
        );
    }

    #[test]
    fn describe_contrast_unchanged_omits_backend() {
        let out = contrast_outcome(
            "RMOD Fake Monitor 1 [:1]",
            75,
            ContrastBackend::Gamma,
            true,
            false,
        );
        assert_eq!(
            describe_contrast(&out),
            "RMOD Fake Monitor 1 [:1] is already at 75%"
        );
    }

    #[test]
    fn describe_contrast_clipped_warning_included() {
        let out = contrast_outcome(
            "RMOD Fake Monitor 1 [:1]",
            130,
            ContrastBackend::Gamma,
            false,
            true,
        );
        assert_eq!(
            contrast_clip_warning(&out),
            Some("contrast boost clips shadows and highlights")
        );
    }

    #[test]
    fn describe_contrast_clipped_warning_omitted_when_unchanged() {
        let out = contrast_outcome(
            "RMOD Fake Monitor 1 [:1]",
            130,
            ContrastBackend::Gamma,
            true,
            true,
        );
        assert_eq!(contrast_clip_warning(&out), None);
    }

    #[test]
    fn describe_contrast_clipped_warning_omitted_when_not_clipped() {
        let out = contrast_outcome(
            "RMOD Fake Monitor 1 [:1]",
            60,
            ContrastBackend::Ddc,
            false,
            false,
        );
        assert_eq!(contrast_clip_warning(&out), None);
    }

    const SERIAL_A: &str = "ABC12345678";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn monitor_detach_requires_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "detach"]),
            Err("monitor detach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor detach -m a1b2c3d4".to_string())
        );
        assert_eq!(
            parse(&["monitor", "disable"]),
            Err("monitor detach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor detach -m a1b2c3d4".to_string())
        );
        assert_eq!(
            parse(&["monitor", "off"]),
            Err("monitor detach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor detach -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn monitor_attach_requires_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "attach"]),
            Err("monitor attach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor attach -m a1b2c3d4".to_string())
        );
        assert_eq!(
            parse(&["monitor", "enable"]),
            Err("monitor attach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor attach -m a1b2c3d4".to_string())
        );
        assert_eq!(
            parse(&["monitor", "on"]),
            Err("monitor attach needs -m, --monitor. a monitor ID or all\ne.g. rmod monitor attach -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn monitor_disable_and_off_are_aliases_for_detach() {
        assert_eq!(
            parse(&["monitor", "disable", "-m", SERIAL_A]),
            parse(&["monitor", "detach", "-m", SERIAL_A])
        );
        assert_eq!(
            parse(&["monitor", "off", "-m", SERIAL_A]),
            parse(&["monitor", "detach", "-m", SERIAL_A])
        );
    }

    #[test]
    fn monitor_enable_and_on_are_aliases_for_attach() {
        assert_eq!(
            parse(&["monitor", "enable", "-m", SERIAL_A]),
            parse(&["monitor", "attach", "-m", SERIAL_A])
        );
        assert_eq!(
            parse(&["monitor", "on", "-m", SERIAL_A]),
            parse(&["monitor", "attach", "-m", SERIAL_A])
        );
    }

    #[test]
    fn monitor_detach_with_monitor_and_yes() {
        for args in [
            &["monitor", "detach", "-m", SERIAL_A, "-y"][..],
            &["monitor", "detach", "-y", "-m", SERIAL_A][..],
            &["monitor", "disable", "-m", "all", "-y"][..],
        ] {
            let expected = Command::Monitor {
                action: MonitorAction::Disable,
                monitor: if args.contains(&"all") {
                    MonitorTarget::All
                } else {
                    MonitorTarget::Id(SERIAL_A.to_string())
                },
                yes: true,
            };
            assert_eq!(parse(args), Ok(expected), "args: {:?}", args);
        }
    }

    #[test]
    fn monitor_attach_with_monitor() {
        assert_eq!(
            parse(&["monitor", "attach", "-m", SERIAL_A, "-y"]),
            Ok(Command::Monitor {
                action: MonitorAction::Enable,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                yes: true
            })
        );
    }

    #[test]
    fn monitor_sleep_command() {
        assert_eq!(
            parse(&["monitor", "sleep"]),
            Ok(Command::Monitor {
                action: MonitorAction::Sleep,
                monitor: MonitorTarget::Primary,
                yes: false
            })
        );
    }

    #[test]
    fn monitor_wake_command() {
        assert_eq!(
            parse(&["monitor", "wake"]),
            Ok(Command::Monitor {
                action: MonitorAction::Wake,
                monitor: MonitorTarget::Primary,
                yes: false
            })
        );
    }

    #[test]
    fn monitor_sleep_rejects_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "sleep", "-m", SERIAL_A]),
            Err(
                "-m, --monitor is not valid for monitor sleep. sleep applies to all monitors"
                    .to_string()
            )
        );
        assert_eq!(
            parse(&["monitor", "wake", "-m", SERIAL_A]),
            Err(
                "-m, --monitor is not valid for monitor wake. wake applies to all monitors"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_sleep_rejects_yes_flag() {
        assert_eq!(
            parse(&["monitor", "sleep", "-y"]),
            Err(
                "-y, --yes is not valid for monitor sleep. sleep applies to all monitors"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_missing_action_is_error() {
        assert_eq!(
            parse(&["monitor"]),
            Err(
"monitor needs an action. attach, detach, sleep, wake, brightness, or contrast\ne.g. rmod monitor detach -m 2"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_unknown_action_is_error() {
        assert_eq!(
            parse(&["monitor", "frobnicate"]),
            Err(
                "unknown action frobnicate for monitor. use attach, detach, sleep, wake, brightness, or contrast"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_any_string_is_id() {
        assert_eq!(
            parse(&["monitor", "detach", "-m", "x"]),
            Ok(Command::Monitor {
                action: MonitorAction::Disable,
                monitor: MonitorTarget::Id("x".to_string()),
                yes: false
            })
        );
        assert_eq!(
            parse(&["monitor", "detach", "-m", "2"]),
            Ok(Command::Monitor {
                action: MonitorAction::Disable,
                monitor: MonitorTarget::Index(2),
                yes: false
            })
        );
        assert!(parse(&["monitor", "detach", "-m", "0"]).is_err());
    }

    #[test]
    fn monitor_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["monitor", "detach", "-m"]),
            Err("-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn monitor_unknown_argument_is_error() {
        assert_eq!(
            parse(&["monitor", "detach", "foo"]),
            Err("unexpected argument foo for monitor detach. use --monitor or --yes".to_string())
        );
    }

    #[test]
    fn monitor_help_flag() {
        assert_eq!(
            parse(&["monitor", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor { action: None })
            })
        );
        assert_eq!(
            parse(&["monitor", "disable", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Disable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "detach", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Disable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "attach", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Enable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "sleep", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Sleep)
                })
            })
        );
    }

    #[test]
    fn monitor_brightness_primary_default() {
        assert_eq!(
            parse(&["monitor", "brightness", "60"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(60),
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_with_monitor_and_backend() {
        assert_eq!(
            parse(&["monitor", "brightness", "40", "-m", "2", "--via", "ddc"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(40),
                    via: Some(BrightnessBackend::Ddc)
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "40", "-m", "all", "--via", "gamma"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(40),
                    via: Some(BrightnessBackend::Gamma)
                },
                monitor: MonitorTarget::All,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_via_short_flag() {
        assert_eq!(
            parse(&["monitor", "brightness", "80", "-v", "slider"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(80),
                    via: Some(BrightnessBackend::Slider)
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_via_short_flag_missing_value() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-v"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_brightness_via_short_flag_flag_like_value() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-v", "-m"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_brightness_zero_is_valid() {
        assert_eq!(
            parse(&["monitor", "brightness", "0", "-m", "1"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(0),
                    via: None
                },
                monitor: MonitorTarget::Index(1),
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_missing_value_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness"]),
            Err("monitor brightness needs a value. a number between 0 and 100\ne.g. rmod monitor brightness 60".to_string())
        );
    }

    #[test]
    fn monitor_brightness_out_of_range_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "150"]),
            Err("invalid brightness 150. use a number between 0 and 100".to_string())
        );
        assert_eq!(
            parse(&["monitor", "brightness", "abc"]),
            Err("invalid brightness abc. use a number between 0 and 100".to_string())
        );
    }

    #[test]
    fn monitor_brightness_unknown_backend_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "--via", "gamma2"]),
            Err("unknown backend gamma2. use ddc, slider, or gamma".to_string())
        );
    }

    #[test]
    fn monitor_brightness_missing_backend_value_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "--via"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_brightness_rejects_yes_flag() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-y"]),
            Err("-y, --yes is not valid for monitor brightness. brightness does not prompt for confirmation".to_string())
        );
    }

    #[test]
    fn monitor_brightness_help_routes() {
        assert_eq!(
            parse(&["monitor", "brightness", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Brightness {
                        value: BrightnessValue::Percent(0),
                        via: None
                    })
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "60", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Brightness {
                        value: BrightnessValue::Percent(60),
                        via: None
                    })
                })
            })
        );
    }

    #[test]
    fn monitor_brightness_unknown_argument_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "foo"]),
            Err(
                "unexpected argument foo for monitor brightness. use -m/--monitor or -v/--via"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_brightness_flag_like_monitor_value_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-m", "--via"]),
            Err("-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string())
        );
    }

    #[test]
    fn monitor_brightness_flag_like_backend_value_is_error() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "--via", "-y"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_brightness_max_is_valid() {
        assert_eq!(
            parse(&["monitor", "brightness", "100"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(100),
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_min_keyword() {
        assert_eq!(
            parse(&["monitor", "brightness", "min"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Min,
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_max_keyword() {
        assert_eq!(
            parse(&["monitor", "brightness", "max"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Max,
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_boost_keyword() {
        assert_eq!(
            parse(&["monitor", "brightness", "boost"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Boost,
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_keyword_with_monitor() {
        assert_eq!(
            parse(&["monitor", "brightness", "min", "-m", "2"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Min,
                    via: None
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "max", "-m", "all"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Max,
                    via: None
                },
                monitor: MonitorTarget::All,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_brightness_keywords_are_case_insensitive() {
        for (arg, value) in [
            ("Min", BrightnessValue::Min),
            ("MIN", BrightnessValue::Min),
            ("mAx", BrightnessValue::Max),
            ("BOOST", BrightnessValue::Boost),
        ] {
            assert_eq!(
                parse(&["monitor", "brightness", arg]),
                Ok(Command::Monitor {
                    action: MonitorAction::Brightness { value, via: None },
                    monitor: MonitorTarget::Primary,
                    yes: false,
                }),
                "arg '{arg}'"
            );
        }
    }

    #[test]
    fn monitor_brightness_keyword_rejects_via() {
        for args in [
            &["monitor", "brightness", "min", "-v", "ddc"][..],
            &["monitor", "brightness", "min", "--via", "ddc"][..],
            &["monitor", "brightness", "max", "-v", "slider"][..],
            &["monitor", "brightness", "boost", "--via", "gamma"][..],
        ] {
            assert_eq!(
                parse(args),
                Err("-v, --via is not valid with min, max, or boost. use a number to choose a backend".to_string()),
                "args: {:?}",
                args
            );
        }
    }

    #[test]
    fn monitor_brightness_keyword_still_rejects_yes_flag() {
        assert_eq!(
            parse(&["monitor", "brightness", "min", "-y"]),
            Err("-y, --yes is not valid for monitor brightness. brightness does not prompt for confirmation".to_string())
        );
    }

    #[test]
    fn monitor_brightness_keyword_still_rejects_unknown_arguments() {
        assert_eq!(
            parse(&["monitor", "brightness", "max", "foo"]),
            Err(
                "unexpected argument foo for monitor brightness. use -m/--monitor or -v/--via"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_contrast_primary_default() {
        assert_eq!(
            parse(&["monitor", "contrast", "60"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 60,
                    via: None
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_contrast_with_monitor_and_backend() {
        assert_eq!(
            parse(&["monitor", "contrast", "40", "-m", "2", "--via", "ddc"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 40,
                    via: Some(ContrastBackend::Ddc)
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "contrast", "40", "-m", "all", "--via", "gamma"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 40,
                    via: Some(ContrastBackend::Gamma)
                },
                monitor: MonitorTarget::All,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_contrast_via_short_flag() {
        assert_eq!(
            parse(&["monitor", "contrast", "80", "-v", "ddc"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 80,
                    via: Some(ContrastBackend::Ddc)
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "contrast", "80", "-v", "gamma"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 80,
                    via: Some(ContrastBackend::Gamma)
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_contrast_slider_backend_is_error() {
        assert_eq!(
            parse(&["monitor", "contrast", "60", "-v", "slider"]),
            Err("unknown backend slider. use ddc or gamma".to_string())
        );
    }

    #[test]
    fn monitor_contrast_via_short_flag_missing_value() {
        assert_eq!(
            parse(&["monitor", "contrast", "60", "-v"]),
            Err("-v, --via needs a value. ddc or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_contrast_zero_hundred_and_hundredthirty_are_valid() {
        for value in ["0", "100", "130"] {
            assert_eq!(
                parse(&["monitor", "contrast", value]),
                Ok(Command::Monitor {
                    action: MonitorAction::Contrast {
                        value: value.parse().unwrap(),
                        via: None
                    },
                    monitor: MonitorTarget::Primary,
                    yes: false,
                }),
                "value {value}"
            );
        }
    }

    #[test]
    fn monitor_contrast_out_of_range_is_error() {
        assert_eq!(
            parse(&["monitor", "contrast", "131"]),
            Err("invalid contrast 131. use a number between 0 and 130".to_string())
        );
    }

    #[test]
    fn monitor_contrast_non_numeric_is_error() {
        assert_eq!(
            parse(&["monitor", "contrast", "fifty"]),
            Err("invalid contrast fifty. use a number between 0 and 130".to_string())
        );
    }

    #[test]
    fn monitor_contrast_missing_value_is_error() {
        assert_eq!(
            parse(&["monitor", "contrast"]),
            Err("monitor contrast needs a value. a number between 0 and 130\ne.g. rmod monitor contrast 60".to_string())
        );
    }

    #[test]
    fn monitor_contrast_rejects_yes_flag() {
        assert_eq!(
            parse(&["monitor", "contrast", "60", "-y"]),
            Err("-y, --yes is not valid for monitor contrast. contrast does not prompt for confirmation".to_string())
        );
    }

    #[test]
    fn monitor_contrast_help_routes() {
        assert_eq!(
            parse(&["monitor", "contrast", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Contrast {
                        value: 0,
                        via: None
                    })
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "contrast", "60", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Contrast {
                        value: 60,
                        via: None
                    })
                })
            })
        );
    }

    #[test]
    fn monitor_contrast_unknown_argument_is_error() {
        assert_eq!(
            parse(&["monitor", "contrast", "60", "foo"]),
            Err(
                "unexpected argument foo for monitor contrast. use -m/--monitor or -v/--via"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_contrast_flag_like_values_are_error() {
        assert_eq!(
            parse(&["monitor", "contrast", "60", "-m", "--via"]),
            Err("-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string())
        );
        assert_eq!(
            parse(&["monitor", "contrast", "60", "--via", "-y"]),
            Err("-v, --via needs a value. ddc or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn monitor_flag_before_action() {
        assert_eq!(
            parse(&["monitor", "-m", "2", "detach"]),
            Ok(Command::Monitor {
                action: MonitorAction::Disable,
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "-m", SERIAL_A, "-y", "detach"]),
            Ok(Command::Monitor {
                action: MonitorAction::Disable,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                yes: true,
            })
        );
    }

    #[test]
    fn monitor_flag_before_sleep_is_rejected() {
        assert_eq!(
            parse(&["monitor", "-m", "2", "sleep"]),
            Err(
                "-m, --monitor is not valid for monitor sleep. sleep applies to all monitors"
                    .to_string()
            )
        );
    }

    #[test]
    fn brightness_flags_before_value() {
        assert_eq!(
            parse(&["monitor", "brightness", "-m", "2", "60"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(60),
                    via: None
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "-v", "ddc", "60"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Percent(60),
                    via: Some(BrightnessBackend::Ddc)
                },
                monitor: MonitorTarget::Primary,
                yes: false,
            })
        );
    }

    #[test]
    fn flags_before_brightness_action_rejected_for_yes() {
        assert_eq!(
            parse(&["monitor", "-m", "2", "brightness", "60", "-y"]),
            Err("-y, --yes is not valid for monitor brightness. brightness does not prompt for confirmation".to_string())
        );
    }

    #[test]
    fn brightness_min_keyword_with_flag_before_value() {
        assert_eq!(
            parse(&["monitor", "brightness", "-m", "2", "min"]),
            Ok(Command::Monitor {
                action: MonitorAction::Brightness {
                    value: BrightnessValue::Min,
                    via: None
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "min", "-v", "ddc"]),
            Err(
                "-v, --via is not valid with min, max, or boost. use a number to choose a backend"
                    .to_string()
            )
        );
    }

    #[test]
    fn contrast_flags_before_value() {
        assert_eq!(
            parse(&["monitor", "contrast", "-m", "2", "60"]),
            Ok(Command::Monitor {
                action: MonitorAction::Contrast {
                    value: 60,
                    via: None
                },
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
    }

    #[test]
    fn contrast_reset_with_monitor_flag() {
        assert_eq!(
            parse(&["monitor", "contrast", "-m", "2", "reset"]),
            Ok(Command::Monitor {
                action: MonitorAction::ContrastReset,
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
        assert_eq!(
            parse(&["monitor", "contrast", "reset", "-m", "2"]),
            Ok(Command::Monitor {
                action: MonitorAction::ContrastReset,
                monitor: MonitorTarget::Index(2),
                yes: false,
            })
        );
    }

    #[test]
    fn monitor_flag_missing_value_before_action() {
        assert_eq!(
            parse(&["monitor", "-m"]),
            Err("-m, --monitor needs a value. a monitor ID or all\ne.g. -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn brightness_missing_monitor_value_still_errors() {
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-m"]),
            Err("-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string())
        );
    }

    #[test]
    fn monitor_short_help_flag() {
        assert_eq!(
            parse(&["monitor", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor { action: None })
            })
        );
        assert_eq!(
            parse(&["monitor", "detach", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Disable)
                })
            })
        );
        assert_eq!(
            parse(&["monitor", "brightness", "60", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Monitor {
                    action: Some(MonitorAction::Brightness {
                        value: BrightnessValue::Percent(60),
                        via: None
                    })
                })
            })
        );
    }

    #[test]
    fn monitor_version_flags() {
        assert_eq!(parse(&["monitor", "--version"]), Ok(Command::Version));
        assert_eq!(
            parse(&["monitor", "detach", "--version"]),
            Ok(Command::Version)
        );
        assert_eq!(
            parse(&["monitor", "brightness", "60", "--version"]),
            Ok(Command::Version)
        );
    }

    #[test]
    fn monitor_long_form_yes_matches_short_form() {
        let expected = Ok(Command::Monitor {
            action: MonitorAction::Disable,
            monitor: MonitorTarget::Id(SERIAL_A.to_string()),
            yes: true,
        });
        assert_eq!(
            parse(&["monitor", "-y", "detach", "-m", SERIAL_A]),
            expected
        );
        assert_eq!(
            parse(&["monitor", "--yes", "detach", "-m", SERIAL_A]),
            expected
        );
    }

    #[test]
    fn monitor_long_form_monitor_matches_short_form() {
        let expected = Ok(Command::Monitor {
            action: MonitorAction::Disable,
            monitor: MonitorTarget::Index(2),
            yes: false,
        });
        assert_eq!(parse(&["monitor", "-m", "2", "detach"]), expected);
        assert_eq!(parse(&["monitor", "--monitor", "2", "detach"]), expected);
    }

    #[test]
    fn monitor_brightness_via_before_keyword_rejects() {
        assert_eq!(
            parse(&["monitor", "brightness", "-v", "ddc", "min"]),
            Err(
                "-v, --via is not valid with min, max, or boost. use a number to choose a backend"
                    .to_string()
            )
        );
    }

    #[test]
    fn monitor_contrast_via_before_reset_rejects() {
        assert_eq!(
            parse(&["monitor", "contrast", "-v", "ddc", "reset"]),
            Err("-v, --via is not valid with reset. reset restores defaults".to_string())
        );
    }
}
