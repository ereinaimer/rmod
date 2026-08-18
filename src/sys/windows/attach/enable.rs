//! Re-attaches a monitor to the desktop by restoring its saved settings.

use super::super::fade;
use super::super::query;
use super::{AttachAction, AttachChange, AttachOutcome, apply_attach, restore_devmode};

/// Re-attaches the monitor with the 1-based number `monitor` (the primary
/// when `None`) to the desktop.
///
/// The monitor's detached state is captured for revert, then the
/// registry-persisted settings (falling back to the best supported mode)
/// are applied and persisted under a fade. A monitor already attached is
/// reported as [`AttachOutcome::Unchanged`].
///
/// # Errors
/// Unknown monitor, a monitor with no saved settings and no supported
/// modes, or a rejected display change.
#[allow(dead_code)]
pub fn enable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    let names = query::enumerate_all_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let change = AttachChange {
        monitor: index as u32 + 1,
        display,
        action: AttachAction::Enable,
        previous: base,
    };
    if change.previous.dm_pels_width > 0 {
        return Ok(AttachOutcome::Unchanged(change));
    }
    let devmode = restore_devmode(name, &change.display, &change.previous)?;
    fade::transition(name, &devmode, || apply_attach(name, &devmode))?;
    Ok(AttachOutcome::Applied(change))
}
