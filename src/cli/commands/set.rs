//! `set` command: applies a resolution, refresh and orientation policy to
//! the targeted display(s).
//!
//! [`run_set`] reports the outcome per display, then runs the shared
//! keep-or-revert confirmation flow for the changes it applied.

use crate::cli::flags::{ORIENTATIONS, PROFILES, TEMP_MAX_KELVIN, TEMP_MIN_KELVIN, TEMP_PRESETS};
use crate::cli::parser::parse_monitor_target;
use crate::cli::{Command, HelpTopic, MonitorTarget, SetSpec};
use crate::sys::windows::{self, ApplyOutcome, BrightnessOutcome, BrightnessValue, BrightnessLayer, Refresh};

use super::{confirm_or_revert, confirm_or_revert_all, describe_outcome, resolve_target};

/// Runs the brightness+temp composition: applies brightness with a color temperature.
fn run_brightness_temp(
    brightness: BrightnessValue,
    temp: u32,
    monitor: MonitorTarget,
    _yes: bool,
) -> i32 {
    match monitor {
        MonitorTarget::Primary | MonitorTarget::Id(_) | MonitorTarget::Index(_) => {
            let monitor_idx = match resolve_target(&monitor) {
                Ok(idx) => idx,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            match windows::set_brightness(monitor_idx, brightness, None, Some(temp)) {
                Ok(outcome) => {
                    println!("{}", describe_brightness_temp(&outcome, temp));
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        MonitorTarget::All => {
            let devices = windows::enumerate_devices();
            let mut any_error = false;
            for (idx, _name) in devices.iter().enumerate() {
                let monitor_num = (idx + 1) as u32;
                match windows::set_brightness(Some(monitor_num), brightness, None, Some(temp)) {
                    Ok(outcome) => println!("{}", describe_brightness_temp(&outcome, temp)),
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

/// Describes a brightness+temp outcome.
fn describe_brightness_temp(outcome: &BrightnessOutcome, temp: u32) -> String {
    match outcome.kind {
        BrightnessValue::Percent(value) => {
            if outcome.unchanged {
                format!(
                    "{} is already at {}% with {}K",
                    outcome.display, value, temp
                )
            } else {
                format!(
                    "set {} brightness to {}% with {}K via {}",
                    outcome.display,
                    value,
                    temp,
                    layer_backend(outcome)
                )
            }
        }
        BrightnessValue::Min | BrightnessValue::Max | BrightnessValue::Boost => {
            if outcome.unchanged {
                format!(
                    "{} is already at {} with {}K",
                    outcome.display,
                    mode_word(outcome.kind),
                    temp
                )
            } else {
                format!(
                    "set {} brightness to {} ({}) with {}K",
                    outcome.display,
                    mode_word(outcome.kind),
                    describe_layers(outcome),
                    temp
                )
            }
        }
    }
}

/// The backend word of the outcome's hardware layer, or `gamma` for a
/// gamma layer.
fn layer_backend(outcome: &BrightnessOutcome) -> &str {
    match outcome.layers.first() {
        Some(crate::sys::windows::BrightnessLayer::Hardware { backend, .. }) => backend.name(),
        Some(crate::sys::windows::BrightnessLayer::Gamma { .. }) => "gamma",
        None => unreachable!("outcomes always carry at least one layer"),
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
fn describe_layer(layer: &crate::sys::windows::BrightnessLayer) -> String {
    match layer {
        crate::sys::windows::BrightnessLayer::Hardware { backend, level } => {
            format!("{} {}", backend.name(), level)
        }
        crate::sys::windows::BrightnessLayer::Gamma { level } => format!("gamma {level}%"),
    }
}

/// The lowercase CLI word of a mode, used in errors and output.
fn mode_word(mode: BrightnessValue) -> &'static str {
    match mode {
        BrightnessValue::Min => "min",
        BrightnessValue::Max => "max",
        BrightnessValue::Boost => "boost",
        BrightnessValue::Percent(_) => unreachable!("percent is not a mode"),
    }
}

/// Resolves a SetSpec to width, height, and refresh using current display state.
fn resolve_spec(
    spec: &SetSpec,
    _current_width: u32,
    _current_height: u32,
) -> (Option<u32>, Option<u32>, Refresh) {
    use crate::cli::flags::PROFILES;
    match spec {
        SetSpec::Profile(name) => {
            let (_, w, h) = PROFILES
                .iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
                .unwrap();
            (Some(*w), Some(*h), Refresh::Keep)
        }
        SetSpec::ProfileWithRefresh(name, refresh) => {
            let (_, w, h) = PROFILES
                .iter()
                .find(|(n, _, _)| n.eq_ignore_ascii_case(name))
                .unwrap();
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
        SetSpec::BrightnessTemp { .. } => (None, None, Refresh::Keep),
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
    if let SetSpec::BrightnessTemp { brightness, temp } = spec {
        return run_brightness_temp(brightness, temp, monitor, yes);
    }
    if spec == SetSpec::Max {
        match monitor {
            MonitorTarget::Primary | MonitorTarget::Id(_) | MonitorTarget::Index(_) => {
                let monitor_idx = match resolve_target(&monitor) {
                    Ok(idx) => idx,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                };
                match windows::max(monitor_idx, orientation) {
                    Ok(ApplyOutcome::Unchanged(change)) => {
                        println!("{}", describe_outcome(&change, Some(&change.display), true));
                        0
                    }
                    Ok(ApplyOutcome::Applied(change)) => {
                        println!("{}", describe_outcome(&change, Some(&change.display), true));
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
            MonitorTarget::Primary | MonitorTarget::Id(_) | MonitorTarget::Index(_) => {
                let monitor_idx = match resolve_target(&monitor) {
                    Ok(idx) => idx,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                };
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
                        println!(
                            "{}",
                            describe_outcome(&change, Some(&change.display), mode_requested)
                        );
                        0
                    }
                    Ok(ApplyOutcome::Applied(change)) => {
                        println!(
                            "{}",
                            describe_outcome(&change, Some(&change.display), mode_requested)
                        );
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

pub(crate) fn parse_set(args: &[impl AsRef<str>]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("set needs something to change. width/height, refresh rate, profile, --max, or -b/--brightness with -t/--temp\ne.g. rmod set -p 1080".to_string());
    }

    let mut width = None;
    let mut height = None;
    let mut refresh = None;
    let mut profile = None;
    let mut monitor = MonitorTarget::Primary;
    let mut orientation = None;
    let mut yes = false;
    let mut max_flag = false;
    let mut brightness: Option<BrightnessValue> = None;
    let mut temp: Option<u32> = None;
    let mut brightness_set = false;
    let mut temp_set = false;

    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Set),
                });
            }
            "--version" => return Ok(Command::Version),
            "-w" | "--width" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-w, --width needs a value. a number of pixels\ne.g. -w 1920".to_string(),
                    );
                };
                width = Some(val.as_ref().parse::<u32>().map_err(|_| {
                    format!(
                        "invalid width {}. use a number of pixels\ne.g. 1920",
                        val.as_ref()
                    )
                })?);
                i += 1;
            }
            "-h" | "--height" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-h, --height needs a value. a number of pixels\ne.g. -h 1080".to_string(),
                    );
                };
                height = Some(val.as_ref().parse::<u32>().map_err(|_| {
                    format!(
                        "invalid height {}. use a number of pixels\ne.g. 1080",
                        val.as_ref()
                    )
                })?);
                i += 1;
            }
            "-r" | "--refresh" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-r, --refresh needs a value. a number in Hz or max\ne.g. -r 144"
                            .to_string(),
                    );
                };
                refresh = Some(parse_refresh(val.as_ref())?);
                i += 1;
            }
            "-p" | "--profile" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-p, --profile needs a value. 720, 1080, 1440, 4k, or 8k\ne.g. -p 1080"
                            .to_string(),
                    );
                };
                let val = val.as_ref();
                let lower = val.to_lowercase();
                let canonical = lower.strip_suffix('p').unwrap_or(&lower);
                if let Some((name, _, _)) = PROFILES.iter().find(|(n, _, _)| *n == canonical) {
                    profile = Some(name.to_string());
                } else {
                    let names = PROFILES
                        .iter()
                        .map(|(name, _, _)| *name)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!("unknown profile {}. use one of: {}", lower, names));
                }
                i += 1;
            }
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID, 'primary', or 'all'\ne.g. -m a1b2c3d4".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID, 'primary', or 'all'\ne.g. -m a1b2c3d4".to_string(),
                    );
                }
                monitor = parse_monitor_target(val)?;
                i += 1;
            }
            "-o" | "--orientation" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-o, --orientation needs a value. 0, 90, 180, or 270\ne.g. -o 90"
                            .to_string(),
                    );
                };
                orientation = Some(parse_orientation(val.as_ref())?);
                i += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "--max" => {
                max_flag = true;
                i += 1;
            }
            "-b" | "--brightness" => {
                if brightness_set {
                    return Err("--brightness specified multiple times".to_string());
                }
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-b, --brightness needs a value. a number between 0 and 100, or min, max, boost\ne.g. -b 50".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-b, --brightness needs a value. a number between 0 and 100, or min, max, boost\ne.g. -b 50".to_string(),
                    );
                }
                let new_value = match val.to_lowercase().as_str() {
                    "min" => BrightnessValue::Min,
                    "max" => BrightnessValue::Max,
                    "boost" => BrightnessValue::Boost,
                    _ => BrightnessValue::Percent(val.parse::<u32>().map_err(|_| {
                        format!("invalid brightness {val}. use a number between 0 and 100")
                    })?),
                };
                if let BrightnessValue::Percent(v) = new_value
                    && v > 100
                {
                    return Err(format!(
                        "invalid brightness {val}. use a number between 0 and 100"
                    ));
                }
                brightness = Some(new_value);
                brightness_set = true;
                i += 1;
            }
            "-t" | "--temp" => {
                if temp_set {
                    return Err("--temp specified multiple times".to_string());
                }
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-t, --temp needs a value. a Kelvin value (1000-6500) or a preset\ne.g. -t 3400".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-t, --temp needs a value. a Kelvin value (1000-6500) or a preset\ne.g. -t 3400".to_string(),
                    );
                }
                let kelvin = parse_temp_value(val)?;
                if !(TEMP_MIN_KELVIN..=TEMP_MAX_KELVIN).contains(&kelvin) {
                    return Err(format!(
                        "invalid temperature {val}. use a Kelvin value (1000-6500) or a preset"
                    ));
                }
                temp = Some(kelvin);
                temp_set = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for set. use --width, --height, --refresh, --profile, --monitor, --orientation, --max, --brightness, or --temp",
                    other
                ));
            }
        }
    }

    let has_resolution_spec = width.is_some() || height.is_some() || refresh.is_some() || profile.is_some() || max_flag;
    let has_brightness_temp = brightness_set || temp_set;

    if has_resolution_spec && has_brightness_temp {
        return Err(
            "use `rmod monitor brightness` / `rmod temp` for single actions; `-b` and `-t` compose".to_string(),
        );
    }

    if max_flag && (width.is_some() || height.is_some() || refresh.is_some() || profile.is_some()) {
        return Err("use --max alone or one of: width/height, refresh, profile".to_string());
    }

    if brightness_set && !temp_set {
        return Err(
            "use rmod monitor brightness for brightness only".to_string(),
        );
    }
    if temp_set && !brightness_set {
        return Err(
            "use rmod temp for temperature only".to_string(),
        );
    }

    if (width.is_some() && height.is_none()) || (width.is_none() && height.is_some()) {
        return Err(
            "-w, --width and -h, --height must be used together\ne.g. -w 1920 -h 1080".to_string(),
        );
    }

    if profile.is_some() && (width.is_some() || height.is_some()) {
        return Err("use --profile or explicit width/height, not both".to_string());
    }

    let spec = if brightness_set && temp_set {
        SetSpec::BrightnessTemp {
            brightness: brightness.unwrap(),
            temp: temp.unwrap(),
        }
    } else if max_flag {
        SetSpec::Max
    } else if let Some(p) = profile {
        if let Some(r) = refresh {
            SetSpec::ProfileWithRefresh(p, r)
        } else {
            SetSpec::Profile(p)
        }
    } else if let Some(w) = width {
        let h = height.unwrap();
        let r = refresh.unwrap_or(Refresh::Keep);
        SetSpec::Explicit {
            width: w,
            height: h,
            refresh: r,
        }
    } else if let Some(r) = refresh {
        SetSpec::RefreshOnly(r)
    } else {
        SetSpec::Keep
    };

    Ok(Command::Set {
        spec,
        monitor,
        orientation,
        yes,
    })
}

fn parse_temp_value(arg: &str) -> Result<u32, String> {
    let lower = arg.to_lowercase();
    if let Some((_, _, kelvin)) = TEMP_PRESETS
        .iter()
        .find(|(name, alias, _)| *name == lower || *alias == lower)
    {
        return Ok(*kelvin);
    }
    let digits = lower.strip_suffix('k').unwrap_or(&lower);
    digits.parse::<u32>().map_err(|_| {
        format!(
            "invalid temperature {arg}. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400"
        )
    })
}

fn parse_refresh(arg: &str) -> Result<Refresh, String> {
    match arg.to_lowercase().as_str() {
        "max" => Ok(Refresh::Max),
        _ => arg
            .parse::<u32>()
            .map(Refresh::Fixed)
            .map_err(|_| format!("invalid refresh rate {}. use a number in Hz or max", arg)),
    }
}

fn parse_orientation(arg: &str) -> Result<u32, String> {
    match arg.to_lowercase().as_str() {
        "0" | "l" | "landscape" => Ok(0),
        "90" | "p" | "portrait" => Ok(90),
        "180" | "lf" | "landscape-flipped" => Ok(180),
        "270" | "pf" | "portrait-flipped" => Ok(270),
        _ => {
            let angles = ORIENTATIONS
                .iter()
                .map(|(angle, _, _)| angle.to_string())
                .collect::<Vec<_>>();
            let aliases = ORIENTATIONS
                .iter()
                .map(|(_, _, alias)| *alias)
                .collect::<Vec<_>>()
                .join(", ");
            let names = ORIENTATIONS
                .iter()
                .map(|(_, name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "invalid orientation {}. use {}, or {} (also: {aliases}, {names})",
                arg,
                angles[..angles.len() - 1].join(", "),
                angles[angles.len() - 1],
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_spec_matches_profile_case_insensitively() {
        assert_eq!(
            resolve_spec(&SetSpec::Profile("4K".to_string()), 1920, 1080),
            (Some(3840), Some(2160), Refresh::Keep)
        );
        assert_eq!(
            resolve_spec(
                &SetSpec::ProfileWithRefresh("8K".to_string(), Refresh::Max),
                1920,
                1080
            ),
            (Some(7680), Some(4320), Refresh::Max)
        );
    }

    const SERIAL_A: &str = "ABC12345678";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn set_max_command() {
        assert_eq!(
            parse(&["set", "--max"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_with_monitor() {
        assert_eq!(
            parse(&["set", "--max", "-m", SERIAL_A]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_with_all() {
        assert_eq!(
            parse(&["set", "--max", "-m", "all"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::All,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_max_yes_flag() {
        assert_eq!(
            parse(&["set", "--max", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_yes_flag_with_monitor() {
        assert_eq!(
            parse(&["set", "--max", "-m", SERIAL_A, "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: None,
                yes: true
            })
        );
        assert_eq!(
            parse(&["set", "-y", "--max", "-m", SERIAL_A]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_all_with_yes() {
        assert_eq!(
            parse(&["set", "--max", "-m", "all", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::All,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_any_string_is_id() {
        assert_eq!(
            parse(&["set", "--max", "-m", "x"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Id("x".to_string()),
                orientation: None,
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "--max", "-m", "2"]),
            Ok(Command::Set {
                spec: SetSpec::Max,
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: false
            })
        );
        assert!(parse(&["set", "--max", "-m", "0"]).is_err());
    }

    #[test]
    fn set_duplicate_monitor_last_wins() {
        assert_eq!(
            parse(&["set", "-m", "2", "-m", "3", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Index(3),
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_max_conflicting_spec_is_error() {
        assert!(parse(&["set", "-p", "1080", "--max"]).is_err());
        assert!(parse(&["set", "--max", "-p", "1080"]).is_err());
        assert!(parse(&["set", "-w", "1920", "-h", "1080", "--max"]).is_err());
    }

    #[test]
    fn set_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["set", "-m", "--max"]),
            Err(
                "-m, --monitor needs a value. a monitor ID, 'primary', or 'all'\ne.g. -m a1b2c3d4"
                    .to_string()
            )
        );
    }

    #[test]
    fn set_command() {
        assert_eq!(
            parse(&["set", "-p", "1080"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1080".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_monitor() {
        assert_eq!(
            parse(&["set", "-p", "4k", "-r", "144", "-m", SERIAL_A]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("4k".to_string(), Refresh::Fixed(144)),
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_all() {
        assert_eq!(
            parse(&["set", "-r", "60", "-m", "all"]),
            Ok(Command::Set {
                spec: SetSpec::RefreshOnly(Refresh::Fixed(60)),
                monitor: MonitorTarget::All,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_with_orientation() {
        assert_eq!(
            parse(&[
                "set", "-w", "1920", "-h", "1080", "-m", SERIAL_A, "-o", "90"
            ]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: Some(90),
                yes: false
            })
        );
    }

    #[test]
    fn set_with_yes() {
        assert_eq!(
            parse(&["set", "-p", "1440", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1440".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_explicit_resolution_and_refresh() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-r", "144"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Fixed(144)
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_long_form_full_invocation() {
        assert_eq!(
            parse(&[
                "set",
                "--width",
                "1920",
                "--height",
                "1080",
                "--refresh",
                "60",
                "--orientation",
                "landscape",
                "--monitor",
                "2",
                "--yes"
            ]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Fixed(60)
                },
                monitor: MonitorTarget::Index(2),
                orientation: Some(0),
                yes: true
            })
        );
    }

    #[test]
    fn set_long_form_profile() {
        assert_eq!(
            parse(&["set", "--profile", "1080", "--monitor", SERIAL_A, "--yes"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1080".to_string()),
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_explicit_no_refresh() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_refresh_only() {
        assert_eq!(
            parse(&["set", "-r", "max"]),
            Ok(Command::Set {
                spec: SetSpec::RefreshOnly(Refresh::Max),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_profile_with_refresh() {
        assert_eq!(
            parse(&["set", "-p", "720", "-r", "60"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("720".to_string(), Refresh::Fixed(60)),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_profile_with_max_refresh() {
        assert_eq!(
            parse(&["set", "-p", "720", "-r", "max"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("720".to_string(), Refresh::Max),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_all_profiles() {
        for (name, _, _) in PROFILES {
            assert!(parse(&["set", "-p", name]).is_ok(), "profile '{}'", name);
        }
    }

    #[test]
    fn set_unknown_profile_is_error() {
        assert!(parse(&["set", "-p", "480"]).is_err());
        assert!(parse(&["set", "-p", "1080px"]).is_err());
    }

    #[test]
    fn set_profile_case_insensitive() {
        for (upper, lower) in [("4K", "4k"), ("720P", "720p")] {
            assert_eq!(
                parse(&["set", "-p", upper]),
                parse(&["set", "-p", lower]),
                "profile '{}' must parse to the same result as '{}'",
                upper,
                lower
            );
        }
    }

    #[test]
    fn set_profile_p_suffix_resolves_to_the_profile() {
        for variant in ["1080P", "1080p", "720P", "1440P", "4KP", "8KP"] {
            let (canonical, refresh) = match variant.to_lowercase().trim_end_matches('p') {
                "720" => ("720", None),
                "1080" => ("1080", None),
                "1440" => ("1440", None),
                "4k" => ("4k", None),
                "8k" => ("8k", Some("60")),
                _ => unreachable!(),
            };
            let mut args = vec!["set", "-p", variant];
            if let Some(r) = refresh {
                args.push("-r");
                args.push(r);
            }
            let expected = if let Some(r) = refresh {
                SetSpec::ProfileWithRefresh(
                    canonical.to_string(),
                    Refresh::Fixed(r.parse().unwrap()),
                )
            } else {
                SetSpec::Profile(canonical.to_string())
            };
            assert_eq!(
                parse(&args),
                Ok(Command::Set {
                    spec: expected,
                    monitor: MonitorTarget::Primary,
                    orientation: None,
                    yes: false
                }),
                "profile '{}' with a p suffix must resolve to '{}'",
                variant,
                canonical
            );
        }
    }

    #[test]
    fn set_profile_upper_case_stores_canonical_name() {
        assert_eq!(
            parse(&["set", "-p", "4K"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("4k".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-p", "8K", "-r", "60"]),
            Ok(Command::Set {
                spec: SetSpec::ProfileWithRefresh("8k".to_string(), Refresh::Fixed(60)),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_profile_p_suffix_stores_canonical_name() {
        assert_eq!(
            parse(&["set", "-p", "1080P"]),
            Ok(Command::Set {
                spec: SetSpec::Profile("1080".to_string()),
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_invalid_width_is_error() {
        assert!(parse(&["set", "-w", "abc", "-h", "1080"]).is_err());
    }

    #[test]
    fn set_invalid_height_is_error() {
        assert!(parse(&["set", "-w", "1920", "-h", "xyz"]).is_err());
    }

    #[test]
    fn set_invalid_refresh_is_error() {
        assert!(parse(&["set", "-r", "fast"]).is_err());
    }

    #[test]
    fn set_missing_spec_is_error() {
        assert_eq!(
            parse(&["set"]),
            Err(
                "set needs something to change. width/height, refresh rate, profile, --max, or -b/--brightness with -t/--temp\ne.g. rmod set -p 1080"
                    .to_string()
            )
        );
    }

    #[test]
    fn set_orientation_aliases() {
        for (token, angle) in [
            ("0", 0),
            ("l", 0),
            ("landscape", 0),
            ("90", 90),
            ("p", 90),
            ("portrait", 90),
            ("180", 180),
            ("lf", 180),
            ("landscape-flipped", 180),
            ("270", 270),
            ("pf", 270),
            ("portrait-flipped", 270),
        ] {
            assert_eq!(
                parse(&["set", "-w", "1920", "-h", "1080", "-o", token]),
                Ok(Command::Set {
                    spec: SetSpec::Explicit {
                        width: 1920,
                        height: 1080,
                        refresh: Refresh::Keep
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: Some(angle),
                    yes: false
                }),
                "angle '{}'",
                token
            );
        }
    }

    #[test]
    fn set_orientation_case_insensitive() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-o", "Portrait"]),
            Ok(Command::Set {
                spec: SetSpec::Explicit {
                    width: 1920,
                    height: 1080,
                    refresh: Refresh::Keep
                },
                monitor: MonitorTarget::Primary,
                orientation: Some(90),
                yes: false
            })
        );
    }

    #[test]
    fn set_invalid_orientation_is_error() {
        assert!(parse(&["set", "-w", "1920", "-h", "1080", "-o", "45"]).is_err());
    }

    #[test]
    fn set_missing_orientation_value_is_error() {
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "-o"]),
            Err("-o, --orientation needs a value. 0, 90, 180, or 270\ne.g. -o 90".to_string())
        );
    }

    #[test]
    fn set_help_flag() {
        assert!(parse(&["set", "-h"]).is_err());
        assert_eq!(
            parse(&["set", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Set)
            })
        );
    }

    #[test]
    fn set_version_flag() {
        assert_eq!(parse(&["set", "--version"]), Ok(Command::Version));
        assert_eq!(
            parse(&["set", "-w", "1920", "-h", "1080", "--version"]),
            Ok(Command::Version)
        );
    }

    #[test]
    fn set_optional_spec() {
        assert_eq!(
            parse(&["set", "-o", "portrait"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Primary,
                orientation: Some(90),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-m", SERIAL_A, "-o", "90"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                orientation: Some(90),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-m", "all", "-o", "landscape"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::All,
                orientation: Some(0),
                yes: false
            })
        );
        assert_eq!(
            parse(&["set", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::Keep,
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_brightness_temp_together_parse_correctly() {
        for (b_arg, b_val, t_arg, t_kelvin) in [
            ("50", BrightnessValue::Percent(50), "3400", 3400),
            ("min", BrightnessValue::Min, "warm", 2700),
            ("max", BrightnessValue::Max, "cool", 4500),
            ("boost", BrightnessValue::Boost, "candle", 1900),
            ("75", BrightnessValue::Percent(75), "4000k", 4000),
        ] {
            assert_eq!(
                parse(&["set", "-b", b_arg, "-t", t_arg]),
                Ok(Command::Set {
                    spec: SetSpec::BrightnessTemp {
                        brightness: b_val,
                        temp: t_kelvin,
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: None,
                    yes: false
                }),
                "b='{}', t='{}'",
                b_arg,
                t_arg
            );
        }
    }

    #[test]
    fn set_brightness_temp_long_flags() {
        assert_eq!(
            parse(&["set", "--brightness", "50", "--temp", "3400"]),
            Ok(Command::Set {
                spec: SetSpec::BrightnessTemp {
                    brightness: BrightnessValue::Percent(50),
                    temp: 3400,
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_brightness_alone_errors() {
        let err = parse(&["set", "-b", "50"]).unwrap_err();
        assert_eq!(err, "use rmod monitor brightness for brightness only");
    }

    #[test]
    fn set_temp_alone_errors() {
        let err = parse(&["set", "-t", "3400"]).unwrap_err();
        assert_eq!(err, "use rmod temp for temperature only");
    }

    #[test]
    fn set_brightness_temp_mutual_exclusion_with_resolution() {
        for args in [
            &["set", "-b", "50", "-t", "3400", "-w", "1920", "-h", "1080"][..],
            &["set", "-b", "50", "-t", "3400", "-p", "1080"][..],
            &["set", "-b", "50", "-t", "3400", "-r", "60"][..],
            &["set", "-b", "50", "-t", "3400", "--max"][..],
        ] {
            let err = parse(args).unwrap_err();
            assert_eq!(
                err,
                "use `rmod monitor brightness` / `rmod temp` for single actions; `-b` and `-t` compose",
                "args: {:?}",
                args
            );
        }
    }

    #[test]
    fn set_brightness_mode_words_with_temp() {
        for (mode, val) in [
            ("min", BrightnessValue::Min),
            ("max", BrightnessValue::Max),
            ("boost", BrightnessValue::Boost),
        ] {
            assert_eq!(
                parse(&["set", "-b", mode, "-t", "3400"]),
                Ok(Command::Set {
                    spec: SetSpec::BrightnessTemp {
                        brightness: val,
                        temp: 3400,
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: None,
                    yes: false
                }),
                "mode '{}'",
                mode
            );
        }
    }

    #[test]
    fn set_brightness_mode_words_case_insensitive_with_temp() {
        assert_eq!(
            parse(&["set", "-b", "Min", "-t", "3400"]),
            parse(&["set", "-b", "min", "-t", "3400"])
        );
        assert_eq!(
            parse(&["set", "-b", "BOOST", "-t", "3400"]),
            parse(&["set", "-b", "boost", "-t", "3400"])
        );
    }

    #[test]
    fn set_temp_presets_and_aliases_with_brightness() {
        for (preset, alias, kelvin) in crate::cli::flags::TEMP_PRESETS {
            assert_eq!(
                parse(&["set", "-b", "50", "-t", preset]),
                Ok(Command::Set {
                    spec: SetSpec::BrightnessTemp {
                        brightness: BrightnessValue::Percent(50),
                        temp: *kelvin,
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: None,
                    yes: false
                }),
                "preset '{}'",
                preset
            );
            assert_eq!(
                parse(&["set", "-b", "50", "-t", alias]),
                Ok(Command::Set {
                    spec: SetSpec::BrightnessTemp {
                        brightness: BrightnessValue::Percent(50),
                        temp: *kelvin,
                    },
                    monitor: MonitorTarget::Primary,
                    orientation: None,
                    yes: false
                }),
                "alias '{}'",
                alias
            );
        }
    }

    #[test]
    fn set_temp_kelvin_bounds_enforced() {
        for arg in ["0", "500", "999", "6501", "9000"] {
            let err = parse(&["set", "-b", "50", "-t", arg]).unwrap_err();
            assert!(
                err.contains("invalid temperature"),
                "arg '{}' gave: {}",
                arg,
                err
            );
        }
        // Boundaries are accepted
        assert!(parse(&["set", "-b", "50", "-t", "1000"]).is_ok());
        assert!(parse(&["set", "-b", "50", "-t", "6500"]).is_ok());
    }

    #[test]
    fn set_brightness_out_of_range_with_temp_errors() {
        let err = parse(&["set", "-b", "150", "-t", "3400"]).unwrap_err();
        assert!(err.contains("invalid brightness"));
        let err = parse(&["set", "-b", "abc", "-t", "3400"]).unwrap_err();
        assert!(err.contains("invalid brightness"));
    }

    #[test]
    fn set_brightness_temp_with_monitor() {
        assert_eq!(
            parse(&["set", "-b", "50", "-t", "3400", "-m", "2"]),
            Ok(Command::Set {
                spec: SetSpec::BrightnessTemp {
                    brightness: BrightnessValue::Percent(50),
                    temp: 3400,
                },
                monitor: MonitorTarget::Index(2),
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_brightness_temp_with_all() {
        assert_eq!(
            parse(&["set", "-b", "50", "-t", "3400", "-m", "all"]),
            Ok(Command::Set {
                spec: SetSpec::BrightnessTemp {
                    brightness: BrightnessValue::Percent(50),
                    temp: 3400,
                },
                monitor: MonitorTarget::All,
                orientation: None,
                yes: false
            })
        );
    }

    #[test]
    fn set_brightness_temp_with_yes() {
        assert_eq!(
            parse(&["set", "-b", "50", "-t", "3400", "-y"]),
            Ok(Command::Set {
                spec: SetSpec::BrightnessTemp {
                    brightness: BrightnessValue::Percent(50),
                    temp: 3400,
                },
                monitor: MonitorTarget::Primary,
                orientation: None,
                yes: true
            })
        );
    }

    #[test]
    fn set_brightness_temp_duplicate_brightness_error() {
        let err = parse(&["set", "-b", "50", "-b", "60", "-t", "3400"]).unwrap_err();
        assert_eq!(err, "--brightness specified multiple times");
    }

    #[test]
    fn set_brightness_temp_duplicate_temp_error() {
        let err = parse(&["set", "-b", "50", "-t", "3400", "-t", "4000"]).unwrap_err();
        assert_eq!(err, "--temp specified multiple times");
    }

    #[test]
    fn set_brightness_temp_missing_brightness_value_error() {
        let err = parse(&["set", "-b", "-t", "3400"]).unwrap_err();
        assert!(err.contains("-b, --brightness needs a value"));
    }

    #[test]
    fn set_brightness_temp_missing_temp_value_error() {
        let err = parse(&["set", "-b", "50", "-t"]).unwrap_err();
        assert!(err.contains("-t, --temp needs a value"));
    }
}
