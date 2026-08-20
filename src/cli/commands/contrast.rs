//! `contrast` command: set or reset the display contrast.
//!
//! [`run_contrast`] applies the level (0-130; 100 = neutral, above 100
//! overdrives the gamma ramp) and [`run_contrast_reset`] restores the
//! defaults (DDC 100 + gamma identity), reporting the outcome and printing
//! a clip warning when the applied boost ramp was clipped.

use crate::cli::parser::parse_monitor_target;
use crate::cli::{Command, HelpTopic, MonitorTarget};
use crate::sys::windows::{self, ContrastBackend, ContrastOutcome};

use super::resolve_target;

/// Runs the contrast command against the targeted display(s).
pub(super) fn run_contrast(
    value: u32,
    via: Option<ContrastBackend>,
    monitor: MonitorTarget,
) -> i32 {
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
pub(super) fn run_contrast_reset(monitor: MonitorTarget) -> i32 {
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

/// Parses `rmod contrast <VALUE> [OPTIONS]`.
///
/// The verb sits at `args[0]`; the value is the first positional after it:
/// a token that does not start with `-` and is not consumed as a `-m` or
/// `-v` value. A value of `reset` (in either position) selects reset
/// semantics. `name` is the command word embedded in error messages
/// (`contrast` at root, `monitor contrast` through the old shim).
pub(crate) fn parse_contrast(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    let mut value: Option<u32> = None;
    let mut reset = false;
    let mut monitor = MonitorTarget::Primary;
    let mut via = None;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Contrast {
                        value: value.unwrap_or(0),
                        via,
                        reset,
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
                        "unexpected argument {arg} for {name} reset. use -m/--monitor"
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
                    format!(
                        "-y, --yes is not valid for {name} reset. reset does not prompt for confirmation"
                    )
                } else {
                    format!(
                        "-y, --yes is not valid for {name}. contrast does not prompt for confirmation"
                    )
                });
            }
            other if other.starts_with('-') => {
                return Err(if reset {
                    format!("unexpected argument {other} for {name} reset. use -m/--monitor")
                } else {
                    format!("unexpected argument {other} for {name}. use -m/--monitor or -v/--via")
                });
            }
            other => {
                if reset || value.is_some() {
                    return Err(if reset {
                        format!("unexpected argument {other} for {name} reset. use -m/--monitor")
                    } else {
                        format!(
                            "unexpected argument {other} for {name}. use -m/--monitor or -v/--via"
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
    if reset {
        return Ok(Command::ContrastReset { monitor });
    }
    let Some(value) = value else {
        return Err(format!(
            "{name} needs a value. a number between 0 and 130\ne.g. rmod {name} 60"
        ));
    };
    Ok(Command::Contrast {
        value,
        via,
        monitor,
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
    use crate::sys::windows::{ContrastBackend, ContrastOutcome};

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
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

    #[test]
    fn contrast_primary_default() {
        assert_eq!(
            parse(&["contrast", "60"]),
            Ok(Command::Contrast {
                value: 60,
                via: None,
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn contrast_with_monitor_and_backend() {
        assert_eq!(
            parse(&["contrast", "40", "-m", "2", "--via", "ddc"]),
            Ok(Command::Contrast {
                value: 40,
                via: Some(ContrastBackend::Ddc),
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["contrast", "40", "-m", "all", "--via", "gamma"]),
            Ok(Command::Contrast {
                value: 40,
                via: Some(ContrastBackend::Gamma),
                monitor: MonitorTarget::All,
            })
        );
    }

    #[test]
    fn contrast_via_short_flag() {
        assert_eq!(
            parse(&["contrast", "80", "-v", "ddc"]),
            Ok(Command::Contrast {
                value: 80,
                via: Some(ContrastBackend::Ddc),
                monitor: MonitorTarget::Primary,
            })
        );
    }

    #[test]
    fn contrast_slider_backend_is_error() {
        assert_eq!(
            parse(&["contrast", "60", "-v", "slider"]),
            Err("unknown backend slider. use ddc or gamma".to_string())
        );
    }

    #[test]
    fn contrast_zero_hundred_and_hundredthirty_are_valid() {
        for value in ["0", "100", "130"] {
            assert_eq!(
                parse(&["contrast", value]),
                Ok(Command::Contrast {
                    value: value.parse().unwrap(),
                    via: None,
                    monitor: MonitorTarget::Primary,
                }),
                "value {value}"
            );
        }
    }

    #[test]
    fn contrast_out_of_range_is_error() {
        assert_eq!(
            parse(&["contrast", "131"]),
            Err("invalid contrast 131. use a number between 0 and 130".to_string())
        );
    }

    #[test]
    fn contrast_non_numeric_is_error() {
        assert_eq!(
            parse(&["contrast", "fifty"]),
            Err("invalid contrast fifty. use a number between 0 and 130".to_string())
        );
    }

    #[test]
    fn contrast_missing_value_is_error() {
        assert_eq!(
            parse(&["contrast"]),
            Err(
                "contrast needs a value. a number between 0 and 130\ne.g. rmod contrast 60"
                    .to_string()
            )
        );
    }

    #[test]
    fn contrast_rejects_yes_flag() {
        assert_eq!(
            parse(&["contrast", "60", "-y"]),
            Err(
                "-y, --yes is not valid for contrast. contrast does not prompt for confirmation"
                    .to_string()
            )
        );
    }

    #[test]
    fn contrast_reset_with_monitor_flag() {
        assert_eq!(
            parse(&["contrast", "-m", "2", "reset"]),
            Ok(Command::ContrastReset {
                monitor: MonitorTarget::Index(2),
            })
        );
        assert_eq!(
            parse(&["contrast", "reset", "-m", "2"]),
            Ok(Command::ContrastReset {
                monitor: MonitorTarget::Index(2),
            })
        );
    }

    #[test]
    fn contrast_help_routes() {
        assert_eq!(
            parse(&["contrast", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Contrast {
                    value: 0,
                    via: None,
                    reset: false
                })
            })
        );
        assert_eq!(
            parse(&["contrast", "60", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Contrast {
                    value: 60,
                    via: None,
                    reset: false
                })
            })
        );
        assert_eq!(
            parse(&["contrast", "reset", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Contrast {
                    value: 0,
                    via: None,
                    reset: true
                })
            })
        );
    }

    #[test]
    fn contrast_version_flag() {
        assert_eq!(parse(&["contrast", "--version"]), Ok(Command::Version));
    }

    #[test]
    fn contrast_unknown_argument_is_error() {
        assert_eq!(
            parse(&["contrast", "60", "foo"]),
            Err("unexpected argument foo for contrast. use -m/--monitor or -v/--via".to_string())
        );
    }

    #[test]
    fn contrast_flag_like_values_are_error() {
        assert_eq!(
            parse(&["contrast", "60", "-m", "--via"]),
            Err("-m, --monitor needs a value. a monitor number or all\ne.g. -m 2".to_string())
        );
        assert_eq!(
            parse(&["contrast", "60", "--via", "-y"]),
            Err("-v, --via needs a value. ddc or gamma\ne.g. -v ddc".to_string())
        );
    }

    #[test]
    fn contrast_flags_before_value() {
        assert_eq!(
            parse(&["contrast", "-m", "2", "60"]),
            Ok(Command::Contrast {
                value: 60,
                via: None,
                monitor: MonitorTarget::Index(2),
            })
        );
    }

    #[test]
    fn contrast_via_before_reset_rejects() {
        assert_eq!(
            parse(&["contrast", "-v", "ddc", "reset"]),
            Err("-v, --via is not valid with reset. reset restores defaults".to_string())
        );
    }

    #[test]
    fn contrast_via_after_reset_rejects() {
        assert_eq!(
            parse(&["contrast", "reset", "-v", "ddc"]),
            Err("unexpected argument -v for contrast reset. use -m/--monitor".to_string())
        );
    }
}
