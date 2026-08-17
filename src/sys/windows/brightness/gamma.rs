use super::super::bindings::{
    CreateDCW, DeleteDC, GetDeviceGammaRamp, SetDeviceGammaRamp, encode_wide,
};

/// Sets brightness through a gamma ramp; this is the fallback that works on
/// every display. `exact` selects the mode-leg semantics (an exact ramp
/// match and a pure [`gamma_ramp`] write) over the percent-leg semantics
/// (a [`b_est`] tolerance and a shape-preserving re-scale).
pub(crate) fn set_via_gamma(
    name: &str,
    value: u32,
    display: &str,
    exact: bool,
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
}
