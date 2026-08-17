//! Color temperature backend: gamma-ramp blue-light reduction.
//!
//! [`set_temp`] scales the per-channel gamma ramp of a display with
//! multipliers derived from a Kelvin value (Tanner Helland conversion),
//! preserving the current brightness dim level; [`reset_temp`] restores the
//! identity ramp, keeping the dim level; [`get_temp`] reads the current ramp
//! and reports the closest known preset as an approximation, also on dimmed
//! ramps.

use super::bindings::{
    CreateDCW, DeleteDC, GetDeviceGammaRamp, Ramp, SetDeviceGammaRamp, encode_wide,
};
use super::query;

/// Lowest accepted temperature in Kelvin; colder values are rejected.
pub const MIN_KELVIN: u32 = 1000;
/// Highest accepted temperature in Kelvin; hotter values are rejected.
pub const MAX_KELVIN: u32 = 6500;

/// The Kelvin values used to match a read ramp against when estimating.
pub const PRESET_KELVINS: &[u32] = &[1900, 2700, 3400, 4500, 6500];

/// The outcome of a temperature action for one display.
pub struct TempChange {
    /// Display label like `AOC 24G2 [:1]`.
    pub display: String,
    /// Requested or current Kelvin value.
    pub kelvin: u32,
}

/// Sets the color temperature of a display to `kelvin`, which must be in
/// [`MIN_KELVIN`]–[`MAX_KELVIN`], by scaling its gamma ramp.
///
/// `None` selects the primary display; `Some(n)` the 1-based monitor.
///
/// # Errors
/// Out-of-range temperature, unknown monitor, or a display that rejects
/// the gamma ramp change.
pub fn set_temp(monitor: Option<u32>, kelvin: u32) -> Result<TempChange, String> {
    if !(MIN_KELVIN..=MAX_KELVIN).contains(&kelvin) {
        return Err(format!(
            "invalid temperature {kelvin}. use a Kelvin value (1000-6500), a preset, or reset\ne.g. rmod temp 3400"
        ));
    }
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let (r, g, b) = kelvin_to_rgb(kelvin);
    let ratio = dim_ratio(&read_ramp(name)?);
    let ramp = scale(&build_ramp(r, g, b), ratio);
    apply_ramp(name, &ramp)?;
    Ok(TempChange {
        display: query::display_label(name, index as u32 + 1),
        kelvin,
    })
}

/// Restores the identity gamma ramp of a display (the `6500K` baseline),
/// using a pure identity ramp rather than a computed one to avoid drift,
/// scaled to keep the current brightness dim level.
///
/// `None` selects the primary display; `Some(n)` the 1-based monitor.
///
/// # Errors
/// Unknown monitor or a display that rejects the gamma ramp change.
pub fn reset_temp(monitor: Option<u32>) -> Result<TempChange, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let estimated = dim_ratio(&read_ramp(name)?);
    apply_ramp(name, &scale(&IDENTITY_RAMP, estimated))?;
    Ok(TempChange {
        display: query::display_label(name, index as u32 + 1),
        kelvin: MAX_KELVIN,
    })
}

/// Reports the current approximate color temperature of a display by
/// matching its read gamma ramp against the identity and preset ramps.
///
/// `None` selects the primary display; `Some(n)` the 1-based monitor.
///
/// # Errors
/// Unknown monitor or a display that rejects the gamma ramp read.
pub fn get_temp(monitor: Option<u32>) -> Result<TempChange, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let ramp = read_ramp(name)?;
    Ok(TempChange {
        display: query::display_label(name, index as u32 + 1),
        kelvin: estimate_kelvin(&ramp),
    })
}

/// Converts a Kelvin value to per-channel multipliers in `[0, 1]` using the
/// Tanner Helland approximation, clamped to [`MIN_KELVIN`]–[`MAX_KELVIN`].
fn kelvin_to_rgb(kelvin: u32) -> (f64, f64, f64) {
    let temp = (kelvin as f64).clamp(MIN_KELVIN as f64, MAX_KELVIN as f64) / 100.0;
    let red = if temp <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (temp - 60.0).powf(-0.133_204_759_2)
    }
    .clamp(0.0, 255.0);
    let green = if temp <= 66.0 {
        99.470_802_586_1 * temp.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (temp - 60.0).powf(-0.075_514_849_2)
    }
    .clamp(0.0, 255.0);
    let blue = if temp >= 66.0 {
        255.0
    } else if temp <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (temp - 10.0).ln() - 305.044_792_730_7
    }
    .clamp(0.0, 255.0);
    (red / 255.0, green / 255.0, blue / 255.0)
}

/// Some display drivers reject gamma ramps whose channels fall below half
/// brightness (verified empirically: `SetDeviceGammaRamp` returns false for
/// any channel multiplier under `0.5`). Flooring every channel here keeps
/// warm temperatures applicable on such displays; on capable displays the
/// floor only caps how far a channel can drop.
const CHANNEL_FLOOR: f64 = 0.5;

/// Builds a gamma ramp by scaling the linear ramp `i * 256` per channel,
/// capped at `65535`, with every channel kept at or above [`CHANNEL_FLOOR`].
fn build_ramp(red: f64, green: f64, blue: f64) -> Ramp {
    let (red, green, blue) = (
        red.max(CHANNEL_FLOOR),
        green.max(CHANNEL_FLOOR),
        blue.max(CHANNEL_FLOOR),
    );
    let mut ramp = Ramp {
        red: [0; 256],
        green: [0; 256],
        blue: [0; 256],
    };
    for i in 0..256 {
        let base = i as u32 * 256;
        ramp.red[i] = (base as f64 * red).min(65535.0) as u16;
        ramp.green[i] = (base as f64 * green).min(65535.0) as u16;
        ramp.blue[i] = (base as f64 * blue).min(65535.0) as u16;
    }
    ramp
}

/// The identity gamma ramp (all multipliers `1.0`), the `6500K` baseline.
static IDENTITY_RAMP: Ramp = {
    let mut ramp = Ramp {
        red: [0; 256],
        green: [0; 256],
        blue: [0; 256],
    };
    let mut i = 0;
    while i < 256 {
        let value = (i as u32 * 256) as u16;
        ramp.red[i] = value;
        ramp.green[i] = value;
        ramp.blue[i] = value;
        i += 1;
    }
    ramp
};

/// The brightness percent recovered from a gamma ramp: the red channel's
/// max entry scaled to 0-100. An approximation: a boosted ramp (value above
/// 100) saturates at 100, and integer math rounds down (a 50% ramp reads
/// 49). 0 for an all-zero ramp. The canonical definition lives alongside
/// `brightness::b_est`; the red channel carries only brightness because its
/// Kelvin multiplier is 1.0 for every valid temperature.
fn b_est(ramp: &Ramp) -> u32 {
    ramp.red[255] as u32 * 100 / 65535
}

/// The dim ratio of `ramp`: its [`b_est`] relative to the full-scale
/// reading, so a ramp at full scale maps to exactly `1.0` and a dimmed one
/// to its preserved level. An all-zero ramp (nothing to preserve) maps to
/// `1.0` as well.
fn dim_ratio(ramp: &Ramp) -> f64 {
    let estimated = b_est(ramp);
    if estimated == 0 {
        1.0
    } else {
        estimated as f64 / b_est(&IDENTITY_RAMP) as f64
    }
}

/// Scales every entry of `ramp` by `ratio` with round-to-nearest, clamped at
/// 65535. The ramp keeps its shape: a channel at half scale stays at half
/// scale, and a channel already at full scale stays pinned there.
fn scale(ramp: &Ramp, ratio: f64) -> Ramp {
    let mut out = Ramp {
        red: [0; 256],
        green: [0; 256],
        blue: [0; 256],
    };
    for i in 0..256 {
        out.red[i] = (ramp.red[i] as f64 * ratio).round().min(65535.0) as u16;
        out.green[i] = (ramp.green[i] as f64 * ratio).round().min(65535.0) as u16;
        out.blue[i] = (ramp.blue[i] as f64 * ratio).round().min(65535.0) as u16;
    }
    out
}

/// Applies a gamma ramp to the display named by `name` (a `\\.\DISPLAYN`
/// device name). The DC is created per call and released immediately.
///
/// # Errors
/// The display rejects the ramp when the DC cannot be created or
/// `SetDeviceGammaRamp` returns false.
fn apply_ramp(name: &str, ramp: &Ramp) -> Result<(), String> {
    let device = encode_wide(name);
    let dc = unsafe {
        CreateDCW(std::ptr::null(), device.as_ptr(), std::ptr::null(), std::ptr::null())
    };
    if dc == 0 {
        return Err(format!("{name} does not support gamma ramp adjustment"));
    }
let ok = unsafe { SetDeviceGammaRamp(dc, ramp.red.as_ptr() as *mut u16) };
    unsafe { DeleteDC(dc) };
    if ok == 0 {
        return Err(format!("{name} does not support gamma ramp adjustment"));
    }
    Ok(())
}

/// Reads the current gamma ramp of the display named by `name`.
///
/// # Errors
/// The display rejects the read when the DC cannot be created or
/// `GetDeviceGammaRamp` returns false.
fn read_ramp(name: &str) -> Result<Ramp, String> {
    let device = encode_wide(name);
    let dc = unsafe {
        CreateDCW(std::ptr::null(), device.as_ptr(), std::ptr::null(), std::ptr::null())
    };
    if dc == 0 {
        return Err(format!("{name} does not support gamma ramp adjustment"));
    }
    let mut ramp: Ramp = unsafe { std::mem::zeroed() };
let ok = unsafe { GetDeviceGammaRamp(dc, ramp.red.as_mut_ptr()) };
    unsafe { DeleteDC(dc) };
    if ok == 0 {
        return Err(format!("{name} does not support gamma ramp adjustment"));
    }
    Ok(ramp)
}

/// Sum of the absolute per-channel differences across both ramps.
fn ramp_diff(a: &Ramp, b: &Ramp) -> u64 {
    let mut diff = 0u64;
    for i in 0..256 {
        diff += (a.red[i] as i64 - b.red[i] as i64).unsigned_abs();
        diff += (a.green[i] as i64 - b.green[i] as i64).unsigned_abs();
        diff += (a.blue[i] as i64 - b.blue[i] as i64).unsigned_abs();
    }
    diff
}

/// Reports the preset Kelvin closest to the read ramp; the identity ramp
/// maps to [`MAX_KELVIN`]. A dimmed ramp (brightness below full scale) is
/// compared against the candidates scaled to the same dim level, so the
/// estimate survives a brightness change; an all-zero ramp compares
/// unscaled.
fn estimate_kelvin(ramp: &Ramp) -> u32 {
    let ratio = dim_ratio(ramp);
    let mut best = MAX_KELVIN;
    let mut best_diff = ramp_diff(ramp, &scale(&IDENTITY_RAMP, ratio));
    for k in PRESET_KELVINS {
        let (r, g, b) = kelvin_to_rgb(*k);
        let diff = ramp_diff(ramp, &scale(&build_ramp(r, g, b), ratio));
        if diff < best_diff {
            best_diff = diff;
            best = *k;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_ramp() -> &'static Ramp {
        &IDENTITY_RAMP
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn kelvin_to_rgb_1900_is_warm() {
        let (r, g, b) = kelvin_to_rgb(1900);
        assert!(approx(r, 1.0));
        assert!(approx(g, 0.517));
        assert!(approx(b, 0.0));
    }

    #[test]
    fn kelvin_to_rgb_6500_is_daylight() {
        let (r, g, b) = kelvin_to_rgb(6500);
        assert!(approx(r, 1.0));
        assert!(approx(g, 0.997));
        assert!(approx(b, 0.981));
    }

    #[test]
    fn kelvin_to_rgb_clamps_below_min() {
        assert_eq!(kelvin_to_rgb(500), kelvin_to_rgb(1000));
    }

    #[test]
    fn kelvin_to_rgb_clamps_above_max() {
        assert_eq!(kelvin_to_rgb(9000), kelvin_to_rgb(6500));
    }

    #[test]
    fn identity_ramp_is_linear() {
        let ramp = identity_ramp();
        for i in 0..256 {
            assert_eq!(ramp.red[i], i as u16 * 256);
            assert_eq!(ramp.green[i], i as u16 * 256);
            assert_eq!(ramp.blue[i], i as u16 * 256);
        }
    }

    #[test]
    fn build_ramp_entries_are_monotonic() {
        let ramp = build_ramp(1.0, 0.5, 0.25);
        for channel in [&ramp.red, &ramp.green, &ramp.blue] {
            assert!(channel.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn build_ramp_floors_channels_at_half_brightness() {
        let ramp = build_ramp(1.0, 0.5, 0.0);
        assert_eq!(ramp.red[255], 255 * 256);
        assert_eq!(ramp.green[255], 255 * 256 / 2);
        assert_eq!(ramp.blue[255], 255 * 256 / 2);
    }

    #[test]
    fn every_channel_meets_the_driver_floor() {
        for k in PRESET_KELVINS {
            let (r, g, b) = kelvin_to_rgb(*k);
            let ramp = build_ramp(r, g, b);
            for (label, channel) in
                [("red", &ramp.red), ("green", &ramp.green), ("blue", &ramp.blue)]
            {
                assert!(channel[255] >= 255 * 256 / 2, "kelvin {k} {label}");
            }
        }
    }

    #[test]
    fn estimate_identity_ramp_is_6500() {
        assert_eq!(estimate_kelvin(identity_ramp()), 6500);
    }

    #[test]
    fn estimate_known_preset_ramps_match() {
        for k in PRESET_KELVINS {
            let (r, g, b) = kelvin_to_rgb(*k);
            assert_eq!(estimate_kelvin(&build_ramp(r, g, b)), *k, "kelvin {k}");
        }
    }

    #[test]
    fn b_est_reads_brightness_from_a_ramp() {
        assert_eq!(b_est(identity_ramp()), 99);
        assert_eq!(b_est(&scale(identity_ramp(), 0.5)), 49);
        assert_eq!(b_est(&build_ramp(1.0, 0.5, 0.5)), 99);
        let black = Ramp {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
        };
        assert_eq!(b_est(&black), 0);
    }

    #[test]
    fn dim_ratio_is_one_at_full_scale_and_zero_for_black() {
        assert_eq!(dim_ratio(identity_ramp()), 1.0);
        let black = Ramp {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
        };
        assert_eq!(dim_ratio(&black), 1.0);
        let half = scale(identity_ramp(), 0.5);
        assert!((dim_ratio(&half) - 0.494_949).abs() < 0.0001);
    }

    #[test]
    fn scale_preserves_a_temp_ramps_shape() {
        let ramp = build_ramp(1.0, 0.5, 0.5);
        let scaled = scale(&ramp, 0.5);
        assert_eq!(scaled.red[255], 32640);
        assert_eq!(scaled.green[255], 16320);
        assert_eq!(scaled.blue[255], 16320);
        assert_eq!(b_est(&scaled), 49);
        let full = scale(&ramp, 1.0);
        assert_eq!(full.red[255], 65280);
        assert_eq!(full.green[255], 32640);
    }

    #[test]
    fn scale_is_stable_across_repeated_application() {
        let dimmed = scale(&build_ramp(1.0, 0.5, 0.5), 0.5);
        let again = scale(&build_ramp(1.0, 0.5, 0.5), dim_ratio(&dimmed));
        assert_eq!(b_est(&again), b_est(&dimmed));
        assert_eq!(
            scale(&build_ramp(1.0, 0.5, 0.5), dim_ratio(&again)),
            again
        );
    }

    #[test]
    fn estimate_a_dimmed_preset_ramp_matches() {
        for k in PRESET_KELVINS {
            let (r, g, b) = kelvin_to_rgb(*k);
            let dimmed = scale(&build_ramp(r, g, b), 0.5);
            assert_eq!(estimate_kelvin(&dimmed), *k, "kelvin {k}");
        }
    }

    #[test]
    fn estimate_a_dimmed_identity_ramp_is_6500() {
        assert_eq!(estimate_kelvin(&scale(identity_ramp(), 0.5)), 6500);
    }
}