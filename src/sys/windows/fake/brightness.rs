//! Fake brightness control: the `RMOD_SYS_FAKE` twin of [`super::super::brightness`].
//!
//! Mirroring the real backend, monitor 1 supports a hardware path (`ddc` and
//! `slider`) while monitor 2 is gamma-only. Modes compose a hardware write
//! with a gamma write and remember the last applied mode to report
//! `unchanged`.

use std::sync::{Mutex, OnceLock};

use super::super::brightness::{
    BrightnessBackend, BrightnessLayer, BrightnessOutcome, BrightnessValue, gamma_level_for,
    mode_word,
};
use super::Monitor;
use super::{display_label, resolve};

/// Per-monitor brightness-mode state, indexed by 1-based monitor number.
/// `None` means no mode has been applied yet.
static BRIGHTNESS_MODES: OnceLock<Mutex<Vec<Option<Vec<BrightnessLayer>>>>> = OnceLock::new();

fn brightness_modes() -> &'static Mutex<Vec<Option<Vec<BrightnessLayer>>>> {
    BRIGHTNESS_MODES.get_or_init(|| Mutex::new(vec![None, None]))
}

/// The layers of a composite brightness mode on a fake monitor: a
/// hardware write followed by the mode's gamma write when the monitor has a
/// hardware path, or the gamma write alone on gamma-only monitors.
fn mode_layers(monitor: &Monitor, mode: BrightnessValue) -> Vec<BrightnessLayer> {
    let gamma = BrightnessLayer::Gamma {
        level: gamma_level_for(mode),
    };
    if monitor.number == 1 {
        let hardware = match mode {
            BrightnessValue::Min => BrightnessLayer::Hardware {
                backend: BrightnessBackend::Slider,
                level: 5,
            },
            BrightnessValue::Max => BrightnessLayer::Hardware {
                backend: BrightnessBackend::Ddc,
                level: 100,
            },
            BrightnessValue::Boost => BrightnessLayer::Hardware {
                backend: BrightnessBackend::Slider,
                level: 100,
            },
            BrightnessValue::Percent(_) => unreachable!("percent is not a mode"),
        };
        vec![hardware, gamma]
    } else {
        vec![gamma]
    }
}

/// Sets a fake monitor's brightness. Monitor 1 supports `ddc` and `slider`
/// (current 60); monitor 2 is gamma-only (current 40).
///
/// Percent behavior mirrors the real backend's legacy chain. Modes
/// [`BrightnessValue::Min`], [`BrightnessValue::Max`] and
/// [`BrightnessValue::Boost`] compose a hardware write with a gamma write;
/// they reject a forced backend and report `unchanged` when the same mode
/// was applied last.
///
/// `temp` is an optional color temperature in Kelvin for mode+temp composition.
pub(crate) fn set_brightness(
    monitor: Option<u32>,
    value: BrightnessValue,
    via: Option<BrightnessBackend>,
    _temp: Option<u32>,
) -> Result<BrightnessOutcome, String> {
    let monitor = resolve(monitor)?;
    let display = display_label(&monitor);
    match value {
        BrightnessValue::Percent(level) => {
            let current = if monitor.number == 1 { 60 } else { 40 };
            let backend = match via {
                Some(backend) => backend,
                None if monitor.number == 1 => BrightnessBackend::Ddc,
                None => BrightnessBackend::Gamma,
            };
            if let Some(backend) = via
                && monitor.number == 2
                && matches!(backend, BrightnessBackend::Ddc | BrightnessBackend::Slider)
            {
                return Err(format!(
                    "{display} does not support {} brightness control",
                    backend.name()
                ));
            }
            let layer = match backend {
                BrightnessBackend::Gamma => BrightnessLayer::Gamma { level },
                backend => BrightnessLayer::Hardware { backend, level },
            };
            Ok(BrightnessOutcome {
                display,
                kind: BrightnessValue::Percent(level),
                unchanged: current == level,
                layers: vec![layer],
                clipped: false,
            })
        }
        mode => {
            if via.is_some() {
                return Err(format!(
                    "{} does not take a backend. use a number to choose a backend",
                    mode_word(mode)
                ));
            }
            let layers = mode_layers(&monitor, mode);
            let mut states = brightness_modes().lock().unwrap();
            let previous = &mut states[monitor.number as usize - 1];
            let unchanged = previous.as_ref() == Some(&layers);
            *previous = Some(layers.clone());
            Ok(BrightnessOutcome {
                display,
                kind: mode,
                unchanged,
                layers,
                clipped: matches!(mode, BrightnessValue::Boost),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_brightness_modes() {
        *brightness_modes().lock().unwrap() = vec![None, None];
    }

    #[test]
    fn set_brightness_primary_auto_uses_ddc() {
        let outcome = set_brightness(None, BrightnessValue::Percent(30), None, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(
            outcome.layers,
            vec![BrightnessLayer::Hardware {
                backend: BrightnessBackend::Ddc,
                level: 30
            }]
        );
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_already_at_is_unchanged() {
        let outcome = set_brightness(None, BrightnessValue::Percent(60), None, None).unwrap();
        assert_eq!(
            outcome.layers,
            vec![BrightnessLayer::Hardware {
                backend: BrightnessBackend::Ddc,
                level: 60
            }]
        );
        assert!(outcome.unchanged);
    }

    #[test]
    fn set_brightness_second_monitor_auto_falls_back_to_gamma() {
        let outcome = set_brightness(Some(2), BrightnessValue::Percent(30), None, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 2 [:2]");
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 30 }]);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_forced_unsupported_backend_is_error() {
        assert_eq!(
            set_brightness(
                Some(2),
                BrightnessValue::Percent(30),
                Some(BrightnessBackend::Ddc),
                None
            )
            .err(),
            Some("RMOD Fake Monitor 2 [:2] does not support ddc brightness control".to_string())
        );
        assert_eq!(
            set_brightness(
                Some(2),
                BrightnessValue::Percent(30),
                Some(BrightnessBackend::Slider),
                None
            )
            .err(),
            Some("RMOD Fake Monitor 2 [:2] does not support slider brightness control".to_string())
        );
    }

    #[test]
    fn set_brightness_forced_gamma_on_ddc_monitor_applies() {
        let outcome = set_brightness(
            Some(1),
            BrightnessValue::Percent(30),
            Some(BrightnessBackend::Gamma),
            None,
        )
        .unwrap();
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 30 }]);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_unknown_monitor_is_error() {
        assert_eq!(
            set_brightness(Some(99), BrightnessValue::Percent(30), None, None).err(),
            Some("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn set_brightness_min_on_monitor_1_layers_slider_floor_and_gamma() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(outcome.kind, BrightnessValue::Min);
        assert!(!outcome.unchanged);
        assert_eq!(
            outcome.layers,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 5
                },
                BrightnessLayer::Gamma { level: 50 },
            ]
        );
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_brightness_max_on_monitor_1_layers_ddc_full_and_gamma() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(1), BrightnessValue::Max, None, None).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Max);
        assert!(!outcome.unchanged);
        assert_eq!(
            outcome.layers,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Ddc,
                    level: 100
                },
                BrightnessLayer::Gamma { level: 100 },
            ]
        );
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_brightness_boost_on_monitor_1_layers_slider_full_and_overdriven_gamma() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(1), BrightnessValue::Boost, None, None).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Boost);
        assert!(!outcome.unchanged);
        assert_eq!(
            outcome.layers,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 100
                },
                BrightnessLayer::Gamma { level: 130 },
            ]
        );
        assert!(outcome.clipped);
    }

    #[test]
    fn set_brightness_min_on_gamma_only_monitor_2_is_gamma_only() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(2), BrightnessValue::Min, None, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 2 [:2]");
        assert_eq!(outcome.kind, BrightnessValue::Min);
        assert!(!outcome.unchanged);
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 50 }]);
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_brightness_max_on_gamma_only_monitor_2_is_gamma_only() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(2), BrightnessValue::Max, None, None).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Max);
        assert!(!outcome.unchanged);
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 100 }]);
        assert!(!outcome.clipped);
    }

    #[test]
    fn set_brightness_boost_on_gamma_only_monitor_2_is_gamma_only() {
        reset_brightness_modes();
        let outcome = set_brightness(Some(2), BrightnessValue::Boost, None, None).unwrap();
        assert_eq!(outcome.kind, BrightnessValue::Boost);
        assert!(!outcome.unchanged);
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 130 }]);
        assert!(outcome.clipped);
    }

    #[test]
    fn set_brightness_mode_unknown_monitor_is_error() {
        assert_eq!(
            set_brightness(Some(99), BrightnessValue::Min, None, None).err(),
            Some("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn set_brightness_repeated_mode_is_unchanged() {
        set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        let outcome = set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        assert!(outcome.unchanged);
        assert_eq!(
            outcome.layers,
            vec![
                BrightnessLayer::Hardware {
                    backend: BrightnessBackend::Slider,
                    level: 5
                },
                BrightnessLayer::Gamma { level: 50 },
            ]
        );
    }

    #[test]
    fn set_brightness_different_mode_after_mode_is_a_change() {
        set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        let outcome = set_brightness(Some(1), BrightnessValue::Max, None, None).unwrap();
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_mode_after_percent_is_a_change() {
        reset_brightness_modes();
        set_brightness(Some(1), BrightnessValue::Percent(30), None, None).unwrap();
        let outcome = set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_percent_after_mode_keeps_percent_unchanged_detection() {
        set_brightness(Some(1), BrightnessValue::Min, None, None).unwrap();
        let outcome = set_brightness(Some(1), BrightnessValue::Percent(60), None, None).unwrap();
        assert!(outcome.unchanged);
    }

    #[test]
    fn set_brightness_repeated_mode_on_second_monitor_is_unchanged() {
        set_brightness(Some(2), BrightnessValue::Boost, None, None).unwrap();
        let outcome = set_brightness(Some(2), BrightnessValue::Boost, None, None).unwrap();
        assert!(outcome.unchanged);
        assert_eq!(outcome.layers, vec![BrightnessLayer::Gamma { level: 130 }]);
        assert!(outcome.clipped);
    }

    #[test]
    fn set_brightness_mode_with_backend_is_error() {
        for (mode, word) in [
            (BrightnessValue::Min, "min"),
            (BrightnessValue::Max, "max"),
            (BrightnessValue::Boost, "boost"),
        ] {
            assert_eq!(
                set_brightness(Some(1), mode, Some(BrightnessBackend::Ddc), None).err(),
                Some(format!(
                    "{word} does not take a backend. use a number to choose a backend"
                )),
                "mode {word}"
            );
        }
    }
}
