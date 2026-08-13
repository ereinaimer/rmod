//! `caps` command: lists the supported modes of the targeted display(s).
//!
//! [`run_caps`] prints one section per monitor; [`print_caps`] marks the
//! active mode with a highlighted `*` using the `GREEN`/`RESET` escape
//! sequences.

use crate::cli::Target;
use crate::sys::windows::{self, Mode, Monitor};

use super::monitor_of;

const GREEN: &str = "\x1b[92m";
const RESET: &str = "\x1b[0m";

/// Lists the supported modes of the targeted display(s).
pub(super) fn run_caps(target: Target) -> i32 {
    match target {
        Target::Primary | Target::Index(_) => {
            let monitor = monitor_of(target);
            match windows::caps(monitor) {
                Ok((mon, modes)) => {
                    print_caps(&mon, &modes);
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        Target::All => match windows::caps_all() {
            Ok(monitors) => {
                for (mon, modes) in monitors {
                    print_caps(&mon, &modes);
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}

/// Prints the supported modes of one monitor, marking the active mode.
fn print_caps(mon: &Monitor, modes: &[Mode]) {
    let primary = if mon.is_primary { " (primary)" } else { "" };
    println!("{}{}:", mon.name, primary);
    let res_width = modes
        .iter()
        .map(|m| format!("{}x{}", m.width, m.height).len())
        .max()
        .unwrap_or(0);
    for mode in modes {
        let active =
            mode.width == mon.width && mode.height == mon.height && mode.refresh == mon.refresh;
        let marker = if active {
            format!("{GREEN}*{RESET} ")
        } else {
            "  ".to_string()
        };
        println!(
            "  {marker}{:<res_width$} @ {}Hz",
            format!("{}x{}", mode.width, mode.height),
            mode.refresh
        );
    }
}
