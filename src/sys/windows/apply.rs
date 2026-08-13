//! Mode-application backend for the `max` and `set` commands.
//!
//! Picks the highest-resolution supported mode (`max`) or applies a
//! requested resolution/refresh (`set`), tests it with a dry run, then
//! applies it and persists it to the registry.

use super::bindings::{
    encode_wide, ChangeDisplaySettingsExW, DEVMODEW, CDS_TEST, CDS_UPDATEREGISTRY,
    DISP_CHANGE_BADDUALVIEW, DISP_CHANGE_BADFLAGS, DISP_CHANGE_BADMODE, DISP_CHANGE_BADPARAM,
    DISP_CHANGE_FAILED, DISP_CHANGE_NOTUPDATED, DISP_CHANGE_RESTART, DISP_CHANGE_SUCCESSFUL,
    DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH,
};
use super::capabilities::{self, Mode};
use super::query;

/// Refresh rate handling for the set command.
#[derive(Debug, PartialEq)]
pub enum Refresh {
    /// Leave the refresh rate unchanged.
    Keep,
    /// Use the highest refresh rate supported at the requested resolution.
    Max,
    /// Use an explicit refresh rate.
    Fixed(u32),
}

/// Applies a resolution and refresh policy to a display.
///
/// `monitor` is the 1-based number from `ls`; `None` selects the primary.
///
/// # Errors
/// Unknown monitor, no matching mode for `@max`, or a mode the display
/// rejects.
pub fn set(monitor: Option<u32>, width: u32, height: u32, refresh: Refresh) -> Result<Mode, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let base = query::current_mode(&name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let modes = capabilities::enumerate_modes(&name);
    let refresh =
        resolve_refresh(refresh, &modes, width, height, base.dm_display_frequency, index as u32 + 1)?;
    let mode = Mode { width, height, refresh };
    let devmode = build_devmode(&mode, &base);
    apply_mode(&name, &devmode)?;
    Ok(mode)
}

/// Applies the best supported mode to a monitor and returns it.
///
/// `monitor` is the 1-based number from [`super::list`]; `None` selects the
/// primary display. The mode is validated with `CDS_TEST` before being
/// applied and written to the registry.
///
/// # Errors
/// Returns `Err` for an unknown monitor number, no supported modes, or a
/// rejected display change.
pub fn max(monitor: Option<u32>) -> Result<Mode, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let best = best_mode(capabilities::enumerate_modes(&name))
        .ok_or_else(|| format!("no supported modes found for monitor {}", index + 1))?;
    let base = query::current_mode(&name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let devmode = build_devmode(&best, &base);
    apply_mode(&name, &devmode)?;
    Ok(best)
}

/// Validates a mode with a dry run, then applies and persists it.
fn apply_mode(name: &str, devmode: &DEVMODEW) -> Result<(), String> {
    let name_ptr = encode_wide(name);
    let test = unsafe {
        ChangeDisplaySettingsExW(name_ptr.as_ptr(), devmode, 0, CDS_TEST, std::ptr::null())
    };
    if test != DISP_CHANGE_SUCCESSFUL {
        return Err(format!("failed to apply mode: {}", describe_change_result(test)));
    }
    let applied = unsafe {
        ChangeDisplaySettingsExW(
            name_ptr.as_ptr(),
            devmode,
            0,
            CDS_UPDATEREGISTRY,
            std::ptr::null(),
        )
    };
    if applied != DISP_CHANGE_SUCCESSFUL {
        return Err(format!(
            "failed to apply mode: {}",
            describe_change_result(applied)
        ));
    }
    Ok(())
}

fn describe_change_result(code: i32) -> String {
    match code {
        DISP_CHANGE_SUCCESSFUL => "success".to_string(),
        DISP_CHANGE_RESTART => "a restart is required to apply this mode".to_string(),
        DISP_CHANGE_BADMODE => "the display does not support this mode".to_string(),
        DISP_CHANGE_FAILED => "the display change failed".to_string(),
        DISP_CHANGE_NOTUPDATED => "the display settings were not updated".to_string(),
        DISP_CHANGE_BADFLAGS | DISP_CHANGE_BADPARAM | DISP_CHANGE_BADDUALVIEW => {
            "invalid parameters".to_string()
        }
        _ => format!("unknown error ({code})"),
    }
}

fn build_devmode(mode: &Mode, current: &DEVMODEW) -> DEVMODEW {
    let mut devmode = *current;
    devmode.dm_pels_width = mode.width;
    devmode.dm_pels_height = mode.height;
    devmode.dm_display_frequency = mode.refresh;
    devmode.dm_fields |= DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    devmode.dm_size = std::mem::size_of::<DEVMODEW>() as u16;
    devmode.dm_driver_extra = 0;
    devmode
}

fn best_mode(modes: Vec<Mode>) -> Option<Mode> {
    capabilities::normalize_modes(modes).pop()
}

fn best_refresh(modes: &[Mode], width: u32, height: u32) -> Option<u32> {
    modes
        .iter()
        .filter(|m| m.width == width && m.height == height)
        .map(|m| m.refresh)
        .max()
}

fn resolve_refresh(
    policy: Refresh,
    modes: &[Mode],
    width: u32,
    height: u32,
    current_refresh: u32,
    monitor_number: u32,
) -> Result<u32, String> {
    match policy {
        Refresh::Keep => Ok(current_refresh),
        Refresh::Fixed(r) => Ok(r),
        Refresh::Max => best_refresh(modes, width, height)
            .ok_or_else(|| format!("monitor {monitor_number} does not support {width}x{height}")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::bindings::POINTL;
    use super::*;

    #[test]
    fn describe_change_result_maps_every_disp_change_code() {
        assert_eq!(describe_change_result(DISP_CHANGE_SUCCESSFUL), "success");
        assert_eq!(
            describe_change_result(DISP_CHANGE_RESTART),
            "a restart is required to apply this mode"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_BADMODE),
            "the display does not support this mode"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_FAILED),
            "the display change failed"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_NOTUPDATED),
            "the display settings were not updated"
        );
        assert_eq!(describe_change_result(DISP_CHANGE_BADFLAGS), "invalid parameters");
        assert_eq!(describe_change_result(DISP_CHANGE_BADPARAM), "invalid parameters");
        assert_eq!(
            describe_change_result(DISP_CHANGE_BADDUALVIEW),
            "invalid parameters"
        );
    }

    #[test]
    fn describe_change_result_unknown_code() {
        assert_eq!(describe_change_result(12345), "unknown error (12345)");
    }

    #[test]
    fn build_devmode_sets_mode_fields_and_flags() {
        let mode = Mode {
            width: 3840,
            height: 2160,
            refresh: 144,
        };
        let mut current: DEVMODEW = unsafe { std::mem::zeroed() };
        current.dm_position = POINTL { x: -1, y: -1 };
        let devmode = build_devmode(&mode, &current);
        assert_eq!(devmode.dm_pels_width, 3840);
        assert_eq!(devmode.dm_pels_height, 2160);
        assert_eq!(devmode.dm_display_frequency, 144);
        assert_eq!(devmode.dm_size, 220);
        assert_eq!(devmode.dm_driver_extra, 0);
        assert_eq!(devmode.dm_fields, DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY);
        assert_eq!(devmode.dm_position.x, -1);
        assert_eq!(devmode.dm_position.y, -1);
    }

    #[test]
    fn best_mode_empty_returns_none() {
        assert_eq!(best_mode(Vec::new()), None);
    }

    #[test]
    fn best_mode_picks_highest_resolution() {
        let modes = vec![
            Mode {
                width: 1920,
                height: 1080,
                refresh: 60,
            },
            Mode {
                width: 2560,
                height: 1440,
                refresh: 75,
            },
            Mode {
                width: 3840,
                height: 2160,
                refresh: 60,
            },
        ];
        assert_eq!(
            best_mode(modes),
            Some(Mode {
                width: 3840,
                height: 2160,
                refresh: 60,
            })
        );
    }

    #[test]
    fn best_mode_picks_highest_refresh_at_same_resolution() {
        let modes = vec![
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
        ];
        assert_eq!(
            best_mode(modes),
            Some(Mode {
                width: 1920,
                height: 1080,
                refresh: 144,
            })
        );
    }

    #[test]
    fn best_mode_single_mode_passes_through() {
        let modes = vec![Mode {
            width: 1024,
            height: 768,
            refresh: 60,
        }];
        assert_eq!(
            best_mode(modes),
            Some(Mode {
                width: 1024,
                height: 768,
                refresh: 60,
            })
        );
    }

    #[test]
    fn best_refresh_picks_highest_at_matching_resolution() {
        let modes = vec![
            Mode { width: 1920, height: 1080, refresh: 60 },
            Mode { width: 1920, height: 1080, refresh: 120 },
            Mode { width: 1920, height: 1080, refresh: 144 },
            Mode { width: 2560, height: 1440, refresh: 240 },
        ];
        assert_eq!(best_refresh(&modes, 1920, 1080), Some(144));
    }

    #[test]
    fn best_refresh_ignores_other_resolutions() {
        let modes = vec![
            Mode { width: 1920, height: 1080, refresh: 60 },
            Mode { width: 2560, height: 1440, refresh: 240 },
        ];
        assert_eq!(best_refresh(&modes, 1920, 1080), Some(60));
    }

    #[test]
    fn best_refresh_no_matching_resolution_returns_none() {
        let modes = vec![Mode { width: 2560, height: 1440, refresh: 144 }];
        assert_eq!(best_refresh(&modes, 1920, 1080), None);
    }

    #[test]
    fn resolve_refresh_keep_returns_current_refresh() {
        let modes = vec![Mode { width: 1920, height: 1080, refresh: 144 }];
        assert_eq!(resolve_refresh(Refresh::Keep, &modes, 1920, 1080, 59, 1), Ok(59));
    }

    #[test]
    fn resolve_refresh_fixed_returns_the_value() {
        let modes = vec![Mode { width: 1920, height: 1080, refresh: 60 }];
        assert_eq!(
            resolve_refresh(Refresh::Fixed(75), &modes, 1920, 1080, 59, 1),
            Ok(75)
        );
    }

    #[test]
    fn resolve_refresh_max_picks_best_matching_mode() {
        let modes = vec![
            Mode { width: 1920, height: 1080, refresh: 60 },
            Mode { width: 1920, height: 1080, refresh: 144 },
        ];
        assert_eq!(resolve_refresh(Refresh::Max, &modes, 1920, 1080, 60, 1), Ok(144));
    }

    #[test]
    fn resolve_refresh_max_no_matching_mode_is_error() {
        let modes = vec![Mode { width: 2560, height: 1440, refresh: 60 }];
        assert_eq!(
            resolve_refresh(Refresh::Max, &modes, 320, 200, 60, 1),
            Err("monitor 1 does not support 320x200".to_string())
        );
    }

    #[test]
    fn apply_mode_accepts_current_mode() {
        let names = query::enumerate_devices();
        if names.is_empty() {
            return;
        }
        let Some(current) = query::current_mode(&names[0]) else {
            return;
        };
        assert_eq!(apply_mode(&names[0], &current), Ok(()));
    }

    #[test]
    fn apply_mode_rejects_unsupported_mode() {
        let names = query::enumerate_devices();
        if names.is_empty() {
            return;
        }
        let base = query::current_mode(&names[0]).unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let devmode = build_devmode(
            &Mode { width: 1, height: 1, refresh: 1 },
            &base,
        );
        assert_eq!(
            apply_mode(&names[0], &devmode),
            Err("failed to apply mode: the display does not support this mode".to_string())
        );
    }
}
