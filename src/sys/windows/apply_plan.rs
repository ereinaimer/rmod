//! Planning half of the mode-application backend: computes the devmode
//! and outcome for every target monitor without touching the display.
//!
//! [`plan_set`] and [`plan_max`] resolve the requested policy against
//! each target's current mode and supported modes into a [`Planned`]
//! change per monitor; [`super::apply`] validates, applies and persists
//! them.

use super::apply::{
    ApplyOutcome, Refresh, angle_of, best_mode, build_devmode, effective_dims, mode_of,
    orientation_of, outcome_of, resolve_refresh,
};
use super::bindings::DevmodeW;
use super::capabilities::{self, Mode};
use super::query;

/// A planned change for one monitor: everything needed to validate and
/// apply a mode and report the resulting outcome.
pub(crate) struct Planned<'a> {
    pub(crate) name: &'a str,
    pub(crate) devmode: DevmodeW,
    pub(crate) outcome: ApplyOutcome,
}

#[allow(dead_code)]
pub(crate) fn plan_set<'a>(
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

pub(crate) fn plan_max<'a>(
    targets: &'a [(usize, &'a str)],
    orientation: Option<u32>,
) -> Result<Vec<Planned<'a>>, String> {
    let mut planned = Vec::new();
    let mut failures = Vec::new();
    for (index, name) in targets {
        let display = query::display_label(name, *index as u32 + 1);
        let Some(mode) = best_mode(capabilities::enumerate_modes(name)) else {
            failures.push(format!(
                "{display} has no supported modes, the display may be disabled or not connected"
            ));
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
pub(crate) fn has_applied(planned: &[Planned<'_>]) -> bool {
    planned
        .iter()
        .any(|p| matches!(p.outcome, ApplyOutcome::Applied(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
