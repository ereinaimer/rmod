//! `brightness` command: set the backlight level of a display.
//!
//! [`run_brightness`] applies the level (0-100 or a composite mode: min,
//! max, boost) to the targeted display(s) and reports the outcome, printing
//! a clip warning when the applied boost ramp was clipped.

use crate::cli::parser::parse_monitor_target;
use crate::cli::{BrightnessBackend, Command, HelpTopic, MonitorTarget};
use crate::sys::windows::{
    self, BrightnessLayer, BrightnessOutcome, BrightnessValue, brightness::mode_word,
};

use super::resolve_target;

/// Runs the `brightness` command against the targeted display(s).
pub(super) fn run_brightness(
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

/// Parses `rmod brightness <VALUE> [OPTIONS]`.
///
/// The verb sits at `args[0]`; the value is the first positional after it:
/// a token that does not start with `-` and is not consumed as a `-m` or
/// `-v` value. `name` is the command word embedded in error messages
/// (`brightness` at root, `monitor brightness` through the old shim).
pub(crate) fn parse_brightness(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    let mut value: Option<BrightnessValue> = None;
    let mut monitor = MonitorTarget::Primary;
    let mut via = None;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Brightness {
                        value: value.unwrap_or(BrightnessValue::Percent(0)),
                        via,
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
                return Err(format!(
                    "-y, --yes is not valid for {name}. brightness does not prompt for confirmation"
                ));
            }
            other if other.starts_with('-') => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use -m/--monitor or -v/--via"
                ));
            }
            other => {
                if value.is_some() {
                    return Err(format!(
                        "unexpected argument {other} for {name}. use -m/--monitor or -v/--via"
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
        return Err(format!(
            "{name} needs a value. a number between 0 and 100\ne.g. rmod {name} 60"
        ));
    };
    Ok(Command::Brightness {
        value,
        via,
        monitor,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::{BrightnessBackend, BrightnessLayer, BrightnessOutcome};

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

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

    #[test]
    fn brightness_primary_default() {
        assert_eq!(
            parse(&["brightness", "60"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(60),
                via: None,
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn brightness_with_monitor_and_backend() {
        assert_eq!(
            parse(&["brightness", "40", "-m", "2", "--via", "ddc"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(40),
                via: Some(BrightnessBackend::Ddc),
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["brightness", "40", "-m", "all", "--via", "gamma"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(40),
                via: Some(BrightnessBackend::Gamma),
                monitor: MonitorTarget::All,
            })
        );
    }

    #[test]
    fn brightness_via_short_flag() {
        assert_eq!(
            parse(&["brightness", "80", "-v", "slider"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(80),
                via: Some(BrightnessBackend::Slider),
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn brightness_zero_is_valid() {
        assert_eq!(
            parse(&["brightness", "0", "-m", "1"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(0),
                via: None,
                monitor: MonitorTarget::Index(1),
            })
        );
    }

    #[test]
    fn brightness_missing_value_is_error() {
        assert_eq!(
            parse(&["brightness"]),
            Err(
                "brightness needs a value. a number between 0 and 100\ne.g. rmod brightness 60"
                    .to_string()
            )
        );
    }

    #[test]
    fn brightness_out_of_range_is_error() {
        assert_eq!(
            parse(&["brightness", "150"]),
            Err("invalid brightness 150. use a number between 0 and 100".to_string())
        );
        assert_eq!(
            parse(&["brightness", "abc"]),
            Err("invalid brightness abc. use a number between 0 and 100".to_string())
        );
    }

    #[test]
    fn brightness_unknown_backend_is_error() {
        assert_eq!(
            parse(&["brightness", "60", "--via", "gamma2"]),
            Err("unknown backend gamma2. use ddc, slider, or gamma".to_string())
        );
    }

    #[test]
    fn brightness_rejects_yes_flag() {
        assert_eq!(
            parse(&["brightness", "60", "-y"]),
            Err(
                "-y, --yes is not valid for brightness. brightness does not prompt for confirmation"
                    .to_string()
            )
        );
    }

    #[test]
    fn brightness_help_routes() {
        assert_eq!(
            parse(&["brightness", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Brightness {
                    value: BrightnessValue::Percent(0),
                    via: None
                })
            })
        );
        assert_eq!(
            parse(&["brightness", "60", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Brightness {
                    value: BrightnessValue::Percent(60),
                    via: None
                })
            })
        );
    }

    #[test]
    fn brightness_version_flag() {
        assert_eq!(parse(&["brightness", "--version"]), Ok(Command::Version));
    }

    #[test]
    fn brightness_min_max_boost_keywords() {
        for (arg, value) in [
            ("min", BrightnessValue::Min),
            ("max", BrightnessValue::Max),
            ("boost", BrightnessValue::Boost),
        ] {
            assert_eq!(
                parse(&["brightness", arg]),
                Ok(Command::Brightness {
                    value,
                    via: None,
                    monitor: MonitorTarget::Primary,
                }),
                "arg '{arg}'"
            );
        }
    }

    #[test]
    fn brightness_keyword_with_monitor() {
        assert_eq!(
            parse(&["brightness", "min", "-m", "2"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Min,
                via: None,
                monitor: MonitorTarget::Index(2),
            })
        );
    }

    #[test]
    fn brightness_keywords_are_case_insensitive() {
        for (arg, value) in [
            ("Min", BrightnessValue::Min),
            ("MIN", BrightnessValue::Min),
            ("mAx", BrightnessValue::Max),
            ("BOOST", BrightnessValue::Boost),
        ] {
            assert_eq!(
                parse(&["brightness", arg]),
                Ok(Command::Brightness {
                    value,
                    via: None,
                    monitor: MonitorTarget::Primary,
                }),
                "arg '{arg}'"
            );
        }
    }

    #[test]
    fn brightness_keyword_rejects_via() {
        for args in [
            &["brightness", "min", "-v", "ddc"][..],
            &["brightness", "min", "--via", "ddc"][..],
            &["brightness", "max", "-v", "slider"][..],
            &["brightness", "boost", "--via", "gamma"][..],
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
    fn brightness_via_before_keyword_rejects() {
        assert_eq!(
            parse(&["brightness", "-v", "ddc", "min"]),
            Err(
                "-v, --via is not valid with min, max, or boost. use a number to choose a backend"
                    .to_string()
            )
        );
    }

    #[test]
    fn brightness_unknown_argument_is_error() {
        assert_eq!(
            parse(&["brightness", "60", "foo"]),
            Err("unexpected argument foo for brightness. use -m/--monitor or -v/--via".to_string())
        );
    }

    #[test]
    fn brightness_flag_like_values_are_error() {
        assert_eq!(
            parse(&["brightness", "60", "-m", "--via"]),
            Err("-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string())
        );
        assert_eq!(
            parse(&["brightness", "60", "--via", "-y"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn brightness_via_short_flag_missing_value() {
        assert_eq!(
            parse(&["brightness", "60", "-v"]),
            Err("-v, --via needs a value. ddc, slider, or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn brightness_flags_before_value() {
        assert_eq!(
            parse(&["brightness", "-m", "2", "60"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(60),
                via: None,
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["brightness", "-v", "ddc", "60"]),
            Ok(Command::Brightness {
                value: BrightnessValue::Percent(60),
                via: Some(BrightnessBackend::Ddc),
                monitor: MonitorTarget::Primary,
            })
        );
    }
}
