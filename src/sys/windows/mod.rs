//! Windows backend: query and apply display settings via Win32 functions.
//!
//! [`list`] reports every attached display, [`caps`] returns the modes a
//! display supports, [`max`] applies the best supported mode. Raw FFI lives
//! in [`bindings`]; device querying in [`query`]; mode enumeration in
//! [`capabilities`]; mode application in [`apply`].

pub(crate) mod apply;
mod bindings;
mod capabilities;
mod query;

pub use apply::{max, set};
pub use capabilities::Mode;
pub use query::Monitor;

/// Lists every display attached to the desktop with its current settings.
///
/// Monitor numbers are 1-based and match the `:N` suffix on other commands.
///
/// # Errors
/// Returns `Err` when no displays are attached.
pub fn list() -> Result<Vec<Monitor>, String> {
    let names = query::enumerate_devices();
    let monitors: Vec<Monitor> = names
        .iter()
        .enumerate()
        .map(|(i, name)| query::describe(i, name))
        .collect();
    if monitors.is_empty() {
        return Err("no displays found".into());
    }
    Ok(monitors)
}

/// Returns the supported modes for a monitor, sorted ascending by
/// resolution then refresh rate.
///
/// `monitor` is the 1-based number from [`list`]; `None` selects the
/// primary display.
///
/// # Errors
/// Returns `Err` for an unknown monitor number or when the display reports
/// no supported modes.
pub fn caps(monitor: Option<u32>) -> Result<(Monitor, Vec<Mode>), String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let modes = capabilities::enumerate_modes(&name);
    if modes.is_empty() {
        return Err(format!(
            "{} has no supported modes",
            query::display_label(&name, index as u32 + 1)
        ));
    }
    Ok((
        query::describe(index, &name),
        capabilities::normalize_modes(modes),
    ))
}