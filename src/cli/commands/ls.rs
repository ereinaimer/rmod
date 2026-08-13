//! `ls` command: lists every display and its current settings.
//!
//! [`run_list`] renders a table of monitor numbers, primary markers,
//! names, resolutions and refresh rates, aligned to the widest entries.

use crate::sys::windows;

/// Lists every display and its current settings.
pub(super) fn run_list() -> i32 {
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
