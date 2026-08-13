use super::bindings::{
    encode_wide, wide_to_string, DEVMODEW, DISPLAY_DEVICEW, EnumDisplayDevicesW,
    EnumDisplaySettingsW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, ENUM_CURRENT_SETTINGS,
};

pub struct Monitor {
    pub number: u32,
    pub name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

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

pub(crate) fn current_mode(name: &str) -> Option<DEVMODEW> {
    let name_wide = encode_wide(name);
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) };
    if ok == 0 {
        None
    } else {
        Some(mode)
    }
}

pub(crate) fn friendly_name(device: &[u16]) -> Option<String> {
    let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let ok = unsafe { EnumDisplayDevicesW(device.as_ptr(), 0, &mut monitor, 0) };
    if ok == 0 {
        return None;
    }
    Some(wide_to_string(&monitor.device_string))
}

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
    }
}

pub(crate) fn resolve_device(
    monitor: Option<u32>,
    names: &[String],
) -> Result<(usize, String), String> {
    match monitor {
        None => {
            for (i, name) in names.iter().enumerate() {
                if let Some(mode) = current_mode(name) {
                    if mode.dm_position.x == 0 && mode.dm_position.y == 0 {
                        return Ok((i, name.clone()));
                    }
                }
            }
            names
                .first()
                .cloned()
                .map(|name| (0, name))
                .ok_or_else(|| "no displays found".to_string())
        }
        Some(n) => {
            let index = n
                .checked_sub(1)
                .ok_or_else(|| format!("monitor {n} not found"))? as usize;
            names
                .get(index)
                .cloned()
                .map(|name| (index, name))
                .ok_or_else(|| format!("monitor {n} not found"))
        }
    }
}