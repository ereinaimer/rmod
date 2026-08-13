pub(crate) const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
pub(crate) const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: u32 = 0x1;

#[repr(C)]
pub(crate) struct POINTL {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub(crate) struct DEVMODEW {
    pub dm_device_name: [u16; 32],
    pub dm_spec_version: u16,
    pub dm_driver_version: u16,
    pub dm_size: u16,
    pub dm_driver_extra: u16,
    pub dm_fields: u32,
    pub dm_position: POINTL,
    pub dm_display_orientation: u32,
    pub dm_display_fixed_output: u32,
    pub dm_color: i16,
    pub dm_duplex: i16,
    pub dm_y_resolution: i16,
    pub dm_tt_option: i16,
    pub dm_collate: i16,
    pub dm_form_name: [u16; 32],
    pub dm_log_pixels: u16,
    pub dm_bits_per_pel: u32,
    pub dm_pels_width: u32,
    pub dm_pels_height: u32,
    pub dm_display_flags: u32,
    pub dm_display_frequency: u32,
    pub dm_icm_method: u32,
    pub dm_icm_intent: u32,
    pub dm_media_type: u32,
    pub dm_dither_type: u32,
    pub dm_reserved1: u32,
    pub dm_reserved2: u32,
    pub dm_panning_width: u32,
    pub dm_panning_height: u32,
}

#[repr(C)]
pub(crate) struct DISPLAY_DEVICEW {
    pub cb: u32,
    pub device_name: [u16; 32],
    pub device_string: [u16; 128],
    pub state_flags: u32,
    pub device_id: [u16; 128],
    pub device_key: [u16; 128],
}

#[link(name = "user32")]
unsafe extern "system" {
    pub(crate) fn EnumDisplayDevicesW(
        lp_device: *const u16,
        i_dev_num: u32,
        lp_display_device: *mut DISPLAY_DEVICEW,
        dw_flags: u32,
    ) -> i32;
    pub(crate) fn EnumDisplaySettingsW(
        lpsz_device_name: *const u16,
        i_mode_num: u32,
        lp_dev_mode: *mut DEVMODEW,
    ) -> i32;
}

pub(crate) fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn wide_to_string(w: &[u16]) -> String {
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