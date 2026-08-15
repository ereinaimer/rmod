//! Fake Windows backend used by the integration test suite.
//!
//! When the `RMOD_SYS_FAKE` environment variable is `1`, every public entry
//! point in [`super`] delegates here instead of calling Win32, so running
//! the tests never changes the host display. The fake presents a fixed
//! world: two monitors with a known set of supported modes and error
//! strings matching the real backend.

use std::sync::OnceLock;

use super::apply::{ApplyOutcome, Change, MainChange, MainOutcome, Refresh};
use super::attach::{AttachAction, AttachChange, AttachOutcome};
use super::bindings::{DM_POSITION, DevmodeW, Pointl};
use super::brightness::{BrightnessBackend, BrightnessOutcome};
use super::capabilities::Mode;
use super::layout::{self, Direction, PlacementChange, PlacementOutcome};
use super::query::Monitor;

const MONITOR_1_NAME: &str = "RMOD Fake Monitor 1";
const MONITOR_2_NAME: &str = "RMOD Fake Monitor 2";

/// True when the fake backend is active (`RMOD_SYS_FAKE=1`).
pub(crate) fn enabled() -> bool {
    static ACTIVE: OnceLock<bool> = OnceLock::new();
    *ACTIVE.get_or_init(|| std::env::var("RMOD_SYS_FAKE").is_ok_and(|v| v == "1"))
}

/// The monitor with the given 1-based number, or `None` when unknown.
fn monitor(number: u32) -> Option<Monitor> {
    match number {
        1 => Some(Monitor {
            number: 1,
            name: MONITOR_1_NAME.to_string(),
            is_primary: true,
            width: 1920,
            height: 1080,
            refresh: 60,
            x: 0,
            y: 0,
        }),
        2 => Some(Monitor {
            number: 2,
            name: MONITOR_2_NAME.to_string(),
            is_primary: false,
            width: 1920,
            height: 1080,
            refresh: 60,
            x: 1920,
            y: 0,
        }),
        _ => None,
    }
}

/// Resolves a monitor target; `None` selects the primary fake monitor.
fn resolve(target: Option<u32>) -> Result<Monitor, String> {
    match target {
        None => Ok(monitor(1).expect("fake monitor 1 exists")),
        Some(n) => monitor(n).ok_or_else(|| {
            format!("monitor {n} not found. run rmod list to see connected displays")
        }),
    }
}

/// The supported modes of every fake monitor.
fn modes() -> Vec<Mode> {
    vec![
        Mode {
            width: 1280,
            height: 720,
            refresh: 60,
        },
        Mode {
            width: 1920,
            height: 1080,
            refresh: 60,
        },
        Mode {
            width: 1920,
            height: 1080,
            refresh: 144,
        },
        Mode {
            width: 2560,
            height: 1440,
            refresh: 60,
        },
        Mode {
            width: 2560,
            height: 1440,
            refresh: 144,
        },
        Mode {
            width: 3840,
            height: 2160,
            refresh: 60,
        },
        Mode {
            width: 3840,
            height: 2160,
            refresh: 144,
        },
    ]
}

/// The best supported mode, used by `max`.
fn best_mode() -> Mode {
    Mode {
        width: 3840,
        height: 2160,
        refresh: 144,
    }
}

/// The display label used in output and error messages.
fn display_label(monitor: &Monitor) -> String {
    format!("{} [:{number}]", monitor.name, number = monitor.number)
}

/// The mode currently reported for a fake monitor.
fn current_mode(monitor: &Monitor) -> Mode {
    Mode {
        width: monitor.width,
        height: monitor.height,
        refresh: monitor.refresh,
    }
}

/// Builds a change and classifies it, mirroring `apply::outcome_of`.
fn outcome(monitor: &Monitor, mode: Mode, orientation: Option<u32>) -> ApplyOutcome {
    let change = Change {
        monitor: monitor.number,
        display: display_label(monitor),
        mode,
        previous: current_mode(monitor),
        orientation,
        previous_orientation: orientation.map(|_| 0),
    };
    let orientation_matches = match (change.orientation, change.previous_orientation) {
        (Some(angle), Some(previous)) => angle == previous,
        _ => true,
    };
    if change.mode == change.previous && orientation_matches {
        ApplyOutcome::Unchanged(change)
    } else {
        ApplyOutcome::Applied(change)
    }
}

/// Lists every fake monitor with its current settings.
pub(crate) fn list() -> Result<Vec<Monitor>, String> {
    Ok(vec![
        monitor(1).expect("fake monitor 1 exists"),
        monitor(2).expect("fake monitor 2 exists"),
    ])
}

/// Returns the supported modes for a fake monitor.
pub(crate) fn caps(monitor: Option<u32>) -> Result<(Monitor, Vec<Mode>), String> {
    let monitor = resolve(monitor)?;
    Ok((monitor, modes()))
}

/// Returns the supported modes for every fake monitor.
pub(crate) fn caps_all() -> Result<Vec<(Monitor, Vec<Mode>)>, String> {
    Ok(vec![
        (monitor(1).expect("fake monitor 1 exists"), modes()),
        (monitor(2).expect("fake monitor 2 exists"), modes()),
    ])
}

/// Applies a resolution, refresh and orientation policy to a fake monitor.
pub(crate) fn set(
    monitor: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<ApplyOutcome, String> {
    let monitor = resolve(monitor)?;
    let (w, h) = (
        width.unwrap_or(monitor.width),
        height.unwrap_or(monitor.height),
    );
    let r = match refresh {
        Refresh::Keep => monitor.refresh,
        Refresh::Max => modes()
            .iter()
            .filter(|m| m.width == w && m.height == h)
            .map(|m| m.refresh)
            .max()
            .unwrap_or(monitor.refresh),
        Refresh::Fixed(f) => f,
    };
    if !modes()
        .iter()
        .any(|m| m.width == w && m.height == h && m.refresh == r)
    {
        return Err(format!(
            "{} does not support {w}x{h} @ {r}Hz. run rmod list --caps to see supported modes",
            display_label(&monitor)
        ));
    }
    Ok(outcome(
        &monitor,
        Mode {
            width: w,
            height: h,
            refresh: r,
        },
        orientation,
    ))
}

/// Applies the best supported mode to a fake monitor.
pub(crate) fn max(monitor: Option<u32>, orientation: Option<u32>) -> Result<ApplyOutcome, String> {
    let monitor = resolve(monitor)?;
    Ok(outcome(&monitor, best_mode(), orientation))
}

/// Applies the best supported mode to every fake monitor.
pub(crate) fn max_all(orientation: Option<u32>) -> Result<Vec<ApplyOutcome>, String> {
    Ok(vec![max(Some(1), orientation)?, max(Some(2), orientation)?])
}

/// Applies a resolution, refresh and orientation policy to every fake monitor.
pub(crate) fn set_all(
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<Vec<ApplyOutcome>, String> {
    Ok(vec![
        set(Some(1), width, height, refresh, orientation)?,
        set(Some(2), width, height, refresh, orientation)?,
    ])
}

/// Re-applies a previously captured mode to undo a fake change.
pub(crate) fn revert(
    _monitor: Option<u32>,
    previous: Mode,
    _previous_orientation: Option<u32>,
) -> Result<Mode, String> {
    Ok(previous)
}

/// Promotes a fake monitor to the main display.
pub(crate) fn make_main(monitor: u32, _names: &[String]) -> Result<MainOutcome<'_>, String> {
    match monitor {
        1 => Ok(MainOutcome::Unchanged(MONITOR_1_NAME.to_string())),
        2 => Ok(MainOutcome::Applied(MainChange {
            monitor: 2,
            display: MONITOR_2_NAME.to_string(),
            applied: vec![],
            previous: vec![],
        })),
        n => Err(format!(
"monitor {n} not found. run rmod list to see connected displays"
        )),
    }
}

/// Undoes a promotion; the fake never persists anything.
pub(crate) fn revert_main(_change: &MainChange<'_>) -> Result<(), String> {
    Ok(())
}

/// The synthetic devmode of a fake monitor.
fn fake_devmode(monitor: &Monitor) -> DevmodeW {
    let mut devmode: DevmodeW = unsafe { std::mem::zeroed() };
    devmode.dm_position = Pointl {
        x: monitor.x,
        y: monitor.y,
    };
    devmode.dm_pels_width = monitor.width;
    devmode.dm_pels_height = monitor.height;
    devmode.dm_display_frequency = monitor.refresh;
    devmode
}

/// Places a fake monitor relative to another using the real placement
/// math; the two-monitor fake world has no landing-spot collisions.
#[allow(dead_code)]
pub(crate) fn apply_placement(
    monitor: u32,
    direction: Direction,
    reference: u32,
) -> Result<PlacementOutcome, String> {
    let target = resolve(Some(monitor))?;
    let reference_monitor = resolve(Some(reference)).map_err(|e| format!("reference {e}"))?;
    if reference_monitor.number == target.number {
        return Err(format!(
            "cannot place monitor {} relative to itself, use a different reference monitor",
            target.number
        ));
    }
    let target_dev = fake_devmode(&target);
    let reference_dev = fake_devmode(&reference_monitor);
    let landing = layout::landing_position(direction, &reference_dev, &target_dev);
    if landing == target_dev.dm_position {
        return Ok(PlacementOutcome::Unchanged {
            display: display_label(&target),
            reference_display: display_label(&reference_monitor),
        });
    }
    let mut moved = target_dev;
    moved.dm_position = landing;
    moved.dm_fields |= DM_POSITION;
    let names = enumerate_devices();
    let target_name = names[target.number as usize - 1].clone();
    Ok(PlacementOutcome::Applied(PlacementChange {
        display: display_label(&target),
        reference_display: display_label(&reference_monitor),
        swap_display: None,
        applied: vec![(target_name.clone(), moved)],
        previous: vec![(target_name, target_dev)],
    }))
}

/// Undoes a fake placement; the fake never persists anything.
#[allow(dead_code)]
pub(crate) fn revert_placement(_change: &PlacementChange) -> Result<(), String> {
    Ok(())
}

/// The fake device names, mirroring the two-monitor world.
pub(crate) fn enumerate_devices() -> Vec<String> {
    vec![r"\\.\DISPLAY1".to_string(), r"\\.\DISPLAY2".to_string()]
}

/// Returns the current mode for a fake monitor number.
pub(crate) fn get_current_mode(monitor: u32) -> Result<Monitor, String> {
    resolve(Some(monitor))
}

/// Returns the current mode for the primary fake monitor.
pub(crate) fn get_primary_mode() -> Result<Monitor, String> {
    resolve(None)
}

/// Detaches a fake monitor from the desktop.
///
/// The fake world is stateless: both monitors are always attached, so only
/// the primary guard can fail and no change is ever `Unchanged`.
pub(crate) fn disable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    let monitor = resolve(monitor)?;
    if monitor.is_primary {
        return Err("cannot detach the primary display".to_string());
    }
    Ok(AttachOutcome::Applied(AttachChange {
        monitor: monitor.number,
        display: display_label(&monitor),
        action: AttachAction::Disable,
        previous: fake_devmode(&monitor),
    }))
}

/// Re-attaches a fake monitor to the desktop.
///
/// The fake world is stateless: both monitors are always attached, so
/// every enable reports `Unchanged`.
pub(crate) fn enable(monitor: Option<u32>) -> Result<AttachOutcome, String> {
    let monitor = resolve(monitor)?;
    Ok(AttachOutcome::Unchanged(AttachChange {
        monitor: monitor.number,
        display: display_label(&monitor),
        action: AttachAction::Enable,
        previous: fake_devmode(&monitor),
    }))
}

/// Undoes a fake attach/detach change; the fake never persists anything.
pub(crate) fn revert_attach(_change: &AttachChange) -> Result<(), String> {
    Ok(())
}

/// The label of every fake monitor.
fn fake_labels() -> Vec<String> {
    vec![
        display_label(&monitor(1).expect("fake monitor 1 exists")),
        display_label(&monitor(2).expect("fake monitor 2 exists")),
    ]
}

/// Puts the fake monitors to sleep.
pub(crate) fn sleep_monitor() -> Result<Vec<String>, String> {
    Ok(fake_labels())
}

/// Wakes the fake monitors.
pub(crate) fn wake_monitor() -> Result<Vec<String>, String> {
    Ok(fake_labels())
}

/// The fake device names of every display, attached or detached.
pub(crate) fn enumerate_all_devices() -> Vec<String> {
    enumerate_devices()
}

/// Sets a fake monitor's brightness. Monitor 1 supports `ddc` and `slider`
/// (current 60); monitor 2 is gamma-only (current 40).
pub(crate) fn set_brightness(
    monitor: Option<u32>,
    value: u32,
    via: Option<BrightnessBackend>,
) -> Result<BrightnessOutcome, String> {
    let monitor = resolve(monitor)?;
    let display = display_label(&monitor);
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
    Ok(BrightnessOutcome {
        display,
        value,
        backend,
        unchanged: current == value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_two_monitors() {
        let monitors = list().unwrap();
        assert_eq!(monitors.len(), 2);
        assert!(monitors[0].is_primary);
        assert!(!monitors[1].is_primary);
    }

    #[test]
    fn caps_returns_supported_modes() {
        let (monitor, modes) = caps(None).unwrap();
        assert_eq!(monitor.number, 1);
        assert_eq!(modes.len(), 7);
        assert!(modes.contains(&Mode {
            width: 1920,
            height: 1080,
            refresh: 60
        }));
    }

    #[test]
    fn caps_unknown_monitor_is_error() {
        assert_eq!(
            caps(Some(99)).err(),
            Some("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn set_current_mode_is_unchanged() {
        assert_eq!(
            set(Some(1), Some(1920), Some(1080), Refresh::Fixed(60), None),
            Ok(ApplyOutcome::Unchanged(Change {
                monitor: 1,
                display: "RMOD Fake Monitor 1 [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 60
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 60
                },
                orientation: None,
                previous_orientation: None,
            }))
        );
    }

    #[test]
    fn set_other_supported_mode_is_applied() {
        assert_eq!(
            set(None, Some(1920), Some(1080), Refresh::Fixed(144), None),
            Ok(ApplyOutcome::Applied(Change {
                monitor: 1,
                display: "RMOD Fake Monitor 1 [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 144
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 60
                },
                orientation: None,
                previous_orientation: None,
            }))
        );
    }

    #[test]
    fn set_unsupported_mode_is_error() {
        assert_eq!(
            set(None, Some(9999), Some(9999), Refresh::Fixed(1), None),
            Err("RMOD Fake Monitor 1 [:1] does not support 9999x9999 @ 1Hz. run rmod list --caps to see supported modes".to_string())
        );
    }

    #[test]
    fn set_unknown_monitor_is_error() {
        assert_eq!(
            set(Some(99), Some(1920), Some(1080), Refresh::Keep, None),
            Err("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn set_max_refresh_uses_highest_supported() {
        assert_eq!(
            set(None, None, None, Refresh::Max, None),
            Ok(ApplyOutcome::Applied(Change {
                monitor: 1,
                display: "RMOD Fake Monitor 1 [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 144
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 60
                },
                orientation: None,
                previous_orientation: None,
            }))
        );
    }

    #[test]
    fn orientation_change_is_applied() {
        let outcome = set(None, None, None, Refresh::Keep, Some(90)).unwrap();
        match outcome {
            ApplyOutcome::Applied(change) => {
                assert_eq!(change.orientation, Some(90));
                assert_eq!(change.previous_orientation, Some(0));
                assert_eq!(change.mode, change.previous);
            }
            ApplyOutcome::Unchanged(_) => panic!("orientation change must be applied"),
        }
    }

    #[test]
    fn max_returns_best_mode() {
        let outcome = max(None, None).unwrap();
        match outcome {
            ApplyOutcome::Applied(change) => {
                assert_eq!(
                    change.mode,
                    Mode {
                        width: 3840,
                        height: 2160,
                        refresh: 144
                    }
                );
            }
            ApplyOutcome::Unchanged(_) => panic!("best mode differs from current"),
        }
    }

    #[test]
    fn make_main_primary_is_unchanged() {
        assert_eq!(
            make_main(1, &[]),
            Ok(MainOutcome::Unchanged(MONITOR_1_NAME.to_string()))
        );
    }

    #[test]
    fn make_main_second_is_applied() {
        match make_main(2, &[]).unwrap() {
            MainOutcome::Applied(change) => assert_eq!(change.display, MONITOR_2_NAME),
            MainOutcome::Unchanged(_) => panic!("monitor 2 is not primary"),
        }
    }

    #[test]
    fn make_main_unknown_is_error() {
        assert_eq!(
            make_main(99, &[]),
            Err("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn apply_placement_places_monitor_left_of_primary() {
        let outcome = apply_placement(2, Direction::Left, 1).unwrap();
        let PlacementOutcome::Applied(change) = outcome else {
            panic!("placement must be applied");
        };
        assert_eq!(change.display, "RMOD Fake Monitor 2 [:2]");
        assert_eq!(change.reference_display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(change.swap_display, None);
        assert_eq!(change.applied.len(), 1);
        assert_eq!(change.applied[0].0, r"\\.\DISPLAY2");
        assert_eq!(change.applied[0].1.dm_position, Pointl { x: -1920, y: 0 });
        assert_ne!(change.applied[0].1.dm_fields & DM_POSITION, 0);
        assert_eq!(change.applied[0].1.dm_pels_width, 1920);
        assert_eq!(change.previous.len(), 1);
        assert_eq!(change.previous[0].1.dm_position, Pointl { x: 1920, y: 0 });
    }

    #[test]
    fn apply_placement_below_explicit_reference() {
        let outcome = apply_placement(2, Direction::Below, 1).unwrap();
        let PlacementOutcome::Applied(change) = outcome else {
            panic!("placement must be applied");
        };
        assert_eq!(change.reference_display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(change.applied[0].1.dm_position, Pointl { x: 0, y: 1080 });
    }

    #[test]
    fn apply_placement_noop_right_of_primary_is_unchanged() {
        assert_eq!(
            apply_placement(2, Direction::Right, 1),
            Ok(PlacementOutcome::Unchanged {
                display: "RMOD Fake Monitor 2 [:2]".to_string(),
                reference_display: "RMOD Fake Monitor 1 [:1]".to_string(),
            })
        );
    }

    #[test]
    fn apply_placement_noop_left_of_second_is_unchanged() {
        assert_eq!(
            apply_placement(1, Direction::Left, 2),
            Ok(PlacementOutcome::Unchanged {
                display: "RMOD Fake Monitor 1 [:1]".to_string(),
                reference_display: "RMOD Fake Monitor 2 [:2]".to_string(),
            })
        );
    }

    #[test]
    fn apply_placement_self_reference_is_error() {
        assert_eq!(
            apply_placement(1, Direction::Left, 1),
            Err(
                "cannot place monitor 1 relative to itself, use a different reference monitor"
                    .to_string()
            )
        );
        assert_eq!(
            apply_placement(2, Direction::Left, 2),
            Err(
                "cannot place monitor 2 relative to itself, use a different reference monitor"
                    .to_string()
            )
        );
    }

    #[test]
    fn apply_placement_unknown_monitor_is_error() {
        assert_eq!(
            apply_placement(99, Direction::Left, 1),
            Err("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn apply_placement_unknown_reference_is_error() {
        assert_eq!(
            apply_placement(1, Direction::Left, 99),
            Err(
                "reference monitor 99 not found. run rmod list to see connected displays"
                    .to_string()
            )
        );
    }

    #[test]
    fn revert_placement_restores_fake_positions() {
        let outcome = apply_placement(2, Direction::Left, 1).unwrap();
        let PlacementOutcome::Applied(change) = outcome else {
            panic!("placement must be applied");
        };
        assert_eq!(revert_placement(&change), Ok(()));
    }

    #[test]
    fn disable_primary_is_error() {
        assert_eq!(
            disable(None),
            Err("cannot detach the primary display".to_string())
        );
        assert_eq!(
            disable(Some(1)),
            Err("cannot detach the primary display".to_string())
        );
    }

    #[test]
    fn disable_second_monitor_is_applied() {
        match disable(Some(2)).unwrap() {
            AttachOutcome::Applied(change) => {
                assert_eq!(change.monitor, 2);
                assert_eq!(change.display, "RMOD Fake Monitor 2 [:2]");
                assert_eq!(change.action, AttachAction::Disable);
                assert_eq!(change.previous.dm_pels_width, 1920);
                assert_eq!(
                    change.previous.dm_position,
                    Pointl { x: 1920, y: 0 }
                );
            }
            AttachOutcome::Unchanged(_) => panic!("disable must be applied"),
        }
    }

    #[test]
    fn disable_unknown_monitor_is_error() {
        assert_eq!(
            disable(Some(99)),
            Err("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn enable_always_unchanged_because_fake_monitors_are_attached() {
        match enable(Some(2)).unwrap() {
            AttachOutcome::Unchanged(change) => {
                assert_eq!(change.monitor, 2);
                assert_eq!(change.action, AttachAction::Enable);
                assert_eq!(change.previous.dm_pels_width, 1920);
            }
            AttachOutcome::Applied(_) => panic!("fake monitors are always attached"),
        }
    }

    #[test]
    fn enable_unknown_monitor_is_error() {
        assert_eq!(
            enable(Some(99)),
            Err("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }

    #[test]
    fn revert_attach_restores_fake_state() {
        let outcome = disable(Some(2)).unwrap();
        let AttachOutcome::Applied(change) = outcome else {
            panic!("disable must be applied");
        };
        assert_eq!(revert_attach(&change), Ok(()));
    }

    #[test]
    fn sleep_returns_all_labels() {
        let expected = vec![
            "RMOD Fake Monitor 1 [:1]".to_string(),
            "RMOD Fake Monitor 2 [:2]".to_string(),
        ];
        assert_eq!(sleep_monitor(), Ok(expected));
    }

    #[test]
    fn wake_returns_all_labels() {
        let expected = vec![
            "RMOD Fake Monitor 1 [:1]".to_string(),
            "RMOD Fake Monitor 2 [:2]".to_string(),
        ];
        assert_eq!(wake_monitor(), Ok(expected));
    }

    #[test]
    fn enumerate_all_devices_matches_attached_devices() {
        assert_eq!(
            enumerate_all_devices(),
            vec![r"\\.\DISPLAY1".to_string(), r"\\.\DISPLAY2".to_string()]
        );
    }

    #[test]
    fn set_brightness_primary_auto_uses_ddc() {
        let outcome = set_brightness(None, 30, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 1 [:1]");
        assert_eq!(outcome.backend, BrightnessBackend::Ddc);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_already_at_is_unchanged() {
        let outcome = set_brightness(None, 60, None).unwrap();
        assert_eq!(outcome.backend, BrightnessBackend::Ddc);
        assert!(outcome.unchanged);
    }

    #[test]
    fn set_brightness_second_monitor_auto_falls_back_to_gamma() {
        let outcome = set_brightness(Some(2), 30, None).unwrap();
        assert_eq!(outcome.display, "RMOD Fake Monitor 2 [:2]");
        assert_eq!(outcome.backend, BrightnessBackend::Gamma);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_forced_unsupported_backend_is_error() {
        assert_eq!(
            set_brightness(Some(2), 30, Some(BrightnessBackend::Ddc)).err(),
            Some("RMOD Fake Monitor 2 [:2] does not support ddc brightness control".to_string())
        );
        assert_eq!(
            set_brightness(Some(2), 30, Some(BrightnessBackend::Slider)).err(),
            Some("RMOD Fake Monitor 2 [:2] does not support slider brightness control".to_string())
        );
    }

    #[test]
    fn set_brightness_forced_gamma_on_ddc_monitor_applies() {
        let outcome = set_brightness(Some(1), 30, Some(BrightnessBackend::Gamma)).unwrap();
        assert_eq!(outcome.backend, BrightnessBackend::Gamma);
        assert!(!outcome.unchanged);
    }

    #[test]
    fn set_brightness_unknown_monitor_is_error() {
        assert_eq!(
            set_brightness(Some(99), 30, None).err(),
            Some("monitor 99 not found. run rmod list to see connected displays".to_string())
        );
    }
}
