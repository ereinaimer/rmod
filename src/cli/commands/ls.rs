//! `ls` command: lists every display and its current settings.
//!
//! [`run_list`] renders a table of monitor numbers, primary markers,
//! names, resolutions and refresh rates, aligned to the widest entries.
//! With `caps` set it lists the supported modes of the targeted
//! display(s) instead, marking the active mode with a highlighted `*`
//! using the `GREEN`/`RESET` escape sequences.

use crate::cli::MonitorTarget;
use crate::sys::windows::{self, Mode, Monitor};

use super::monitor_of;

const GREEN: &str = "\x1b[92m";
const RESET: &str = "\x1b[0m";

/// Lists every display and its current settings, or — with `caps` — the
/// supported modes of the targeted display(s).
pub(super) fn run_list(caps: bool, monitor: MonitorTarget) -> i32 {
    if caps {
        return run_caps(monitor);
    }
    match windows::list() {
        Ok(monitors) => {
            let number_width = monitors
                .iter()
                .map(|m| m.number.to_string().len())
                .max()
                .unwrap_or(1)
                .max(1);
            let name_width = monitors
                .iter()
                .map(|m| m.name.len())
                .max()
                .unwrap_or(4)
                .max(4);
            let res_width = monitors
                .iter()
                .map(|m| format!("{}x{}", m.width, m.height).len())
                .max()
                .unwrap_or(10)
                .max(10);
            let header = format!(
                "{:<number_width$}  {:<7}  {:<name_width$}        {:<res_width$}  {:<7}",
                "#", "PRIMARY", "NAME", "RESOLUTION", "REFRESH"
            );
            println!("{header}");
            println!("{}", "─".repeat(header.len()));
            for m in &monitors {
                let primary = if m.is_primary { "*" } else { "" };
                println!(
                    "{:<number_width$}  {:<7}  {:<name_width$}        {:<res_width$}  {:<7}",
                    m.number,
                    primary,
                    m.name,
                    format!("{}x{}", m.width, m.height),
                    format!("{}Hz", m.refresh)
                );
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Lists the supported modes of the targeted display(s).
fn run_caps(target: MonitorTarget) -> i32 {
    match target {
        MonitorTarget::Primary | MonitorTarget::Index(_) => {
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
        MonitorTarget::All => match windows::caps_all() {
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
