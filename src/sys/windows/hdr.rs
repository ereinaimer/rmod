//! HDR capability and connector-type query: the Windows display-config
//! API first, the EDID CTA-861 static metadata as HDR fallback.
//!
//! [`match_path`] finds the active display-config path whose GDI source
//! device name equals the target display; [`hdr_from_path`] reads its
//! advanced-color capability via `DisplayConfigGetDeviceInfo` and falls
//! back to [`from_edid`] when the API fails (old Windows, headless
//! sessions). [`connector_for_path`] reports the same path's connector
//! type. Nothing here panics and every FFI failure degrades gracefully.

use super::bindings::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DisplayConfigDeviceInfoHeader, DisplayConfigGetAdvancedColorInfo, DisplayConfigGetDeviceInfo,
    DisplayConfigModeInfo, DisplayConfigPathInfo, DisplayConfigSourceDeviceName, ERROR_SUCCESS,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig, wide_to_string,
};
use super::edid;
use std::mem;
use std::ptr;

/// HDR capability of one display.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HdrInfo {
    /// The display supports HDR (per the OS, or per its EDID).
    pub supported: bool,
    /// HDR is currently enabled on the display.
    pub active: bool,
    /// Advertised HDR formats, e.g. `["HDR10"]`, `["HLG"]`,
    /// `["HDR10", "HLG"]`, or `["HDR"]` when only generic support is known.
    pub formats: Vec<&'static str>,
}

/// The label strings a display supports (HDR10/HLG from EDID), or a generic
/// `["HDR"]` when the panel advertises HDR but no specific format.
fn api_formats(edid_hdr: Option<&edid::HdrEdid>) -> Vec<&'static str> {
    let mut formats = Vec::new();
    if let Some(h) = edid_hdr {
        if h.hdr10 {
            formats.push("HDR10");
        }
        if h.hlg {
            formats.push("HLG");
        }
    }
    if formats.is_empty() {
        formats.push("HDR");
    }
    formats
}

/// Builds the display string for an HDR query result.
///
/// Format rules: `formats.join(" + ")` plus `" (active)"` or
/// `" (not active)"`; `supported == false` yields `"Not supported"`;
/// `None` (unknown) yields `"Unknown"`.
#[allow(dead_code)]
pub(crate) fn hdr_label(info: Option<&HdrInfo>) -> String {
    match info {
        None => "Unknown".to_string(),
        Some(info) if !info.supported => "Not supported".to_string(),
        Some(info) => format!(
            "{} ({})",
            info.formats.join(" + "),
            if info.active { "active" } else { "not active" }
        ),
    }
}

/// Fallback decision from EDID static metadata alone: any HDR10/HLG flag
/// yields a supported-but-inactive result; no flags (or no EDID data) means
/// unknown (`None`).
pub(crate) fn from_edid(edid_hdr: Option<&edid::HdrEdid>) -> Option<HdrInfo> {
    let mut formats = Vec::new();
    if let Some(h) = edid_hdr {
        if h.hdr10 {
            formats.push("HDR10");
        }
        if h.hlg {
            formats.push("HLG");
        }
    }
    if formats.is_empty() {
        None
    } else {
        Some(HdrInfo {
            supported: true,
            active: false,
            formats,
        })
    }
}

/// The display-config API query: enumerates active paths, matches the one
/// whose GDI source device name equals `device_name` (reported by the OS
/// itself via `DisplayConfigGetDeviceInfo`, so no adapter-LUID
/// assumptions). `None` on every FFI failure.
///
/// The matched path also carries the connector type (`output_technology`),
/// so HDR and the `Connector:` value share a single enumeration.
pub(crate) fn match_path(device_name: &str) -> Option<DisplayConfigPathInfo> {
    let mut num_paths: u32 = 0;
    let mut num_modes: u32 = 0;
    let rc = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes)
    };
    if rc != ERROR_SUCCESS || num_paths == 0 {
        return None;
    }
    let mut paths: Vec<DisplayConfigPathInfo> = vec![unsafe { mem::zeroed() }; num_paths as usize];
    let mut modes: Vec<DisplayConfigModeInfo> = vec![unsafe { mem::zeroed() }; num_modes as usize];
    let rc = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            ptr::null_mut(),
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    paths.truncate(num_paths as usize);

    for path in paths.iter() {
        let mut name: DisplayConfigSourceDeviceName = unsafe { mem::zeroed() };
        name.header = DisplayConfigDeviceInfoHeader {
            device_info_type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: mem::size_of::<DisplayConfigSourceDeviceName>() as u32,
            adapter_id: path.source_info.adapter_id,
            id: path.source_info.id,
        };
        let rc = unsafe { DisplayConfigGetDeviceInfo(&mut name.header) };
        if rc != ERROR_SUCCESS {
            continue;
        }
        let matches = source_name_from_wide(&name.view_gdi_device_name)
            .is_some_and(|n| n.eq_ignore_ascii_case(device_name));
        if matches {
            return Some(*path);
        }
    }
    None
}

/// Reads the advanced-color capability of the display addressed by
/// `path`'s target device. `Some` whenever the OS answers (including
/// "not supported"); `None` on FFI failure.
pub(crate) fn advanced_color(
    path: &DisplayConfigPathInfo,
    edid_hdr: Option<&edid::HdrEdid>,
) -> Option<HdrInfo> {
    let mut info: DisplayConfigGetAdvancedColorInfo = unsafe { mem::zeroed() };
    info.header = DisplayConfigDeviceInfoHeader {
        device_info_type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
        size: mem::size_of::<DisplayConfigGetAdvancedColorInfo>() as u32,
        adapter_id: path.target_info.adapter_id,
        id: path.target_info.id,
    };
    let rc = unsafe { DisplayConfigGetDeviceInfo(&mut info.header) };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let (supported, active) = decode_advanced_color(info.value);
    let formats = if supported {
        api_formats(edid_hdr)
    } else {
        Vec::new()
    };
    Some(HdrInfo {
        supported,
        active,
        formats,
    })
}

/// HDR capability of the path found by [`match_path`], falling back to the
/// EDID static metadata when the API did not answer.
pub(crate) fn hdr_from_path(
    path: Option<&DisplayConfigPathInfo>,
    edid_hdr: Option<&edid::HdrEdid>,
) -> Option<HdrInfo> {
    path.and_then(|p| advanced_color(p, edid_hdr))
        .or_else(|| from_edid(edid_hdr))
}

/// Maps a `DISPLAYCONFIG_OUTPUT_TECHNOLOGY_*` value to a short connector
/// label; unmapped values (including reserved ones) yield `"Unknown"`.
pub(crate) fn connector_label(technology: u32) -> &'static str {
    match technology {
        0 => "VGA",
        1 => "S-Video",
        2 => "Composite",
        3 => "Component",
        4 => "DVI",
        5 => "HDMI",
        6 => "LVDS",
        8 => "D-Terminal",
        9 => "SDI",
        10 => "DisplayPort",
        11 => "eDP",
        12 => "UDI",
        13 => "UDI (embedded)",
        14 => "SDTV dongle",
        15 => "Miracast",
        16 => "DisplayPort (wireless)",
        17 => "USB",
        0x8000_0000 => "Internal",
        0xFFFF_FFFF => "Other",
        _ => "Unknown",
    }
}

/// The connector label of the display addressed by `path`.
pub(crate) fn connector_for_path(path: &DisplayConfigPathInfo) -> &'static str {
    connector_label(path.target_info.output_technology)
}

/// Queries the connector type of the display with Win32 device name
/// `device_name` (e.g. `\\.\DISPLAY1`); `None` when no active path
/// matches (headless sessions, old Windows).
pub(crate) fn query_connector(device_name: &str) -> Option<&'static str> {
    match_path(device_name).map(|p| connector_for_path(&p))
}

/// Extracts the GDI source device name from a `viewGdiDeviceName` buffer,
/// trimming at the first NUL. `None` when the buffer holds no name.
fn source_name_from_wide(w: &[u16]) -> Option<String> {
    let s = wide_to_string(w);
    if s.is_empty() { None } else { Some(s) }
}

/// Extracts `(advancedColorSupported, advancedColorEnabled)` from the
/// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO` bitfield value.
fn decode_advanced_color(value: u32) -> (bool, bool) {
    (value & 1 != 0, value & 2 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_name_exact_length_without_nul_is_valid() {
        // The API fills all 32 WCHARs without a terminator when the name
        // is exactly CCHDEVICENAME long; still a valid source name.
        let w: [u16; 32] = std::array::from_fn(|i| b'A' as u16 + i as u16);
        assert_eq!(
            source_name_from_wide(&w).as_deref(),
            Some("ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`")
        );
    }

    #[test]
    fn source_name_short_name_trims_at_nul() {
        let w = [
            0x0044, 0x0049, 0x0053, 0x0050, 0x004C, 0x0041, 0x0059, 0x0031, 0, 0, 0,
        ];
        assert_eq!(source_name_from_wide(&w).as_deref(), Some("DISPLAY1"));
    }

    #[test]
    fn source_name_empty_slice_is_none() {
        assert_eq!(source_name_from_wide(&[]), None);
    }

    #[test]
    fn source_name_nul_first_is_none() {
        assert_eq!(source_name_from_wide(&[0, 0, 0, 0]), None);
    }

    #[test]
    fn decode_advanced_color_value_bits() {
        assert_eq!(decode_advanced_color(0), (false, false));
        assert_eq!(decode_advanced_color(1), (true, false));
        assert_eq!(decode_advanced_color(2), (false, true));
        assert_eq!(decode_advanced_color(3), (true, true));
        assert_eq!(decode_advanced_color(4), (false, false));
    }

    fn edid(hdr10: bool, hlg: bool) -> edid::HdrEdid {
        edid::HdrEdid { hdr10, hlg }
    }

    #[test]
    fn from_edid_hdr10_only() {
        let info = from_edid(Some(&edid(true, false))).unwrap();
        assert!(info.supported);
        assert!(!info.active);
        assert_eq!(info.formats, vec!["HDR10"]);
    }

    #[test]
    fn from_edid_hlg_only() {
        let info = from_edid(Some(&edid(false, true))).unwrap();
        assert!(info.supported);
        assert!(!info.active);
        assert_eq!(info.formats, vec!["HLG"]);
    }

    #[test]
    fn from_edid_both_formats_in_order() {
        let info = from_edid(Some(&edid(true, true))).unwrap();
        assert!(info.supported);
        assert!(!info.active);
        assert_eq!(info.formats, vec!["HDR10", "HLG"]);
    }

    #[test]
    fn from_edid_none_without_flags_or_data() {
        assert_eq!(from_edid(Some(&edid(false, false))), None);
        assert_eq!(from_edid(None), None);
    }

    #[test]
    fn hdr_label_unknown_for_none() {
        assert_eq!(hdr_label(None), "Unknown");
    }

    #[test]
    fn hdr_label_not_supported() {
        let info = HdrInfo {
            supported: false,
            active: false,
            formats: Vec::new(),
        };
        assert_eq!(hdr_label(Some(&info)), "Not supported");
    }

    #[test]
    fn hdr_label_hdr10_active() {
        let info = HdrInfo {
            supported: true,
            active: true,
            formats: vec!["HDR10"],
        };
        assert_eq!(hdr_label(Some(&info)), "HDR10 (active)");
    }

    #[test]
    fn hdr_label_hdr10_not_active() {
        let info = HdrInfo {
            supported: true,
            active: false,
            formats: vec!["HDR10"],
        };
        assert_eq!(hdr_label(Some(&info)), "HDR10 (not active)");
    }

    #[test]
    fn hdr_label_hlg_not_active() {
        let info = HdrInfo {
            supported: true,
            active: false,
            formats: vec!["HLG"],
        };
        assert_eq!(hdr_label(Some(&info)), "HLG (not active)");
    }

    #[test]
    fn hdr_label_both_not_active() {
        let info = HdrInfo {
            supported: true,
            active: false,
            formats: vec!["HDR10", "HLG"],
        };
        assert_eq!(hdr_label(Some(&info)), "HDR10 + HLG (not active)");
    }

    #[test]
    fn hdr_label_generic_hdr_active() {
        let info = HdrInfo {
            supported: true,
            active: true,
            formats: vec!["HDR"],
        };
        assert_eq!(hdr_label(Some(&info)), "HDR (active)");
    }

    #[test]
    fn hdr_label_generic_hdr_not_active() {
        let info = HdrInfo {
            supported: true,
            active: false,
            formats: vec!["HDR"],
        };
        assert_eq!(hdr_label(Some(&info)), "HDR (not active)");
    }

    #[test]
    fn connector_label_maps_known_technologies() {
        assert_eq!(connector_label(0), "VGA");
        assert_eq!(connector_label(1), "S-Video");
        assert_eq!(connector_label(2), "Composite");
        assert_eq!(connector_label(3), "Component");
        assert_eq!(connector_label(4), "DVI");
        assert_eq!(connector_label(5), "HDMI");
        assert_eq!(connector_label(6), "LVDS");
        assert_eq!(connector_label(8), "D-Terminal");
        assert_eq!(connector_label(9), "SDI");
        assert_eq!(connector_label(10), "DisplayPort");
        assert_eq!(connector_label(11), "eDP");
        assert_eq!(connector_label(12), "UDI");
        assert_eq!(connector_label(13), "UDI (embedded)");
        assert_eq!(connector_label(14), "SDTV dongle");
        assert_eq!(connector_label(15), "Miracast");
        assert_eq!(connector_label(16), "DisplayPort (wireless)");
        assert_eq!(connector_label(17), "USB");
        assert_eq!(connector_label(0x8000_0000), "Internal");
        assert_eq!(connector_label(0xFFFF_FFFF), "Other");
    }

    #[test]
    fn connector_label_unknown_for_unmapped_values() {
        assert_eq!(connector_label(9999), "Unknown");
        assert_eq!(connector_label(7), "Unknown");
    }

    #[test]
    fn connector_for_path_reads_output_technology() {
        let mut path: DisplayConfigPathInfo = unsafe { mem::zeroed() };
        path.target_info.output_technology = 5;
        assert_eq!(connector_for_path(&path), "HDMI");
    }
}
