//! Raw Win32 FFI bindings: structs, externs, and string marshalling.
//!
//! `DevmodeW`/`DISPLAY_DEVICEW` layouts and offsets are pinned by tests;
//! do not reorder fields. Used by [`super::query`], [`super::capabilities`],
//! and [`super::apply`].

use std::ffi::c_void;

// Registry access for reading the monitor's cached EDID blob (no COM/DCOM).
pub(crate) const HKEY_LOCAL_MACHINE: isize = -0x7FFF_FFFE;
pub(crate) const KEY_READ: u32 = 0x0002_0019;
pub(crate) const REG_BINARY: u32 = 3;
pub(crate) const REG_SZ: u32 = 1;
pub(crate) const ERROR_SUCCESS: i32 = 0;

#[link(name = "advapi32")]
unsafe extern "system" {
    pub(crate) fn RegOpenKeyExW(
        h_key: *mut c_void,
        lp_sub_key: *const u16,
        ul_options: u32,
        sam_desired: u32,
        phk_result: *mut *mut c_void,
    ) -> i32;
    pub(crate) fn RegEnumKeyExW(
        h_key: *mut c_void,
        dw_index: u32,
        lp_name: *mut u16,
        lpc_name: *mut u32,
        lp_reserved: *mut u32,
        lp_class: *mut u16,
        lpc_class: *mut u32,
        lpft_last_write_time: *mut c_void,
    ) -> i32;
    pub(crate) fn RegQueryValueExW(
        h_key: *mut c_void,
        lp_value_name: *const u16,
        lp_reserved: *mut u32,
        lp_type: *mut u32,
        lp_data: *mut u8,
        lpcb_data: *mut u32,
    ) -> i32;
    pub(crate) fn RegCloseKey(h_key: *mut c_void) -> i32;
}

pub(crate) const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
pub(crate) const ENUM_REGISTRY_SETTINGS: u32 = 0xFFFF_FFFE;
pub(crate) const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: u32 = 0x1;
pub(crate) const DISPLAY_DEVICE_MIRRORING_DRIVER: u32 = 0x8;
pub(crate) const DISPLAY_DEVICE_DISCONNECT: u32 = 0x0200_0000;
pub(crate) const WM_SYSCOMMAND: u32 = 0x0112;
pub(crate) const SC_MONITORPOWER: usize = 0xF170;
pub(crate) const HWND_BROADCAST: usize = 0xFFFF;
pub(crate) const CDS_UPDATEREGISTRY: u32 = 0x1;
pub(crate) const CDS_TEST: u32 = 0x2;
pub(crate) const DM_PELSWIDTH: u32 = 0x0008_0000;
pub(crate) const DM_PELSHEIGHT: u32 = 0x0010_0000;
pub(crate) const DM_DISPLAYFREQUENCY: u32 = 0x0040_0000;
pub(crate) const DM_DISPLAYORIENTATION: u32 = 0x0000_0080;
pub(crate) const DM_POSITION: u32 = 0x0000_0020;
pub(crate) const DISP_CHANGE_SUCCESSFUL: i32 = 0;
pub(crate) const DISP_CHANGE_RESTART: i32 = 1;
pub(crate) const DISP_CHANGE_FAILED: i32 = -1;
pub(crate) const DISP_CHANGE_BADMODE: i32 = -2;
pub(crate) const DISP_CHANGE_NOTUPDATED: i32 = -3;
pub(crate) const DISP_CHANGE_BADFLAGS: i32 = -4;
pub(crate) const DISP_CHANGE_BADPARAM: i32 = -5;
pub(crate) const DISP_CHANGE_BADDUALVIEW: i32 = -6;
pub(crate) const WS_POPUP: u32 = 0x8000_0000;
pub(crate) const WS_EX_LAYERED: u32 = 0x0008_0000;
pub(crate) const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub(crate) const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub(crate) const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
pub(crate) const LWA_ALPHA: u32 = 0x2;
pub(crate) const SM_XVIRTUALSCREEN: i32 = 76;
pub(crate) const SM_YVIRTUALSCREEN: i32 = 77;
pub(crate) const SM_CXVIRTUALSCREEN: i32 = 78;
pub(crate) const SM_CYVIRTUALSCREEN: i32 = 79;
pub(crate) const BLACK_BRUSH: i32 = 4;
pub(crate) const SWP_NOACTIVATE: u32 = 0x0010;
pub(crate) const SWP_SHOWWINDOW: u32 = 0x0040;
pub(crate) const HWND_TOPMOST: isize = -1;
pub(crate) const PM_REMOVE: u32 = 0x1;
pub(crate) const MCCS_BRIGHTNESS: u8 = 0x10;
pub(crate) const QDC_ONLY_ACTIVE_PATHS: u32 = 2;
pub(crate) const DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME: i32 = 1;
pub(crate) const DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO: i32 = 9;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pointl {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DevmodeW {
    pub dm_device_name: [u16; 32],
    pub dm_spec_version: u16,
    pub dm_driver_version: u16,
    pub dm_size: u16,
    pub dm_driver_extra: u16,
    pub dm_fields: u32,
    pub dm_position: Pointl,
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

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Msg {
    pub hwnd: isize,
    pub message: u32,
    pub w_param: usize,
    pub l_param: isize,
    pub time: u32,
    pub pt: Pointl,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ramp {
    pub red: [u16; 256],
    pub green: [u16; 256],
    pub blue: [u16; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct WndClassExW {
    pub cb_size: u32,
    pub style: u32,
    pub lpfn_wnd_proc: usize,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: usize,
    pub h_icon: usize,
    pub h_cursor: usize,
    pub h_background: usize,
    pub lpsz_menu_name: *const u16,
    pub lpsz_class_name: *const u16,
    pub h_icon_sm: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct MonitorInfoExW {
    pub cb_size: u32,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
    pub dw_flags: u32,
    pub sz_device: [u16; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct PhysicalMonitor {
    pub handle: usize,
    pub description: [u16; 128],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Luid {
    pub low_part: u32,
    pub high_part: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigPathSourceInfo {
    pub adapter_id: Luid,
    pub id: u32,
    pub mode_info_idx: u32,
    pub status_flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigRational {
    pub numerator: u32,
    pub denominator: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigPathTargetInfo {
    pub adapter_id: Luid,
    pub id: u32,
    pub mode_info_idx: u32,
    pub output_technology: u32,
    pub rotation: u32,
    pub scaling: u32,
    pub refresh_rate: DisplayConfigRational,
    pub scan_line_ordering: u32,
    pub target_available: i32,
    pub status_flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigPathInfo {
    pub source_info: DisplayConfigPathSourceInfo,
    pub target_info: DisplayConfigPathTargetInfo,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigDeviceInfoHeader {
    pub device_info_type: i32,
    pub size: u32,
    pub adapter_id: Luid,
    pub id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigGetAdvancedColorInfo {
    pub header: DisplayConfigDeviceInfoHeader,
    /// Bitfield: bit 0 `advancedColorSupported`, bit 1 `advancedColorEnabled`,
    /// bit 2 `wideColorEnforced`, bit 3 `advancedColorForceDisabled`.
    pub value: u32,
    pub color_encoding: u32,
    pub bits_per_color_channel: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigSourceDeviceName {
    pub header: DisplayConfigDeviceInfoHeader,
    pub view_gdi_device_name: [u16; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DisplayConfigModeInfo {
    pub info_type: i32,
    pub id: u32,
    pub adapter_id: Luid,
    pub mode: [u8; 48],
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
        lp_dev_mode: *mut DevmodeW,
    ) -> i32;
    pub(crate) fn ChangeDisplaySettingsExW(
        lpsz_device_name: *const u16,
        lp_dev_mode: *const DevmodeW,
        hwnd: usize,
        dw_flags: u32,
        l_param: *const (),
    ) -> i32;
    pub(crate) fn RegisterClassExW(lp_wnd_class: *const WndClassExW) -> u16;
    pub(crate) fn CreateWindowExW(
        dw_ex_style: u32,
        lp_class_name: *const u16,
        lp_window_name: *const u16,
        dw_style: u32,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: usize,
        h_menu: usize,
        h_instance: usize,
        lp_param: *const (),
    ) -> usize;
    pub(crate) fn DestroyWindow(h_wnd: usize) -> i32;
    pub(crate) fn DefWindowProcW(h_wnd: usize, msg: u32, w_param: usize, l_param: isize) -> isize;
    pub(crate) fn SetLayeredWindowAttributes(
        h_wnd: usize,
        cr_key: u32,
        b_alpha: u8,
        dw_flags: u32,
    ) -> i32;
    pub(crate) fn GetSystemMetrics(n_index: i32) -> i32;
    pub(crate) fn SetWindowPos(
        h_wnd: usize,
        h_wnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        u_flags: u32,
    ) -> i32;
    pub(crate) fn PeekMessageW(
        lp_msg: *mut Msg,
        h_wnd: usize,
        w_msg_filter_min: u32,
        w_msg_filter_max: u32,
        w_remove_msg: u32,
    ) -> i32;
    pub(crate) fn TranslateMessage(lp_msg: *const Msg) -> i32;
    pub(crate) fn DispatchMessageW(lp_msg: *const Msg) -> isize;
    pub(crate) fn SendMessageW(
        h_wnd: usize,
        msg: u32,
        w_param: usize,
        l_param: isize,
    ) -> isize;
    pub(crate) fn EnumDisplayMonitors(
        h_dc: usize,
        lprc_clip: *const (),
        lpfn_enum: Option<
            unsafe extern "system" fn(
                h_monitor: usize,
                h_dc: usize,
                lprc_clip: *mut Rect,
                l_param: isize,
            ) -> i32,
        >,
        dw_data: isize,
    ) -> i32;
    pub(crate) fn GetMonitorInfoW(h_monitor: usize, lpmi: *mut MonitorInfoExW) -> i32;
    pub(crate) fn GetDisplayConfigBufferSizes(
        flags: u32,
        num_path_array_elements: *mut u32,
        num_mode_info_array_elements: *mut u32,
    ) -> i32;
    pub(crate) fn QueryDisplayConfig(
        flags: u32,
        num_path_array_elements: *mut u32,
        path_array: *mut DisplayConfigPathInfo,
        num_mode_info_array_elements: *mut u32,
        mode_info_array: *mut DisplayConfigModeInfo,
        current_topology_id: *mut u32,
    ) -> i32;
    pub(crate) fn DisplayConfigGetDeviceInfo(
        device_info: *mut DisplayConfigDeviceInfoHeader,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(crate) fn GetModuleHandleW(lp_module_name: *const u16) -> usize;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub(crate) fn GetStockObject(i: i32) -> usize;
    pub(crate) fn CreateDCW(
        pwsz_driver: *const u16,
        pwsz_device: *const u16,
        pszdta: *const u16,
        pdvta: *const (),
    ) -> usize;
    pub(crate) fn DeleteDC(h_dc: usize) -> i32;
    pub(crate) fn GetDeviceGammaRamp(h_dc: usize, lp_ramp: *mut u16) -> i32;
    pub(crate) fn SetDeviceGammaRamp(h_dc: usize, lp_ramp: *mut u16) -> i32;
}

#[link(name = "dxva2")]
unsafe extern "system" {
    pub(crate) fn GetPhysicalMonitorsFromHMONITOR(
        h_monitor: usize,
        pdw_number_of_physical_monitors: *mut u32,
        p_physical_monitor_array: *mut PhysicalMonitor,
    ) -> i32;
    pub(crate) fn GetVCPFeatureAndVCPFeatureReply(
        h_monitor: usize,
        b_vcp_code: u8,
        p_vct: *mut u32,
        pdw_current_value: *mut u32,
        pdw_maximum_value: *mut u32,
    ) -> i32;
    pub(crate) fn SetVCPFeature(h_monitor: usize, b_vcp_code: u8, dw_new_value: u32) -> i32;
    pub(crate) fn GetMonitorBrightness(
        h_monitor: usize,
        pdw_minimum_brightness: *mut u32,
        pdw_current_brightness: *mut u32,
        pdw_maximum_brightness: *mut u32,
    ) -> i32;
    pub(crate) fn SetMonitorBrightness(h_monitor: usize, dw_new_brightness: u32) -> i32;
    pub(crate) fn DestroyPhysicalMonitors(
        dw_number_of_physical_monitors: u32,
        p_physical_monitor_array: *mut PhysicalMonitor,
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
    fn attach_and_power_constants() {
        assert_eq!(ENUM_REGISTRY_SETTINGS, 0xFFFF_FFFE);
        assert_eq!(DISPLAY_DEVICE_MIRRORING_DRIVER, 0x8);
        assert_eq!(DISPLAY_DEVICE_DISCONNECT, 0x0200_0000);
        assert_eq!(WM_SYSCOMMAND, 0x0112);
        assert_eq!(SC_MONITORPOWER, 0xF170);
        assert_eq!(HWND_BROADCAST, 0xFFFF);
    }

    #[test]
    fn display_change_constants() {
        assert_eq!(CDS_UPDATEREGISTRY, 0x1);
        assert_eq!(CDS_TEST, 0x2);
        assert_eq!(DM_PELSWIDTH, 0x0008_0000);
        assert_eq!(DM_PELSHEIGHT, 0x0010_0000);
        assert_eq!(DM_DISPLAYFREQUENCY, 0x0040_0000);
        assert_eq!(DM_DISPLAYORIENTATION, 0x0000_0080);
        assert_eq!(DM_POSITION, 0x0000_0020);
        assert_eq!(DISP_CHANGE_SUCCESSFUL, 0);
        assert_eq!(DISP_CHANGE_RESTART, 1);
        assert_eq!(DISP_CHANGE_FAILED, -1);
        assert_eq!(DISP_CHANGE_BADMODE, -2);
        assert_eq!(DISP_CHANGE_NOTUPDATED, -3);
        assert_eq!(DISP_CHANGE_BADFLAGS, -4);
        assert_eq!(DISP_CHANGE_BADPARAM, -5);
        assert_eq!(DISP_CHANGE_BADDUALVIEW, -6);
    }

    #[test]
    fn registry_constants() {
        // HKEY handles carry the pseudo-handle in the low 32 bits; on 64-bit
        // the SDK constant is sign-extended (0xFFFFFFFF80000002).
        assert_eq!(HKEY_LOCAL_MACHINE as u32, 0x8000_0002);
        assert_eq!(REG_BINARY, 3);
        assert_eq!(ERROR_SUCCESS, 0);
    }

    #[test]
    fn devmode_layout_is_220_bytes() {
        assert_eq!(std::mem::size_of::<DevmodeW>(), 220);
    }

    #[test]
    fn display_device_layout_is_840_bytes() {
        assert_eq!(std::mem::size_of::<DISPLAY_DEVICEW>(), 840);
    }

    #[test]
    fn devmode_field_offsets() {
        assert_eq!(offset_of!(DevmodeW, dm_position), 76);
        assert_eq!(offset_of!(DevmodeW, dm_pels_width), 172);
        assert_eq!(offset_of!(DevmodeW, dm_pels_height), 176);
        assert_eq!(offset_of!(DevmodeW, dm_display_frequency), 184);
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
    fn fade_window_constants() {
        assert_eq!(WS_POPUP, 0x8000_0000);
        assert_eq!(WS_EX_LAYERED, 0x0008_0000);
        assert_eq!(WS_EX_TOPMOST, 0x0000_0008);
        assert_eq!(WS_EX_TOOLWINDOW, 0x0000_0080);
        assert_eq!(WS_EX_NOACTIVATE, 0x0800_0000);
        assert_eq!(LWA_ALPHA, 0x2);
        assert_eq!(SM_XVIRTUALSCREEN, 76);
        assert_eq!(SM_YVIRTUALSCREEN, 77);
        assert_eq!(SM_CXVIRTUALSCREEN, 78);
        assert_eq!(SM_CYVIRTUALSCREEN, 79);
        assert_eq!(BLACK_BRUSH, 4);
        assert_eq!(SWP_NOACTIVATE, 0x0010);
        assert_eq!(SWP_SHOWWINDOW, 0x0040);
        assert_eq!(HWND_TOPMOST, -1);
        assert_eq!(PM_REMOVE, 0x1);
    }

    #[test]
    fn msg_layout_is_48_bytes_on_x64() {
        assert_eq!(std::mem::size_of::<Msg>(), 48);
    }

    #[test]
    fn wnd_class_layout_is_80_bytes_on_x64() {
        assert_eq!(std::mem::size_of::<WndClassExW>(), 80);
    }

    #[test]
    fn gamma_ramp_layout_is_1536_bytes() {
        assert_eq!(std::mem::size_of::<Ramp>(), 1536);
    }

    #[test]
    fn wnd_class_field_offsets() {
        assert_eq!(offset_of!(WndClassExW, cb_size), 0);
        assert_eq!(offset_of!(WndClassExW, lpfn_wnd_proc), 8);
        assert_eq!(offset_of!(WndClassExW, h_background), 48);
        assert_eq!(offset_of!(WndClassExW, lpsz_class_name), 64);
        assert_eq!(offset_of!(WndClassExW, h_icon_sm), 72);
    }

    #[test]
    fn brightness_constants() {
        assert_eq!(MCCS_BRIGHTNESS, 0x10);
    }

    #[test]
    fn rect_layout_is_16_bytes() {
        assert_eq!(std::mem::size_of::<Rect>(), 16);
    }

    #[test]
    fn monitor_info_ex_layout_is_104_bytes() {
        assert_eq!(std::mem::size_of::<MonitorInfoExW>(), 104);
    }

    #[test]
    fn monitor_info_ex_field_offsets() {
        assert_eq!(offset_of!(MonitorInfoExW, cb_size), 0);
        assert_eq!(offset_of!(MonitorInfoExW, rc_monitor), 4);
        assert_eq!(offset_of!(MonitorInfoExW, rc_work), 20);
        assert_eq!(offset_of!(MonitorInfoExW, dw_flags), 36);
        assert_eq!(offset_of!(MonitorInfoExW, sz_device), 40);
    }

    #[test]
    fn physical_monitor_layout_is_264_bytes_on_x64() {
        assert_eq!(std::mem::size_of::<PhysicalMonitor>(), 264);
    }

    #[test]
    fn physical_monitor_field_offsets() {
        assert_eq!(offset_of!(PhysicalMonitor, handle), 0);
        assert_eq!(offset_of!(PhysicalMonitor, description), 8);
    }

    #[test]
    fn display_config_constants() {
        assert_eq!(QDC_ONLY_ACTIVE_PATHS, 2);
        assert_eq!(DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, 1);
        assert_eq!(DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, 9);
    }

    #[test]
    fn display_config_layouts_match_win32_sdk() {
        assert_eq!(std::mem::size_of::<Luid>(), 8);
        assert_eq!(std::mem::size_of::<DisplayConfigPathSourceInfo>(), 20);
        assert_eq!(std::mem::size_of::<DisplayConfigPathTargetInfo>(), 48);
        assert_eq!(std::mem::size_of::<DisplayConfigPathInfo>(), 72);
        assert_eq!(std::mem::size_of::<DisplayConfigDeviceInfoHeader>(), 20);
        assert_eq!(std::mem::size_of::<DisplayConfigGetAdvancedColorInfo>(), 32);
        assert_eq!(std::mem::size_of::<DisplayConfigModeInfo>(), 64);
    }

    #[test]
    fn source_device_name_layout_matches_win32_sdk() {
        // DISPLAYCONFIG_SOURCE_DEVICE_NAME: header (20) + viewGdiDeviceName[32]
        // WCHARs (64), CCHDEVICENAME = 32.
        assert_eq!(std::mem::size_of::<DisplayConfigSourceDeviceName>(), 84);
        assert_eq!(offset_of!(DisplayConfigSourceDeviceName, view_gdi_device_name), 20);
    }

    #[test]
    fn display_config_field_offsets() {
        assert_eq!(offset_of!(DisplayConfigPathSourceInfo, id), 8);
        assert_eq!(offset_of!(DisplayConfigPathTargetInfo, id), 8);
        assert_eq!(offset_of!(DisplayConfigPathTargetInfo, output_technology), 16);
        assert_eq!(offset_of!(DisplayConfigPathTargetInfo, refresh_rate), 28);
        assert_eq!(offset_of!(DisplayConfigPathTargetInfo, target_available), 40);
        assert_eq!(offset_of!(DisplayConfigPathInfo, target_info), 20);
        assert_eq!(offset_of!(DisplayConfigPathInfo, flags), 68);
        assert_eq!(offset_of!(DisplayConfigDeviceInfoHeader, adapter_id), 8);
        assert_eq!(offset_of!(DisplayConfigDeviceInfoHeader, id), 16);
        assert_eq!(offset_of!(DisplayConfigGetAdvancedColorInfo, value), 20);
        assert_eq!(offset_of!(DisplayConfigGetAdvancedColorInfo, color_encoding), 24);
        assert_eq!(offset_of!(DisplayConfigGetAdvancedColorInfo, bits_per_color_channel), 28);
    }

    #[test]
    fn display_config_externs_are_resolvable() {
        let _: unsafe extern "system" fn(u32, *mut u32, *mut u32) -> i32 =
            GetDisplayConfigBufferSizes;
        let _: unsafe extern "system" fn(
            u32,
            *mut u32,
            *mut DisplayConfigPathInfo,
            *mut u32,
            *mut DisplayConfigModeInfo,
            *mut u32,
        ) -> i32 = QueryDisplayConfig;
        let _: unsafe extern "system" fn(*mut DisplayConfigDeviceInfoHeader) -> i32 =
            DisplayConfigGetDeviceInfo;
    }

    #[test]
    fn brightness_externs_are_resolvable() {
        let _: unsafe extern "system" fn(usize, *const (), Option<unsafe extern "system" fn(usize, usize, *mut Rect, isize) -> i32>, isize) -> i32 = EnumDisplayMonitors;
        let _: unsafe extern "system" fn(usize, *mut MonitorInfoExW) -> i32 = GetMonitorInfoW;
        let _: unsafe extern "system" fn(usize, *mut u32, *mut PhysicalMonitor) -> i32 = GetPhysicalMonitorsFromHMONITOR;
        let _: unsafe extern "system" fn(usize, u8, *mut u32, *mut u32, *mut u32) -> i32 = GetVCPFeatureAndVCPFeatureReply;
        let _: unsafe extern "system" fn(usize, u8, u32) -> i32 = SetVCPFeature;
        let _: unsafe extern "system" fn(usize, *mut u32, *mut u32, *mut u32) -> i32 = GetMonitorBrightness;
        let _: unsafe extern "system" fn(usize, u32) -> i32 = SetMonitorBrightness;
        let _: unsafe extern "system" fn(u32, *mut PhysicalMonitor) -> i32 = DestroyPhysicalMonitors;
        let _: unsafe extern "system" fn(*const u16, *const u16, *const u16, *const ()) -> usize = CreateDCW;
        let _: unsafe extern "system" fn(usize) -> i32 = DeleteDC;
        let _: unsafe extern "system" fn(usize, *mut u16) -> i32 = GetDeviceGammaRamp;
        let _: unsafe extern "system" fn(usize, *mut u16) -> i32 = SetDeviceGammaRamp;
    }
}
