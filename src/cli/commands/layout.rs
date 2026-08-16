//! `layout` command: shows the monitor grid, places monitors relative to
//! each other, and promotes a display to the main display.
//!
//! [`run_layout`] dispatches the three [`LayoutAction`] variants: [`run_show`]
//! renders the arrangement grid, [`run_place`] applies a placement via
//! [`crate::sys::windows::apply_placement`] and runs the shared
//! keep-or-revert confirmation flow, and [`run_primary`] promotes a display
//! via [`crate::sys::windows::make_main`].

use crate::cli::{Confirm, Direction, LayoutAction, MonitorTarget, confirm_keep};
use crate::sys::windows::{
    MainChange, MainOutcome, Monitor, PlacementChange, PlacementOutcome, apply_placement,
    make_main, revert_main, revert_placement,
};

use super::CONFIRM_TIMEOUT_SECS;

/// Resolves a layout target to a 1-based monitor number. `Primary` resolves
/// through the primary display's monitor number; an unknown id or a
/// primary lookup failure is a hard error. `All` is rejected by the parser.
///
/// # Errors
/// Returns the message to print when the target matches no display.
fn resolve_layout_target(target: &MonitorTarget, not_found: &str) -> Result<u32, String> {
    match target {
        MonitorTarget::Primary => crate::sys::windows::get_primary_mode().map(|m| m.number),
        MonitorTarget::Index(n) => Ok(*n),
        MonitorTarget::Id(id) => crate::sys::windows::resolve_by_id(id).ok_or_else(|| {
            format!("{not_found} '{id}' not found. run rmod list to see connected displays")
        }),
        MonitorTarget::All => unreachable!(),
    }
}

/// Promotes the targeted monitor to the main display.
pub(super) fn run_primary(monitor: &MonitorTarget, yes: bool) -> i32 {
    let names = crate::sys::windows::enumerate_devices();
    let monitor_num = match resolve_layout_target(monitor, "monitor with id") {
        Ok(n) => n,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    match make_main(monitor_num, &names) {
        Ok(MainOutcome::Unchanged(display)) => {
            println!("{display} is already the main display");
            0
        }
        Ok(MainOutcome::Applied(change)) => {
            println!("{} is now the main display", change.display);
            confirm_main_revert(
                &change,
                yes,
                || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
                revert_main,
            )
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Runs the keep-or-revert confirmation for a promoted main display; `yes`
/// skips the prompt. Reverts the promotion and prints the revert line.
///
/// Injectable variant: the confirm prompt and the revert call are supplied
/// as closures so tests can exercise the Revert branch without touching the
/// display.
fn confirm_main_revert<C, R>(change: &MainChange<'_>, yes: bool, confirm: C, revert: R) -> i32
where
    C: FnOnce() -> Confirm,
    R: FnOnce(&MainChange<'_>) -> Result<(), String>,
{
    if yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => match revert(change) {
            Ok(()) => {
                println!("reverted to the previous main display");
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}

/// Shows the arrangement grid: every display with its position relative to
/// the primary display (the main display marked `(primary)`), resolution
/// and refresh, aligned to the widest entries.
pub(super) fn run_show() -> i32 {
    match crate::sys::windows::list() {
        Ok(monitors) => {
            let rels: Vec<String> = monitors.iter().map(|m| relative_to(&monitors, m)).collect();
            let number_width = monitors
                .iter()
                .map(|m| m.number.to_string().len())
                .max()
                .unwrap_or(1)
                .max(1);
            let name_width = monitors
                .iter()
                .map(|m| m.name.len())
                .max()
                .unwrap_or(4)
                .max(4);
            let rel_width = rels.iter().map(|r| r.len()).max().unwrap_or(11).max(11);
            let res_width = monitors
                .iter()
                .map(|m| format!("{}x{}", m.width, m.height).len())
                .max()
                .unwrap_or(10)
                .max(10);
            let header = format!(
                "{:<number_width$}  {:<name_width$}  {:<rel_width$}  {:<res_width$}  {:<7}",
                "#", "NAME", "RELATIVE TO", "RESOLUTION", "REFRESH"
            );
            println!("{header}");
            println!("{}", "─".repeat(header.len()));
            for (m, rel) in monitors.iter().zip(&rels) {
                println!(
                    "{:<number_width$}  {:<name_width$}  {:<rel_width$}  {:<res_width$}  {:<7}",
                    m.number,
                    m.name,
                    rel,
                    format!("{}x{}", m.width, m.height),
                    format!("{}Hz", m.refresh)
                );
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Where `monitor` sits relative to the primary display, e.g. `right of 1`;
/// `(primary)` for the main display itself. Positions on diagonals are
/// described by the dominant axis, horizontal winning ties. When no
/// monitor is primary, the first monitor is the reference (mirrors
/// [`crate::sys::windows::resolve_device`]).
fn relative_to(monitors: &[Monitor], monitor: &Monitor) -> String {
    let Some(primary) = monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.first())
    else {
        return String::new();
    };
    if monitor.number == primary.number {
        return "(primary)".to_string();
    }
    let dx = monitor.x - primary.x;
    let dy = monitor.y - primary.y;
    let side = if dx.abs() >= dy.abs() {
        if dx > 0 { "right of" } else { "left of" }
    } else if dy > 0 {
        "below"
    } else {
        "above"
    };
    format!("{side} {}", primary.number)
}

/// Places a monitor on a side of another monitor and runs the keep-or-revert
/// confirmation flow; `yes` skips the prompt. Reverts the placement and
/// prints the revert line on Revert.
pub(super) fn run_place(
    monitor: &MonitorTarget,
    direction: Direction,
    reference: &MonitorTarget,
    yes: bool,
) -> i32 {
    let m = match resolve_layout_target(monitor, "monitor with id") {
        Ok(n) => n,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let r = match resolve_layout_target(reference, "reference monitor with id") {
        Ok(n) => n,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };

    match apply_placement(m, direction, r) {
        Ok(PlacementOutcome::Unchanged {
            display,
            reference_display,
        }) => {
            println!(
                "{display} is already {} {reference_display}",
                side_phrase(direction)
            );
            0
        }
        Ok(PlacementOutcome::Applied(change)) => {
            println!("{}", describe_placement(&change, direction));
            confirm_placement_revert(
                &change,
                yes,
                || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
                revert_placement,
            )
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// The phrase for a side, shared by the applied and already-there messages.
fn side_phrase(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "to the left of",
        Direction::Right => "to the right of",
        Direction::Above => "above",
        Direction::Below => "below",
    }
}

/// Runs the keep-or-revert confirmation for an applied placement; `yes`
/// skips the prompt. Reverts the placement and prints the revert line.
///
/// Injectable variant: the confirm prompt and the revert call are supplied
/// as closures so tests can exercise the Revert branch without touching the
/// display.
fn confirm_placement_revert<C, R>(change: &PlacementChange, yes: bool, confirm: C, revert: R) -> i32
where
    C: FnOnce() -> Confirm,
    R: FnOnce(&PlacementChange) -> Result<(), String>,
{
    if yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => match revert(change) {
            Ok(()) => {
                println!("reverted to the previous layout");
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}

/// Describes an applied placement: where the monitor was placed relative to
/// its reference, with the swapped occupant appended when a swap happened.
fn describe_placement(change: &PlacementChange, direction: Direction) -> String {
    let mut message = format!(
        "placed {} {} {}",
        change.display,
        side_phrase(direction),
        change.reference_display
    );
    if let Some(swap) = &change.swap_display {
        message.push_str(&format!(" (swapped with {swap})"));
    }
    message
}

/// Runs a parsed layout action and returns the process exit code.
pub(super) fn run_layout(action: LayoutAction, yes: bool) -> i32 {
    match action {
        LayoutAction::Show => run_show(),
        LayoutAction::Place {
            monitor,
            direction,
            reference,
        } => run_place(&monitor, direction, &reference, yes),
        LayoutAction::Primary { monitor } => run_primary(&monitor, yes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Direction;
    use crate::sys::windows::MainChange;

    fn main_change() -> MainChange<'static> {
        MainChange {
            monitor: 2,
            display: "RMOD Fake Monitor 2".to_string(),
            applied: Vec::new(),
            previous: Vec::new(),
        }
    }

    fn placement_change() -> PlacementChange {
        PlacementChange {
            display: "RMOD Fake Monitor 2 [:2]".to_string(),
            reference_display: "RMOD Fake Monitor 1 [:1]".to_string(),
            swap_display: None,
            applied: Vec::new(),
            previous: Vec::new(),
        }
    }

    #[test]
    fn confirm_main_revert_yes_skips_confirm_and_revert() {
        assert_eq!(
            confirm_main_revert(
                &main_change(),
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_main_revert_keep_skips_revert() {
        assert_eq!(
            confirm_main_revert(
                &main_change(),
                false,
                || Confirm::Keep,
                |_| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_main_revert_revert_calls_revert_with_change() {
        assert_eq!(
            confirm_main_revert(
                &main_change(),
                false,
                || Confirm::Revert,
                |change| {
                    assert_eq!(change.monitor, 2);
                    Ok(())
                },
            ),
            0
        );
    }

    #[test]
    fn confirm_main_revert_revert_error_returns_2() {
        assert_eq!(
            confirm_main_revert(
                &main_change(),
                false,
                || Confirm::Revert,
                |_| Err("boom".to_string()),
            ),
            2
        );
    }

    #[test]
    fn confirm_placement_revert_yes_skips_confirm_and_revert() {
        assert_eq!(
            confirm_placement_revert(
                &placement_change(),
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_placement_revert_keep_skips_revert() {
        assert_eq!(
            confirm_placement_revert(
                &placement_change(),
                false,
                || Confirm::Keep,
                |_| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_placement_revert_revert_calls_revert_with_change() {
        assert_eq!(
            confirm_placement_revert(
                &placement_change(),
                false,
                || Confirm::Revert,
                |change| {
                    assert_eq!(change.display, "RMOD Fake Monitor 2 [:2]");
                    Ok(())
                },
            ),
            0
        );
    }

    #[test]
    fn confirm_placement_revert_revert_error_returns_2() {
        assert_eq!(
            confirm_placement_revert(
                &placement_change(),
                false,
                || Confirm::Revert,
                |_| Err("boom".to_string()),
            ),
            2
        );
    }

    #[test]
    fn describe_placement_left_wording() {
        assert_eq!(
            describe_placement(&placement_change(), Direction::Left),
            "placed RMOD Fake Monitor 2 [:2] to the left of RMOD Fake Monitor 1 [:1]"
        );
    }

    #[test]
    fn describe_placement_right_wording() {
        assert_eq!(
            describe_placement(&placement_change(), Direction::Right),
            "placed RMOD Fake Monitor 2 [:2] to the right of RMOD Fake Monitor 1 [:1]"
        );
    }

    #[test]
    fn describe_placement_above_wording() {
        assert_eq!(
            describe_placement(&placement_change(), Direction::Above),
            "placed RMOD Fake Monitor 2 [:2] above RMOD Fake Monitor 1 [:1]"
        );
    }

    #[test]
    fn describe_placement_below_wording() {
        assert_eq!(
            describe_placement(&placement_change(), Direction::Below),
            "placed RMOD Fake Monitor 2 [:2] below RMOD Fake Monitor 1 [:1]"
        );
    }

    #[test]
    fn describe_placement_with_swap_appends_swapped_suffix() {
        let mut change = placement_change();
        change.swap_display = Some("RMOD Fake Monitor 3 [:3]".to_string());
        assert_eq!(
            describe_placement(&change, Direction::Left),
            "placed RMOD Fake Monitor 2 [:2] to the left of RMOD Fake Monitor 1 [:1] (swapped with RMOD Fake Monitor 3 [:3])"
        );
    }

    #[test]
    fn describe_placement_without_swap_has_no_suffix() {
        assert!(!describe_placement(&placement_change(), Direction::Below).contains("swapped"));
    }

    fn monitor(number: u32, x: i32, y: i32, is_primary: bool) -> Monitor {
        Monitor {
            number,
            name: format!("M{number}"),
            device_name: format!(r"\\.\DISPLAY{number}"),
            x,
            y,
            width: 1920,
            height: 1080,
            refresh: 60,
            is_primary,
            manufacturer: "TEST".to_string(),
            serial: format!("SERIAL{number}"),
            fingerprint: format!("finger{number}"),
            manufactured_week: 1,
            manufactured_year: 2024,
            native_width: 1920,
            native_height: 1080,
            native_refresh: 60,
        }
    }

    #[test]
    fn relative_to_primary_row_is_marked() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 1920, 0, false)];
        assert_eq!(relative_to(&monitors, &monitors[0]), "(primary)");
    }

    #[test]
    fn relative_to_right_of_primary() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 1920, 0, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "right of 1");
    }

    #[test]
    fn relative_to_left_of_primary() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, -1920, 0, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "left of 1");
    }

    #[test]
    fn relative_to_below_primary() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 0, 1080, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "below 1");
    }

    #[test]
    fn relative_to_above_primary() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 0, -1080, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "above 1");
    }

    #[test]
    fn relative_to_diagonal_uses_dominant_axis_horizontal_wins_ties() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 1920, 1080, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "right of 1");
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, -200, 1080, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "below 1");
    }

    #[test]
    fn relative_to_without_primary_uses_first_as_reference() {
        let monitors = vec![monitor(2, 1920, 0, false), monitor(1, 0, 0, false)];
        assert_eq!(relative_to(&monitors, &monitors[1]), "left of 2");
    }

    #[test]
    fn relative_to_empty_list_is_empty() {
        assert_eq!(relative_to(&[], &monitor(1, 0, 0, true)), "");
    }
}
