//! Windows backend: query and apply display settings via Win32 functions.
//!
//! [`list`] reports every attached display, [`caps`] returns the modes a
//! display supports, [`max`] applies the best supported mode. Raw FFI lives
//! in [`bindings`]; device querying in [`query`]; mode enumeration in
//! [`capabilities`]; mode application in [`apply`]. When the `RMOD_SYS_FAKE`
//! environment variable is `1`, every entry point delegates to [`fake`]
//! instead, so the integration tests never touch the real display.

pub(crate) mod apply;
pub(crate) mod attach;
pub(crate) mod bindings;
mod brightness;
mod capabilities;
mod fade;
mod fake;
mod layout;
pub(crate) mod power;
pub(crate) mod query;
mod wmi;
pub(crate) mod temp;

pub use apply::{ApplyOutcome, Change, MainChange, MainOutcome, Refresh};
pub use attach::{AttachAction, AttachChange, AttachOutcome};
pub use brightness::{BrightnessBackend, BrightnessOutcome};
pub use capabilities::Mode;
pub use layout::{Direction, PlacementChange, PlacementOutcome};
pub use query::Monitor;
pub use temp::TempChange;

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
        return Err("no displays found, connect a display and try again".into());
    }
    Ok(monitors)
}

/// Lists every display with full EDID information and supported modes.
///
/// # Errors
/// Returns `Err` when no displays are attached or EDID reading fails.
pub fn list_detailed() -> Result<Vec<Monitor>, String> {
    if fake::enabled() {
        return fake::list_detailed();
    }
    query::list_detailed()
}

/// Returns every supported mode for a device by name, sorted ascending by
/// resolution then refresh rate.
pub fn caps_all_modes_for_device(name: &str) -> Vec<Mode> {
    if fake::enabled() {
        fake::caps_all_modes_for_device(name)
    } else {
        capabilities::caps_all_modes_for_device(name)
    }
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
/// Unknown monitor or reference, placing a monitor relative to itself, a
/// blocked swap destination, or a rejected position change.
#[allow(dead_code)]
pub fn apply_placement(
    monitor: u32,
    direction: Direction,
    reference: u32,
) -> Result<PlacementOutcome, String> {
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

/// Enumerates the device names of every display, attached or detached,
/// skipping mirroring drivers and disconnected virtual devices.
#[allow(dead_code)]
pub(crate) fn enumerate_all_devices() -> Vec<String> {
    if fake::enabled() {
        fake::enumerate_all_devices()
    } else {
        query::enumerate_all_devices()
    }
}

/// Detaches the monitor with the 1-based number `monitor` (the primary
/// when `None`) from the desktop.
///
/// See [`attach::disable`].
///
/// # Errors
/// Unknown monitor, an attempt to disable the primary display, or a
/// rejected display change.
#[allow(dead_code)]
pub fn disable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    if fake::enabled() {
        fake::disable(monitor)
    } else {
        attach::disable::disable(monitor)
    }
}

/// Re-attaches the monitor with the 1-based number `monitor` (the primary
/// when `None`) to the desktop.
///
/// See [`attach::enable`].
///
/// # Errors
/// Unknown monitor, a monitor with no saved settings and no supported
/// modes, or a rejected display change.
#[allow(dead_code)]
pub fn enable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    if fake::enabled() {
        fake::enable(monitor)
    } else {
        attach::enable::enable(monitor)
    }
}

/// Undoes an attach/detach change by re-applying the previous device mode.
///
/// See [`attach::revert_attach`].
///
/// # Errors
/// Unknown monitor or a rejected display change.
#[allow(dead_code)]
pub fn revert_attach(change: &AttachChange) -> Result<(), String> {
    if fake::enabled() {
        fake::revert_attach(change)
    } else {
        attach::revert_attach(change)
    }
}

/// Puts every monitor to sleep (backlight off). Returns the label of every
/// affected monitor.
///
/// See [`power::sleep_monitor`].
///
/// # Errors
/// No displays attached.
#[allow(dead_code)]
pub fn sleep_monitor() -> Result<Vec<String>, String> {
    if fake::enabled() {
        fake::sleep_monitor()
    } else {
        power::sleep_monitor()
    }
}

/// Wakes every monitor (backlight on). Returns the label of every affected
/// monitor.
///
/// See [`power::wake_monitor`].
///
/// # Errors
/// No displays attached.
#[allow(dead_code)]
pub fn wake_monitor() -> Result<Vec<String>, String> {
    if fake::enabled() {
        fake::wake_monitor()
    } else {
        power::wake_monitor()
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

/// Sets a display's brightness to `value` (0-100), auto-detecting the
/// backend chain `ddc -> slider -> gamma`, or forcing the backend in `via`.
///
/// `monitor` is the 1-based number from `rmod list`; `None` selects the
/// primary display.
///
/// # Errors
/// Unknown monitor, a forced backend the display does not support, or no
/// brightness-control path at all.
pub fn set_brightness(
    monitor: Option<u32>,
    value: u32,
    via: Option<BrightnessBackend>,
) -> Result<BrightnessOutcome, String> {
    if fake::enabled() {
        fake::set_brightness(monitor, value, via)
    } else {
        brightness::set_brightness(monitor, value, via)
    }
}

/// Sets the color temperature of a display (see [`temp::set_temp`]).
///
/// # Errors
/// Unknown monitor or a display that rejects the gamma ramp change.
pub fn set_temp(monitor: Option<u32>, kelvin: u32) -> Result<TempChange, String> {
    if fake::enabled() {
        fake::set_temp(monitor, kelvin)
    } else {
        temp::set_temp(monitor, kelvin)
    }
}

/// Restores the identity gamma ramp of a display (see [`temp::reset_temp`]).
///
/// # Errors
/// Unknown monitor or a display that rejects the gamma ramp change.
pub fn reset_temp(monitor: Option<u32>) -> Result<TempChange, String> {
    if fake::enabled() {
        fake::reset_temp(monitor)
    } else {
        temp::reset_temp(monitor)
    }
}

/// Reports the current approximate temperature of a display (see
/// [`temp::get_temp`]).
///
/// # Errors
/// Unknown monitor or a display that rejects the gamma ramp read.
pub fn get_temp(monitor: Option<u32>) -> Result<TempChange, String> {
    if fake::enabled() {
        fake::get_temp(monitor)
    } else {
        temp::get_temp(monitor)
    }
}

/// Finds a monitor by its EDID identifier (case-insensitive): the serial
/// when present, otherwise the EDID fingerprint. Returns the 1-based monitor
/// number, or None if not found.
pub fn resolve_by_id(id: &str) -> Option<u32> {
    if fake::enabled() {
        fake::resolve_by_id(id)
    } else {
        query::resolve_by_id(id)
    }
}

/// Resolves a 1-based monitor number to its device pair, validating that the
/// monitor is attached. `None` selects the primary display; `0` or an
/// out-of-range number is an error.
pub use query::resolve_device;
