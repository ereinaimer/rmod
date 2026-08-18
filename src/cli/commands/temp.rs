//! `temp` command: set, reset, or show the display color temperature.
//!
//! [`run_temp`] applies the action to the targeted display(s) and prints one
//! line per affected display. There is no confirmation flow: `temp reset`
//! undoes a change in a single keystroke.

use crate::cli::flags::{TEMP_MAX_KELVIN, TEMP_MIN_KELVIN, TEMP_PRESETS};
use crate::cli::parser::parse_monitor_target;
use crate::cli::{Command, HelpTopic, MonitorTarget, TempAction};
use crate::sys::windows::{self, TempChange};

use super::resolve_target;

/// Runs the `temp` command with the parsed action and target.
pub(super) fn run_temp(action: TempAction, monitor: MonitorTarget) -> i32 {
    match monitor {
        MonitorTarget::Primary | MonitorTarget::Id(_) | MonitorTarget::Index(_) => {
            let monitor_idx = match resolve_target(&monitor) {
                Ok(idx) => idx,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let result = match action {
                TempAction::Set(kelvin) => windows::set_temp(monitor_idx, kelvin),
                TempAction::Reset => windows::reset_temp(monitor_idx),
                TempAction::Show => windows::get_temp(monitor_idx),
            };
            match result {
                Ok(change) => {
                    println!("{}", report(&change, &action));
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        MonitorTarget::All => run_all(action),
    }
}

/// Applies a temperature action to every attached display.
fn run_all(action: TempAction) -> i32 {
    let devices = windows::enumerate_devices();
    if devices.is_empty() {
        eprintln!("error: no displays found, connect a display and try again");
        return 2;
    }
    let mut any_error = false;
    for (idx, _name) in devices.iter().enumerate() {
        let monitor_num = (idx + 1) as u32;
        let result = match action {
            TempAction::Set(kelvin) => windows::set_temp(Some(monitor_num), kelvin),
            TempAction::Reset => windows::reset_temp(Some(monitor_num)),
            TempAction::Show => windows::get_temp(Some(monitor_num)),
        };
        match result {
            Ok(change) => println!("{}", report(&change, &action)),
            Err(e) => {
                eprintln!("error: {e}");
                any_error = true;
            }
        }
    }
    if any_error { 2 } else { 0 }
}

/// Renders one line of output for a temperature action.
fn report(change: &TempChange, action: &TempAction) -> String {
    match action {
        TempAction::Set(_) => format!("set {} to {}K", change.display, change.kelvin),
        TempAction::Reset => format!("reset {} to 6500K", change.display),
        TempAction::Show => format!("{} is currently approx {}K", change.display, change.kelvin),
    }
}

pub(crate) fn parse_temp(args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut action = TempAction::Show;
    let mut monitor = MonitorTarget::Primary;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Temp),
                });
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
            other => {
                if !matches!(action, TempAction::Show) {
                    return Err(format!(
                        "unexpected argument {other} for temp. use a Kelvin value, a preset, or reset"
                    ));
                }
                action = parse_temp_value(other)?;
                i += 1;
            }
        }
    }

    Ok(Command::Temp { action, monitor })
}

fn parse_temp_value(arg: &str) -> Result<TempAction, String> {
    if arg == "reset" {
        return Ok(TempAction::Reset);
    }
    let lower = arg.to_lowercase();
    if let Some((_, _, kelvin)) = TEMP_PRESETS
        .iter()
        .find(|(name, alias, _)| *name == lower || *alias == lower)
    {
        return Ok(TempAction::Set(*kelvin));
    }
    let digits = lower.strip_suffix('k').unwrap_or(&lower);
    if let Ok(kelvin) = digits.parse::<u32>()
        && (TEMP_MIN_KELVIN..=TEMP_MAX_KELVIN).contains(&kelvin)
    {
        return Ok(TempAction::Set(kelvin));
    }
    Err(format!(
        "invalid temperature {arg}. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERIAL_A: &str = "ABC12345678";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn temp_no_args_is_show() {
        assert_eq!(
            parse(&["temp"]),
            Ok(Command::Temp {
                action: TempAction::Show,
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn temp_kelvin_sets_value() {
        assert_eq!(
            parse(&["temp", "3400"]),
            Ok(Command::Temp {
                action: TempAction::Set(3400),
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn temp_kelvin_k_suffix() {
        for arg in ["4000k", "4000K"] {
            assert_eq!(
                parse(&["temp", arg]),
                Ok(Command::Temp {
                    action: TempAction::Set(4000),
                    monitor: MonitorTarget::Primary
                }),
                "arg '{arg}'"
            );
        }
    }

    #[test]
    fn temp_preset_sets_value() {
        assert_eq!(
            parse(&["temp", "warm"]),
            Ok(Command::Temp {
                action: TempAction::Set(2700),
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn temp_alias_sets_value() {
        assert_eq!(
            parse(&["temp", "incandescent"]),
            Ok(Command::Temp {
                action: TempAction::Set(2700),
                monitor: MonitorTarget::Primary
            })
        );
        assert_eq!(
            parse(&["temp", "ember"]),
            Ok(Command::Temp {
                action: TempAction::Set(1900),
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn temp_all_presets_and_aliases() {
        for (name, alias, kelvin) in TEMP_PRESETS {
            let expected = Ok(Command::Temp {
                action: TempAction::Set(*kelvin),
                monitor: MonitorTarget::Primary,
            });
            assert_eq!(parse(&["temp", name]), expected, "name '{name}'");
            assert_eq!(parse(&["temp", alias]), expected, "alias '{alias}'");
        }
    }

    #[test]
    fn temp_preset_case_insensitive() {
        assert_eq!(parse(&["temp", "Warm"]), parse(&["temp", "warm"]));
    }

    #[test]
    fn temp_reset() {
        assert_eq!(
            parse(&["temp", "reset"]),
            Ok(Command::Temp {
                action: TempAction::Reset,
                monitor: MonitorTarget::Primary
            })
        );
    }

    #[test]
    fn temp_with_monitor() {
        assert_eq!(
            parse(&["temp", "-m", SERIAL_A, "4000"]),
            Ok(Command::Temp {
                action: TempAction::Set(4000),
                monitor: MonitorTarget::Id(SERIAL_A.to_string())
            })
        );
    }

    #[test]
    fn temp_with_all() {
        assert_eq!(
            parse(&["temp", "-m", "all", "3000"]),
            Ok(Command::Temp {
                action: TempAction::Set(3000),
                monitor: MonitorTarget::All
            })
        );
    }

    #[test]
    fn temp_monitor_only_is_show() {
        assert_eq!(
            parse(&["temp", "-m", SERIAL_A]),
            Ok(Command::Temp {
                action: TempAction::Show,
                monitor: MonitorTarget::Id(SERIAL_A.to_string())
            })
        );
    }

    #[test]
    fn temp_help_flag() {
        assert!(parse(&["temp", "-h"]).is_err());
        assert_eq!(
            parse(&["temp", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Temp)
            })
        );
    }

    #[test]
    fn temp_invalid_value_is_error() {
        assert_eq!(
            parse(&["temp", "bogus"]),
            Err("invalid temperature bogus. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400".to_string())
        );
    }

    #[test]
    fn temp_out_of_range_is_error() {
        for arg in ["0", "500", "999", "6501", "9000", "19200"] {
            let expected = Err(format!(
                "invalid temperature {arg}. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400"
            ));
            assert_eq!(parse(&["temp", arg]), expected, "arg '{arg}'");
            assert_eq!(
                parse(&["temp", &format!("{arg}k")]),
                Err(format!(
                    "invalid temperature {arg}k. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400"
                )),
                "arg '{arg}k'"
            );
        }
    }

    #[test]
    fn temp_range_boundaries_are_accepted() {
        for arg in ["1000", "6500"] {
            assert_eq!(
                parse(&["temp", arg]),
                Ok(Command::Temp {
                    action: TempAction::Set(arg.parse().unwrap()),
                    monitor: MonitorTarget::Primary
                }),
                "arg '{arg}'"
            );
        }
    }

    #[test]
    fn temp_second_positional_is_error() {
        assert_eq!(
            parse(&["temp", "3000", "4000"]),
            Err(
                "unexpected argument 4000 for temp. use a Kelvin value, a preset, or reset"
                    .to_string()
            )
        );
    }

    #[test]
    fn temp_missing_monitor_value_is_error() {
        assert_eq!(
            parse(&["temp", "-m"]),
            Err(
                "-m, --monitor needs a value. a monitor ID, 'primary', or 'all'\ne.g. -m a1b2c3d4"
                    .to_string()
            )
        );
    }
}
