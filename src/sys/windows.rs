const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: u32 = 0x1;

#[repr(C)]
struct POINTL {
    x: i32,
    y: i32,
}

#[repr(C)]
struct DEVMODEW {
    dm_device_name: [u16; 32],
    dm_spec_version: u16,
    dm_driver_version: u16,
    dm_size: u16,
    dm_driver_extra: u16,
    dm_fields: u32,
    dm_position: POINTL,
    dm_display_orientation: u32,
    dm_display_fixed_output: u32,
    dm_color: i16,
    dm_duplex: i16,
    dm_y_resolution: i16,
    dm_tt_option: i16,
    dm_collate: i16,
    dm_form_name: [u16; 32],
    dm_log_pixels: u16,
    dm_bits_per_pel: u32,
    dm_pels_width: u32,
    dm_pels_height: u32,
    dm_display_flags: u32,
    dm_display_frequency: u32,
    dm_icm_method: u32,
    dm_icm_intent: u32,
    dm_media_type: u32,
    dm_dither_type: u32,
    dm_reserved1: u32,
    dm_reserved2: u32,
    dm_panning_width: u32,
    dm_panning_height: u32,
}

#[repr(C)]
struct DISPLAY_DEVICEW {
    cb: u32,
    device_name: [u16; 32],
    device_string: [u16; 128],
    state_flags: u32,
    device_id: [u16; 128],
    device_key: [u16; 128],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumDisplayDevicesW(
        lp_device: *const u16,
        i_dev_num: u32,
        lp_display_device: *mut DISPLAY_DEVICEW,
        dw_flags: u32,
    ) -> i32;
    fn EnumDisplaySettingsW(
        lpsz_device_name: *const u16,
        i_mode_num: u32,
        lp_dev_mode: *mut DEVMODEW,
    ) -> i32;
}

pub struct Monitor {
    pub number: u32,
    pub name: String,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

#[derive(Debug, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

pub fn list() -> Result<Vec<Monitor>, String> {
    let names = enumerate_devices();
    let monitors: Vec<Monitor> = names
        .iter()
        .enumerate()
        .map(|(i, name)| describe(i, name))
        .collect();
    if monitors.is_empty() {
        return Err("no displays found".into());
    }
    Ok(monitors)
}

pub fn caps(monitor: Option<u32>) -> Result<(Monitor, Vec<Mode>), String> {
    let names = enumerate_devices();
    let (index, name) = resolve_device(monitor, &names)?;
    let name_wide = encode_wide(&name);
    let mut modes = Vec::new();
    let mut mode_index = 0u32;
    loop {
        let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
        let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), mode_index, &mut mode) };
        if ok == 0 {
            break;
        }
        modes.push(Mode {
            width: mode.dm_pels_width,
            height: mode.dm_pels_height,
            refresh: mode.dm_display_frequency,
        });
        mode_index += 1;
    }
    if modes.is_empty() {
        return Err(format!("no supported modes found for monitor {}", index + 1));
    }
    Ok((describe(index, &name), normalize_modes(modes)))
}

fn enumerate_devices() -> Vec<String> {
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

fn resolve_device(monitor: Option<u32>, names: &[String]) -> Result<(usize, String), String> {
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

fn describe(index: usize, name: &str) -> Monitor {
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

fn current_mode(name: &str) -> Option<DEVMODEW> {
    let name_wide = encode_wide(name);
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) };
    if ok == 0 {
        None
    } else {
        Some(mode)
    }
}

fn friendly_name(device: &[u16]) -> Option<String> {
    let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let ok = unsafe { EnumDisplayDevicesW(device.as_ptr(), 0, &mut monitor, 0) };
    if ok == 0 {
        return None;
    }
    Some(wide_to_string(&monitor.device_string))
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
}

fn normalize_modes(mut modes: Vec<Mode>) -> Vec<Mode> {
    modes.sort_by_key(|m| (m.width, m.height, m.refresh));
    modes.dedup_by(|a, b| a.width == b.width && a.height == b.height && a.refresh == b.refresh);
    modes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn devmode_layout_is_220_bytes() {
        assert_eq!(std::mem::size_of::<DEVMODEW>(), 220);
    }

    #[test]
    fn display_device_layout_is_840_bytes() {
        assert_eq!(std::mem::size_of::<DISPLAY_DEVICEW>(), 840);
    }

    #[test]
    fn devmode_field_offsets() {
        assert_eq!(offset_of!(DEVMODEW, dm_position), 76);
        assert_eq!(offset_of!(DEVMODEW, dm_pels_width), 172);
        assert_eq!(offset_of!(DEVMODEW, dm_pels_height), 176);
        assert_eq!(offset_of!(DEVMODEW, dm_display_frequency), 184);
    }

    #[test]
    fn display_device_field_offsets() {
        assert_eq!(offset_of!(DISPLAY_DEVICEW, cb), 0);
        assert_eq!(offset_of!(DISPLAY_DEVICEW, device_name), 4);
        assert_eq!(offset_of!(DISPLAY_DEVICEW, state_flags), 324);
    }

    #[test]
    fn wide_to_string_trims_at_nul() {
        let w = [b'D'.into(), b'E'.into(), 0, 0, 0];
        assert_eq!(wide_to_string(&w), "DE");
    }

    #[test]
    fn wide_to_string_without_nul_uses_all() {
        let w = [b'A'.into(), b'B'.into(), b'C'.into()];
        assert_eq!(wide_to_string(&w), "ABC");
    }

    #[test]
    fn wide_to_string_empty() {
        assert_eq!(wide_to_string(&[0, 0, 0]), "");
    }

    #[test]
    fn wide_to_string_surrogate_pair() {
        let w = [0xD83D, 0xDCA9, 0];
        assert_eq!(wide_to_string(&w), "\u{1F4A9}");
    }

    #[test]
    fn wide_to_string_lone_surrogate_is_replacement_char() {
        let w = [0xD800, 0];
        assert_eq!(wide_to_string(&w), "\u{FFFD}");
    }

    #[test]
    fn normalize_modes_dedupes_and_sorts() {
        let modes = vec![
            Mode { width: 1920, height: 1080, refresh: 144 },
            Mode { width: 3840, height: 2160, refresh: 60 },
            Mode { width: 1920, height: 1080, refresh: 144 },
            Mode { width: 1920, height: 1080, refresh: 60 },
        ];
        assert_eq!(
            normalize_modes(modes),
            vec![
                Mode { width: 1920, height: 1080, refresh: 60 },
                Mode { width: 1920, height: 1080, refresh: 144 },
                Mode { width: 3840, height: 2160, refresh: 60 },
            ]
        );
    }

    #[test]
    fn normalize_modes_empty() {
        assert_eq!(normalize_modes(Vec::new()), Vec::new());
    }
}