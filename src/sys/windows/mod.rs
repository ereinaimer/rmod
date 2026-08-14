//! Windows backend: query and apply display settings via Win32 functions.
//!
//! [`list`] reports every attached display, [`caps`] returns the modes a
//! display supports, [`max`] applies the best supported mode. Raw FFI lives
//! in [`bindings`]; device querying in [`query`]; mode enumeration in
//! [`capabilities`]; mode application in [`apply`]. When the `RMOD_SYS_FAKE`
//! environment variable is `1`, every entry point delegates to [`fake`]
//! instead, so the integration tests never touch the real display.

pub(crate) mod apply;
mod bindings;
mod capabilities;
mod fake;
mod fade;
mod layout;
pub(crate) mod query;

pub use apply::{ApplyOutcome, Change, MainChange, MainOutcome, Refresh};
pub use capabilities::Mode;
pub use layout::{Direction, PlacementChange};
pub use query::Monitor;

/// Lists every display attached to the desktop with its current settings.
///
/// Monitor numbers are 1-based and match the `:N` suffix on other commands.
///
/// # Errors
/// Returns `Err` when no displays are attached.
pub fn list() -> Result<Vec<Monitor>, String> {
    if fake::enabled() {
        return fake::list();
    }
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
    if fake::enabled() {
        return fake::caps(monitor);
    }
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let modes = capabilities::enumerate_modes(name);
    if modes.is_empty() {
        return Err(format!(
            "{} has no supported modes",
            query::display_label(name, index as u32 + 1)
        ));
    }
    Ok((
        query::describe(index, name),
        capabilities::normalize_modes(modes),
    ))
}

/// Returns the supported modes for every attached monitor, sorted
/// ascending by resolution then refresh rate.
///
/// # Errors
/// Returns `Err` when no displays are attached or any display reports
/// no supported modes.
pub fn caps_all() -> Result<Vec<(Monitor, Vec<Mode>)>, String> {
    if fake::enabled() {
        return fake::caps_all();
    }
    let names = query::enumerate_devices();
    let targets = query::resolve_all(&names)?;
    let mut monitors = Vec::with_capacity(targets.len());
    for (index, name) in targets {
        let modes = capabilities::enumerate_modes(name);
        if modes.is_empty() {
            return Err(format!(
                "{} has no supported modes",
                query::display_label(name, index as u32 + 1)
            ));
        }
        monitors.push((
            query::describe(index, name),
            capabilities::normalize_modes(modes),
        ));
    }
    Ok(monitors)
}

/// Applies a resolution, refresh and rotation policy to a display.
///
/// See [`apply::set`]; `None` selects the primary display.
///
/// # Errors
/// Unknown monitor, no matching mode, or a mode the display rejects.
pub fn set(
    monitor: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<ApplyOutcome, String> {
    if fake::enabled() {
        fake::set(monitor, width, height, refresh, orientation)
    } else {
        apply::set(monitor, width, height, refresh, orientation)
    }
}

/// Applies the best supported mode to a display.
///
/// See [`apply::max`]; `None` selects the primary display.
///
/// # Errors
/// Unknown monitor, no supported modes, or a rejected display change.
pub fn max(monitor: Option<u32>, orientation: Option<u32>) -> Result<ApplyOutcome, String> {
    if fake::enabled() {
        fake::max(monitor, orientation)
    } else {
        apply::max(monitor, orientation)
    }
}

/// Applies the best supported mode to every attached display.
///
/// See [`apply::max_all`].
///
/// # Errors
/// No displays found, a display with no supported modes, or preflight
/// failures.
pub fn max_all(orientation: Option<u32>) -> Result<Vec<ApplyOutcome>, String> {
    if fake::enabled() {
        fake::max_all(orientation)
    } else {
        apply::max_all(orientation)
    }
}

/// Re-applies a previously captured mode to undo a display change.
///
/// See [`apply::revert`]; `None` selects the primary display.
///
/// # Errors
/// Unknown monitor or a mode the display rejects.
pub fn revert(
    monitor: Option<u32>,
    previous: Mode,
    previous_orientation: Option<u32>,
) -> Result<Mode, String> {
    if fake::enabled() {
        fake::revert(monitor, previous, previous_orientation)
    } else {
        apply::revert(monitor, previous, previous_orientation)
    }
}

/// Promotes a display to the main display by swapping desktop positions.
///
/// See [`apply::make_main`].
///
/// # Errors
/// Unknown monitor or a rejected position change.
pub fn make_main(monitor: u32, names: &[String]) -> Result<MainOutcome<'_>, String> {
    if fake::enabled() {
        fake::make_main(monitor, names)
    } else {
        apply::make_main(monitor, names)
    }
}

/// Undoes a promotion by re-applying the original positions.
///
/// See [`apply::revert_main`].
///
/// # Errors
/// A rejected position change.
pub fn revert_main(change: &MainChange<'_>) -> Result<(), String> {
    if fake::enabled() {
        fake::revert_main(change)
    } else {
        apply::revert_main(change)
    }
}

/// Places a monitor on a side of another monitor, swapping positions when
/// the landing spot is occupied.
///
/// `monitor` is the 1-based number from [`list`]; `reference` is the
/// monitor to position relative to.
///
/// # Errors
/// Unknown monitor, placing a monitor relative to itself, or a rejected
/// position change.
#[allow(dead_code)]
pub fn apply_placement(
    monitor: u32,
    direction: Direction,
    reference: u32,
) -> Result<PlacementChange, String> {
    if fake::enabled() {
        fake::apply_placement(monitor, direction, reference)
    } else {
        let names = query::enumerate_devices();
        layout::apply_placement(monitor, direction, reference, &names)
    }
}

/// Undoes a placement by re-applying the original positions.
///
/// See [`layout::revert_placement`].
///
/// # Errors
/// A rejected position change.
#[allow(dead_code)]
pub fn revert_placement(change: &PlacementChange) -> Result<(), String> {
    if fake::enabled() {
        fake::revert_placement(change)
    } else {
        layout::revert_placement(change)
    }
}

/// Applies a resolution, refresh and rotation policy to every attached
/// display.
///
/// See [`apply::set_all`].
///
/// # Errors
/// No displays found, a mode no display supports, or preflight failures.
#[allow(dead_code)]
pub fn set_all(
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<Vec<ApplyOutcome>, String> {
    if fake::enabled() {
        fake::set_all(width, height, refresh, orientation)
    } else {
        apply::set_all(width, height, refresh, orientation)
    }
}

/// Enumerates the device names of every display attached to the desktop.
pub(crate) fn enumerate_devices() -> Vec<String> {
    if fake::enabled() {
        fake::enumerate_devices()
    } else {
        query::enumerate_devices()
    }
}

/// Returns the current mode for a specific monitor number (1-based).
///
/// # Errors
/// Unknown monitor.
pub fn get_current_mode(monitor: u32) -> Result<Monitor, String> {
    if fake::enabled() {
        fake::get_current_mode(monitor)
    } else {
        query::get_current_mode(monitor)
    }
}

/// Returns the current mode for the primary monitor.
///
/// # Errors
/// No displays found.
pub fn get_primary_mode() -> Result<Monitor, String> {
    if fake::enabled() {
        fake::get_primary_mode()
    } else {
        query::get_primary_mode()
    }
}
