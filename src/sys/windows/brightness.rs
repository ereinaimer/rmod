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
    SetMonitorBrightness, SetVCPFeature, encode_wide,
};
use super::{query, wmi};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

/// Budget for DDC/CI and dxva2 probe operations.
const DDC_BUDGET: Duration = Duration::from_millis(500);

/// A boxed probe result: the closure's full `Result<Option<T>, String>`,
/// downcast back to `T` by the caller.
type ProbeResult = Box<dyn Any + Send>;

/// A probe job: the closure plus the per-call response channel, so results
/// are never cross-delivered between concurrent callers.
type ProbeJob = (Box<dyn FnOnce() -> ProbeResult + Send>, mpsc::Sender<ProbeResult>);

/// The shared probe worker: one thread running DDC/dxva2 probes
/// sequentially, started on first use. `busy` is the caller-side gate: a
/// claim precedes every queued job, so a hung or dead worker redirects
/// later calls to fresh threads instead of queueing behind the stuck job.
struct ProbeWorker {
    tx: mpsc::Sender<ProbeJob>,
    busy: Arc<AtomicBool>,
}

/// The lazily-started worker shared by every `timed` call.
static WORKER: OnceLock<ProbeWorker> = OnceLock::new();

/// The worker loop: runs each job, replies on its per-call channel, and
/// releases the busy claim. A hung closure leaves the claim held; a
/// panicking closure kills the thread with the claim still held, so all
/// later calls take the fresh-thread fallback.
fn worker_loop(rx: mpsc::Receiver<ProbeJob>, busy: Arc<AtomicBool>) {
    while let Ok((job, response_tx)) = rx.recv() {
        let result = job();
        let _ = response_tx.send(result);
        busy.store(false, Ordering::Release);
    }
}

/// Runs `f` with a time budget.
/// Returns the closure's result if it completes within `budget`,
/// otherwise returns `Ok(None)` (treated as "backend unsupported").
/// A panicking closure also yields `Ok(None)`.
///
/// An idle worker runs `f` on the shared probe thread; a busy worker (a
/// previous probe hung or panicked) spawns a fresh thread instead, so the
/// probe still runs concurrently with independent monitor handles.
fn timed<T, F>(budget: Duration, f: F) -> Result<Option<T>, String>
where
    F: FnOnce() -> Result<Option<T>, String> + Send + 'static,
    T: Send + 'static,
{
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        thread::spawn({
            let busy = Arc::clone(&busy);
            move || worker_loop(rx, busy)
        });
        ProbeWorker { tx, busy }
    });
    if worker
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return timed_on_fresh_thread(budget, f);
    }
    let (response_tx, response_rx) = mpsc::channel();
    let job: ProbeJob = (Box::new(move || -> ProbeResult { Box::new(f()) }), response_tx);
    if worker.tx.send(job).is_err() {
        worker.busy.store(false, Ordering::Release);
        return Ok(None);
    }
    match response_rx.recv_timeout(budget) {
        Ok(result) => match result.downcast::<Result<Option<T>, String>>() {
            Ok(inner) => *inner,
            Err(_) => Ok(None),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

/// Runs `f` on a spawned thread with a time budget: the fallback used
/// while the worker is busy or dead. Same semantics as the worker path —
/// a fresh thread with independent monitor handles.
fn timed_on_fresh_thread<T, F>(budget: Duration, f: F) -> Result<Option<T>, String>
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
    let gamma_unchanged = match set_via_gamma(name, level, display, true)? {
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
    let Some(session) = wmi::Session::for_display(name).ok().flatten() else {
        return Ok(None);
    };
    let Some(floor) = session.min_level() else {
        return Ok(None);
    };
    match session.set(floor)? {
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
        BrightnessBackend::Gamma => set_via_gamma(name, value, display, false),
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

/// True when the NUL-terminated wide strings are equal (each read up to
/// its first NUL).
fn wide_eq(a: &[u16; 32], b: &[u16; 32]) -> bool {
    let a_end = a.iter().position(|&c| c == 0).unwrap_or(32);
    let b_end = b.iter().position(|&c| c == 0).unwrap_or(32);
    a_end == b_end && a[..a_end] == b[..b_end]
}

/// The physical monitors attached to the display whose device name matches
/// `name`, or `None` when the display exposes no physical monitor API.
fn physical_monitors(name: &str) -> Result<Option<PhysicalMonitors>, String> {
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

/// Sets brightness through a gamma ramp; this is the fallback that works on
/// every display. `exact` selects the mode-leg semantics (an exact ramp
/// match and a pure [`gamma_ramp`] write) over the percent-leg semantics
/// (a [`b_est`] tolerance and a shape-preserving re-scale).
fn set_via_gamma(
    name: &str,
    value: u32,
    display: &str,
    exact: bool,
) -> Result<Option<bool>, String> {
    let name_wide = encode_wide(name);
    let dc = unsafe {
        CreateDCW(std::ptr::null(), name_wide.as_ptr(), std::ptr::null(), std::ptr::null())
    };
    if dc == 0 {
        return Err(format!(
            "cannot open the display for gamma control: {name}"
        ));
    }
    let result = set_via_gamma_dc(dc, value, display, exact);
    let _ = unsafe { DeleteDC(dc) };
    result
}

fn set_via_gamma_dc(
    dc: usize,
    value: u32,
    display: &str,
    exact: bool,
) -> Result<Option<bool>, String> {
    let mut ramp = [0u16; 768];
    let ok = unsafe { GetDeviceGammaRamp(dc, ramp.as_mut_ptr()) };
    let write = match (exact, ok != 0) {
        (true, true) if gamma_matches(&ramp, value) => return Ok(Some(true)),
        (true, _) => gamma_ramp(value),
        (false, true) => match percent_write(&ramp, value) {
            None => return Ok(Some(true)),
            Some(write) => write,
        },
        (false, false) => gamma_ramp(value),
    };
    let set = unsafe { SetDeviceGammaRamp(dc, write.as_ptr() as *mut u16) };
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
    ramp.iter().enumerate().all(|(i, &entry)| {
        let channel = i % 256;
        let expected = (channel as u32 * 257 * value / 100).min(65535) as u16;
        entry == expected
    })
}

/// The brightness percent recovered from a gamma ramp: the red channel's
/// max entry scaled to 0-100. An approximation: a boosted ramp (value above
/// 100) saturates at 100, and integer math rounds down (a 50% ramp reads
/// 49). 0 for an all-zero ramp.
pub(crate) fn b_est(ramp: &[u16; 768]) -> u32 {
    ramp[255] as u32 * 100 / 65535
}

/// True when the ramp already reads within 1 of `value` percent, the
/// rounding tolerance of [`b_est`].
fn percent_unchanged(ramp: &[u16; 768], value: u32) -> bool {
    b_est(ramp).abs_diff(value) <= 1
}

/// Scales `ramp` by `value / estimated` with round-to-nearest, clamped at
/// 65535. The ramp keeps its shape: a channel at half scale stays at half
/// scale, and a channel already at full scale stays pinned there.
fn scale_ramp(ramp: &[u16; 768], value: u32, estimated: u32) -> [u16; 768] {
    let mut out = [0u16; 768];
    let ratio = value as f64 / estimated as f64;
    for (i, entry) in ramp.iter().enumerate() {
        let scaled = (*entry as f64 * ratio).round() as u64;
        out[i] = scaled.min(65535) as u16;
    }
    out
}

/// The percent write decision: `None` when the ramp is already within the
/// [`percent_unchanged`] tolerance of `value`, else the ramp to write — the
/// current ramp re-scaled to `value` when it carries brightness, a fresh
/// [`gamma_ramp`] when it is all zero (nothing to preserve).
fn percent_write(ramp: &[u16; 768], value: u32) -> Option<[u16; 768]> {
    let estimated = b_est(ramp);
    if percent_unchanged(ramp, value) {
        return None;
    }
    if estimated > 0 {
        Some(scale_ramp(ramp, value, estimated))
    } else {
        Some(gamma_ramp(value))
    }
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
    fn gamma_matches_early_exits_on_first_mismatch() {
        let mut ramp = gamma_ramp(50);
        ramp[0] = 1;
        assert!(!gamma_matches(&ramp, 50));
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
    fn b_est_reads_the_identity_ramp_as_full_brightness() {
        assert_eq!(b_est(&gamma_ramp(100)), 100);
    }

    #[test]
    fn b_est_reads_a_fifty_percent_ramp_as_forty_nine() {
        assert_eq!(b_est(&gamma_ramp(50)), 49);
    }

    #[test]
    fn b_est_reads_a_black_ramp_as_zero() {
        assert_eq!(b_est(&[0u16; 768]), 0);
    }

    #[test]
    fn b_est_saturates_at_full_scale_for_a_boost_ramp() {
        assert_eq!(b_est(&gamma_ramp(130)), 100);
    }

    #[test]
    fn percent_unchanged_accepts_the_rounding_off_by_one() {
        assert!(percent_unchanged(&gamma_ramp(50), 50));
        assert!(percent_unchanged(&gamma_ramp(50), 49));
        assert!(percent_unchanged(&gamma_ramp(50), 48));
        assert!(percent_unchanged(&[0u16; 768], 0));
        assert!(percent_unchanged(&[0u16; 768], 1));
        assert!(!percent_unchanged(&gamma_ramp(50), 51));
        assert!(!percent_unchanged(&gamma_ramp(50), 40));
    }

    #[test]
    fn percent_write_returns_none_within_tolerance() {
        assert_eq!(percent_write(&gamma_ramp(50), 50), None);
        assert_eq!(percent_write(&gamma_ramp(50), 49), None);
        assert_eq!(percent_write(&[0u16; 768], 0), None);
    }

    #[test]
    fn percent_write_scales_a_ramp_that_carries_brightness() {
        let scaled = percent_write(&gamma_ramp(100), 50).unwrap();
        assert_eq!(scaled[255], 32768);
        assert_eq!(scaled[100], 12850);
        assert!(percent_unchanged(&scaled, 50));
    }

    #[test]
    fn percent_write_falls_back_to_a_pure_ramp_when_black() {
        assert_eq!(percent_write(&[0u16; 768], 2), Some(gamma_ramp(2)));
        assert_eq!(percent_write(&[0u16; 768], 100), Some(gamma_ramp(100)));
    }

    #[test]
    fn scale_ramp_preserves_a_temp_shaped_ramp() {
        let mut ramp = gamma_ramp(100);
        for i in 256..768 {
            ramp[i] = ramp[i] / 2;
        }
        let scaled = scale_ramp(&ramp, 50, b_est(&ramp));
        assert_eq!(scaled[255], 32768);
        assert_eq!(scaled[255 + 256], 16384);
        assert_eq!(scaled[255 + 512], 16384);
        assert!(percent_unchanged(&scaled, 50));
    }

    #[test]
    fn scale_ramp_clamps_entries_at_full_scale() {
        let scaled = scale_ramp(&gamma_ramp(50), 100, b_est(&gamma_ramp(50)));
        assert_eq!(scaled[255], 65535);
        assert_eq!(b_est(&scaled), 100);
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
        assert!(!wide_eq(&wide("\\\\.\\DISPLAY1"), &wide("\\\\.\\DISPLAY10")));
    }

    #[test]
    fn timed_busy_worker_falls_back_to_a_fresh_thread() {
        use std::time::Duration;
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let blocker = std::thread::spawn(move || {
            let result = timed(Duration::from_millis(2000), move || {
                let _ = started_tx.send(());
                let _ = release_rx.recv();
                Ok(Some(true))
            });
            assert_eq!(result, Ok(Some(true)));
        });
        started_rx.recv_timeout(Duration::from_millis(1000)).unwrap();
        let result = timed(Duration::from_millis(50), || Ok(Some(42)));
        assert_eq!(result, Ok(Some(42)));
        let _ = release_tx.send(());
        blocker.join().unwrap();
    }
}
