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

pub fn list() -> Result<Vec<Monitor>, String> {
    let mut monitors = Vec::new();
    let mut index = 0u32;
    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) };
        if ok == 0 {
            break;
        }
        if device.state_flags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP != 0 {
            let name = wide_to_string(&device.device_name);
            let friendly = friendly_name(&device.device_name).unwrap_or(name.clone());
            let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
            let ok = unsafe { EnumDisplaySettingsW(device.device_name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode) };
            if ok != 0 {
                monitors.push(Monitor {
                    number: monitors.len() as u32 + 1,
                    name: friendly,
                    is_primary: mode.dm_position.x == 0 && mode.dm_position.y == 0,
                    width: mode.dm_pels_width,
                    height: mode.dm_pels_height,
                    refresh: mode.dm_display_frequency,
                });
            }
        }
        index += 1;
    }
    if monitors.is_empty() {
        return Err("no displays found".into());
    }
    Ok(monitors)
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

fn wide_to_string(w: &[u16]) -> String {
    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..end])
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
}
