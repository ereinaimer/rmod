//! Mode-application backend for the `set` command and layout promotion.
//!
//! Picks the highest-resolution supported mode (`max`) or applies a
//! requested resolution/refresh (`set`), tests it with a dry run, then
//! applies it and persists it to the registry. A mode that is already
//! active is reported as unchanged and never re-applied. `make_main`
//! swaps desktop positions so a display becomes the primary (origin 0,0).

use super::bindings::{
    CDS_TEST, CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DISP_CHANGE_BADDUALVIEW,
    DISP_CHANGE_BADFLAGS, DISP_CHANGE_BADMODE, DISP_CHANGE_BADPARAM, DISP_CHANGE_FAILED,
    DISP_CHANGE_NOTUPDATED, DISP_CHANGE_RESTART, DISP_CHANGE_SUCCESSFUL, DM_DISPLAYFREQUENCY,
    DM_DISPLAYORIENTATION, DM_PELSHEIGHT, DM_PELSWIDTH, DM_POSITION, DevmodeW, Pointl, encode_wide,
};
use super::capabilities::{self, Mode};
use super::fade;
use super::query;

/// Refresh rate handling for the set command.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Refresh {
    /// Leave the refresh rate unchanged.
    Keep,
    /// Use the highest refresh rate supported at the requested resolution.
    Max,
    /// Use an explicit refresh rate.
    Fixed(u32),
}

/// A display change: the applied mode, the mode it replaced, and the
/// monitor the change applies to.
#[derive(Debug, PartialEq)]
pub struct Change {
    /// The 1-based monitor number the change applies to.
    pub monitor: u32,
    /// The display label used in batch output.
    pub display: String,
    /// The mode that was applied.
    pub mode: Mode,
    /// The mode in effect before the change.
    pub previous: Mode,
    /// The requested rotation angle in degrees; `None` when the change
    /// does not include an orientation request.
    pub orientation: Option<u32>,
    /// The rotation angle in degrees in effect before the change;
    /// `None` when the change does not include an orientation request.
    pub previous_orientation: Option<u32>,
}

/// The result of applying a mode policy to a display.
#[derive(Debug, PartialEq)]
pub enum ApplyOutcome {
    /// The mode was applied and can be reverted with its previous mode.
    Applied(Change),
    /// The requested mode was already active; nothing was applied.
    Unchanged(Change),
}

/// A display that is being promoted to main display.
#[derive(Debug, PartialEq)]
pub struct MainChange<'a> {
    /// The 1-based monitor number of the promoted display.
    pub monitor: u32,
    /// The display label of the promoted display.
    pub display: String,
    /// The `(device, devmode)` pairs to apply, in order: the promoted
    /// display moved to origin 0,0, then the old primary moved to the
    /// promoted display's previous position.
    pub applied: Vec<(&'a str, DevmodeW)>,
    /// The original devmodes for revert, in order: the old primary back
    /// to origin 0,0, then the promoted display back to its old position.
    pub previous: Vec<(&'a str, DevmodeW)>,
}

/// The result of promoting a display to main display.
#[derive(Debug, PartialEq)]
pub enum MainOutcome<'a> {
    /// The display was promoted and can be reverted with the change.
    Applied(MainChange<'a>),
    /// The display was already the main display; nothing was applied.
    Unchanged(String),
}

/// Builds the outcome for an attempted change; identical modes and a
/// matching orientation produce [`ApplyOutcome::Unchanged`].
fn outcome_of(
    monitor: u32,
    display: String,
    mode: Mode,
    previous: Mode,
    orientation: Option<u32>,
    previous_orientation: Option<u32>,
) -> ApplyOutcome {
    let change = Change {
        monitor,
        display,
        mode,
        previous,
        orientation,
        previous_orientation,
    };
    let orientation_matches = match (change.orientation, change.previous_orientation) {
        (Some(angle), Some(previous_angle)) => dmdo(angle) == dmdo(previous_angle),
        _ => true,
    };
    if change.mode == change.previous && orientation_matches {
        ApplyOutcome::Unchanged(change)
    } else {
        ApplyOutcome::Applied(change)
    }
}

/// Maps a rotation angle in degrees to its display orientation value.
fn dmdo(angle: u32) -> u32 {
    match angle {
        0 => 0,
        90 => 1,
        180 => 2,
        270 => 3,
        _ => unreachable!(),
    }
}

/// Maps a display orientation value back to its angle in degrees.
fn angle_of(orientation: u32) -> u32 {
    match orientation {
        0 => 0,
        1 => 90,
        2 => 180,
        _ => 270,
    }
}

/// The display orientation value in effect for a device mode.
fn orientation_of(devmode: &DevmodeW) -> u32 {
    devmode.dm_display_orientation
}

/// Applies a resolution, refresh and rotation policy to a display.
///
/// `monitor` is the 1-based number from `ls`; `None` selects the primary.
/// `orientation` is a rotation angle in degrees (0/90/180/270); `None`
/// leaves the current orientation untouched. Returns
/// [`ApplyOutcome::Unchanged`] when the requested mode and orientation are
/// already active.
///
/// # Errors
/// Unknown monitor, no matching mode for `@max`, or a mode the display
/// rejects.
pub fn set(
    monitor: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<ApplyOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let (width, height) = effective_dims(width, height, orientation, &base);
    let modes = capabilities::enumerate_modes(name);
    let refresh = resolve_refresh(
        refresh,
        &modes,
        width,
        height,
        base.dm_display_frequency,
        &display,
    )?;
    let mode = Mode {
        width,
        height,
        refresh,
    };
    let previous = mode_of(&base);
    let previous_orientation = orientation.map(|_| angle_of(orientation_of(&base)));
    let result = outcome_of(
        index as u32 + 1,
        display,
        mode,
        previous,
        orientation,
        previous_orientation,
    );
    if let ApplyOutcome::Applied(change) = &result {
        let devmode = build_devmode(&change.mode, &base, change.orientation);
        fade::transition(name, &devmode, || {
            apply_mode(name, &change.display, &devmode)
        })?;
    }
    Ok(result)
}

/// Applies the best supported mode to a monitor and returns the outcome.
///
/// `monitor` is the 1-based number from [`super::list`]; `None` selects the
/// primary display. The mode is validated with `CDS_TEST` before being
/// applied and written to the registry. Returns
/// [`ApplyOutcome::Unchanged`] when the display is already at its best
/// mode.
///
/// # Errors
/// Returns `Err` for an unknown monitor number, no supported modes, or a
/// rejected display change.
pub fn max(monitor: Option<u32>, orientation: Option<u32>) -> Result<ApplyOutcome, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    let best = best_mode(capabilities::enumerate_modes(name))
        .ok_or_else(|| format!("{display} has no supported modes"))?;
    let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let previous = mode_of(&base);
    let previous_orientation = orientation_of(&base);
    let result = outcome_of(
        index as u32 + 1,
        display,
        best,
        previous,
        orientation,
        Some(previous_orientation),
    );
    if let ApplyOutcome::Applied(change) = &result {
        let devmode = build_devmode(&change.mode, &base, orientation);
        fade::transition(name, &devmode, || {
            apply_mode(name, &change.display, &devmode)
        })?;
    }
    Ok(result)
}

/// Applies a resolution, refresh and rotation policy to every attached
/// display.
///
/// Every monitor is dry-run validated before anything is applied; when any
/// display rejects the mode, nothing changes and the failures are listed.
/// Monitors already at the requested mode and orientation are reported as
/// unchanged and left untouched.
///
/// # Errors
/// No displays found, a mode no display supports, or preflight failures.
#[allow(dead_code)]
pub fn set_all(
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
) -> Result<Vec<ApplyOutcome>, String> {
    let names = query::enumerate_devices();
    let targets = query::resolve_all(&names)?;
    apply_all(plan_set(&targets, width, height, refresh, orientation)?)
}

/// Applies the best supported mode to every attached display.
///
/// Every monitor is dry-run validated before anything is applied; when any
/// display rejects its best mode, nothing changes and the failures are
/// listed. Monitors already at their best mode are reported as unchanged
/// and left untouched.
///
/// # Errors
/// No displays found, a display with no supported modes, or preflight
/// failures.
pub fn max_all(orientation: Option<u32>) -> Result<Vec<ApplyOutcome>, String> {
    let names = query::enumerate_devices();
    let targets = query::resolve_all(&names)?;
    apply_all(plan_max(&targets, orientation)?)
}

/// Re-applies a previously captured mode to undo a display change.
///
/// `monitor` is the 1-based number from `ls`; `None` selects the primary.
/// `previous` is the `previous` field of the [`Change`] returned when the
/// mode was applied; it is applied over the current settings and returned
/// on success. `previous_orientation`, when `Some`, is the rotation angle
/// in effect before the change and is restored along with the mode.
///
/// # Errors
/// Unknown monitor or a mode the display rejects.
#[allow(dead_code)]
pub fn revert(
    monitor: Option<u32>,
    previous: Mode,
    previous_orientation: Option<u32>,
) -> Result<Mode, String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let display = query::display_label(name, index as u32 + 1);
    let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let devmode = build_devmode(&previous, &base, previous_orientation);
    fade::transition(name, &devmode, || apply_mode(name, &display, &devmode))?;
    Ok(previous)
}

/// Makes the monitor with the 1-based number `monitor` the main display by
/// swapping desktop positions with the current primary.
///
/// The promoted display's current mode is moved to origin 0,0 and the old
/// primary takes its previous position; both changes are applied in that
/// order and persisted to the registry. Returns
/// [`MainOutcome::Unchanged`] when the display is already the main
/// display.
///
/// # Errors
/// Unknown monitor number or a rejected display change.
#[allow(dead_code)]
pub fn make_main(monitor: u32, names: &[String]) -> Result<MainOutcome<'_>, String> {
    let (target_index, target_name) = query::resolve_device(Some(monitor), names)?;
    let (_, partner_name) = query::resolve_device(None, names)?;
    let display = query::display_label(target_name, target_index as u32 + 1);
    let target_dev =
        query::current_mode(target_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let partner_dev =
        query::current_mode(partner_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    if is_primary(&target_dev) {
        return Ok(MainOutcome::Unchanged(display));
    }
    let (new_primary, new_partner) = build_swap(&target_dev, &partner_dev);
    let applied = vec![(target_name, new_primary), (partner_name, new_partner)];
    for (name, devmode) in &applied {
        apply_position(name, devmode)?;
    }
    let previous = vec![(partner_name, partner_dev), (target_name, target_dev)];
    Ok(MainOutcome::Applied(MainChange {
        monitor,
        display,
        applied,
        previous,
    }))
}

/// Undoes a promotion by re-applying the original positions captured in a
/// [`MainChange`]: the old primary back to origin 0,0, then the promoted
/// display back to its previous position.
///
/// # Errors
/// A rejected display change.
#[allow(dead_code)]
pub fn revert_main(change: &MainChange<'_>) -> Result<(), String> {
    for (name, devmode) in &change.previous {
        apply_position(name, devmode)?;
    }
    Ok(())
}

/// True when a device mode sits at desktop origin (0,0), the definition
/// of the main display.
fn is_primary(devmode: &DevmodeW) -> bool {
    devmode.dm_position.x == 0 && devmode.dm_position.y == 0
}

/// Builds the two devmodes of a primary swap: the target moved to origin
/// 0,0 and the current primary moved to the target's previous position.
/// Both devmodes gain the `DM_POSITION` field flag; everything else is
/// copied through unchanged and the inputs are not modified.
fn build_swap(target: &DevmodeW, partner: &DevmodeW) -> (DevmodeW, DevmodeW) {
    let mut new_primary = *target;
    new_primary.dm_position = Pointl { x: 0, y: 0 };
    new_primary.dm_fields |= DM_POSITION;
    let mut new_partner = *partner;
    new_partner.dm_position = target.dm_position;
    new_partner.dm_fields |= DM_POSITION;
    (new_primary, new_partner)
}

/// Applies a position change to a device and persists it; positions have
/// no dry-run validation, so the change is applied directly.
pub(crate) fn apply_position(name: &str, devmode: &DevmodeW) -> Result<(), String> {
    let name_ptr = encode_wide(name);
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
        return Err(describe_change_result(applied));
    }
    Ok(())
}

/// Validates a mode with a dry run, then applies and persists it.
fn apply_mode(name: &str, display: &str, devmode: &DevmodeW) -> Result<(), String> {
    validate_mode(name, display, devmode)?;
    let name_ptr = encode_wide(name);
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
        return Err(describe_change_failure(applied, display, devmode));
    }
    Ok(())
}

/// Runs the CDS_TEST dry run for a mode; returns an error description when
/// the display rejects it.
fn validate_mode(name: &str, display: &str, devmode: &DevmodeW) -> Result<(), String> {
    let name_ptr = encode_wide(name);
    let test = unsafe {
        ChangeDisplaySettingsExW(name_ptr.as_ptr(), devmode, 0, CDS_TEST, std::ptr::null())
    };
    if test != DISP_CHANGE_SUCCESSFUL {
        return Err(describe_change_failure(test, display, devmode));
    }
    Ok(())
}

/// Describes a rejected display change; a bad mode names the display and
/// the attempted resolution and refresh rate.
fn describe_change_failure(code: i32, display: &str, devmode: &DevmodeW) -> String {
    if code == DISP_CHANGE_BADMODE {
        return format!(
            "{display} does not support {}x{}@{}Hz",
            devmode.dm_pels_width, devmode.dm_pels_height, devmode.dm_display_frequency
        );
    }
    describe_change_result(code)
}

fn describe_change_result(code: i32) -> String {
    match code {
        DISP_CHANGE_SUCCESSFUL => "success".to_string(),
        DISP_CHANGE_RESTART => "a restart is required to apply this mode".to_string(),
        DISP_CHANGE_FAILED => "the display change failed".to_string(),
        DISP_CHANGE_NOTUPDATED => "the display settings were not updated".to_string(),
        DISP_CHANGE_BADFLAGS | DISP_CHANGE_BADPARAM | DISP_CHANGE_BADDUALVIEW => {
            "invalid parameters".to_string()
        }
        _ => format!("unknown error ({code})"),
    }
}

fn mode_of(devmode: &DevmodeW) -> Mode {
    Mode {
        width: devmode.dm_pels_width,
        height: devmode.dm_pels_height,
        refresh: devmode.dm_display_frequency,
    }
}

fn build_devmode(mode: &Mode, current: &DevmodeW, orientation: Option<u32>) -> DevmodeW {
    let mut devmode = *current;
    devmode.dm_pels_width = mode.width;
    devmode.dm_pels_height = mode.height;
    devmode.dm_display_frequency = mode.refresh;
    devmode.dm_fields |= DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
    if let Some(angle) = orientation {
        devmode.dm_display_orientation = dmdo(angle);
        devmode.dm_fields |= DM_DISPLAYORIENTATION;
    }
    devmode.dm_size = std::mem::size_of::<DevmodeW>() as u16;
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
    display: &str,
) -> Result<u32, String> {
    match policy {
        Refresh::Keep => Ok(current_refresh),
        Refresh::Fixed(r) => Ok(r),
        Refresh::Max => best_refresh(modes, width, height)
            .ok_or_else(|| format!("{display} does not support {width}x{height}")),
    }
}

/// A planned change for one monitor: everything needed to validate and
/// apply a mode and report the resulting outcome.
struct Planned<'a> {
    name: &'a str,
    devmode: DevmodeW,
    outcome: ApplyOutcome,
}

/// Resolves optional dimensions against a display's current mode.
///
/// `None` keeps the corresponding current value from `base`.
#[cfg(test)]
fn resolve_dims(width: Option<u32>, height: Option<u32>, base: &DevmodeW) -> (u32, u32) {
    (
        width.unwrap_or(base.dm_pels_width),
        height.unwrap_or(base.dm_pels_height),
    )
}

/// True when hardware-touching tests should run: `RMOD_HW_TEST` is `"1"`.
#[cfg(test)]
fn hw_tests_enabled_for(value: Option<&str>) -> bool {
    value == Some("1")
}

/// True when `RMOD_HW_TEST` is set to `"1"`, gating the tests that
/// re-apply modes to the host display.
#[cfg(test)]
fn hw_tests_enabled() -> bool {
    hw_tests_enabled_for(std::env::var("RMOD_HW_TEST").ok().as_deref())
}

/// The effective dimensions for an orientation request.
///
/// Dimensions are resolved against the display's physical panel size
/// (the current mode rotated back to landscape) and then swapped when
/// the request rotates the display 90° or 270°.
pub(crate) fn effective_dims(
    width: Option<u32>,
    height: Option<u32>,
    orientation: Option<u32>,
    base: &DevmodeW,
) -> (u32, u32) {
    let (w, h) = match base.dm_display_orientation {
        1 | 3 => (base.dm_pels_height, base.dm_pels_width),
        _ => (base.dm_pels_width, base.dm_pels_height),
    };
    let w = width.unwrap_or(w);
    let h = height.unwrap_or(h);
    match orientation {
        Some(90 | 270) => (h, w),
        _ => (w, h),
    }
}

#[allow(dead_code)]
fn plan_set<'a>(
    targets: &'a [(usize, &'a str)],
    width: Option<u32>,
    height: Option<u32>,
    policy: Refresh,
    orientation: Option<u32>,
) -> Result<Vec<Planned<'a>>, String> {
    let mut planned = Vec::new();
    for (index, name) in targets {
        let display = query::display_label(name, *index as u32 + 1);
        let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let (width, height) = effective_dims(width, height, orientation, &base);
        let modes = capabilities::enumerate_modes(name);
        let refresh = resolve_refresh(
            policy,
            &modes,
            width,
            height,
            base.dm_display_frequency,
            &display,
        )?;
        let mode = Mode {
            width,
            height,
            refresh,
        };
        let previous = mode_of(&base);
        let previous_orientation = orientation.map(|_| angle_of(orientation_of(&base)));
        let devmode = build_devmode(&mode, &base, orientation);
        let outcome = outcome_of(
            *index as u32 + 1,
            display,
            mode,
            previous,
            orientation,
            previous_orientation,
        );
        planned.push(Planned {
            name,
            devmode,
            outcome,
        });
    }
    Ok(planned)
}

fn plan_max<'a>(
    targets: &'a [(usize, &'a str)],
    orientation: Option<u32>,
) -> Result<Vec<Planned<'a>>, String> {
    let mut planned = Vec::new();
    let mut failures = Vec::new();
    for (index, name) in targets {
        let display = query::display_label(name, *index as u32 + 1);
        let Some(mode) = best_mode(capabilities::enumerate_modes(name)) else {
            failures.push(format!("{display} has no supported modes"));
            continue;
        };
        let base = query::current_mode(name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let previous = mode_of(&base);
        let previous_orientation = orientation_of(&base);
        let devmode = build_devmode(&mode, &base, orientation);
        let outcome = outcome_of(
            *index as u32 + 1,
            display,
            mode,
            previous,
            orientation,
            Some(previous_orientation),
        );
        planned.push(Planned {
            name,
            devmode,
            outcome,
        });
    }
    if failures.is_empty() {
        Ok(planned)
    } else {
        Err(failures.join("\n"))
    }
}

/// True when a plan contains at least one mode to apply; a batch with no
/// changes must not fade.
fn has_applied(planned: &[Planned<'_>]) -> bool {
    planned
        .iter()
        .any(|p| matches!(p.outcome, ApplyOutcome::Applied(_)))
}

fn apply_planned(planned: Vec<Planned<'_>>) -> Result<Vec<ApplyOutcome>, String> {
    let mut outcomes = Vec::with_capacity(planned.len());
    for p in planned {
        if let ApplyOutcome::Applied(change) = &p.outcome {
            apply_mode(p.name, &change.display, &p.devmode)?;
        }
        outcomes.push(p.outcome);
    }
    Ok(outcomes)
}

fn apply_all(planned: Vec<Planned<'_>>) -> Result<Vec<ApplyOutcome>, String> {
    let mut failures = Vec::new();
    for p in &planned {
        let ApplyOutcome::Applied(change) = &p.outcome else {
            continue;
        };
        if let Err(e) = validate_mode(p.name, &change.display, &p.devmode) {
            failures.push(e);
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    if has_applied(&planned) {
        fade::transition_all(|| apply_planned(planned))
    } else {
        apply_planned(planned)
    }
}

#[cfg(test)]
mod tests {
    use super::super::bindings::Pointl;
    use super::*;

    #[test]
    fn describe_change_result_maps_disp_change_codes() {
        assert_eq!(describe_change_result(DISP_CHANGE_SUCCESSFUL), "success");
        assert_eq!(
            describe_change_result(DISP_CHANGE_RESTART),
            "a restart is required to apply this mode"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_FAILED),
            "the display change failed"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_NOTUPDATED),
            "the display settings were not updated"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_BADFLAGS),
            "invalid parameters"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_BADPARAM),
            "invalid parameters"
        );
        assert_eq!(
            describe_change_result(DISP_CHANGE_BADDUALVIEW),
            "invalid parameters"
        );
    }

    #[test]
    fn describe_change_failure_badmode_names_display_and_mode() {
        let devmode = build_devmode(
            &Mode {
                width: 9999,
                height: 9999,
                refresh: 1,
            },
            &unsafe { std::mem::zeroed() },
            None,
        );
        assert_eq!(
            describe_change_failure(DISP_CHANGE_BADMODE, "Generic PnP Monitor [:1]", &devmode),
            "Generic PnP Monitor [:1] does not support 9999x9999@1Hz"
        );
    }

    #[test]
    fn describe_change_failure_passes_through_other_codes() {
        let devmode = build_devmode(
            &Mode {
                width: 1920,
                height: 1080,
                refresh: 120,
            },
            &unsafe { std::mem::zeroed() },
            None,
        );
        assert_eq!(
            describe_change_failure(DISP_CHANGE_RESTART, "Generic PnP Monitor [:1]", &devmode),
            "a restart is required to apply this mode"
        );
    }

    #[test]
    fn describe_change_result_unknown_code() {
        assert_eq!(describe_change_result(12345), "unknown error (12345)");
    }

    #[test]
    fn outcome_of_identical_modes_is_unchanged() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 120,
        };
        assert_eq!(
            outcome_of(
                1,
                "Generic PnP Monitor [:1]".to_string(),
                mode,
                Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                None,
                None,
            ),
            ApplyOutcome::Unchanged(Change {
                monitor: 1,
                display: "Generic PnP Monitor [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                orientation: None,
                previous_orientation: None,
            })
        );
    }

    #[test]
    fn outcome_of_different_modes_is_applied() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 120,
        };
        let previous = Mode {
            width: 1280,
            height: 720,
            refresh: 120,
        };
        assert_eq!(
            outcome_of(
                1,
                "Generic PnP Monitor [:1]".to_string(),
                mode,
                previous,
                None,
                None
            ),
            ApplyOutcome::Applied(Change {
                monitor: 1,
                display: "Generic PnP Monitor [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                previous: Mode {
                    width: 1280,
                    height: 720,
                    refresh: 120,
                },
                orientation: None,
                previous_orientation: None,
            })
        );
    }

    #[test]
    fn build_devmode_sets_mode_fields_and_flags() {
        let mode = Mode {
            width: 3840,
            height: 2160,
            refresh: 144,
        };
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_position = Pointl { x: -1, y: -1 };
        let devmode = build_devmode(&mode, &current, None);
        assert_eq!(devmode.dm_pels_width, 3840);
        assert_eq!(devmode.dm_pels_height, 2160);
        assert_eq!(devmode.dm_display_frequency, 144);
        assert_eq!(devmode.dm_size, 220);
        assert_eq!(devmode.dm_driver_extra, 0);
        assert_eq!(
            devmode.dm_fields,
            DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY
        );
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
            Mode {
                width: 1920,
                height: 1080,
                refresh: 60,
            },
            Mode {
                width: 1920,
                height: 1080,
                refresh: 120,
            },
            Mode {
                width: 1920,
                height: 1080,
                refresh: 144,
            },
            Mode {
                width: 2560,
                height: 1440,
                refresh: 240,
            },
        ];
        assert_eq!(best_refresh(&modes, 1920, 1080), Some(144));
    }

    #[test]
    fn best_refresh_ignores_other_resolutions() {
        let modes = vec![
            Mode {
                width: 1920,
                height: 1080,
                refresh: 60,
            },
            Mode {
                width: 2560,
                height: 1440,
                refresh: 240,
            },
        ];
        assert_eq!(best_refresh(&modes, 1920, 1080), Some(60));
    }

    #[test]
    fn best_refresh_no_matching_resolution_returns_none() {
        let modes = vec![Mode {
            width: 2560,
            height: 1440,
            refresh: 144,
        }];
        assert_eq!(best_refresh(&modes, 1920, 1080), None);
    }

    #[test]
    fn resolve_refresh_keep_returns_current_refresh() {
        let modes = vec![Mode {
            width: 1920,
            height: 1080,
            refresh: 144,
        }];
        assert_eq!(
            resolve_refresh(
                Refresh::Keep,
                &modes,
                1920,
                1080,
                59,
                "Generic PnP Monitor [:1]"
            ),
            Ok(59)
        );
    }

    #[test]
    fn resolve_refresh_fixed_returns_the_value() {
        let modes = vec![Mode {
            width: 1920,
            height: 1080,
            refresh: 60,
        }];
        assert_eq!(
            resolve_refresh(
                Refresh::Fixed(75),
                &modes,
                1920,
                1080,
                59,
                "Generic PnP Monitor [:1]"
            ),
            Ok(75)
        );
    }

    #[test]
    fn resolve_refresh_max_picks_best_matching_mode() {
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
            resolve_refresh(
                Refresh::Max,
                &modes,
                1920,
                1080,
                60,
                "Generic PnP Monitor [:1]"
            ),
            Ok(144)
        );
    }

    #[test]
    fn resolve_refresh_max_no_matching_mode_is_error() {
        let modes = vec![Mode {
            width: 2560,
            height: 1440,
            refresh: 60,
        }];
        assert_eq!(
            resolve_refresh(
                Refresh::Max,
                &modes,
                320,
                200,
                60,
                "Generic PnP Monitor [:1]"
            ),
            Err("Generic PnP Monitor [:1] does not support 320x200".to_string())
        );
    }

    #[test]
    fn hw_tests_enabled_for_one_is_true() {
        assert!(hw_tests_enabled_for(Some("1")));
    }

    #[test]
    fn hw_tests_enabled_for_zero_is_false() {
        assert!(!hw_tests_enabled_for(Some("0")));
    }

    #[test]
    fn hw_tests_enabled_for_none_is_false() {
        assert!(!hw_tests_enabled_for(None));
    }

    #[test]
    fn apply_mode_accepts_current_mode() {
        // Skipped by default so `cargo test` never touches the display; run
        // with `RMOD_HW_TEST=1` in a hardware lab.
        if !hw_tests_enabled() {
            return;
        }
        let names = query::enumerate_devices();
        if names.is_empty() {
            return;
        }
        let Some(current) = query::current_mode(&names[0]) else {
            return;
        };
        let result = apply_mode(&names[0], &query::display_label(&names[0], 1), &current);
        assert!(result.is_ok() || result.unwrap_err().contains("the display change failed"));
    }

    #[test]
    fn apply_mode_rejects_unsupported_mode() {
        // Skipped by default so `cargo test` never touches the display; run
        // with `RMOD_HW_TEST=1` in a hardware lab.
        if !hw_tests_enabled() {
            return;
        }
        let names = query::enumerate_devices();
        if names.is_empty() {
            return;
        }
        let base = query::current_mode(&names[0]).unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let devmode = build_devmode(
            &Mode {
                width: 1,
                height: 1,
                refresh: 1,
            },
            &base,
            None,
        );
        let result = apply_mode(&names[0], &query::display_label(&names[0], 1), &devmode);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("does not support 1x1@1Hz") || err.contains("the display change failed")
        );
    }

    #[test]
    fn resolve_dims_none_none_uses_current() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(resolve_dims(None, None, &base), (1920, 1080));
    }

    #[test]
    fn resolve_dims_width_only_keeps_current_height() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(resolve_dims(Some(2560), None, &base), (2560, 1080));
    }

    #[test]
    fn resolve_dims_height_only_keeps_current_width() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(resolve_dims(None, Some(1440), &base), (1920, 1440));
    }

    #[test]
    fn resolve_dims_both_passthrough() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(resolve_dims(Some(2560), Some(1440), &base), (2560, 1440));
    }

    #[test]
    fn effective_dims_no_orientation_matches_resolve_dims() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(effective_dims(None, None, None, &base), (1920, 1080));
    }

    #[test]
    fn effective_dims_landscape_base_rotates_90() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(effective_dims(None, None, Some(90), &base), (1080, 1920));
    }

    #[test]
    fn effective_dims_landscape_base_rotates_270() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(effective_dims(None, None, Some(270), &base), (1080, 1920));
    }

    #[test]
    fn effective_dims_landscape_base_stays_for_0_and_180() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(effective_dims(None, None, Some(0), &base), (1920, 1080));
        assert_eq!(effective_dims(None, None, Some(180), &base), (1920, 1080));
    }

    #[test]
    fn effective_dims_rotated_base_keeps_effective_for_90() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1080;
        base.dm_pels_height = 1920;
        base.dm_display_orientation = 1;
        assert_eq!(effective_dims(None, None, Some(90), &base), (1080, 1920));
    }

    #[test]
    fn effective_dims_rotated_base_restores_landscape_for_0() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1080;
        base.dm_pels_height = 1920;
        base.dm_display_orientation = 1;
        assert_eq!(effective_dims(None, None, Some(0), &base), (1920, 1080));
    }

    #[test]
    fn effective_dims_rotated_base_restores_landscape_for_180() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1080;
        base.dm_pels_height = 1920;
        base.dm_display_orientation = 3;
        assert_eq!(effective_dims(None, None, Some(180), &base), (1920, 1080));
    }

    #[test]
    fn effective_dims_explicit_dims_are_panel_dims() {
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_pels_width = 1920;
        base.dm_pels_height = 1080;
        assert_eq!(
            effective_dims(Some(1920), Some(1080), Some(90), &base),
            (1080, 1920)
        );
    }

    #[test]
    fn dmdo_maps_angles() {
        assert_eq!(dmdo(0), 0);
        assert_eq!(dmdo(90), 1);
        assert_eq!(dmdo(180), 2);
        assert_eq!(dmdo(270), 3);
    }

    #[test]
    fn orientation_of_reads_devmode() {
        let mut devmode: DevmodeW = unsafe { std::mem::zeroed() };
        devmode.dm_display_orientation = 2;
        assert_eq!(orientation_of(&devmode), 2);
    }

    #[test]
    fn outcome_of_same_mode_and_matching_orientation_is_unchanged() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 120,
        };
        assert_eq!(
            outcome_of(
                1,
                "Generic PnP Monitor [:1]".to_string(),
                mode,
                Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                Some(90),
                Some(90),
            ),
            ApplyOutcome::Unchanged(Change {
                monitor: 1,
                display: "Generic PnP Monitor [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                orientation: Some(90),
                previous_orientation: Some(90),
            })
        );
    }

    #[test]
    fn outcome_of_same_mode_different_orientation_is_applied() {
        let mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 120,
        };
        assert_eq!(
            outcome_of(
                1,
                "Generic PnP Monitor [:1]".to_string(),
                mode,
                Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                Some(90),
                Some(0),
            ),
            ApplyOutcome::Applied(Change {
                monitor: 1,
                display: "Generic PnP Monitor [:1]".to_string(),
                mode: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                orientation: Some(90),
                previous_orientation: Some(0),
            })
        );
    }

    #[test]
    fn outcome_of_mode_difference_with_orientation_is_applied() {
        let mode = Mode {
            width: 2560,
            height: 1440,
            refresh: 144,
        };
        let previous = Mode {
            width: 1920,
            height: 1080,
            refresh: 120,
        };
        assert_eq!(
            outcome_of(
                1,
                "Generic PnP Monitor [:1]".to_string(),
                mode,
                previous,
                Some(90),
                Some(0),
            ),
            ApplyOutcome::Applied(Change {
                monitor: 1,
                display: "Generic PnP Monitor [:1]".to_string(),
                mode: Mode {
                    width: 2560,
                    height: 1440,
                    refresh: 144,
                },
                previous: Mode {
                    width: 1920,
                    height: 1080,
                    refresh: 120,
                },
                orientation: Some(90),
                previous_orientation: Some(0),
            })
        );
    }

    #[test]
    fn build_devmode_sets_orientation_when_present() {
        let mode = Mode {
            width: 3840,
            height: 2160,
            refresh: 144,
        };
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_display_orientation = 1;
        let devmode = build_devmode(&mode, &current, Some(270));
        assert_eq!(devmode.dm_display_orientation, 3);
        assert_ne!(devmode.dm_fields & DM_DISPLAYORIENTATION, 0);
    }

    #[test]
    fn build_devmode_leaves_orientation_when_absent() {
        let mode = Mode {
            width: 3840,
            height: 2160,
            refresh: 144,
        };
        let mut current: DevmodeW = unsafe { std::mem::zeroed() };
        current.dm_display_orientation = 1;
        let devmode = build_devmode(&mode, &current, None);
        assert_eq!(devmode.dm_display_orientation, 1);
        assert_eq!(devmode.dm_fields & DM_DISPLAYORIENTATION, 0);
    }

    #[test]
    fn revert_with_previous_orientation_restores_it() {
        let previous = Mode {
            width: 1920,
            height: 1080,
            refresh: 60,
        };
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_display_orientation = 2;
        let devmode = build_devmode(&previous, &base, Some(270));
        assert_eq!(devmode.dm_display_orientation, 3);
        assert_ne!(devmode.dm_fields & DM_DISPLAYORIENTATION, 0);
    }

    #[test]
    fn revert_without_previous_orientation_keeps_current() {
        let previous = Mode {
            width: 1920,
            height: 1080,
            refresh: 60,
        };
        let mut base: DevmodeW = unsafe { std::mem::zeroed() };
        base.dm_display_orientation = 2;
        let devmode = build_devmode(&previous, &base, None);
        assert_eq!(devmode.dm_display_orientation, 2);
        assert_eq!(devmode.dm_fields & DM_DISPLAYORIENTATION, 0);
    }

    #[test]
    fn has_applied_false_for_empty_plan() {
        assert!(!has_applied(&[]));
    }

    #[test]
    fn has_applied_false_when_all_unchanged() {
        let planned = vec![planned_unchanged(), planned_unchanged()];
        assert!(!has_applied(&planned));
    }

    #[test]
    fn has_applied_true_when_any_applied() {
        let planned = vec![planned_unchanged(), planned_applied()];
        assert!(has_applied(&planned));
    }

    fn planned_unchanged() -> Planned<'static> {
        let base = query::current_mode("").unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let mode = mode_of(&base);
        Planned {
            name: "",
            devmode: base,
            outcome: outcome_of(1, String::new(), mode, mode_of(&base), None, None),
        }
    }

    fn planned_applied() -> Planned<'static> {
        let base = query::current_mode("").unwrap_or_else(|| unsafe { std::mem::zeroed() });
        let previous = mode_of(&base);
        let mode = Mode {
            width: previous.width + 1,
            height: previous.height + 1,
            refresh: previous.refresh,
        };
        Planned {
            name: "",
            devmode: base,
            outcome: outcome_of(1, String::new(), mode, previous, None, None),
        }
    }
}

#[cfg(test)]
mod main_tests {
    use super::super::bindings::Pointl;
    use super::*;

    fn devmode_at(x: i32, y: i32) -> DevmodeW {
        let mut devmode: DevmodeW = unsafe { std::mem::zeroed() };
        devmode.dm_position = Pointl { x, y };
        devmode
    }

    #[test]
    fn is_primary_origin_is_primary() {
        assert!(is_primary(&devmode_at(0, 0)));
    }

    #[test]
    fn is_primary_other_position_is_not_primary() {
        assert!(!is_primary(&devmode_at(-1920, 0)));
        assert!(!is_primary(&devmode_at(0, 1080)));
        assert!(!is_primary(&devmode_at(1920, 1080)));
    }

    #[test]
    fn build_swap_moves_target_to_origin() {
        let target = devmode_at(1920, 0);
        let partner = devmode_at(0, 0);
        let (new_primary, new_partner) = build_swap(&target, &partner);
        assert_eq!(new_primary.dm_position.x, 0);
        assert_eq!(new_primary.dm_position.y, 0);
        assert_eq!(new_partner.dm_position.x, 1920);
        assert_eq!(new_partner.dm_position.y, 0);
    }

    #[test]
    fn build_swap_marks_position_in_fields() {
        let mut target = devmode_at(1920, 0);
        target.dm_fields = DM_PELSWIDTH | DM_PELSHEIGHT;
        let partner = devmode_at(0, 0);
        let (new_primary, new_partner) = build_swap(&target, &partner);
        assert_ne!(new_primary.dm_fields & DM_POSITION, 0);
        assert_ne!(new_partner.dm_fields & DM_POSITION, 0);
        assert_eq!(new_primary.dm_fields, target.dm_fields | DM_POSITION);
        assert_eq!(new_partner.dm_fields, partner.dm_fields | DM_POSITION);
    }

    #[test]
    fn build_swap_preserves_mode_fields() {
        let mut target = devmode_at(1920, 0);
        target.dm_pels_width = 1920;
        target.dm_pels_height = 1080;
        target.dm_display_frequency = 120;
        target.dm_display_orientation = 1;
        let mut partner = devmode_at(0, 0);
        partner.dm_pels_width = 1920;
        partner.dm_pels_height = 1080;
        partner.dm_display_frequency = 60;
        partner.dm_display_orientation = 0;
        let (new_primary, new_partner) = build_swap(&target, &partner);
        assert_eq!(new_primary.dm_pels_width, 1920);
        assert_eq!(new_primary.dm_pels_height, 1080);
        assert_eq!(new_primary.dm_display_frequency, 120);
        assert_eq!(new_primary.dm_display_orientation, 1);
        assert_eq!(new_partner.dm_pels_width, 1920);
        assert_eq!(new_partner.dm_pels_height, 1080);
        assert_eq!(new_partner.dm_display_frequency, 60);
        assert_eq!(new_partner.dm_display_orientation, 0);
    }

    #[test]
    fn build_swap_does_not_modify_inputs() {
        let target = devmode_at(1920, 0);
        let partner = devmode_at(0, 0);
        let _ = build_swap(&target, &partner);
        assert_eq!(target.dm_position.x, 1920);
        assert_eq!(target.dm_position.y, 0);
        assert_eq!(partner.dm_position.x, 0);
        assert_eq!(partner.dm_position.y, 0);
        assert_eq!(target.dm_fields, 0);
        assert_eq!(partner.dm_fields, 0);
    }
}
