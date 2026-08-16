//! Backlight control with an auto-detect backend chain.
//!
//! [`set_brightness`] tries, in order, DDC/CI VCP control, the native
//! brightness-slider API (dxva2, falling back to the WMI
//! `WmiMonitorBrightnessMethods` provider the action-center slider uses),
//! and a gamma-ramp fallback. All share the 0-100 value domain, so every
//! display can be set to the same level regardless of which backend ends up
//! carrying the change.

use super::bindings::{
    CreateDCW, DeleteDC, DestroyPhysicalMonitors, EnumDisplayMonitors,
    GetDeviceGammaRamp, GetMonitorBrightness, GetMonitorInfoW,
    GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply,
    MCCS_BRIGHTNESS, MonitorInfoExW, PhysicalMonitor, Rect, SetDeviceGammaRamp,
    SetMonitorBrightness, SetVCPFeature, encode_wide, wide_to_string,
};
use super::{query, wmi};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Budget for DDC/CI and dxva2 probe operations.
const DDC_BUDGET: Duration = Duration::from_millis(500);

/// Runs `f` on a spawned thread with a time budget.
/// Returns the closure's result if it completes within `budget`,
/// otherwise returns `Ok(None)` (treated as "backend unsupported").
/// A panicking closure also yields `Ok(None)`.
fn timed<T, F>(budget: Duration, f: F) -> Result<Option<T>, String>
where
    F: FnOnce() -> Result<Option<T>, String> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(budget) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

/// The brightness-control backend used for a change.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BrightnessBackend {
    /// Hardware backlight register (VCP code 0x10) via DDC/CI.
    Ddc,
    /// The Windows brightness-slider API (dxva2, falling back to WMI).
    Slider,
    /// Software gamma ramp; works on every display but is not OS-persisted.
    Gamma,
}

impl BrightnessBackend {
    /// The lowercase name used by `--via` and in output.
    pub fn name(&self) -> &'static str {
        match self {
            BrightnessBackend::Ddc => "ddc",
            BrightnessBackend::Slider => "slider",
            BrightnessBackend::Gamma => "gamma",
        }
    }
}

/// The requested brightness change: a numeric level or a composite mode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)]
pub enum BrightnessValue {
    /// A numeric level, 0-100.
    Percent(u32),
    /// The hardware floor with a gamma dim layer.
    Min,
    /// Hardware 100 with the identity gamma ramp.
    Max,
    /// Hardware 100 with an overdriven gamma ramp.
    Boost,
}

/// One write of a brightness change: a hardware backlight write or a gamma
/// ramp write. A [`BrightnessOutcome`] always carries at least one layer.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BrightnessLayer {
    /// A hardware backlight write through `backend`.
    Hardware {
        backend: BrightnessBackend,
        level: u32,
    },
    /// A gamma ramp write representing `level` percent.
    Gamma { level: u32 },
}

/// The outcome of a brightness change against one display.
pub struct BrightnessOutcome {
    /// Display label, e.g. `Generic PnP Monitor [:1]`.
    pub display: String,
    /// The requested brightness value.
    pub kind: BrightnessValue,
    /// True when the display was already at `kind`.
    pub unchanged: bool,
    /// The writes that carried the change, hardware first when present.
    pub layers: Vec<BrightnessLayer>,
    /// True when the change overdrives the ramp past full scale.
    #[allow(dead_code)]
    pub clipped: bool,
}

/// Sets a display's brightness, auto-detecting the backend chain
/// `ddc -> slider -> gamma`, or forcing the backend in `via`.
///
/// `value` is a 0-100 [`BrightnessValue::Percent`] or one of the composite
/// modes [`BrightnessValue::Min`], [`BrightnessValue::Max`], and
/// [`BrightnessValue::Boost`], which compose a hardware write with a gamma
/// ramp. Modes reject a forced backend.
///
/// `monitor` is the 1-based number from `rmod list`; `None` selects the
/// primary display.
///
/// # Errors
/// Unknown monitor, a forced backend the display does not support, a mode
/// with a forced backend, or no brightness-control path at all.
pub fn set_brightness(
    monitor: Option<u32>,
    value: BrightnessValue,
    via: Option<BrightnessBackend>,
) -> Result<BrightnessOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    match value {
        BrightnessValue::Percent(level) => set_percent(name, level, via, &display),
        mode => {
            if via.is_some() {
                return Err(mode_backend_error(mode));
            }
            set_mode(name, mode, &display)
        }
    }
}

/// The defensive error for a mode passed with a forced backend.
fn mode_backend_error(mode: BrightnessValue) -> String {
    format!(
        "{} does not take a backend. use a number to choose a backend",
        mode_word(mode)
    )
}

/// The lowercase CLI word of a mode, used in errors and output.
pub(crate) fn mode_word(mode: BrightnessValue) -> &'static str {
    match mode {
        BrightnessValue::Min => "min",
        BrightnessValue::Max => "max",
        BrightnessValue::Boost => "boost",
        BrightnessValue::Percent(_) => unreachable!("percent is not a mode"),
    }
}

/// The gamma layer level of a mode.
pub(crate) fn gamma_level_for(mode: BrightnessValue) -> u32 {
    match mode {
        BrightnessValue::Min => 50,
        BrightnessValue::Max => 100,
        BrightnessValue::Boost => 130,
        BrightnessValue::Percent(_) => unreachable!("percent is not a mode"),
    }
}

/// The percent path: the legacy chain with a single layer in the outcome.
fn set_percent(
    name: &str,
    level: u32,
    via: Option<BrightnessBackend>,
    display: &str,
) -> Result<BrightnessOutcome, String> {
    let outcome = |backend: BrightnessBackend, unchanged: bool| BrightnessOutcome {
        display: display.to_string(),
        kind: BrightnessValue::Percent(level),
        unchanged,
        layers: vec![layer_for(backend, level)],
        clipped: false,
    };
    match via {
        Some(backend) => match set_via(backend, name, level, display) {
            Ok(Some(unchanged)) => Ok(outcome(backend, unchanged)),
            Ok(None) => Err(format!(
                "{display} does not support {} brightness control",
                backend.name()
            )),
            Err(e) => Err(e),
        },
        None => {
            for backend in [
                BrightnessBackend::Ddc,
                BrightnessBackend::Slider,
                BrightnessBackend::Gamma,
            ] {
                match set_via(backend, name, level, display) {
                    Ok(Some(unchanged)) => return Ok(outcome(backend, unchanged)),
                    Ok(None) => continue,
                    Err(e) => return Err(e),
                }
            }
            Err(format!("{display} has no brightness control available"))
        }
    }
}

/// The single layer of a percent change carried by `backend`.
fn layer_for(backend: BrightnessBackend, level: u32) -> BrightnessLayer {
    match backend {
        BrightnessBackend::Gamma => BrightnessLayer::Gamma { level },
        backend => BrightnessLayer::Hardware { backend, level },
    }
}

/// A hardware backlight write: the backend, the level written, and whether
/// the display was already at that level.
struct HardwareChange {
    backend: BrightnessBackend,
    level: u32,
    unchanged: bool,
}

impl HardwareChange {
    fn layer(&self) -> BrightnessLayer {
        BrightnessLayer::Hardware {
            backend: self.backend,
            level: self.level,
        }
    }
}

/// The mode path: the best available hardware write (when any applies) plus
/// the mode's gamma write.
fn set_mode(
    name: &str,
    mode: BrightnessValue,
    display: &str,
) -> Result<BrightnessOutcome, String> {
    let mut layers = Vec::new();
    let mut hardware_unchanged = true;
    match mode {
        BrightnessValue::Min => {
            if let Some(change) = set_via_ddc_floor(name)? {
                layers.push(change.layer());
                hardware_unchanged = change.unchanged;
            } else if let Some(change) = set_via_slider_floor(name)? {
                layers.push(change.layer());
                hardware_unchanged = change.unchanged;
            } else if let Some(change) = set_via_wmi_floor(name)? {
                layers.push(change.layer());
                hardware_unchanged = change.unchanged;
            }
        }
        BrightnessValue::Max | BrightnessValue::Boost => {
            if let Some(unchanged) = set_via_ddc(name, 100)? {
                layers.push(BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Ddc,
                    level: 100,
                });
                hardware_unchanged = unchanged;
            } else if let Some(unchanged) = set_via_slider(name, 100)? {
                layers.push(BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 100,
                });
                hardware_unchanged = unchanged;
            }
        }
        BrightnessValue::Percent(_) => unreachable!("set_mode only runs for modes"),
    }
    let level = gamma_level_for(mode);
    let gamma_unchanged = match set_via_gamma(name, level, display)? {
        Some(unchanged) => unchanged,
        None => unreachable!("gamma control always reports Some; set_via_gamma only returns None for unsupported backends"),
    };
    layers.push(BrightnessLayer::Gamma { level });
    Ok(BrightnessOutcome {
        display: display.to_string(),
        kind: mode,
        unchanged: hardware_unchanged && gamma_unchanged,
        layers,
        clipped: matches!(mode, BrightnessValue::Boost),
    })
}

/// The DDC floor leg for [`BrightnessValue::Min`]: the VCP register at 1.
/// `Ok(None)` when the display exposes no DDC/CI control.
fn set_via_ddc_floor(name: &str) -> Result<Option<HardwareChange>, String> {
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

/// The dxva2 floor leg for [`BrightnessValue::Min`]: the reported minimum.
/// Skipped when the minimum is 0 (off) or unreadable; `Ok(None)` when the
/// display exposes no dxva2 brightness control.
fn set_via_slider_floor(name: &str) -> Result<Option<HardwareChange>, String> {
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

/// The WMI floor leg for [`BrightnessValue::Min`]: the smallest positive
/// `Level` entry. Skipped when the value is unreadable; `Ok(None)` when
/// the display has no WMI brightness instance.
fn set_via_wmi_floor(name: &str) -> Result<Option<HardwareChange>, String> {
    let Some(floor) = wmi::min_level(name) else {
        return Ok(None);
    };
    match wmi::set(name, floor)? {
        Some(unchanged) => Ok(Some(HardwareChange {
            backend: BrightnessBackend::Slider,
            level: floor,
            unchanged,
        })),
        None => Ok(None),
    }
}

/// Applies the change through one backend. `Ok(None)` means the backend is
/// unsupported on this display; `Ok(Some(unchanged))` means it applied (or
/// was already at `value`).
fn set_via(
    backend: BrightnessBackend,
    name: &str,
    value: u32,
    display: &str,
) -> Result<Option<bool>, String> {
    match backend {
        BrightnessBackend::Ddc => set_via_ddc(name, value),
        BrightnessBackend::Slider => set_via_slider(name, value),
        BrightnessBackend::Gamma => set_via_gamma(name, value, display),
    }
}

/// Physical monitor handles for a device name, released on drop.
struct PhysicalMonitors {
    handles: Vec<PhysicalMonitor>,
}

impl Drop for PhysicalMonitors {
    fn drop(&mut self) {
        if !self.handles.is_empty() {
            unsafe { DestroyPhysicalMonitors(self.handles.len() as u32, self.handles.as_mut_ptr()) };
        }
    }
}

/// The physical monitors attached to the display whose device name matches
/// `name`, or `None` when the display exposes no physical monitor API.
fn physical_monitors(name: &str) -> Result<Option<PhysicalMonitors>, String> {
    struct Ctx {
        target: String,
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
        if wide_to_string(&info.sz_device) == ctx.target {
            ctx.found = true;
            ctx.handle = h_monitor;
            return 0;
        }
        1
    }
    let mut ctx = Ctx {
        target: name.to_string(),
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
    let _ = unsafe { GetPhysicalMonitorsFromHMONITOR(ctx.handle, &mut count, std::ptr::null_mut()) };
    if count == 0 {
        return Ok(None);
    }
    let mut handles = vec![PhysicalMonitor { handle: 0, description: [0; 128] }; count as usize];
    let ok = unsafe { GetPhysicalMonitorsFromHMONITOR(ctx.handle, &mut count, handles.as_mut_ptr()) };
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

/// Sets brightness through the DDC/CI VCP register; `Ok(None)` when the
/// display does not support DDC/CI.
fn set_via_ddc(name: &str, value: u32) -> Result<Option<bool>, String> {
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

/// Sets brightness through the native slider APIs; `Ok(None)` when the
/// display supports neither dxva2 nor the WMI brightness provider.
///
/// dxva2's physical-monitor path is unavailable on many laptop panels
/// (hybrid-GPU systems), so it falls back to the WMI
/// `WmiMonitorBrightnessMethods` provider — the same one the action-center
/// brightness slider drives, keeping the system UI in sync.
fn set_via_slider(name: &str, value: u32) -> Result<Option<bool>, String> {
    if let Some(unchanged) = set_via_slider_dxva2(name, value)? {
        return Ok(Some(unchanged));
    }
    wmi::set(name, value)
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

/// Sets brightness through a gamma ramp; this is the fallback that works on
/// every display.
fn set_via_gamma(name: &str, value: u32, display: &str) -> Result<Option<bool>, String> {
    let name_wide = encode_wide(name);
    let dc = unsafe {
        CreateDCW(std::ptr::null(), name_wide.as_ptr(), std::ptr::null(), std::ptr::null())
    };
    if dc == 0 {
        return Err(format!(
            "cannot open the display for gamma control: {name}"
        ));
    }
    let result = set_via_gamma_dc(dc, value, display);
    let _ = unsafe { DeleteDC(dc) };
    result
}

fn set_via_gamma_dc(dc: usize, value: u32, display: &str) -> Result<Option<bool>, String> {
    let mut ramp = [0u16; 768];
    let ok = unsafe { GetDeviceGammaRamp(dc, ramp.as_mut_ptr()) };
    if ok != 0 && gamma_matches(&ramp, value) {
        return Ok(Some(true));
    }
    let new_ramp = gamma_ramp(value);
    let set = unsafe { SetDeviceGammaRamp(dc, new_ramp.as_ptr() as *mut u16) };
    if set == 0 {
        return Err(gamma_error(value, display));
    }
    Ok(Some(false))
}

/// The error for a failed gamma write. Some display drivers reject ramps
/// dimmer than half scale, so values below 50 get an explanatory message.
fn gamma_error(value: u32, display: &str) -> String {
    if value < 50 {
        format!(
            "{display} brightness cannot go below 50% on this system (gamma ramp rejected by the display driver)"
        )
    } else {
        "the gamma brightness change failed".to_string()
    }
}

/// The gamma ramp for a 0-100 brightness: a linear scale of the identity
/// ramp applied to every channel. `100` yields the full range 0-65535;
/// values above 100 clamp at 65535 instead of wrapping.
pub(crate) fn gamma_ramp(value: u32) -> [u16; 768] {
    let mut ramp = [0u16; 768];
    for i in 0..256u32 {
        let v = (i * 257 * value / 100).min(65535) as u16;
        ramp[i as usize] = v;
        ramp[256 + i as usize] = v;
        ramp[512 + i as usize] = v;
    }
    ramp
}

/// True when the ramp already represents `value`.
fn gamma_matches(ramp: &[u16; 768], value: u32) -> bool {
    gamma_ramp(value) == *ramp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_ramp_full_brightness_is_identity() {
        let ramp = gamma_ramp(100);
        for (i, entry) in ramp.iter().enumerate() {
            let channel = i % 256;
            let expected = (channel as u32 * 257) as u16;
            assert_eq!(*entry, expected, "entry {i}");
        }
    }

    #[test]
    fn gamma_ramp_zero_is_black() {
        assert_eq!(gamma_ramp(0), [0u16; 768]);
    }

    #[test]
    fn gamma_ramp_fifty_is_half_scale() {
        let ramp = gamma_ramp(50);
        assert_eq!(ramp[255], 32767);
        assert_eq!(ramp[255 + 256], 32767);
        assert_eq!(ramp[255 + 512], 32767);
    }

    #[test]
    fn gamma_matches_roundtrips_constructed_ramp() {
        assert!(gamma_matches(&gamma_ramp(40), 40));
        assert!(!gamma_matches(&gamma_ramp(40), 41));
    }

    #[test]
    fn gamma_ramp_saturates_above_full_scale() {
        for channel in [0, 256, 512] {
            assert_eq!(
                gamma_ramp(130)[255 + channel],
                65535,
                "channel offset {channel}"
            );
        }
        assert_eq!(gamma_ramp(130)[100], (100u32 * 257 * 130 / 100) as u16);
    }

    #[test]
    fn gamma_ramp_up_to_full_scale_matches_the_legacy_formula() {
        for value in [0, 1, 40, 50, 99, 100] {
            let ramp = gamma_ramp(value);
            for (i, entry) in ramp.iter().enumerate() {
                let channel = i % 256;
                let expected = (channel as u32 * 257 * value / 100) as u16;
                assert_eq!(*entry, expected, "value {value}, entry {i}");
            }
        }
    }

    #[test]
    fn gamma_matches_the_boost_ramp() {
        assert!(gamma_matches(&gamma_ramp(130), 130));
        assert!(!gamma_matches(&gamma_ramp(130), 100));
        assert!(!gamma_matches(&gamma_ramp(100), 130));
    }

    #[test]
    fn mode_backend_error_names_the_mode() {
        for (mode, word) in [
            (BrightnessValue::Min, "min"),
            (BrightnessValue::Max, "max"),
            (BrightnessValue::Boost, "boost"),
        ] {
            assert_eq!(
                mode_backend_error(mode),
                format!("{word} does not take a backend. use a number to choose a backend"),
                "mode {word}"
            );
        }
    }

    #[test]
    fn gamma_error_below_fifty_explains_the_floor() {
        assert_eq!(
            gamma_error(0, "Generic PnP Monitor [:1]"),
            "Generic PnP Monitor [:1] brightness cannot go below 50% on this system (gamma ramp rejected by the display driver)"
        );
        assert_eq!(
            gamma_error(49, "Generic PnP Monitor [:1]"),
            "Generic PnP Monitor [:1] brightness cannot go below 50% on this system (gamma ramp rejected by the display driver)"
        );
    }

    #[test]
    fn gamma_error_at_fifty_is_the_generic_failure() {
        assert_eq!(
            gamma_error(50, "Generic PnP Monitor [:1]"),
            "the gamma brightness change failed"
        );
        assert_eq!(
            gamma_error(100, "Generic PnP Monitor [:1]"),
            "the gamma brightness change failed"
        );
    }

    #[test]
    fn backend_names() {
        assert_eq!(BrightnessBackend::Ddc.name(), "ddc");
        assert_eq!(BrightnessBackend::Slider.name(), "slider");
        assert_eq!(BrightnessBackend::Gamma.name(), "gamma");
    }

    #[test]
    fn timed_fast_closure_returns_result() {
        use std::time::Duration;
        let result = timed(Duration::from_millis(50), || Ok(Some(true)));
        assert_eq!(result, Ok(Some(true)));
    }

    #[test]
    fn timed_slow_closure_returns_none() {
        use std::time::Duration;
        let result = timed(Duration::from_millis(30), || {
            std::thread::sleep(Duration::from_millis(100));
            Ok(Some(false))
        });
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn timed_panicking_closure_returns_none() {
        use std::time::Duration;
        let result: Result<Option<bool>, String> = timed(Duration::from_millis(50), || {
            panic!("intentional panic");
        });
        assert_eq!(result, Ok(None));
    }
}
