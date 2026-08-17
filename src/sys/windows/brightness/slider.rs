use super::super::bindings::{GetMonitorBrightness, SetMonitorBrightness};
use super::ddc::physical_monitors;
use super::probe::{DDC_BUDGET, timed};
use super::super::wmi;
use super::{BrightnessBackend, HardwareChange};

/// The dxva2 floor leg for [`BrightnessValue::Min`]: the reported minimum.
/// Skipped when the minimum is 0 (off) or unreadable; `Ok(None)` when the
/// display exposes no dxva2 brightness control.
pub(crate) fn set_via_slider_floor(name: &str) -> Result<Option<HardwareChange>, String> {
    let name = name.to_string();
    timed(DDC_BUDGET, move || {
        let Some(monitors) = physical_monitors(&name)? else {
            return Ok(None);
        };
        let monitor = monitors.handles[0].handle;
        let mut minimum = 0u32;
        let mut current = 0u32;
        let mut maximum = 0u32;
        let ok = unsafe { GetMonitorBrightness(monitor, &mut minimum, &mut current, &mut maximum) };
        if ok == 0 || minimum == 0 {
            return Ok(None);
        }
        if current == minimum {
            return Ok(Some(HardwareChange {
                backend: BrightnessBackend::Slider,
                level: minimum,
                unchanged: true,
            }));
        }
        let set = unsafe { SetMonitorBrightness(monitor, minimum) };
        if set == 0 {
            return Err("the brightness slider change failed".to_string());
        }
        Ok(Some(HardwareChange {
            backend: BrightnessBackend::Slider,
            level: minimum,
            unchanged: false,
        }))
    })
}

/// Sets brightness through the native slider APIs; `Ok(None)` when the
/// display supports neither dxva2 nor the WMI brightness provider.
///
/// dxva2's physical-monitor path is unavailable on many laptop panels
/// (hybrid-GPU systems), so it falls back to the WMI
/// `WmiMonitorBrightnessMethods` provider — the same one the action-center
/// brightness slider drives, keeping the system UI in sync.
pub(crate) fn set_via_slider(name: &str, value: u32) -> Result<Option<bool>, String> {
    if let Some(unchanged) = set_via_slider_dxva2(name, value)? {
        return Ok(Some(unchanged));
    }
    let Some(session) = wmi::Session::for_display(name)? else {
        return Ok(None);
    };
    session.set(value)
}

/// The dxva2 physical-monitor path; `Ok(None)` when the display exposes no
/// dxva2 brightness control.
fn set_via_slider_dxva2(name: &str, value: u32) -> Result<Option<bool>, String> {
    let name = name.to_string();
    timed(DDC_BUDGET, move || {
        let Some(monitors) = physical_monitors(&name)? else {
            return Ok(None);
        };
        let monitor = monitors.handles[0].handle;
        let mut minimum = 0u32;
        let mut current = 0u32;
        let mut maximum = 0u32;
        let ok = unsafe { GetMonitorBrightness(monitor, &mut minimum, &mut current, &mut maximum) };
        if ok == 0 {
            return Ok(None);
        }
        if current == value {
            return Ok(Some(true));
        }
        let set = unsafe { SetMonitorBrightness(monitor, value) };
        if set == 0 {
            return Err("the brightness slider change failed".to_string());
        }
        Ok(Some(false))
    })
}