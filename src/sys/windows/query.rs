//! Device enumeration and current-state queries.
//!
//! Everything here reads what is connected right now: device names,
//! friendly names, current mode, and the primary-display designation.

use super::bindings::{
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_DISCONNECT, DISPLAY_DEVICE_MIRRORING_DRIVER,
    DISPLAY_DEVICEW, DevmodeW, ENUM_CURRENT_SETTINGS, ENUM_REGISTRY_SETTINGS, EnumDisplayDevicesW,
    EnumDisplaySettingsW, encode_wide, wide_to_string,
};

/// A display attached to the desktop and its current settings.
pub struct Monitor {
    /// 1-based monitor number matching the `:N` command suffix.
    pub number: u32,
    /// Friendly name, falling back to the device name when unavailable.
    pub name: String,
    /// True when this display is the primary (origin 0,0).
    pub is_primary: bool,
    /// Current pixel width.
    pub width: u32,
    /// Current pixel height.
    pub height: u32,
    /// Current refresh rate in Hz.
    pub refresh: u32,
    /// Desktop x coordinate from the current mode.
    #[allow(dead_code)]
    pub x: i32,
    /// Desktop y coordinate from the current mode.
    #[allow(dead_code)]
    pub y: i32,
}

/// Enumerates the device names of every display attached to the desktop.
pub(crate) fn enumerate_devices() -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0u32;
    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) };
        if ok == 0 {
            break;
        }
        if device.state_flags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP != 0 {
            names.push(wide_to_string(&device.device_name));
        }
        index += 1;
    }
    names
}

/// Reads the currently applied mode for a device name.
pub(crate) fn current_mode(name: &str) -> Option<DevmodeW> {
    let name_wide = encode_wide(name);
    let mut mode: DevmodeW = unsafe { std::mem::zeroed() };
    let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) };
    if ok == 0 { None } else { Some(mode) }
}

/// Reads the registry-persisted mode for a device name.
///
/// Windows stores the last attached mode in the registry when a monitor is
/// detached with `CDS_UPDATEREGISTRY`; re-enabling applies these settings
/// to restore the monitor.
pub(crate) fn registry_mode(name: &str) -> Option<DevmodeW> {
    let name_wide = encode_wide(name);
    let mut mode: DevmodeW = unsafe { std::mem::zeroed() };
    let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), ENUM_REGISTRY_SETTINGS, &mut mode) };
    if ok == 0 { None } else { Some(mode) }
}

/// Enumerates every display device, attached or detached, skipping
/// mirroring drivers and disconnected virtual devices.
pub(crate) fn enumerate_all_devices() -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0u32;
    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) };
        if ok == 0 {
            break;
        }
        if device.state_flags & DISPLAY_DEVICE_MIRRORING_DRIVER == 0
            && device.state_flags & DISPLAY_DEVICE_DISCONNECT == 0
        {
            names.push(wide_to_string(&device.device_name));
        }
        index += 1;
    }
    names
}

/// Queries the friendly name of the monitor attached to a device.
pub(crate) fn friendly_name(device: &[u16]) -> Option<String> {
    let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let ok = unsafe { EnumDisplayDevicesW(device.as_ptr(), 0, &mut monitor, 0) };
    if ok == 0 {
        return None;
    }
    Some(wide_to_string(&monitor.device_string))
}

/// Human-readable display label for error messages: friendly name (falling
/// back to the device name) plus the 1-based monitor number, e.g.
/// `Generic PnP Monitor [:1]`.
pub(crate) fn display_label(name: &str, number: u32) -> String {
    let friendly = friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string());
    format!("{friendly} [:{number}]")
}

/// Builds a [`Monitor`] for a device: friendly name (falling back to the
/// raw device name) and current mode; primary is determined by origin 0,0.
pub(crate) fn describe(index: usize, name: &str) -> Monitor {
    let mode = current_mode(name);
    Monitor {
        number: index as u32 + 1,
        name: friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string()),
        is_primary: mode
            .as_ref()
            .is_some_and(|m| m.dm_position.x == 0 && m.dm_position.y == 0),
        width: mode.as_ref().map_or(0, |m| m.dm_pels_width),
        height: mode.as_ref().map_or(0, |m| m.dm_pels_height),
        refresh: mode.as_ref().map_or(0, |m| m.dm_display_frequency),
        x: mode.as_ref().map_or(0, |m| m.dm_position.x),
        y: mode.as_ref().map_or(0, |m| m.dm_position.y),
    }
}

/// Resolves `:N` to a device; `None` selects the primary display. Numbers
/// are 1-based; `0` or an out-of-range value is an error.
pub(crate) fn resolve_device(
    monitor: Option<u32>,
    names: &[String],
) -> Result<(usize, &str), String> {
    match monitor {
        None => {
            for (i, name) in names.iter().enumerate() {
                if let Some(mode) = current_mode(name)
                    && mode.dm_position.x == 0
                    && mode.dm_position.y == 0
                {
                    return Ok((i, name.as_str()));
                }
            }
            names
                .first()
                .map(|name| (0, name.as_str()))
                .ok_or_else(|| "no displays found, connect a display and try again".to_string())
        }
        Some(n) => {
            let index = n.checked_sub(1).ok_or_else(|| {
                format!("monitor {n} not found, run 'rmod list' to see connected displays")
            })? as usize;
            names
                .get(index)
                .map(|name| (index, name.as_str()))
                .ok_or_else(|| {
                    format!("monitor {n} not found, run 'rmod list' to see connected displays")
                })
        }
    }
}

/// Resolves every attached device to its `(index, name)` pair, where
/// `index` is the 0-based position in `names`, matching how
/// [`resolve_device`] reports indices.
///
/// # Errors
/// Returns an error when no displays are attached.
pub(crate) fn resolve_all(names: &[String]) -> Result<Vec<(usize, &str)>, String> {
    if names.is_empty() {
        return Err("no displays found, connect a display and try again".to_string());
    }
    Ok(names
        .iter()
        .enumerate()
        .map(|(i, n)| (i, n.as_str()))
        .collect())
}

/// Returns the current mode for a specific monitor number (1-based).
pub fn get_current_mode(monitor: u32) -> Result<Monitor, String> {
    let names = enumerate_devices();
    let (index, name) = resolve_device(Some(monitor), &names)?;
    Ok(describe(index, name))
}

/// Returns the current mode for the primary monitor.
pub fn get_primary_mode() -> Result<Monitor, String> {
    let names = enumerate_devices();
    let (index, name) = resolve_device(None, &names)?;
    Ok(describe(index, name))
}
