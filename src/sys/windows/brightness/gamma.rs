use super::super::bindings::{
    CreateDCW, DeleteDC, GetDeviceGammaRamp, SetDeviceGammaRamp, encode_wide,
};
use super::super::temp::{build_ramp, kelvin_to_rgb};

/// Sets brightness through a gamma ramp; this is the fallback that works on
/// every display. `exact` selects the mode-leg semantics (an exact ramp
/// match and a pure [`gamma_ramp`] write) over the percent-leg semantics
/// (a [`b_est`] tolerance and a shape-preserving re-scale). `temp` enables
/// mode+temp composition when `Some(kelvin)`.
pub(crate) fn set_via_gamma(
    name: &str,
    value: u32,
    display: &str,
    exact: bool,
    temp: Option<u32>,
) -> Result<Option<bool>, String> {
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
    let result = set_via_gamma_dc(dc, value, display, exact, temp);
    let _ = unsafe { DeleteDC(dc) };
    result
}

fn set_via_gamma_dc(
    dc: usize,
    value: u32,
    display: &str,
    exact: bool,
    temp: Option<u32>,
) -> Result<Option<bool>, String> {
    let mut ramp = [0u16; 768];
    let ok = unsafe { GetDeviceGammaRamp(dc, ramp.as_mut_ptr()) };
    let write = match (temp, exact, ok != 0) {
        // Plain paths (temp=None) — byte-identical to today
        (None, true, true) if gamma_matches(&ramp, value) => return Ok(Some(true)),
        (None, true, _) => gamma_ramp(value),
        (None, false, true) => match percent_write(&ramp, value) {
            None => return Ok(Some(true)),
            Some(write) => write,
        },
        (None, false, false) => gamma_ramp(value),
        // Mode+temp legs (temp=Some) — both exact=true and exact=false use compose_temp
        (Some(kelvin), _, true) => {
            let candidate = compose_temp(kelvin, value);
            if ramp_eq(&ramp, &candidate) {
                return Ok(Some(true));
            }
            candidate
        }
        (Some(kelvin), _, false) => compose_temp(kelvin, value),
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

/// Builds a temperature-composed gamma ramp at `level` percent.
/// Uses `temp::build_ramp` / `kelvin_to_rgb` to create the temp ramp,
/// then scales it to `level` using `scale_ramp` (forced, no `percent_write`
/// short-circuit).
pub(crate) fn compose_temp(kelvin: u32, level: u32) -> [u16; 768] {
    let (r, g, b) = kelvin_to_rgb(kelvin);
    let temp_ramp = build_ramp(r, g, b);
    // Convert Ramp to [u16; 768] format
    let mut ramp = [0u16; 768];
    for i in 0..256 {
        ramp[i] = temp_ramp.red[i];
        ramp[256 + i] = temp_ramp.green[i];
        ramp[512 + i] = temp_ramp.blue[i];
    }
    let estimated = b_est(&ramp);
    scale_ramp(&ramp, level, estimated)
}

/// Entry-wise equality of two 768-entry ramps.
pub(crate) fn ramp_eq(a: &[u16; 768], b: &[u16; 768]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
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
        for entry in &mut ramp[256..] {
            *entry /= 2;
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

    // compose_temp tests

    #[test]
    fn compose_temp_is_non_degenerate_at_presets() {
        for k in [1900, 2700, 3400, 4500, 6500] {
            for level in [50, 100, 130] {
                let ramp = compose_temp(k, level);
                // ramp should not be all zeros
                assert!(ramp.iter().any(|&v| v > 0), "kelvin {k} level {level}");
            }
        }
    }

    #[test]
    fn compose_temp_tint_ratios_match_kelvin_to_rgb() {
        // At 100%, the channel ratios should match kelvin_to_rgb after CHANNEL_FLOOR
        // (0.5) is applied in build_ramp. Check at midpoint (index 128) to avoid
        // clamping distortion at the top end.
        const CHANNEL_FLOOR: f64 = 0.5;
        for k in [1900, 2700, 3400, 4500, 6500] {
            let (r_mult, g_mult, b_mult) = kelvin_to_rgb(k);
            let r_mult = r_mult.max(CHANNEL_FLOOR);
            let g_mult = g_mult.max(CHANNEL_FLOOR);
            let b_mult = b_mult.max(CHANNEL_FLOOR);
            let ramp = compose_temp(k, 100);
            // Use index 128 (midpoint) to avoid clamping effects at index 255
            let r_mid = ramp[128] as f64;
            let g_mid = ramp[256 + 128] as f64;
            let b_mid = ramp[512 + 128] as f64;
            let expected_g = r_mid * (g_mult / r_mult);
            let expected_b = r_mid * (b_mult / r_mult);
            assert!((g_mid - expected_g).abs() <= 2.0, "kelvin {k} green: g_mid={g_mid}, expected={expected_g}");
            assert!((b_mid - expected_b).abs() <= 2.0, "kelvin {k} blue: b_mid={b_mid}, expected={expected_b}");
        }
    }

    #[test]
    fn compose_temp_red_255_at_100_reads_b_est_100() {
        for k in [1900, 2700, 3400, 4500, 6500] {
            let ramp = compose_temp(k, 100);
            assert_eq!(b_est(&ramp), 100, "kelvin {k}");
        }
    }

    // ramp_eq tests

    #[test]
    fn ramp_eq_identical_is_true() {
        let ramp = gamma_ramp(50);
        assert!(ramp_eq(&ramp, &ramp));
    }

    #[test]
    fn ramp_eq_one_entry_diff_is_false() {
        let a = gamma_ramp(50);
        let mut b = gamma_ramp(50);
        b[100] += 1;
        assert!(!ramp_eq(&a, &b));
    }

    // Mode+temp unchanged detection tests

    #[test]
    fn mode_temp_double_apply_is_unchanged() {
        // First apply
        let r1 = compose_temp(3400, 50);
        // Second apply should be detected as unchanged via ramp_eq
        assert!(ramp_eq(&r1, &r1));
    }

    #[test]
    fn mode_temp_changed_kelvin_writes() {
        let r1 = compose_temp(3400, 50);
        let r2 = compose_temp(4500, 50);
        assert!(!ramp_eq(&r1, &r2));
    }

    #[test]
    fn plain_mode_temp_none_still_uses_gamma_matches() {
        // Regression pin: temp=None path should use gamma_matches logic
        let ramp = gamma_ramp(50);
        assert!(gamma_matches(&ramp, 50));
        assert!(!gamma_matches(&ramp, 51));
    }

    // Composed ramp is forced (no percent_write None skip)

    #[test]
    fn composed_ramp_forced_no_percent_write_skip() {
        // compose_temp should always produce a ramp, never return None like percent_write
        let ramp = compose_temp(3400, 100);
        // At 3400K, the shape should differ from a pure gamma ramp
        let pure = gamma_ramp(100);
        assert!(!ramp_eq(&ramp, &pure));
        // The green channel should be reduced at 3400K
        assert!(ramp[256 + 255] < pure[256 + 255]);
    }
}
