use super::super::bindings::{
    DestroyPhysicalMonitors, EnumDisplayMonitors, GetMonitorInfoW, GetPhysicalMonitorsFromHMONITOR,
    GetVCPFeatureAndVCPFeatureReply, MCCS_BRIGHTNESS, MonitorInfoExW, PhysicalMonitor, Rect,
    SetVCPFeature, encode_wide,
};
use super::probe::{DDC_BUDGET, timed};
use super::{BrightnessBackend, HardwareChange};

/// Physical monitor handles for a device name, released on drop.
pub(crate) struct PhysicalMonitors {
    pub(crate) handles: Vec<PhysicalMonitor>,
}

impl Drop for PhysicalMonitors {
    fn drop(&mut self) {
        if !self.handles.is_empty() {
            unsafe {
                DestroyPhysicalMonitors(self.handles.len() as u32, self.handles.as_mut_ptr())
            };
        }
    }
}

/// True when the NUL-terminated wide strings are equal (each read up to
/// its first NUL).
fn wide_eq(a: &[u16; 32], b: &[u16; 32]) -> bool {
    let a_end = a.iter().position(|&c| c == 0).unwrap_or(32);
    let b_end = b.iter().position(|&c| c == 0).unwrap_or(32);
    a_end == b_end && a[..a_end] == b[..b_end]
}

/// The physical monitors attached to the display whose device name matches
/// `name`, or `None` when the display exposes no physical monitor API.
pub(crate) fn physical_monitors(name: &str) -> Result<Option<PhysicalMonitors>, String> {
    struct Ctx {
        /// The NUL-padded target device name; a name longer than 31 chars
        /// would be truncated, which cannot happen for real device names.
        target: [u16; 32],
        found: bool,
        handle: usize,
    }
    unsafe extern "system" fn find_monitor(
        h_monitor: usize,
        _h_dc: usize,
        _lprc_clip: *mut Rect,
        l_param: isize,
    ) -> i32 {
        let ctx = unsafe { &mut *(l_param as *mut Ctx) };
        if ctx.found {
            return 0;
        }
        let mut info: MonitorInfoExW = unsafe { std::mem::zeroed() };
        info.cb_size = std::mem::size_of::<MonitorInfoExW>() as u32;
        if unsafe { GetMonitorInfoW(h_monitor, &mut info) } == 0 {
            return 1;
        }
        if wide_eq(&info.sz_device, &ctx.target) {
            ctx.found = true;
            ctx.handle = h_monitor;
            return 0;
        }
        1
    }
    let encoded = encode_wide(name);
    let mut target = [0u16; 32];
    let copy_len = encoded.len().min(target.len());
    target[..copy_len].copy_from_slice(&encoded[..copy_len]);
    let mut ctx = Ctx {
        target,
        found: false,
        handle: 0,
    };
    unsafe {
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            Some(find_monitor),
            &mut ctx as *mut Ctx as isize,
        );
    }
    if !ctx.found {
        return Ok(None);
    }
    let mut count = 0u32;
    let _ =
        unsafe { GetPhysicalMonitorsFromHMONITOR(ctx.handle, &mut count, std::ptr::null_mut()) };
    if count == 0 {
        return Ok(None);
    }
    let mut handles = vec![
        PhysicalMonitor {
            handle: 0,
            description: [0; 128]
        };
        count as usize
    ];
    let ok =
        unsafe { GetPhysicalMonitorsFromHMONITOR(ctx.handle, &mut count, handles.as_mut_ptr()) };
    if ok == 0 {
        return Ok(None);
    }
    Ok(Some(PhysicalMonitors { handles }))
}

/// Reads the current VCP value of a physical monitor.
fn current_vcp(monitor: usize) -> Option<u32> {
    let mut code_type = 0u32;
    let mut current = 0u32;
    let mut maximum = 0u32;
    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(
            monitor,
            MCCS_BRIGHTNESS,
            &mut code_type,
            &mut current,
            &mut maximum,
        )
    };
    if ok == 0 { None } else { Some(current) }
}

/// The DDC floor leg for [`super::BrightnessValue::Min`]: the VCP register at 1.
/// `Ok(None)` when the display exposes no DDC/CI control.
pub(crate) fn set_via_ddc_floor(name: &str) -> Result<Option<HardwareChange>, String> {
    let name = name.to_string();
    timed(DDC_BUDGET, move || {
        let Some(monitors) = physical_monitors(&name)? else {
            return Ok(None);
        };
        let monitor = monitors.handles[0].handle;
        match current_vcp(monitor) {
            None => Ok(None),
            Some(1) => Ok(Some(HardwareChange {
                backend: BrightnessBackend::Ddc,
                level: 1,
                unchanged: true,
            })),
            Some(_) => {
                let ok = unsafe { SetVCPFeature(monitor, MCCS_BRIGHTNESS, 1) };
                if ok == 0 {
                    return Err("the DDC/CI brightness change failed".to_string());
                }
                Ok(Some(HardwareChange {
                    backend: BrightnessBackend::Ddc,
                    level: 1,
                    unchanged: false,
                }))
            }
        }
    })
}

/// Sets brightness through the DDC/CI VCP register; `Ok(None)` when the
/// display does not support DDC/CI.
pub(crate) fn set_via_ddc(name: &str, value: u32) -> Result<Option<bool>, String> {
    let name = name.to_string();
    timed(DDC_BUDGET, move || {
        let Some(monitors) = physical_monitors(&name)? else {
            return Ok(None);
        };
        let monitor = monitors.handles[0].handle;
        match current_vcp(monitor) {
            None => Ok(None),
            Some(current) if current == value => Ok(Some(true)),
            Some(_) => {
                let ok = unsafe { SetVCPFeature(monitor, MCCS_BRIGHTNESS, value) };
                if ok == 0 {
                    return Err("the DDC/CI brightness change failed".to_string());
                }
                Ok(Some(false))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(s: &str) -> [u16; 32] {
        let mut out = [0u16; 32];
        for (i, unit) in s.encode_utf16().take(32).enumerate() {
            out[i] = unit;
        }
        out
    }

    #[test]
    fn wide_eq_matches_equal_names() {
        let name = wide("\\\\.\\DISPLAY1");
        assert!(wide_eq(&name, &name));
    }

    #[test]
    fn wide_eq_rejects_differing_names() {
        assert!(!wide_eq(&wide("\\\\.\\DISPLAY1"), &wide("\\\\.\\DISPLAY2")));
    }

    #[test]
    fn wide_eq_trims_both_at_the_first_nul() {
        let mut with_junk = wide("AB");
        with_junk[3] = 1;
        assert!(wide_eq(&with_junk, &wide("AB")));
        assert!(wide_eq(&wide("AB"), &with_junk));
    }

    #[test]
    fn wide_eq_empty_matches_only_empty() {
        assert!(wide_eq(&[0u16; 32], &[0u16; 32]));
        assert!(!wide_eq(&[0u16; 32], &wide("\\\\.\\DISPLAY1")));
    }

    #[test]
    fn wide_eq_rejects_a_prefix_match() {
        assert!(!wide_eq(
            &wide("\\\\.\\DISPLAY1"),
            &wide("\\\\.\\DISPLAY10")
        ));
    }
}
