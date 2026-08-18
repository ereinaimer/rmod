//! Puts every monitor to sleep via the `SC_MONITORPOWER` system command.

use super::{attached_labels, broadcast};

/// Puts every monitor to sleep (backlight off). The broadcast reaches every
/// monitor; Windows wakes them again on any input.
///
/// # Errors
/// Returns an error when no displays are attached.
pub fn sleep_monitor() -> Result<Vec<String>, String> {
    let labels = attached_labels()?;
    broadcast(2);
    Ok(labels)
}
