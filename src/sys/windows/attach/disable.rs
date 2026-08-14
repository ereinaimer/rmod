//! Detaches a monitor from the desktop by applying a zero-sized mode.

use super::super::query;
use super::{AttachAction, AttachChange, AttachOutcome, apply_attach, build_disable_devmode};
use super::super::fade;

/// Detaches the monitor with the 1-based number `monitor` (the primary
/// when `None`) from the desktop.
///
/// The monitor's current mode is captured for revert, then a zero-sized
/// mode at origin 0,0 is applied and persisted under a fade. The primary
/// display cannot be detached, and a monitor already detached is reported
/// as [`AttachOutcome::Unchanged`].
///
/// # Errors
/// Unknown monitor, an attempt to disable the primary display, or a
/// rejected display change.
#[allow(dead_code)]
pub fn disable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    if base.dm_position.x == 0 && base.dm_position.y == 0 {
        return Err("cannot detach the primary display".to_string());
    }
    let change = AttachChange {
        monitor: index as u32 + 1,
        display,
        action: AttachAction::Disable,
        previous: base,
    };
    if change.previous.dm_pels_width == 0 {
        return Ok(AttachOutcome::Unchanged(change));
    }
    let devmode = build_disable_devmode(&change.previous);
    fade::transition(name, &devmode, || apply_attach(name, &devmode))?;
    Ok(AttachOutcome::Applied(change))
}