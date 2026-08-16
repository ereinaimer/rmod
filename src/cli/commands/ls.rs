//! `ls` command: lists every display with full EDID information.
//!
//! [`run_list`] prints detailed blocks for each monitor including
//! manufacturer, current/native resolution, manufacture date, and supported
//! modes grouped by resolution. Each block's heading is the display name
//! with the EDID fingerprint suffix (e.g. `Lenovo 9059 [a1b2c3d4]`).

use crate::sys::windows;

/// Lists every display with full EDID information and supported modes.
pub(super) fn run_list() -> i32 {
    match windows::list_detailed() {
        Ok(mut monitors) => {
            monitors.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
            for (i, m) in monitors.iter().enumerate() {
                if i > 0 {
                    println!(); // blank line between monitors
                }
                print_monitor(m);
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Prints a single monitor's detailed information.
fn print_monitor(m: &crate::sys::windows::Monitor) {
    println!("{}", m.name);
    println!("  Primary:         {}", if m.is_primary { "true" } else { "false" });
    println!("  Manufacturer:    {}", m.manufacturer);
    println!("  Current:         {}x{} @ {}Hz", m.width, m.height, m.refresh);
    println!("  Native:          {}x{} @ {}Hz", m.native_width, m.native_height, m.native_refresh);
    println!("  Manufactured:    Week {}, {}", m.manufactured_week, m.manufactured_year);
    println!("  Supported:");

    // Group modes by resolution
    let mut modes_by_res: std::collections::HashMap<(u32, u32), Vec<u32>> = std::collections::HashMap::new();
    for mode in windows::caps_all_modes_for_device(&m.device_name) {
        modes_by_res
            .entry((mode.width, mode.height))
            .or_default()
            .push(mode.refresh);
    }

    // Sort resolutions by width, then height
    let mut resolutions: Vec<_> = modes_by_res.keys().cloned().collect();
    resolutions.sort_by_key(|&(w, h)| (w, h));

    for (width, height) in resolutions {
        if let Some(refresh_rates) = modes_by_res.get(&(width, height)) {
            let mut rates = refresh_rates.clone();
            rates.sort_unstable();
            rates.dedup();
            let rates_str = rates
                .iter()
                .map(|r| format!("{}Hz", r))
                .collect::<Vec<_>>()
                .join(", ");
            println!("    {}x{}  @ {}", width, height, rates_str);
        }
    }
}
