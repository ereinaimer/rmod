//! Wakes every monitor via the `SC_MONITORPOWER` system command.

use super::{attached_labels, broadcast};

/// Wakes every monitor (backlight on).
///
/// # Errors
/// Returns an error when no displays are attached.
pub fn wake_monitor() -> Result<Vec<String>, String> {
    let labels = attached_labels()?;
    broadcast(-1);
    Ok(labels)
}