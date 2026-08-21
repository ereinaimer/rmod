//! Backlight control with an auto-detect backend chain.
//!
//! [`set_brightness`] tries, in order, DDC/CI VCP control, the native
//! brightness-slider API (dxva2, falling back to the WMI
//! `WmiMonitorBrightnessMethods` provider the action-center slider uses),
//! and a gamma-ramp fallback. All share the 0-100 value domain, so every
//! display can be set to the same level regardless of which backend ends up
//! carrying the change.

pub(crate) mod ddc;
pub(crate) mod gamma;
pub(crate) mod probe;
pub(crate) mod slider;

// Re-export internal utilities needed by contrast module.
pub(crate) use ddc::physical_monitors;
pub(crate) use probe::{DDC_BUDGET, timed};

use super::{query, wmi};
use ddc::{set_via_ddc, set_via_ddc_floor};
use gamma::set_via_gamma;
use slider::{set_via_slider, set_via_slider_floor};

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

/// Classification of a brightness value for mode vs percent handling.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ModeKind {
    /// A plain percent value (0-100).
    Plain,
    /// The hardware floor with a gamma dim layer.
    Min,
    /// Hardware 100 with the identity gamma ramp.
    Max,
    /// Hardware 100 with an overdriven gamma ramp.
    Boost,
}

/// The requested brightness change: a numeric level or a composite mode.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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

impl BrightnessValue {
    /// Returns the [`ModeKind`] classification of this brightness value.
    pub fn mode_kind(&self) -> ModeKind {
        match self {
            BrightnessValue::Percent(_) => ModeKind::Plain,
            BrightnessValue::Min => ModeKind::Min,
            BrightnessValue::Max => ModeKind::Max,
            BrightnessValue::Boost => ModeKind::Boost,
        }
    }
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
/// `temp` is an optional color temperature in Kelvin for mode+temp composition.
///
/// # Errors
/// Unknown monitor, a forced backend the display does not support, a mode
/// with a forced backend, or no brightness-control path at all.
pub fn set_brightness(
    monitor: Option<u32>,
    value: BrightnessValue,
    via: Option<BrightnessBackend>,
    temp: Option<u32>,
) -> Result<BrightnessOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    match value {
        BrightnessValue::Percent(level) => set_percent(name, level, via, &display, temp),
        mode => {
            if via.is_some() {
                return Err(mode_backend_error(mode));
            }
            set_mode(name, mode, &display, temp)
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
    temp: Option<u32>,
) -> Result<BrightnessOutcome, String> {
    let outcome = |backend: BrightnessBackend, unchanged: bool| BrightnessOutcome {
        display: display.to_string(),
        kind: BrightnessValue::Percent(level),
        unchanged,
        layers: vec![layer_for(backend, level)],
        clipped: false,
    };
    match via {
        Some(backend) => match set_via(backend, name, level, display, temp) {
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
                match set_via(backend, name, level, display, temp) {
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
pub(crate) struct HardwareChange {
    pub(crate) backend: BrightnessBackend,
    pub(crate) level: u32,
    pub(crate) unchanged: bool,
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
fn set_mode(name: &str, mode: BrightnessValue, display: &str, temp: Option<u32>) -> Result<BrightnessOutcome, String> {
    let mut layers = Vec::with_capacity(2);
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
    let gamma_unchanged = match set_via_gamma(name, level, display, true, temp)? {
        Some(unchanged) => unchanged,
        None => unreachable!(
            "gamma control always reports Some; set_via_gamma only returns None for unsupported backends"
        ),
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
    temp: Option<u32>,
) -> Result<Option<bool>, String> {
    match backend {
        BrightnessBackend::Ddc => set_via_ddc(name, value),
        BrightnessBackend::Slider => set_via_slider(name, value),
        BrightnessBackend::Gamma => set_via_gamma(name, value, display, false, temp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_kind_classification() {
        assert_eq!(BrightnessValue::Percent(0).mode_kind(), ModeKind::Plain);
        assert_eq!(BrightnessValue::Percent(50).mode_kind(), ModeKind::Plain);
        assert_eq!(BrightnessValue::Percent(100).mode_kind(), ModeKind::Plain);
        assert_eq!(BrightnessValue::Min.mode_kind(), ModeKind::Min);
        assert_eq!(BrightnessValue::Max.mode_kind(), ModeKind::Max);
        assert_eq!(BrightnessValue::Boost.mode_kind(), ModeKind::Boost);
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
    fn backend_names() {
        assert_eq!(BrightnessBackend::Ddc.name(), "ddc");
        assert_eq!(BrightnessBackend::Slider.name(), "slider");
        assert_eq!(BrightnessBackend::Gamma.name(), "gamma");
    }

    #[test]
    #[cfg(feature = "fake")]
    fn set_mode_with_temp_passes_temp_through() {
        use crate::sys::windows::fake::brightness::set_brightness;
        use crate::sys::windows::{BrightnessLayer, BrightnessValue};

        // Call set_brightness with a mode and temp - should not error
        let outcome = set_brightness(Some(1), BrightnessValue::Min, None, Some(3400)).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Min);
        // Verify layers are present (hardware + gamma)
        assert_eq!(outcome.layers.len(), 2);
        assert!(matches!(outcome.layers[0], BrightnessLayer::Hardware { .. }));
        assert!(matches!(outcome.layers[1], BrightnessLayer::Gamma { level: 50 }));
    }

    #[test]
    #[cfg(feature = "fake")]
    fn set_percent_with_temp_passes_temp_through() {
        use crate::sys::windows::fake::brightness::set_brightness;
        use crate::sys::windows::{BrightnessLayer, BrightnessValue};

        // Call set_brightness with a percent value and temp - should not error
        let outcome = set_brightness(Some(2), BrightnessValue::Percent(50), None, Some(3400)).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Percent(50));
        // Verify gamma layer is present
        assert_eq!(outcome.layers.len(), 1);
        assert!(matches!(outcome.layers[0], BrightnessLayer::Gamma { level: 50 }));
    }
}
