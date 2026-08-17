//! Fake contrast control: the `RMOD_SYS_FAKE` twin of [`super::super::contrast`].
//!
//! Mirroring the real backend, monitor 1 supports a hardware path (`ddc`,
//! current 75) while monitor 2 is gamma-only (current 100).

use super::super::contrast::{ContrastBackend, ContrastOutcome};
use super::{display_label, resolve};

/// The current contrast of a fake monitor, 0-based like the real
/// backend's device index. Monitor 1 is ddc-capable (75); monitor 2 is
/// gamma-only (100).
fn current_contrast(index: usize) -> Option<u32> {
    match index {
        0 => Some(75),
        1 => Some(100),
        _ => None,
    }
}

/// Whether a fake monitor supports the `ddc` backend.
fn ddc_supported(index: usize) -> bool {
    index == 0
}

/// Sets a fake monitor's contrast. Monitor 1 supports `ddc` (current 75);
/// monitor 2 is gamma-only (current 100).
///
/// Auto-detection tries `ddc` before `gamma`; values above 100 overdrive
/// the gamma ramp and report `clipped`.
pub(crate) fn set_contrast(
    monitor: Option<u32>,
    value: u32,
    via: Option<ContrastBackend>,
) -> Result<ContrastOutcome, String> {
    let monitor = resolve(monitor)?;
    let display = display_label(&monitor);
    let index = monitor.number as usize - 1;
    let current =
        current_contrast(index).expect("fake monitors 1 and 2 always report a current contrast");
    let outcome = |backend: ContrastBackend| ContrastOutcome {
        display: display.clone(),
        value,
        unchanged: current == value,
        backend,
        clipped: value > 100,
    };
    match via {
        Some(ContrastBackend::Ddc) => {
            if value > 100 {
                return Err(format!("{display} contrast cannot go above 100 via ddc"));
            }
            if !ddc_supported(index) {
                return Err(format!("{display} does not support ddc contrast control"));
            }
            Ok(outcome(ContrastBackend::Ddc))
        }
        Some(ContrastBackend::Gamma) => Ok(outcome(ContrastBackend::Gamma)),
        None if ddc_supported(index) && value <= 100 => Ok(outcome(ContrastBackend::Ddc)),
        None => Ok(outcome(ContrastBackend::Gamma)),
    }
}

pub(crate) fn reset_contrast(monitor: Option<u32>) -> Result<ContrastOutcome, String> {
    let monitor = resolve(monitor)?;
    let display = display_label(&monitor);
    Ok(ContrastOutcome {
        display,
        value: 100,
        unchanged: false,
        backend: ContrastBackend::Gamma,
        clipped: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_contrast_primary_auto_uses_ddc() {
        let outcome = set_contrast(None, 60, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(outcome.value, 60);
        assert_eq!(outcome.backend, ContrastBackend::Ddc);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_contrast_already_at_is_unchanged() {
        let outcome = set_contrast(None, 75, None).unwrap();
        assert_eq!(outcome.backend, ContrastBackend::Ddc);
        assert!(outcome.unchanged);
    }

    #[test]
    fn set_contrast_second_monitor_auto_falls_back_to_gamma() {
        let outcome = set_contrast(Some(2), 60, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 2 [:2]");
        assert_eq!(outcome.backend, ContrastBackend::Gamma);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_contrast_forced_ddc_on_gamma_only_monitor_is_error() {
        assert_eq!(
            set_contrast(Some(2), 60, Some(ContrastBackend::Ddc)).err(),
            Some("RMOD Fake Monitor 2 [:2] does not support ddc contrast control".to_string())
        );
    }

    #[test]
    fn set_contrast_forced_gamma_on_ddc_monitor_applies() {
        let outcome = set_contrast(Some(1), 60, Some(ContrastBackend::Gamma)).unwrap();
        assert_eq!(outcome.backend, ContrastBackend::Gamma);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_contrast_forced_ddc_above_hundred_is_error() {
        assert_eq!(
            set_contrast(Some(1), 130, Some(ContrastBackend::Ddc)).err(),
            Some("RMOD Fake Monitor 1 [:1] contrast cannot go above 100 via ddc".to_string())
        );
    }

    #[test]
    fn set_contrast_zero_is_valid() {
        let outcome = set_contrast(None, 0, None).unwrap();
        assert_eq!(outcome.backend, ContrastBackend::Ddc);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_contrast_overdrive_on_gamma_monitor_is_clipped() {
        let outcome = set_contrast(Some(2), 130, None).unwrap();
        assert_eq!(outcome.backend, ContrastBackend::Gamma);
        assert!(outcome.clipped);
    }

    #[test]
    fn set_contrast_within_range_is_not_clipped() {
        let outcome = set_contrast(None, 60, None).unwrap();
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_contrast_gamma_monitor_at_neutral_is_unchanged() {
        let outcome = set_contrast(Some(2), 100, None).unwrap();
        assert_eq!(outcome.backend, ContrastBackend::Gamma);
        assert!(outcome.unchanged);
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_contrast_unknown_monitor_is_error() {
        assert_eq!(
            set_contrast(Some(99), 60, None).err(),
            Some("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }
}
