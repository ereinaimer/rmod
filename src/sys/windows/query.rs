//! Device enumeration and current-state queries.
//!
//! Everything here reads what is connected right now: device names,
//! friendly names, current mode, and the primary-display designation.

use super::bindings::{
    DevmodeW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_DISCONNECT,
    DISPLAY_DEVICE_MIRRORING_DRIVER, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    ENUM_REGISTRY_SETTINGS, ERROR_SUCCESS, EnumDisplayDevicesW, EnumDisplaySettingsW,
    HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
    RegQueryValueExW, encode_wide, wide_to_string,
};
use super::capabilities::{enumerate_modes, normalize_modes};
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr;

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
    /// Desktop x coordinate from the current mode.
    #[allow(dead_code)]
    pub x: i32,
    /// Desktop y coordinate from the current mode.
    #[allow(dead_code)]
    pub y: i32,
    /// EDID manufacturer code (3-char, e.g., "DEL").
    pub manufacturer: String,
    /// EDID serial number, kept for targeting when a panel ships one.
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
    }
}

/// Builds a [`Monitor`] with full EDID data.
///
/// `name` is the Win32 device name; `display_name` is the name shown in
/// output (EDID-derived when available).
fn describe_with_edid(
    index: usize,
    name: &str,
    display_name: String,
    edid: &EdidData,
    native_width: u32,
    native_height: u32,
    native_refresh: u32,
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
    }
}

/// Resolves `:N` to a device; `None` selects the primary display. Numbers
/// are 1-based; `0` or an out-of-range value is an error.
pub fn resolve_device(
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
            && (edid.serial.eq_ignore_ascii_case(id)
                || edid.fingerprint.eq_ignore_ascii_case(id))
        {
            return Some(i as u32 + 1);
        }
    }
    None
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

/// EDID data extracted from the monitor's cached EDID block.
struct EdidData {
    /// Display product name from the 0xFC descriptor.
    name: Option<String>,
    manufacturer: String,
    /// EDID product code (bytes 10-11, little-endian).
    product_code: u16,
    serial: String,
    /// First 8 hex chars of the SHA-256 of the raw EDID blob.
    fingerprint: String,
    manufactured_week: u8,
    manufactured_year: u16,
    native_width: u32,
    native_height: u32,
    native_refresh: u32,
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

/// Enumerates the sub-key names of a registry key.
fn enum_subkeys(hive: isize, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let path_wide = to_wide_string(path);
        let mut key: *mut c_void = ptr::null_mut();
        if RegOpenKeyExW(hive as *mut c_void, path_wide.as_ptr(), 0, KEY_READ, &mut key)
            != ERROR_SUCCESS
        {
            return out;
        }
        let mut index: u32 = 0;
        loop {
            let mut name_buf = [0u16; 260];
            let mut name_len: u32 = name_buf.len() as u32;
            let hr = RegEnumKeyExW(
                key,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if hr != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&name_buf[..name_len as usize]));
            index += 1;
        }
        RegCloseKey(key);
    }
    out
}

/// Reads a REG_BINARY value as raw bytes.
fn read_reg_binary(hive: isize, path: &str, value: &str) -> Option<Vec<u8>> {
    unsafe {
        let path_wide = to_wide_string(path);
        let value_wide = to_wide_string(value);
        let mut key: *mut c_void = ptr::null_mut();
        if RegOpenKeyExW(hive as *mut c_void, path_wide.as_ptr(), 0, KEY_READ, &mut key)
            != ERROR_SUCCESS
        {
            return None;
        }
        let mut size: u32 = 0;
        let mut ty: u32 = 0;
        let hr = RegQueryValueExW(
            key,
            value_wide.as_ptr(),
            ptr::null_mut(),
            &mut ty,
            ptr::null_mut(),
            &mut size,
        );
        let data = if hr == ERROR_SUCCESS && ty == REG_BINARY && size > 0 {
            let mut buf = vec![0u8; size as usize];
            let hr = RegQueryValueExW(
                key,
                value_wide.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            );
            if hr == ERROR_SUCCESS {
                buf.truncate(size as usize);
                Some(buf)
            } else {
                None
            }
        } else {
            None
        };
        RegCloseKey(key);
        data
    }
}

/// Parses the base block of an EDID blob into monitor identity fields.
fn parse_edid(bytes: &[u8]) -> Result<EdidData, String> {
    if bytes.len() < 128 {
        return Err("EDID too short".to_string());
    }
    if bytes[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return Err("bad EDID header".to_string());
    }
    let manufacturer = pnp_manufacturer(&bytes[8..10]);
    let product_code = u16::from_le_bytes([bytes[10], bytes[11]]);
    let fingerprint = edid_fingerprint(bytes);
    let serial = String::from_utf8_lossy(&bytes[12..16])
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string();
    let manufactured_week = bytes[16];
    let manufactured_year = if manufactured_week == 0 {
        let y = bytes[17];
        if y <= 0x0F {
            1990 + y as u16
        } else {
            2000 + y as u16
        }
    } else {
        1990 + bytes[17] as u16
    };
    let (native_width, native_height, native_refresh) = preferred_timing(&bytes[54..72]);

    let mut serial_descriptor = None;
    let mut product_name = None;
    for slot in [&bytes[54..72], &bytes[72..90], &bytes[90..108], &bytes[108..126]] {
        if let Some((kind, text)) = display_descriptor(slot) {
            match kind {
                DescriptorKind::Serial => serial_descriptor = Some(text),
                DescriptorKind::ProductName => {
                    product_name.get_or_insert(text);
                }
            }
        }
    }
    let serial = if serial.is_empty() {
        serial_descriptor.unwrap_or_default()
    } else {
        serial
    };

    Ok(EdidData {
        name: product_name,
        manufacturer,
        product_code,
        serial,
        fingerprint,
        manufactured_week,
        manufactured_year,
        native_width,
        native_height,
        native_refresh,
    })
}

/// The kinds of display descriptors an EDID detailed-timing slot can hold.
enum DescriptorKind {
    Serial,
    ProductName,
}

/// Decodes a non-timing display descriptor slot (pixel clock zero) into its
/// kind and trimmed text, or `None` when the slot is a timing descriptor or
/// holds an unknown/reserved descriptor.
fn display_descriptor(slot: &[u8]) -> Option<(DescriptorKind, String)> {
    if slot.len() < 18 {
        return None;
    }
    let clock = ((slot[1] as u32) << 8) | slot[0] as u32;
    if clock != 0 {
        return None;
    }
    let kind = match slot[3] {
        0xFF => DescriptorKind::Serial,
        0xFC => DescriptorKind::ProductName,
        _ => return None,
    };
    let text = String::from_utf8_lossy(&slot[4..18])
        .trim_matches(|c: char| c == '\0' || c.is_control() || c == ' ')
        .to_string();
    if text.is_empty() {
        return None;
    }
    Some((kind, text))
}

/// Maps a PNP manufacturer code (the EDID manufacturer field) to a readable
/// brand name, or `None` when the code is unknown.
fn manufacturer_name(code: &str) -> Option<&'static str> {
    match code {
        "LEN" => Some("Lenovo"),
        "DEL" => Some("Dell"),
        "HPN" | "HWP" => Some("HP"),
        "SAM" | "SEC" => Some("Samsung"),
        "AOC" => Some("AOC"),
        "ACR" => Some("Acer"),
        "BNQ" => Some("BenQ"),
        "GSM" => Some("LG"),
        "VSC" => Some("ViewSonic"),
        "PHL" => Some("Philips"),
        "SHP" => Some("Sharp"),
        "TOS" => Some("Toshiba"),
        _ => None,
    }
}

/// Decodes the 3-letter PNP manufacturer code from EDID bytes 8-9
/// (e.g. bytes `0x30 0xAE` decode to "LEN").
fn pnp_manufacturer(pair: &[u8]) -> String {
    fn letter(v: u8) -> char {
        if (1..=26).contains(&v) {
            (b'A' + v - 1) as char
        } else {
            '?'
        }
    }
    let a = pair[0] >> 2;
    let b = ((pair[0] & 0x03) << 3) | (pair[1] >> 5);
    let c = pair[1] & 0x1F;
    format!("{}{}{}", letter(a), letter(b), letter(c))
}

/// Reads the preferred timing (first detailed timing descriptor) and
/// returns `(width, height, refresh)`, or `(0, 0, 0)` when the descriptor
/// is a non-timing descriptor or the values are implausible.
fn preferred_timing(dtd: &[u8]) -> (u32, u32, u32) {
    let clock = ((dtd[1] as u32) << 8) | dtd[0] as u32;
    if clock == 0 {
        return (0, 0, 0);
    }
    let width = (((dtd[4] & 0x0F) as u32) << 8) | dtd[2] as u32;
    let height = (((dtd[7] & 0x0F) as u32) << 8) | dtd[5] as u32;
    let h_blank = (((dtd[4] >> 4) as u32) << 8) | dtd[3] as u32;
    let v_blank = (((dtd[7] >> 4) as u32) << 8) | dtd[6] as u32;
    let h_total = width + h_blank;
    let v_total = height + v_blank;
    if h_total == 0 || v_total == 0 {
        return (0, 0, 0);
    }
    let refresh = clock * 10_000 / (h_total * v_total);
    if (320..=7680).contains(&width)
        && (200..=4320).contains(&height)
        && (24..=300).contains(&refresh)
    {
        (width, height, refresh)
    } else {
        (0, 0, 0)
    }
}

fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Builds the base display name from EDID identity, following the industry
/// convention (ddcutil, edid-decode, fastfetch): the 0xFC product name when
/// present, otherwise the manufacturer brand plus the hex product code
/// (e.g. "Lenovo 9059"), otherwise the Windows friendly name.
fn base_display_name(
    product_name: Option<String>,
    manufacturer: &str,
    product_code: u16,
    friendly: String,
) -> String {
    if let Some(name) = product_name {
        return name;
    }
    if let Some(brand) = manufacturer_name(manufacturer) {
        return format!("{brand} {product_code:04X}");
    }
    friendly
}

/// Appends the EDID fingerprint to a base display name: `Name [a1b2c3d4]`.
/// The suffix is omitted when no fingerprint is available (EDID read failed).
fn append_fingerprint(name: String, fingerprint: &str) -> String {
    if fingerprint.is_empty() {
        name
    } else {
        format!("{name} [{fingerprint}]")
    }
}

/// The first 8 hex characters of the SHA-256 of the raw EDID blob: a stable,
/// per-panel identifier that works even when the panel ships no serial.
fn edid_fingerprint(bytes: &[u8]) -> String {
    sha256(bytes)
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// SHA-256 of `data` as a `[u8; 32]`. Implemented inline (the crate has no
/// dependencies) following FIPS 180-4.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1,
        0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
        0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786,
        0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
        0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147,
        0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
        0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
        0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
        0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a,
        0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
        0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let mut h: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c,
        0x1f83_d9ab, 0x5be0_cd19,
    ];
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Lists every display with full EDID data and supported modes.
pub fn list_detailed() -> Result<Vec<Monitor>, String> {
    let names = enumerate_devices();
    if names.is_empty() {
        return Err("no displays found, connect a display and try again".to_string());
    }

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
        });

        let display_name = append_fingerprint(
            base_display_name(
                edid.name.clone(),
                &edid.manufacturer,
                edid.product_code,
                friendly_name(&encode_wide(name)).unwrap_or_else(|| name.to_string()),
            ),
            &edid.fingerprint,
        );

        let modes = normalize_modes(enumerate_modes(name));
        let (native_width, native_height, native_refresh) = if edid.native_width > 0 && edid.native_height > 0 {
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
    fn parse_edid_reads_manufacturer_serial_and_dates() {
        let mut b = base_edid();
        b[8] = 0x30;
        b[9] = 0xAE; // LEN
        b[12..16].copy_from_slice(b"ABC1");
        b[16] = 34;
        b[17] = 29; // week 34, year 2019
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.manufacturer, "LEN");
        assert_eq!(edid.serial, "ABC1");
        assert_eq!(edid.manufactured_week, 34);
        assert_eq!(edid.manufactured_year, 2019);
    }

    #[test]
    fn parse_edid_trims_trailing_nuls_in_serial() {
        let mut b = base_edid();
        b[12..16].copy_from_slice(&[b'A', 0, 0, 0]);
        assert_eq!(parse_edid(&b).unwrap().serial, "A");
    }

    #[test]
    fn parse_edid_model_year_when_week_is_zero() {
        let mut b = base_edid();
        b[16] = 0;
        b[17] = 0x05;
        assert_eq!(parse_edid(&b).unwrap().manufactured_year, 1995);

        let mut b = base_edid();
        b[16] = 0;
        b[17] = 0x18;
        assert_eq!(parse_edid(&b).unwrap().manufactured_year, 2024);
    }

    #[test]
    fn parse_edid_rejects_short_blob_and_bad_header() {
        assert!(parse_edid(&[0u8; 16]).is_err());
        let mut b = base_edid();
        b[0] = 0x01;
        assert!(parse_edid(&b).is_err());
    }

    #[test]
    fn pnp_manufacturer_decodes_len() {
        assert_eq!(pnp_manufacturer(&[0x30, 0xAE]), "LEN");
        assert_eq!(pnp_manufacturer(&[0x10, 0xAC]), "DEL");
        assert_eq!(pnp_manufacturer(&[0x4C, 0x2D]), "SAM");
    }

    #[test]
    fn parse_edid_reads_product_code_little_endian() {
        let mut b = base_edid();
        b[10] = 0x59;
        b[11] = 0x90;
        assert_eq!(parse_edid(&b).unwrap().product_code, 0x9059);
    }

    #[test]
    fn parse_edid_computes_stable_fingerprint() {
        let b = base_edid();
        let fp = parse_edid(&b).unwrap().fingerprint;
        assert_eq!(fp.len(), 8, "fingerprint is 8 hex chars");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(parse_edid(&b).unwrap().fingerprint, fp, "deterministic");
    }

    #[test]
    fn edid_fingerprint_changes_with_blob() {
        let a = base_edid();
        let mut b = base_edid();
        b[16] = 34;
        assert_ne!(edid_fingerprint(&a), edid_fingerprint(&b));
    }

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("abc")
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d,
                0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10,
                0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn base_display_name_prefers_product_name() {
        assert_eq!(
            base_display_name(
                Some("DELL P2411H".to_string()),
                "DEL",
                0x5e42,
                "Generic PnP Monitor".to_string()
            ),
            "DELL P2411H"
        );
    }

    #[test]
    fn base_display_name_uses_brand_and_product_code() {
        assert_eq!(
            base_display_name(None, "LEN", 0x9059, "Generic PnP Monitor".to_string()),
            "Lenovo 9059"
        );
    }

    #[test]
    fn base_display_name_unknown_manufacturer_uses_friendly() {
        assert_eq!(
            base_display_name(None, "XYZ", 0x1234, "Generic PnP Monitor".to_string()),
            "Generic PnP Monitor"
        );
    }

    #[test]
    fn append_fingerprint_adds_suffix() {
        assert_eq!(
            append_fingerprint("Lenovo 9059".to_string(), "a1b2c3d4"),
            "Lenovo 9059 [a1b2c3d4]"
        );
    }

    #[test]
    fn append_fingerprint_omits_empty_suffix() {
        assert_eq!(append_fingerprint("X".to_string(), ""), "X");
    }

    #[test]
    fn preferred_timing_decodes_1920x1080_at_60hz() {
        let mut dtd = [0u8; 18];
        // pixel clock 148.5 MHz, h 1920(+280 blank), v 1080(+45 blank)
        dtd[0] = 0x02;
        dtd[1] = 0x3A; // 0x3A02 = 14850 units of 10 kHz
        dtd[2] = 0x80;
        dtd[3] = 0x18;
        dtd[4] = 0x17; // h blank hi 1, h active hi 7
        dtd[5] = 0x38;
        dtd[6] = 0x2D;
        dtd[7] = 0x04; // v blank hi 0, v active hi 4
        let (w, h, r) = preferred_timing(&dtd);
        assert_eq!((w, h, r), (1920, 1080, 60));
    }

    #[test]
    fn preferred_timing_ignores_zero_clock_descriptor() {
        let dtd = [0u8; 18];
        assert_eq!(preferred_timing(&dtd), (0, 0, 0));
    }

    #[test]
    fn preferred_timing_rejects_implausible_resolution() {
        let mut dtd = [0u8; 18];
        dtd[0] = 0x6F;
        dtd[1] = 0x54; // 216.15 MHz
        dtd[2] = 0x80;
        dtd[3] = 0x9C;
        dtd[4] = 0x70; // h active hi 0 -> width 128
        dtd[5] = 0x38;
        dtd[6] = 0x3E;
        dtd[7] = 0x40;
        assert_eq!(preferred_timing(&dtd), (0, 0, 0));
    }

/// Writes a non-timing display descriptor into a 18-byte EDID slot. The
/// descriptor text field is 13 bytes; longer text is truncated.
fn put_descriptor(b: &mut [u8], offset: usize, tag: u8, text: &str) {
    b[offset..offset + 3].fill(0);
    b[offset + 3] = tag;
    b[offset + 4..offset + 18].fill(0);
    let bytes = text.as_bytes();
    let n = bytes.len().min(13);
    b[offset + 4..offset + 4 + n].copy_from_slice(&bytes[..n]);
}

    #[test]
    fn parse_edid_reads_product_name_descriptor() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0xFC, "XYZZY 9000");
        assert_eq!(parse_edid(&b).unwrap().name.as_deref(), Some("XYZZY 9000"));
    }

    #[test]
    fn parse_edid_ignores_ascii_string_descriptor_for_name() {
        let mut b = base_edid();
        put_descriptor(&mut b, 108, 0xFE, "B156HAN13.1");
        assert_eq!(parse_edid(&b).unwrap().name, None);
    }

    #[test]
    fn parse_edid_reads_serial_descriptor_when_serial_field_empty() {
        let mut b = base_edid();
        put_descriptor(&mut b, 90, 0xFF, "SN1234567");
        assert_eq!(parse_edid(&b).unwrap().serial, "SN1234567");
    }

    #[test]
    fn parse_edid_prefers_serial_field_over_descriptor() {
        let mut b = base_edid();
        b[12..16].copy_from_slice(b"ABC1");
        put_descriptor(&mut b, 90, 0xFF, "SN1234567");
        assert_eq!(parse_edid(&b).unwrap().serial, "ABC1");
    }

    #[test]
    fn parse_edid_prefers_product_name_over_ascii_string() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0xFE, "GenericStr");
        put_descriptor(&mut b, 90, 0xFC, "RealModel");
        assert_eq!(parse_edid(&b).unwrap().name.as_deref(), Some("RealModel"));
    }

    #[test]
    fn manufacturer_name_maps_common_brands() {
        assert_eq!(manufacturer_name("LEN"), Some("Lenovo"));
        assert_eq!(manufacturer_name("DEL"), Some("Dell"));
        assert_eq!(manufacturer_name("HPN"), Some("HP"));
        assert_eq!(manufacturer_name("SAM"), Some("Samsung"));
        assert_eq!(manufacturer_name("XYZ"), None);
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
        };
        let monitor =
            describe_with_edid(0, "\\\\.\\DISPLAY_UNUSED", String::new(), &edid, 0, 0, 0);
        assert_eq!(monitor.manufacturer, "Lenovo");
        let edid_unknown = EdidData {
            manufacturer: "XYZ".to_string(),
            ..edid
        };
        let monitor =
            describe_with_edid(0, "\\\\.\\DISPLAY_UNUSED", String::new(), &edid_unknown, 0, 0, 0);
        assert_eq!(monitor.manufacturer, "XYZ");
    }

    #[test]
    fn parse_edid_ignores_unknown_and_range_limit_descriptors() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0x0F, "dummy"); // reserved tag
        put_descriptor(&mut b, 90, 0xFD, ""); // range limits, no text
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.name, None);
        assert_eq!(edid.serial, "");
    }

    #[test]
    fn parse_edid_matches_lenovo_b156han_real_blob() {
        // The base block captured from this machine's Lenovo panel.
        let b = [
            0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x30, 0xAE, 0x59, 0x90, 0x00, 0x00,
            0x00, 0x00, 0x22, 0x1D, 0x01, 0x04, 0xA5, 0x22, 0x13, 0x78, 0x03, 0x48, 0x35, 0x8F,
            0x57, 0x59, 0x92, 0x29, 0x1E, 0x50, 0x54, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x54, 0x6F,
            0x80, 0x9C, 0x70, 0x38, 0x3E, 0x40, 0x6C, 0x30, 0xAA, 0x00, 0x58, 0xC1, 0x10, 0x00,
            0x00, 0x18, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x00, 0x3C, 0x78, 0x8A,
            0x8A, 0x1D, 0x01, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00, 0x00, 0xFE,
            0x00, 0x42, 0x31, 0x35, 0x36, 0x48, 0x41, 0x4E, 0x31, 0x33, 0x2E, 0x31, 0x20, 0x0A,
            0x00, 0x55,
        ];
        assert_eq!(b.len(), 128);
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.manufacturer, "LEN");
        assert_eq!(edid.product_code, 0x9059);
        assert_eq!(edid.name, None); // panel model lives in a 0xFE string, not a 0xFC product name
        assert_eq!(edid.serial, ""); // this panel ships with no EDID serial
        assert_eq!(edid.fingerprint.len(), 8);
        assert!(
            edid.fingerprint.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex, got '{}'",
            edid.fingerprint
        );
        assert_eq!(edid.manufactured_week, 34);
        assert_eq!(edid.manufactured_year, 2019);
        assert_eq!((edid.native_width, edid.native_height), (0, 0)); // DTD1 implausible
    }
}
