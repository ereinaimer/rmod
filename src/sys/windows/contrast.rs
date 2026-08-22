//! Contrast control with an auto-detect backend chain.
//!
//! [`set_contrast`] tries DDC/CI VCP control (VCP code 0x12) first and
//! falls back to a gamma-ramp transform, which works on every display.
//! The value domain is 0-130 with 100 as neutral; values above 100
//! overdrive the gamma ramp and report `clipped`.

use super::bindings::{
    CreateDCW, DeleteDC, GetDeviceGammaRamp, GetVCPFeatureAndVCPFeatureReply, MCCS_CONTRAST,
    SetDeviceGammaRamp, SetVCPFeature, encode_wide,
};
use super::brightness::{DDC_BUDGET, physical_monitors, timed};
use super::brightness::gamma::ramp_eq;
use super::query;

/// The contrast-control backend used for a change.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ContrastBackend {
    /// Hardware VCP code 0x12 (MCCS Contrast, 0-100) via DDC/CI.
    Ddc,
    /// Software gamma ramp; works on every display but is not OS-persisted.
    Gamma,
}

impl ContrastBackend {
    /// The lowercase name used by `--via` and in output.
    pub fn name(&self) -> &'static str {
        match self {
            ContrastBackend::Ddc => "ddc",
            ContrastBackend::Gamma => "gamma",
        }
    }
}

/// The outcome of a contrast change against one display.
pub struct ContrastOutcome {
    /// Display label, e.g. `Generic PnP Monitor [:1]`.
    pub display: String,
    /// The requested contrast value, 0-130 with 100 as neutral.
    pub value: u32,
    /// True when the display was already at `value`.
    pub unchanged: bool,
    /// The backend that carried the change.
    pub backend: ContrastBackend,
    /// True when the change overdrives the ramp past full scale.
    pub clipped: bool,
}

/// Sets a display's contrast, auto-detecting the backend chain
/// `ddc -> gamma`, or forcing the backend in `via`.
///
/// `value` is 0-130 with 100 as neutral; values above 100 overdrive the
/// gamma ramp and report `clipped` (DDC accepts 0-100 only).
///
/// `monitor` is the 1-based number from `rmod list`; `None` selects the
/// primary display.
///
/// # Errors
/// Unknown monitor, a forced backend the display does not support, or a
/// rejected DDC/CI or gamma write.
pub fn set_contrast(
    monitor: Option<u32>,
    value: u32,
    via: Option<ContrastBackend>,
) -> Result<ContrastOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label_for(name, index as u32 + 1);
    let outcome = |backend: ContrastBackend, unchanged: bool| ContrastOutcome {
        display: display.clone(),
        value,
        unchanged,
        backend,
        clipped: value > 100,
    };
    match via {
        Some(ContrastBackend::Ddc) => {
            if value > 100 {
                return Err(format!("{display} contrast cannot go above 100 via ddc"));
            }
            match set_via_ddc(name, value) {
                Ok(Some(unchanged)) => Ok(outcome(ContrastBackend::Ddc, unchanged)),
                Ok(None) => Err(format!("{display} does not support ddc contrast control")),
                Err(e) => Err(e),
            }
        }
        Some(ContrastBackend::Gamma) => match set_via_gamma(name, value, &display) {
            Ok(Some(unchanged)) => Ok(outcome(ContrastBackend::Gamma, unchanged)),
            Ok(None) => unreachable!(
                "gamma control always reports Some; set_via_gamma only returns None for unsupported backends"
            ),
            Err(e) => Err(e),
        },
        None => {
            // For reset (value == 100), skip DDC and use gamma to ensure
            // the identity ramp is written (DDC VCP doesn't reflect gamma state).
            if value < 100 {
                match set_via_ddc(name, value) {
                    Ok(Some(unchanged)) => return Ok(outcome(ContrastBackend::Ddc, unchanged)),
                    Ok(None) => {}
                    Err(e) => return Err(e),
                }
            }
            match set_via_gamma(name, value, &display) {
                Ok(Some(unchanged)) => Ok(outcome(ContrastBackend::Gamma, unchanged)),
                Ok(None) => unreachable!(
                    "gamma control always reports Some; set_via_gamma only returns None for unsupported backends"
                ),
                Err(e) => Err(e),
            }
        }
    }
}

/// Resets contrast to defaults: DDC VCP 0x12 = 100 + gamma identity ramp.
/// Forces both backends regardless of current state.
pub fn reset_contrast(monitor: Option<u32>) -> Result<ContrastOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label_for(name, index as u32 + 1);

    // Force DDC to 100 if supported
    let ddc_ok = set_via_ddc(name, 100).is_ok();

    // Force gamma to identity
    let gamma_ok = reset_via_gamma(name, &display).is_ok();

    if !ddc_ok && !gamma_ok {
        return Err(format!("{display} has no contrast control available"));
    }

    let backend = if gamma_ok {
        ContrastBackend::Gamma
    } else {
        ContrastBackend::Ddc
    };

    Ok(ContrastOutcome {
        display,
        value: 100,
        unchanged: false,
        backend,
        clipped: false,
    })
}

/// Writes the identity gamma ramp (neutral contrast).
fn reset_via_gamma(name: &str, _display: &str) -> Result<(), String> {
    let name_wide = encode_wide(name);
    let dc = unsafe {
        CreateDCW(
            std::ptr::null(),
            name_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if dc == 0 {
        return Err(format!("cannot open the display for gamma control: {name}"));
    }
    let identity = contrast_ramp(100);
    let set = unsafe { SetDeviceGammaRamp(dc, identity.as_ptr() as *mut u16) };
    let _ = unsafe { DeleteDC(dc) };
    if set == 0 {
        return Err("the gamma contrast reset failed".to_string());
    }
    Ok(())
}

/// Sets contrast through the DDC/CI VCP register; `Ok(None)` when the
/// display does not support DDC/CI.
fn set_via_ddc(name: &str, value: u32) -> Result<Option<bool>, String> {
    let name = name.to_string();
    timed(DDC_BUDGET, move || {
        let Some(monitors) = physical_monitors(&name)? else {
            return Ok(None);
        };
        let monitor = monitors.handles[0].handle;
        match current_contrast(monitor) {
            None => Ok(None),
            Some(current) if current == value => Ok(Some(true)),
            Some(_) => {
                let ok = unsafe { SetVCPFeature(monitor, MCCS_CONTRAST, value) };
                if ok == 0 {
                    return Err("the DDC/CI contrast change failed".to_string());
                }
                Ok(Some(false))
            }
        }
    })
}

/// Reads the current VCP value of a physical monitor.
fn current_contrast(monitor: usize) -> Option<u32> {
    let mut code_type = 0u32;
    let mut current = 0u32;
    let mut maximum = 0u32;
    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(
            monitor,
            MCCS_CONTRAST,
            &mut code_type,
            &mut current,
            &mut maximum,
        )
    };
    if ok == 0 { None } else { Some(current) }
}

/// Sets contrast through a gamma ramp; this is the fallback that works on
/// every display.
fn set_via_gamma(name: &str, value: u32, _display: &str) -> Result<Option<bool>, String> {
    let name_wide = encode_wide(name);
    let dc = unsafe {
        CreateDCW(
            std::ptr::null(),
            name_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if dc == 0 {
        return Err(format!("cannot open the display for gamma control: {name}"));
    }
    let result = set_via_gamma_dc(dc, value);
    let _ = unsafe { DeleteDC(dc) };
    result
}

fn set_via_gamma_dc(dc: usize, value: u32) -> Result<Option<bool>, String> {
    let mut ramp = [0u16; 768];
    let ok = unsafe { GetDeviceGammaRamp(dc, ramp.as_mut_ptr()) };
    if ok == 0 {
        for ch in 0..3 {
            for i in 0..256 {
                ramp[ch * 256 + i] = (i * 257) as u16;
            }
        }
    }
    let c = c_est(&ramp);

    let candidate = if value == 100 {
        if c == 0.0 {
            contrast_ramp(100)
        } else {
            stretch_ramp(&ramp, 100.0 / c)
        }
    } else if c == 0.0 {
        contrast_ramp(value)
    } else if (c - value as f64).abs() <= 2.0 {
        return Ok(Some(true));
    } else {
        stretch_ramp(&ramp, value as f64 / c)
    };

    if ramp_eq(&ramp, &candidate) {
        return Ok(Some(true));
    }

    let set = unsafe { SetDeviceGammaRamp(dc, candidate.as_ptr() as *mut u16) };
    if set == 0 {
        return Err("the gamma contrast change failed".to_string());
    }
    Ok(Some(false))
}

// Estimated current contrast (0-130 scale, 100 = neutral) of a ramp.
// Uses the red channel only: temp's red multiplier is always 1.0 and every
// rmod ramp is linear, so r[i] = b * (M0 + (x[i] - M0) * c) with b the
// brightness dim, c the contrast factor, M0 = 128 * 257, x[i] = i * 257.
pub(crate) fn c_est(ramp: &[u16; 768]) -> f64 {
    let m = ramp[128] as f64;
    let h = ramp[255] as f64;
    if m == 0.0 {
        return 0.0;
    }
    let r = (h - m) / (h + m);
    let m0 = 32896.0; // 128 * 257
    let h0 = 65535.0;
    100.0 * 2.0 * m0 * r / ((h0 - m0) * (1.0 - r))
}

// Pivot-stretch each channel around its own entry 128, per-channel.
pub(crate) fn stretch_ramp(ramp: &[u16; 768], ratio: f64) -> [u16; 768] {
    let mut out = [0u16; 768];
    for ch in 0..3 {
        let mid = ramp[ch * 256 + 128] as f64;
        for i in 0..256 {
            let v = mid + (ramp[ch * 256 + i] as f64 - mid) * ratio;
            out[ch * 256 + i] = v.round().clamp(0.0, 65535.0) as u16;
        }
    }
    out
}

// Fresh stretch of the identity ramp: out[i] = M0 + (x[i] - M0) * value / 100.
pub(crate) fn contrast_ramp(value: u32) -> [u16; 768] {
    let ratio = value as f64 / 100.0;
    let mut out = [0u16; 768];
    for ch in 0..3 {
        for i in 0..256 {
            let v = 32896.0 + ((i * 257) as f64 - 32896.0) * ratio;
            out[ch * 256 + i] = v.round().clamp(0.0, 65535.0) as u16;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [u16; 768] {
        contrast_ramp(100)
    }

    #[test]
    fn c_est_reads_the_identity_ramp_as_neutral() {
        let est = c_est(&identity());
        assert!((est - 100.0).abs() <= 0.5, "c_est = {est}");
    }

    #[test]
    fn c_est_reads_a_stretched_ramp_as_its_value() {
        let est = c_est(&contrast_ramp(60));
        assert!((est - 60.0).abs() <= 0.5, "c_est = {est}");
    }

    #[test]
    fn c_est_reads_a_flat_mid_gray_ramp_as_zero() {
        assert_eq!(c_est(&[32896u16; 768]), 0.0);
    }

    #[test]
    fn c_est_reads_a_black_ramp_as_zero() {
        assert_eq!(c_est(&[0u16; 768]), 0.0);
    }

    #[test]
    fn c_est_saturates_near_neutral_for_a_boosted_ramp() {
        let est = c_est(&contrast_ramp(130));
        assert!((est - 100.0).abs() <= 0.5, "c_est = {est}");
    }

    #[test]
    fn stretch_ramp_pivots_the_identity_ramp_around_its_mid() {
        let stretched = stretch_ramp(&identity(), 0.6);
        assert_eq!(stretched[128], 32896);
        assert!((stretched[255] as f64 - 52479.0).abs() <= 1.0);
    }

    #[test]
    fn stretch_ramp_with_ratio_one_is_identity() {
        let stretched = stretch_ramp(&identity(), 1.0);
        for (i, entry) in stretched.iter().enumerate() {
            let expected = ((i % 256) * 257) as u16;
            assert!((*entry as i32 - expected as i32).abs() <= 1, "entry {i}");
        }
    }

    #[test]
    fn stretch_ramp_with_ratio_above_one_clamps_at_both_ends() {
        let stretched = stretch_ramp(&identity(), 1.3);
        assert_eq!(stretched[0], 0);
        assert_eq!(stretched[255], 65535);
    }

    #[test]
    fn stretch_ramp_keeps_each_channels_own_pivot() {
        let mut dimmed = identity();
        for (i, entry) in dimmed[256..512].iter_mut().enumerate() {
            *entry = i as u16 * 257 / 2;
        }
        let stretched = stretch_ramp(&dimmed, 0.6);
        assert_eq!(stretched[128], 32896);
        assert_eq!(stretched[384], 16448);
    }

    #[test]
    fn contrast_ramp_keeps_the_mid_entry() {
        assert_eq!(contrast_ramp(60)[128], 32896);
    }

    #[test]
    fn contrast_ramp_entry_zero_follows_the_pivot_formula() {
        let ramp = contrast_ramp(60);
        let expected = 32896.0 - 32896.0 * 0.6;
        assert!(
            (ramp[0] as f64 - expected).abs() <= 1.0,
            "ramp[0] = {}",
            ramp[0]
        );
    }

    #[test]
    fn contrast_ramp_at_hundred_is_the_identity() {
        let ramp = contrast_ramp(100);
        for (i, entry) in ramp.iter().enumerate() {
            let expected = ((i % 256) * 257) as u16;
            assert!((*entry as i32 - expected as i32).abs() <= 1, "entry {i}");
        }
    }

    #[test]
    fn contrast_ramp_above_hundred_clamps_at_full_scale() {
        let ramp = contrast_ramp(130);
        assert_eq!(ramp[0], 0);
        assert_eq!(ramp[255], 65535);
    }

    /// Helper: a ramp with a warm tint (green/blue reduced at top)
    fn tinted_ramp() -> [u16; 768] {
        let mut ramp = identity();
        for i in 0..256 {
            let g_idx = 256 + i;
            let b_idx = 512 + i;
            ramp[g_idx] = (ramp[g_idx] as f64 * 0.8).round() as u16;
            ramp[b_idx] = (ramp[b_idx] as f64 * 0.6).round() as u16;
        }
        ramp
    }

    /// Helper: a dimmed ramp (all channels scaled down)
    fn dimmed_ramp() -> [u16; 768] {
        let mut ramp = identity();
        for entry in &mut ramp {
            *entry = (*entry as f64 * 0.5).round() as u16;
        }
        ramp
    }

    #[test]
    fn stretch_ramp_over_tinted_keeps_tint_and_reads_c_est() {
        let tinted = tinted_ramp();
        let stretched = stretch_ramp(&tinted, 0.6);
        let est = c_est(&stretched);
        assert!((est - 60.0).abs() <= 0.5, "c_est = {est}");
        let g_ratio = stretched[256 + 255] as f64 / stretched[255] as f64;
        let b_ratio = stretched[512 + 255] as f64 / stretched[255] as f64;
        let orig_g_ratio = tinted[256 + 255] as f64 / tinted[255] as f64;
        let orig_b_ratio = tinted[512 + 255] as f64 / tinted[255] as f64;
        assert!((g_ratio - orig_g_ratio).abs() <= 0.02, "green tint changed: {g_ratio} vs {orig_g_ratio}");
        assert!((b_ratio - orig_b_ratio).abs() <= 0.02, "blue tint changed: {b_ratio} vs {orig_b_ratio}");
    }

    #[test]
    fn stretch_ramp_over_dimmed_preserves_midpoint_brightness() {
        let dimmed = dimmed_ramp();
        let stretched = stretch_ramp(&dimmed, 0.6);
        let est = c_est(&stretched);
        assert!((est - 60.0).abs() <= 0.5, "c_est = {est}");
        // Midpoint (index 128) is the brightness anchor — should be unchanged by stretch
        assert_eq!(stretched[128], dimmed[128], "midpoint brightness changed");
        assert_eq!(stretched[256 + 128], dimmed[256 + 128], "green midpoint brightness changed");
        assert_eq!(stretched[512 + 128], dimmed[512 + 128], "blue midpoint brightness changed");
    }

    // The c_est == 0 fallback is tested via set_via_gamma_dc integration tests;
    // stretch_ramp itself doesn't have fallback logic (that's in the caller).

    #[test]
    fn stretch_ramp_value_100_on_tinted_neutralizes_contrast_keeps_tint() {
        let tinted = tinted_ramp();
        let est = c_est(&tinted);
        let stretched = stretch_ramp(&tinted, 100.0 / est);
        let new_est = c_est(&stretched);
        assert!((new_est - 100.0).abs() <= 0.5, "c_est after neutralize = {new_est}");
        let g_ratio = stretched[256 + 255] as f64 / stretched[255] as f64;
        let b_ratio = stretched[512 + 255] as f64 / stretched[255] as f64;
        let orig_g_ratio = tinted[256 + 255] as f64 / tinted[255] as f64;
        let orig_b_ratio = tinted[512 + 255] as f64 / tinted[255] as f64;
        assert!((g_ratio - orig_g_ratio).abs() <= 0.02, "green tint changed: {g_ratio} vs {orig_g_ratio}");
        assert!((b_ratio - orig_b_ratio).abs() <= 0.02, "blue tint changed: {b_ratio} vs {orig_b_ratio}");
    }

    #[test]
    fn ramp_eq_identical_is_true() {
        let ramp = identity();
        assert!(ramp_eq(&ramp, &ramp));
    }

    #[test]
    fn ramp_eq_one_entry_diff_is_false() {
        let a = identity();
        let mut b = identity();
        b[100] += 1;
        assert!(!ramp_eq(&a, &b));
    }
}
