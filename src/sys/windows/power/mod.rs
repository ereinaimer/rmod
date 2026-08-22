//! Monitor power-state control.
//!
//! [`set_power_state`] puts the display session to sleep or wakes it via the
//! `SC_MONITORPOWER` system command. The command is a broadcast to every
//! monitor and is reversible by input.

use super::query;

/// Broadcasts a `SC_MONITORPOWER` system command to every monitor.
/// `power = -1` wakes, `power = 2` sleeps.
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
        .map(|(i, name)| query::display_label_for(name, i as u32 + 1))
        .collect())
}

/// Puts every monitor to sleep (backlight off) or wakes it (backlight on).
///
/// `power = -1` wakes, `power = 2` sleeps.
///
/// # Errors
/// Returns an error when no displays are attached.
pub fn set_power_state(power: isize) -> Result<Vec<String>, String> {
    let labels = attached_labels()?;
    broadcast(power);
    Ok(labels)
}

#[cfg(test)]
mod tests {
    #[cfg(any(test, feature = "fake"))]
    use super::super::fake;
    use super::*;

    #[test]
    fn set_power_state_sleep() {
        // Only test with fake backend to avoid affecting host hardware
        #[cfg(any(test, feature = "fake"))]
        if fake::enabled() {
            let _ = set_power_state(2);
        }
    }
}
