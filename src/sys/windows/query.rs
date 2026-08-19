//! Device enumeration and current-state queries.
//!
//! Everything here reads what is connected right now: device names,
//! friendly names, current mode, and the primary-display designation.

use super::bindings::{
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_DISCONNECT, DISPLAY_DEVICE_MIRRORING_DRIVER,
    DISPLAY_DEVICEW, DevmodeW, DisplayConfigPathInfo, ENUM_CURRENT_SETTINGS,
    ENUM_REGISTRY_SETTINGS, EnumDisplayDevicesW, EnumDisplaySettingsW, HKEY_LOCAL_MACHINE,
    encode_wide, wide_to_string,
};
use super::capabilities::{Mode, enumerate_modes, normalize_modes};
use super::edid::{
    self, EdidData, GamutCoverage, append_fingerprint, base_display_name, manufacturer_name,
    parse_edid,
};
use super::hdr::{HdrInfo, connector_for_path, hdr_from_path, match_paths, query_connector};
use super::registry::{enum_subkeys, read_reg_binary};
use std::collections::HashMap;

/// A display attached to the desktop and its current settings.
pub struct Monitor {
    /// 1-based monitor number matching the `:N` command suffix.
    pub number: u32,
    /// Display name: the EDID product/panel name when available, otherwise
    /// the friendly name, falling back to the device name.
    pub name: String,
    /// The Win32 device name (e.g. `\\.\DISPLAY1`) used to query modes.
    pub device_name: String,
    /// True when this display is the primary (origin 0,0).
    pub is_primary: bool,
    /// Current pixel width.
    pub width: u32,
    /// Current pixel height.
    pub height: u32,
    /// Current refresh rate in Hz.
    pub refresh: u32,
    /// Every supported mode of the display, sorted ascending by
    /// resolution then refresh rate.
    pub modes: Vec<Mode>,
    /// Desktop x coordinate from the current mode.
    #[allow(dead_code)]
    pub x: i32,
    /// Desktop y coordinate from the current mode.
    #[allow(dead_code)]
    pub y: i32,
    /// EDID manufacturer code (3-char, e.g., "DEL").
    pub manufacturer: String,
    /// EDID serial number, kept for targeting when a panel ships one.
    #[allow(dead_code)]
    pub serial: String,
    /// EDID fingerprint: the first 8 hex chars of the SHA-256 of the raw
    /// EDID blob. Every panel has one (serials are often absent), so this is
    /// the stable `-m` target and is shown in the display name.
    pub fingerprint: String,
    /// Manufacturing week (1-53).
    pub manufactured_week: u8,
    /// Manufacturing year (e.g., 2023).
    pub manufactured_year: u16,
    /// Native pixel width from EDID.
    pub native_width: u32,
    /// Native pixel height from EDID.
    pub native_height: u32,
    /// Native refresh rate from EDID.
    pub native_refresh: u32,
    /// Physical panel size in cm from EDID bytes 21-22; `None` when unknown.
    #[allow(dead_code)]
    pub physical_size_cm: Option<(f32, f32)>,
    /// Display gamma from EDID byte 23; `None` when unknown.
    #[allow(dead_code)]
    pub gamma: Option<f32>,
    /// Physical DPI (horizontal, vertical) computed from native resolution and
    /// EDID size; `None` when the EDID size is unknown.
    #[allow(dead_code)]
    pub dpi_physical: Option<(u32, u32)>,
    /// sRGB / DCI-P3 gamut coverage percentages from EDID chromaticity.
    #[allow(dead_code)]
    pub gamut: Option<GamutCoverage>,
    /// HDR capability from the Windows API with EDID fallback; `None` = unknown.
    #[allow(dead_code)]
    pub hdr: Option<HdrInfo>,
    /// Color depth from the current mode (`dm_bits_per_pel`).
    #[allow(dead_code)]
    pub bits_per_pel: u32,
    /// Logical DPI from the current mode (`dm_log_pixels`); 0 = unknown.
    #[allow(dead_code)]
    pub log_pixels: u32,
    /// Orientation from the current mode (`dm_display_orientation`).
    #[allow(dead_code)]
    pub orientation: u32,
    /// Connector type from the display-config path (`output_technology`),
    /// e.g. `"Internal"`, `"HDMI"`, `"DisplayPort"`; `None` = unknown.
    #[allow(dead_code)]
    pub connector: Option<&'static str>,
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
    display_label_with_edid(name, number, None)
}

/// Human-readable display label with optional EDID name override.
/// If `edid_name` is provided, it's used instead of the friendly name.
pub(crate) fn display_label_with_edid(
    name: &str,
    number: u32,
    edid_name: Option<String>,
) -> String {
    let friendly = edid_name
        .unwrap_or_else(|| friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string()));
    format!("{friendly} [:{number}]")
}

/// Human-readable display label for a single device from its cached EDID:
/// the EDID display name (see [`display_name_for`]) plus the 1-based monitor
/// number. Falls back to the friendly-name label when the EDID cannot be
/// read.
pub(crate) fn display_label_for(name: &str, number: u32) -> String {
    match read_edid_registry(name) {
        Ok(edid) => format!("{} [:{number}]", display_name_for(&edid, name)),
        Err(_) => display_label_with_edid(name, number, None),
    }
}

/// Physical DPI from native pixel dimensions and the EDID panel size in
/// centimeters, rounded to the nearest integer per axis. `None` when either
/// native dimension is zero or either size is not positive.
fn physical_dpi(native_width: u32, native_height: u32, size_cm: (f32, f32)) -> Option<(u32, u32)> {
    let (w_cm, h_cm) = size_cm;
    if native_width == 0 || native_height == 0 || w_cm <= 0.0 || h_cm <= 0.0 {
        return None;
    }
    Some((
        (native_width as f32 * 2.54 / w_cm).round() as u32,
        (native_height as f32 * 2.54 / h_cm).round() as u32,
    ))
}

/// Builds a [`Monitor`] for a device: friendly name (falling back to the
/// raw device name) and current mode; primary is determined by origin 0,0.
/// EDID fields are set to defaults; use `describe_with_edid` for full data.
pub(crate) fn describe(index: usize, name: &str) -> Monitor {
    let mode = current_mode(name);
    Monitor {
        number: index as u32 + 1,
        name: friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string()),
        device_name: name.to_string(),
        is_primary: mode
            .as_ref()
            .is_some_and(|m| m.dm_position.x == 0 && m.dm_position.y == 0),
        width: mode.as_ref().map_or(0, |m| m.dm_pels_width),
        height: mode.as_ref().map_or(0, |m| m.dm_pels_height),
        refresh: mode.as_ref().map_or(0, |m| m.dm_display_frequency),
        modes: Vec::new(),
        x: mode.as_ref().map_or(0, |m| m.dm_position.x),
        y: mode.as_ref().map_or(0, |m| m.dm_position.y),
        manufacturer: String::new(),
        serial: String::new(),
        fingerprint: String::new(),
        manufactured_week: 0,
        manufactured_year: 0,
        native_width: 0,
        native_height: 0,
        native_refresh: 0,
        physical_size_cm: None,
        gamma: None,
        dpi_physical: None,
        gamut: None,
        hdr: None,
        bits_per_pel: mode.as_ref().map_or(0, |m| m.dm_bits_per_pel),
        log_pixels: mode.as_ref().map_or(0, |m| m.dm_log_pixels as u32),
        orientation: mode.as_ref().map_or(0, |m| m.dm_display_orientation),
        connector: query_connector(name),
    }
}

/// Builds a [`Monitor`] with full EDID data.
///
/// `name` is the Win32 device name; `display_name` is the name shown in
/// output (EDID-derived when available). `path` is the monitor's
/// display-config path from the batched [`match_paths`] table.
#[allow(clippy::too_many_arguments)]
fn describe_with_edid(
    index: usize,
    name: &str,
    display_name: String,
    edid: &EdidData,
    native_width: u32,
    native_height: u32,
    native_refresh: u32,
    modes: Vec<Mode>,
    path: Option<&DisplayConfigPathInfo>,
) -> Monitor {
    let mode = current_mode(name);
    Monitor {
        number: index as u32 + 1,
        name: display_name,
        device_name: name.to_string(),
        is_primary: mode
            .as_ref()
            .is_some_and(|m| m.dm_position.x == 0 && m.dm_position.y == 0),
        width: mode.as_ref().map_or(0, |m| m.dm_pels_width),
        height: mode.as_ref().map_or(0, |m| m.dm_pels_height),
        refresh: mode.as_ref().map_or(0, |m| m.dm_display_frequency),
        modes,
        x: mode.as_ref().map_or(0, |m| m.dm_position.x),
        y: mode.as_ref().map_or(0, |m| m.dm_position.y),
        manufacturer: manufacturer_name(&edid.manufacturer)
            .unwrap_or(&edid.manufacturer)
            .to_string(),
        serial: edid.serial.clone(),
        fingerprint: edid.fingerprint.clone(),
        manufactured_week: edid.manufactured_week,
        manufactured_year: edid.manufactured_year,
        native_width,
        native_height,
        native_refresh,
        physical_size_cm: edid.physical_size_cm,
        gamma: edid.gamma,
        dpi_physical: edid
            .physical_size_cm
            .and_then(|size| physical_dpi(native_width, native_height, size)),
        gamut: edid.chromaticity.map(|c| edid::gamut_coverage(&c)),
        hdr: hdr_from_path(path, edid.hdr.as_ref()),
        connector: path.map(connector_for_path),
        bits_per_pel: mode.as_ref().map_or(0, |m| m.dm_bits_per_pel),
        log_pixels: mode.as_ref().map_or(0, |m| m.dm_log_pixels as u32),
        orientation: mode.as_ref().map_or(0, |m| m.dm_display_orientation),
    }
}

/// Resolves `:N` to a device; `None` selects the primary display. Numbers
/// are 1-based; `0` or an out-of-range value is an error.
pub fn resolve_device(monitor: Option<u32>, names: &[String]) -> Result<(usize, &str), String> {
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
                format!("monitor {n} not found. run rmod list to see connected displays")
            })? as usize;
            names
                .get(index)
                .map(|name| (index, name.as_str()))
                .ok_or_else(|| {
                    format!("monitor {n} not found. run rmod list to see connected displays")
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

/// Finds a monitor by its EDID identifier (case-insensitive): the serial
/// when the panel ships one, otherwise the EDID fingerprint. Returns the
/// 1-based monitor number, or None if not found.
pub fn resolve_by_id(id: &str) -> Option<u32> {
    let names = enumerate_devices();
    for (i, name) in names.iter().enumerate() {
        // Read EDID data to get the serial and fingerprint
        if let Ok(edid) = read_edid_registry(name)
            && (edid.serial.eq_ignore_ascii_case(id) || edid.fingerprint.eq_ignore_ascii_case(id))
        {
            return Some(i as u32 + 1);
        }
    }
    None
}

/// Returns a string listing all connected displays with their numbers and names,
/// for use in error messages when a monitor is not found.
pub fn connected_displays_list() -> String {
    let names = enumerate_devices();
    if names.is_empty() {
        return "no displays connected".to_string();
    }
    let mut parts = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let mode = current_mode(name);
        let display_name = if mode.is_some() {
            friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string())
        } else {
            name.to_string()
        };
        let current_mode_str = mode
            .as_ref()
            .map(|m| {
                format!(
                    "{}x{}@{}Hz",
                    m.dm_pels_width, m.dm_pels_height, m.dm_display_frequency
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        parts.push(format!("{} ({}) {}", i + 1, current_mode_str, display_name));
    }
    parts.join(", ")
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

/// Reads EDID data for a display device from the raw EDID blob the display
/// driver caches in the registry (no COM/DCOM involved, so it works even
/// when the WMI service is unavailable).
fn read_edid_registry(device_name: &str) -> Result<EdidData, String> {
    if let Some(model_id) = monitor_model_id(device_name)
        && let Some(data) = read_model_edid(&model_id)
    {
        return Ok(data);
    }
    // Some headless/virtual sessions do not expose the panel through
    // `EnumDisplayDevicesW`, so the model-id lookup above fails even though
    // Windows caches the panel EDID in the registry. Scan every DISPLAY and
    // MONITOR class instance for a valid EDID as a last resort.
    if let Some(data) = scan_model_edids() {
        return Ok(data);
    }
    Err(format!("no EDID found in registry for {device_name}"))
}

/// Reads the cached EDID for one monitor model (e.g. `LEN9059`) from the
/// DISPLAY and MONITOR device classes.
fn read_model_edid(model_id: &str) -> Option<EdidData> {
    for class in ["DISPLAY", "MONITOR"] {
        let base = format!(r"SYSTEM\CurrentControlSet\Enum\{class}\{model_id}");
        for instance in enum_subkeys(HKEY_LOCAL_MACHINE, &base) {
            let params = format!(r"{base}\{instance}\Device Parameters");
            if let Some(edid) = read_reg_binary(HKEY_LOCAL_MACHINE, &params, "EDID")
                && let Ok(data) = parse_edid(&edid)
            {
                return Some(data);
            }
        }
    }
    None
}

/// Scans every DISPLAY and MONITOR class instance for the first cached EDID
/// that parses. Only used when the per-adapter lookup cannot run.
fn scan_model_edids() -> Option<EdidData> {
    for class in ["DISPLAY", "MONITOR"] {
        let base = format!(r"SYSTEM\CurrentControlSet\Enum\{class}");
        for model_id in enum_subkeys(HKEY_LOCAL_MACHINE, &base) {
            if let Some(data) = read_model_edid(&model_id) {
                return Some(data);
            }
        }
    }
    None
}

/// Extracts the monitor model ID (e.g. `LEN9059`) from the monitor-level
/// device ID reported by [`EnumDisplayDevicesW`] (e.g.
/// `MONITOR\LEN9059\{4d36e96e-...}\0002`).
fn monitor_model_id(device_name: &str) -> Option<String> {
    let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let name_wide = encode_wide(device_name);
    let ok = unsafe { EnumDisplayDevicesW(name_wide.as_ptr(), 0, &mut monitor, 0) };
    if ok == 0 {
        return None;
    }
    wide_to_string(&monitor.device_id)
        .split('\\')
        .nth(1)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Lists every display with full EDID data and supported modes.
pub fn list_detailed() -> Result<Vec<Monitor>, String> {
    let names = enumerate_devices();
    list_detailed_from_names(names)
}

/// Lists every display (including detached) with full EDID data and supported modes.
pub fn list_all_detailed() -> Result<Vec<Monitor>, String> {
    let names = enumerate_all_devices();
    if names.is_empty() {
        return Err("no displays found, connect a display and try again".to_string());
    }
    list_detailed_from_names(names)
}

/// Builds the display name shown in output from a device's cached EDID:
/// the EDID product name when present, otherwise the manufacturer brand
/// plus the hex product code, otherwise the Windows friendly name — with
/// the EDID fingerprint suffix appended when available.
fn display_name_for(edid: &EdidData, name: &str) -> String {
    append_fingerprint(
        base_display_name(
            edid.name.clone(),
            &edid.manufacturer,
            edid.product_code,
            friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string()),
        ),
        &edid.fingerprint,
    )
}

fn list_detailed_from_names(names: Vec<String>) -> Result<Vec<Monitor>, String> {
    if names.is_empty() {
        return Err("no displays found, connect a display and try again".to_string());
    }

    let path_by_name: HashMap<String, DisplayConfigPathInfo> = match_paths()
        .into_iter()
        .map(|(source_name, path)| (source_name.to_ascii_lowercase(), path))
        .collect();

    let mut monitors = Vec::new();

    for (index, name) in names.iter().enumerate() {
        let edid = read_edid_registry(name).unwrap_or_else(|_| EdidData {
            name: None,
            manufacturer: String::new(),
            product_code: 0,
            serial: String::new(),
            fingerprint: String::new(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        });

        let display_name = display_name_for(&edid, name);

        let modes = normalize_modes(enumerate_modes(name));
        let (native_width, native_height, native_refresh) =
            if edid.native_width > 0 && edid.native_height > 0 {
                (edid.native_width, edid.native_height, edid.native_refresh)
            } else if !modes.is_empty() {
                // Fallback: use highest resolution as native
                let best = modes.last().unwrap();
                (best.width, best.height, best.refresh)
            } else {
                (0, 0, 0)
            };

        let monitor = describe_with_edid(
            index,
            name,
            display_name,
            &edid,
            native_width,
            native_height,
            native_refresh,
            modes,
            path_by_name.get(&name.to_ascii_lowercase()),
        );
        monitors.push(monitor);
    }

    Ok(monitors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_edid() -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        b
    }

    #[test]
    fn describe_with_edid_maps_manufacturer_to_brand() {
        let edid = EdidData {
            name: None,
            manufacturer: "LEN".to_string(),
            product_code: 0x9059,
            serial: String::new(),
            fingerprint: "a1b2c3d4".to_string(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        };
        let monitor = describe_with_edid(
            0,
            "\\\\.\\DISPLAY_UNUSED",
            String::new(),
            &edid,
            0,
            0,
            0,
            Vec::new(),
            None,
        );
        assert_eq!(monitor.manufacturer, "Lenovo");
        let edid_unknown = EdidData {
            manufacturer: "XYZ".to_string(),
            ..edid
        };
        let monitor = describe_with_edid(
            0,
            "\\\\.\\DISPLAY_UNUSED",
            String::new(),
            &edid_unknown,
            0,
            0,
            0,
            Vec::new(),
            None,
        );
        assert_eq!(monitor.manufacturer, "XYZ");
    }

    #[test]
    fn display_name_for_composes_edid_label() {
        let edid = EdidData {
            name: Some("Dell U2723QE".to_string()),
            manufacturer: "DEL".to_string(),
            product_code: 0x41C7,
            serial: String::new(),
            fingerprint: "a1b2c3d4".to_string(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        };
        assert_eq!(
            display_name_for(&edid, r"\\.\DISPLAY_UNUSED"),
            "Dell U2723QE [a1b2c3d4]"
        );
    }

    #[test]
    fn display_name_for_uses_brand_and_product_code_without_product_name() {
        let edid = EdidData {
            name: None,
            manufacturer: "DEL".to_string(),
            product_code: 0x41C7,
            serial: String::new(),
            fingerprint: "a1b2c3d4".to_string(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        };
        assert_eq!(
            display_name_for(&edid, r"\\.\DISPLAY_UNUSED"),
            "Dell 41C7 [a1b2c3d4]"
        );
    }

    #[test]
    fn display_name_for_omits_empty_fingerprint() {
        let edid = EdidData {
            name: Some("Dell U2723QE".to_string()),
            manufacturer: "DEL".to_string(),
            product_code: 0x41C7,
            serial: String::new(),
            fingerprint: String::new(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        };
        assert_eq!(
            display_name_for(&edid, r"\\.\DISPLAY_UNUSED"),
            "Dell U2723QE"
        );
    }

    #[test]
    fn physical_dpi_computes_horizontal_and_vertical() {
        assert_eq!(physical_dpi(1920, 1080, (59.8, 33.6)), Some((82, 82)));
        assert_eq!(physical_dpi(1920, 1080, (53.1, 29.9)), Some((92, 92)));
    }

    #[test]
    fn physical_dpi_returns_none_for_zero_dims_or_size() {
        assert_eq!(physical_dpi(0, 1080, (59.8, 33.6)), None);
        assert_eq!(physical_dpi(1920, 0, (59.8, 33.6)), None);
        assert_eq!(physical_dpi(1920, 1080, (0.0, 33.6)), None);
        assert_eq!(physical_dpi(1920, 1080, (59.8, 0.0)), None);
    }

    /// A 1920x1080 preferred timing DTD (148.5 MHz clock, 280/45 blanking).
    fn put_1920x1080_dtd(b: &mut [u8], offset: usize) {
        b[offset..offset + 18].copy_from_slice(&[
            0x02, 0x3A, 0x80, 0x18, 0x17, 0x38, 0x2D, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
    }

    #[test]
    fn describe_with_edid_computes_dpi_from_edid_size() {
        let mut b = base_edid();
        b[21] = 60;
        b[22] = 34;
        b[23] = 120; // gamma 2.2
        put_1920x1080_dtd(&mut b, 54);
        let edid = parse_edid(&b).unwrap();
        assert_eq!((edid.native_width, edid.native_height), (1920, 1080));
        let monitor = describe_with_edid(
            0,
            r"\\.\DISPLAY_UNUSED",
            String::new(),
            &edid,
            edid.native_width,
            edid.native_height,
            edid.native_refresh,
            Vec::new(),
            None,
        );
        assert_eq!(monitor.physical_size_cm, Some((60.0, 34.0)));
        // 1920*2.54/60 = 81.28 -> 81; 1080*2.54/34 = 80.72 -> 81
        assert_eq!(monitor.dpi_physical, Some((81, 81)));
        assert_eq!(monitor.gamma, Some(2.2));
        assert_eq!(monitor.bits_per_pel, 0);
        assert_eq!(monitor.log_pixels, 0);
        assert_eq!(monitor.orientation, 0);
    }

    #[test]
    fn describe_with_edid_no_dpi_without_edid_size() {
        let mut b = base_edid();
        b[23] = 0xFF; // gamma undefined
        let edid = parse_edid(&b).unwrap();
        let monitor = describe_with_edid(
            0,
            r"\\.\DISPLAY_UNUSED",
            String::new(),
            &edid,
            1920,
            1080,
            60,
            Vec::new(),
            None,
        );
        assert_eq!(monitor.physical_size_cm, None);
        assert_eq!(monitor.dpi_physical, None);
        assert_eq!(monitor.gamma, None);
        assert_eq!(monitor.gamut, None);
        assert_eq!(monitor.hdr, None);
    }

    #[test]
    fn describe_with_edid_gamut_from_chromaticity() {
        let mut b = base_edid();
        // sRGB primaries, D65 white
        b[25] = 0x96;
        b[26] = 0x05;
        b[27..35].copy_from_slice(&[0x8F, 0x52, 0x33, 0x66, 0x9A, 0x3D, 0x40, 0x51]);
        let edid = parse_edid(&b).unwrap();
        let monitor = describe_with_edid(
            0,
            r"\\.\DISPLAY_UNUSED",
            String::new(),
            &edid,
            0,
            0,
            0,
            Vec::new(),
            None,
        );
        let gamut = monitor.gamut.expect("gamut from EDID chromaticity");
        assert_eq!(gamut.srgb, 100);
        assert!((gamut.p3 as i32 - 74).abs() <= 1, "p3 = {}", gamut.p3);
    }

    #[test]
    fn describe_with_edid_hdr_falls_back_to_edid_metadata() {
        let mut b = base_edid();
        b[126] = 1;
        b.extend_from_slice(&[0u8; 128]);
        b[128] = 0x02; // CTA-861 extension
        b[131] = (0x07 << 5) | 4;
        b[132] = 0x06;
        b[133] = 0x02; // HDR10 static metadata
        let edid = parse_edid(&b).unwrap();
        let monitor = describe_with_edid(
            0,
            r"\\.\DISPLAY_UNUSED",
            String::new(),
            &edid,
            0,
            0,
            0,
            Vec::new(),
            None,
        );
        let hdr = monitor.hdr.expect("hdr from EDID fallback");
        assert!(hdr.supported);
        assert!(!hdr.active);
        assert_eq!(hdr.formats, vec!["HDR10"]);
    }

    #[test]
    fn describe_with_edid_carries_modes() {
        let edid = EdidData {
            name: None,
            manufacturer: String::new(),
            product_code: 0,
            serial: String::new(),
            fingerprint: String::new(),
            manufactured_week: 0,
            manufactured_year: 0,
            native_width: 0,
            native_height: 0,
            native_refresh: 0,
            physical_size_cm: None,
            gamma: None,
            chromaticity: None,
            hdr: None,
        };
        let modes = vec![
            crate::sys::windows::capabilities::Mode {
                width: 1920,
                height: 1080,
                refresh: 60,
            },
            crate::sys::windows::capabilities::Mode {
                width: 2560,
                height: 1440,
                refresh: 144,
            },
        ];
        let expected = vec![
            crate::sys::windows::capabilities::Mode {
                width: 1920,
                height: 1080,
                refresh: 60,
            },
            crate::sys::windows::capabilities::Mode {
                width: 2560,
                height: 1440,
                refresh: 144,
            },
        ];
        let monitor = describe_with_edid(
            0,
            r"\\.\DISPLAY_UNUSED",
            String::new(),
            &edid,
            0,
            0,
            0,
            modes,
            None,
        );
        assert_eq!(monitor.modes, expected);
    }

    #[test]
    fn describe_sets_new_fields_to_defaults() {
        let monitor = describe(0, r"\\.\DISPLAY_UNUSED");
        assert_eq!(monitor.physical_size_cm, None);
        assert_eq!(monitor.gamma, None);
        assert_eq!(monitor.dpi_physical, None);
        assert_eq!(monitor.gamut, None);
        assert_eq!(monitor.hdr, None);
        assert_eq!(monitor.bits_per_pel, 0);
        assert_eq!(monitor.log_pixels, 0);
        assert_eq!(monitor.orientation, 0);
    }
}
