//! `temp` command: set, reset, or show the display color temperature.
//!
//! [`run_temp`] applies the action to the targeted display(s) and prints one
//! line per affected display. There is no confirmation flow: `temp reset`
//! undoes a change in a single keystroke.

use crate::cli::{MonitorTarget, TempAction};
use crate::sys::windows::{self, TempChange};

use super::monitor_of;

/// Runs the `temp` command with the parsed action and target.
pub(super) fn run_temp(action: TempAction, monitor: MonitorTarget) -> i32 {
    match monitor {
        MonitorTarget::Primary | MonitorTarget::Index(_) => {
            let monitor_idx = monitor_of(monitor);
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
    if any_error {
        2
    } else {
        0
    }
}

/// Renders one line of output for a temperature action.
fn report(change: &TempChange, action: &TempAction) -> String {
    match action {
        TempAction::Set(_) => format!("set {} to {}K", change.display, change.kelvin),
        TempAction::Reset => format!("reset {} to 6500K", change.display),
        TempAction::Show => format!("{} is currently approx {}K", change.display, change.kelvin),
    }
}