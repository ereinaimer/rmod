//! Monitor power-state control.
//!
//! [`sleep_monitor`] and [`wake_monitor`] put the display session to sleep
//! or wake it via the `SC_MONITORPOWER` system command. The command is a
//! broadcast to every monitor and is reversible by input.

mod sleep;
mod wake;

pub use sleep::sleep_monitor;
pub use wake::wake_monitor;

use super::query;

/// Broadcasts a `SC_MONITORPOWER` system command to every monitor.
fn broadcast(power: isize) {
    unsafe {
        super::bindings::SendMessageW(
            super::bindings::HWND_BROADCAST,
            super::bindings::WM_SYSCOMMAND,
            super::bindings::SC_MONITORPOWER,
            power,
        );
    }
}

/// The display label of every attached monitor.
fn attached_labels() -> Result<Vec<String>, String> {
    let names = query::enumerate_devices();
    if names.is_empty() {
        return Err("no displays found, connect a display and try again".to_string());
    }
    Ok(names
        .iter()
        .enumerate()
        .map(|(i, name)| query::display_label(name, i as u32 + 1))
        .collect())
}