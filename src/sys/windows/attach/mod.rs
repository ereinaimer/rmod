//! Monitor attach/detach: removing a display from the desktop (`disable`)
//! and restoring it (`enable`).
//!
//! Disabling writes a zero-sized mode to the device with `CDS_UPDATEREGISTRY`;
//! enabling re-applies the registry-persisted settings (falling back to the
//! best supported mode). Both changes run under the fade and can be undone
//! with [`revert_attach`].

pub(crate) mod disable;
pub(crate) mod enable;

use super::apply::{build_devmode, describe_change_result};
use super::bindings::{
    CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DISP_CHANGE_SUCCESSFUL, DM_PELSHEIGHT,
    DM_PELSWIDTH, DM_POSITION, DevmodeW, Pointl, encode_wide,
};
use super::capabilities::{self, Mode};
use super::fade;
use super::query;

/// The action applied by an attach/detach change.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AttachAction {
    /// The monitor was detached from the desktop.
    Disable,
    /// The monitor was re-attached to the desktop.
    Enable,
}

/// A monitor attach/detach change: the action applied and the full device
/// mode in effect before it, used to undo the change.
#[derive(Debug, PartialEq)]
pub struct AttachChange {
    /// The 1-based monitor number the change applies to.
    pub monitor: u32,
    /// The display label used in batch output.
    pub display: String,
    /// Whether the monitor was detached or re-attached.
    pub action: AttachAction,
    /// The device mode in effect before the change.
    pub previous: DevmodeW,
}

/// The result of attaching or detaching a monitor.
#[derive(Debug, PartialEq)]
pub enum AttachOutcome {
    /// The change was applied and can be reverted with its previous mode.
    Applied(AttachChange),
    /// The monitor was already in the requested state; nothing changed.
    Unchanged(AttachChange),
}

/// Builds the device mode that detaches a monitor: zero dimensions at
/// origin 0,0, flagged with `DM_PELSWIDTH`, `DM_PELSHEIGHT` and
/// `DM_POSITION`. Everything else is copied through unchanged.
fn build_disable_devmode(current: &DevmodeW) -> DevmodeW {
    let mut devmode = *current;
    devmode.dm_pels_width = 0;
    devmode.dm_pels_height = 0;
    devmode.dm_position = Pointl { x: 0, y: 0 };
    devmode.dm_fields |= DM_PELSWIDTH | DM_PELSHEIGHT | DM_POSITION;
    devmode.dm_size = std::mem::size_of::<DevmodeW>() as u16;
    devmode.dm_driver_extra = 0;
    devmode
}

/// The device mode to apply when re-attaching a monitor: the
/// registry-persisted settings when valid, otherwise the best supported
/// mode. Errors when the monitor has no saved settings and no supported
/// modes.
fn restore_devmode(name: &str, display: &str, base: &DevmodeW) -> Result<DevmodeW, String> {
    let registry = query::registry_mode(name);
    let best = capabilities::normalize_modes(capabilities::enumerate_modes(name)).pop();
    choose_restore(registry, best, base, display)
}

/// Selects the mode used to re-attach a monitor: the registry-persisted
/// settings when they describe a valid resolution, otherwise the best
/// supported mode.
fn choose_restore(
    registry: Option<DevmodeW>,
    best: Option<Mode>,
    base: &DevmodeW,
    display: &str,
) -> Result<DevmodeW, String> {
    if let Some(mode) = registry
        && mode.dm_pels_width > 0
    {
        return Ok(mode);
    }
    match best {
        Some(mode) => Ok(build_devmode(&mode, base, None)),
        None => Err(format!(
            "{display} has no saved settings and no supported modes"
        )),
    }
}

/// Applies an attach/detach devmode and persists it. No dry-run
/// validation: a zero-sized (detached) mode cannot be validated with
/// `CDS_TEST`.
fn apply_attach(name: &str, devmode: &DevmodeW) -> Result<(), String> {
    let name_ptr = encode_wide(name);
    let applied = unsafe {
        ChangeDisplaySettingsExW(
            name_ptr.as_ptr(),
            devmode,
            0,
            CDS_UPDATEREGISTRY,
            std::ptr::null(),
        )
    };
    if applied != DISP_CHANGE_SUCCESSFUL {
        return Err(describe_change_result(applied));
    }
    Ok(())
}

/// Undoes an attach/detach change by re-applying the previous device mode.
///
/// # Errors
/// A rejected display change.
#[allow(dead_code)]
pub fn revert_attach(change: &AttachChange) -> Result<(), String> {
    let names = query::enumerate_all_devices();
    let (_index, name) = query::resolve_device(Some(change.monitor), &names)?;
    let mut devmode = change.previous;
    devmode.dm_size = std::mem::size_of::<DevmodeW>() as u16;
    devmode.dm_driver_extra = 0;
    fade::transition(name, &devmode, || apply_attach(name, &devmode))
}

#[cfg(test)]
mod tests {
    use super::super::bindings::DM_DISPLAYFREQUENCY;
    use super::*;

    #[test]
    fn build_disable_devmode_zeroes_dims_and_position() {
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_pels_width = 1920;
        current.dm_pels_height = 1080;
        current.dm_display_frequency = 144;
        current.dm_position = Pointl { x: 1920, y: 0 };
        let devmode = build_disable_devmode(&current);
        assert_eq!(devmode.dm_pels_width, 0);
        assert_eq!(devmode.dm_pels_height, 0);
        assert_eq!(devmode.dm_position, Pointl { x: 0, y: 0 });
    }

    #[test]
    fn build_disable_devmode_flags_dims_and_position() {
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_fields = DM_DISPLAYFREQUENCY;
        let devmode = build_disable_devmode(&current);
        assert_eq!(
            devmode.dm_fields,
            DM_DISPLAYFREQUENCY | DM_PELSWIDTH | DM_PELSHEIGHT | DM_POSITION
        );
    }

    #[test]
    fn build_disable_devmode_preserves_other_fields() {
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_display_frequency = 144;
        current.dm_display_orientation = 1;
        current.dm_bits_per_pel = 32;
        let devmode = build_disable_devmode(&current);
        assert_eq!(devmode.dm_display_frequency, 144);
        assert_eq!(devmode.dm_display_orientation, 1);
        assert_eq!(devmode.dm_bits_per_pel, 32);
    }

    #[test]
    fn build_disable_devmode_sets_size_and_clears_driver_extra() {
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_driver_extra = 8;
        let devmode = build_disable_devmode(&current);
        assert_eq!(devmode.dm_size, 220);
        assert_eq!(devmode.dm_driver_extra, 0);
    }

    #[test]
    fn choose_restore_uses_registry_mode_when_valid() {
        let mut registry: DevmodeW = unsafe { std::mem::zeroed() };
        registry.dm_pels_width = 1920;
        registry.dm_pels_height = 1080;
        registry.dm_display_frequency = 144;
        registry.dm_position = Pointl { x: 1920, y: 0 };
        let base: DevmodeW = unsafe { std::mem::zeroed() };
        assert_eq!(
            choose_restore(Some(registry), None, &base, "Generic PnP Monitor [:1]"),
            Ok(registry)
        );
    }

    #[test]
    fn choose_restore_ignores_zero_width_registry_mode() {
        let registry: DevmodeW = unsafe { std::mem::zeroed() };
        let best = Mode {
            width: 2560,
            height: 1440,
            refresh: 144,
        };
        let base: DevmodeW = unsafe { std::mem::zeroed() };
        let devmode = choose_restore(
            Some(registry),
            Some(best),
            &base,
            "Generic PnP Monitor [:1]",
        )
        .unwrap();
        assert_eq!(devmode.dm_pels_width, 2560);
        assert_eq!(devmode.dm_pels_height, 1440);
        assert_eq!(devmode.dm_display_frequency, 144);
    }

    #[test]
    fn choose_restore_falls_back_to_best_mode() {
        let best = Mode {
            width: 3840,
            height: 2160,
            refresh: 60,
        };
        let base: DevmodeW = unsafe { std::mem::zeroed() };
        let devmode = choose_restore(None, Some(best), &base, "Generic PnP Monitor [:1]").unwrap();
        assert_eq!(devmode.dm_pels_width, 3840);
        assert_eq!(devmode.dm_pels_height, 2160);
        assert_eq!(devmode.dm_display_frequency, 60);
    }

    #[test]
    fn choose_restore_no_modes_is_error() {
        let base: DevmodeW = unsafe { std::mem::zeroed() };
        assert_eq!(
            choose_restore(None, None, &base, "Generic PnP Monitor [:1]"),
            Err(
                "Generic PnP Monitor [:1] has no saved settings and no supported modes".to_string()
            )
        );
    }
}
