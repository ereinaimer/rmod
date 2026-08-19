//! `layout` command: shows the monitor grid, places monitors relative to
//! each other, and promotes a display to the main display.
//!
//! [`run_layout`] dispatches the three [`LayoutAction`] variants: [`run_show`]
//! renders the arrangement grid, [`run_place`] applies a placement via
//! [`crate::sys::windows::apply_placement`] and runs the shared
//! keep-or-revert confirmation flow, and [`run_primary`] promotes a display
//! via [`crate::sys::windows::make_main`].

use crate::cli::parser::parse_monitor_target;
use crate::cli::{
    Command, Confirm, Direction, HelpTopic, LayoutAction, MonitorTarget, confirm_keep,
};
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
            format!(
                "{not_found} '{id}' not found. connected: {}",
                crate::sys::windows::connected_displays_list()
            )
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

/// Shows the arrangement grid: every display with its fingerprint ID, its
/// position relative to the primary display (the main display marked
/// `(primary)`), resolution, refresh and rotation, aligned to the widest
/// entries.
pub(super) fn run_show() -> i32 {
    match crate::sys::windows::list() {
        Ok(monitors) => {
            println!("{}", grid(&monitors));
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// The arrangement grid: header, separator and one aligned row per monitor,
/// every column sized to its widest entry.
fn grid(monitors: &[Monitor]) -> String {
    let rels: Vec<String> = monitors.iter().map(|m| relative_to(monitors, m)).collect();
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
    let id_width = monitors
        .iter()
        .map(|m| m.fingerprint.len())
        .max()
        .unwrap_or(2)
        .max(2);
    let rel_width = rels.iter().map(|r| r.len()).max().unwrap_or(11).max(11);
    let res_width = monitors
        .iter()
        .map(|m| format!("{}x{}", m.width, m.height).len())
        .max()
        .unwrap_or(10)
        .max(10);
    let rot_width = monitors
        .iter()
        .map(|m| rotation_angle(m.orientation).len())
        .max()
        .unwrap_or(3)
        .max(3);
    let header = format!(
        "{:<number_width$}  {:<name_width$}  {:<id_width$}  {:<rel_width$}  {:<res_width$}  {:<7}  {:<rot_width$}",
        "#", "NAME", "ID", "RELATIVE TO", "RESOLUTION", "REFRESH", "ROT"
    );
    let header_len = header.len();
    let mut lines = vec![header, "─".repeat(header_len)];
    for (m, rel) in monitors.iter().zip(&rels) {
        lines.push(format!(
            "{:<number_width$}  {:<name_width$}  {:<id_width$}  {:<rel_width$}  {:<res_width$}  {:<7}  {:<rot_width$}",
            m.number,
            m.name,
            m.fingerprint,
            rel,
            format!("{}x{}", m.width, m.height),
            format!("{}Hz", m.refresh),
            rotation_angle(m.orientation)
        ));
    }
    lines.join("\n")
}

/// The `ROT` cell: the display orientation value as its angle in degrees.
fn rotation_angle(orientation: u32) -> String {
    match orientation {
        0 => "0".to_string(),
        1 => "90".to_string(),
        2 => "180".to_string(),
        3 => "270".to_string(),
        _ => orientation.to_string(),
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

pub(crate) fn parse_layout(args: &[impl AsRef<str>]) -> Result<Command, String> {
    let mut monitor: Option<MonitorTarget> = None;
    let mut monitor_explicit = false;
    let mut placement: Option<(Direction, MonitorTarget)> = None;
    let mut primary = false;
    let mut yes = false;
    let mut i = 1;

    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Layout),
                });
            }
            "--version" => return Ok(Command::Version),
            "-m" | "--monitor" => {
                i += 1;
                let Some(val) = args.get(i) else {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID\ne.g. -m a1b2c3d4".to_string(),
                    );
                };
                let val = val.as_ref();
                if val.starts_with('-') {
                    return Err(
                        "-m, --monitor needs a value. a monitor ID\ne.g. -m a1b2c3d4".to_string(),
                    );
                }
                let target = parse_monitor_target(val)?;
                if matches!(target, MonitorTarget::All) {
                    return Err(
                        "layout -m accepts a monitor ID or 'primary', not 'all'\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5".to_string(),
                    );
                }
                monitor = Some(target);
                monitor_explicit = true;
                i += 1;
            }
            "--left-of" | "--right-of" | "--above" | "--below" => {
                if placement.is_some() {
                    return Err(
                        "use only one direction flag\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
                            .to_string(),
                    );
                }
                let direction = match arg {
                    "--left-of" => Direction::Left,
                    "--right-of" => Direction::Right,
                    "--above" => Direction::Above,
                    _ => Direction::Below,
                };
                i += 1;
                let Some(next) = args.get(i) else {
                    return Err(format!(
                        "{arg} needs a value. a monitor ID\ne.g. {arg} b2c3d4e5"
                    ));
                };
                let next = next.as_ref();
                if next.starts_with('-') {
                    return Err(format!(
                        "{arg} needs a value. a monitor ID\ne.g. {arg} b2c3d4e5"
                    ));
                }
                let target = parse_monitor_target(next)?;
                if matches!(target, MonitorTarget::All) {
                    return Err(format!(
                        "layout {arg} accepts a monitor ID or 'primary', not 'all'\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
                    ));
                }
                placement = Some((direction, target));
                i += 1;
            }
            "--primary" => {
                primary = true;
                i += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            other => {
                return Err(format!(
                    "unexpected argument {} for layout. use --left-of, --right-of, --above, --below, or --primary",
                    other
                ));
            }
        }
    }

    if primary {
        if placement.is_some() {
            return Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m a1b2c3d4 --primary"
                    .to_string(),
            );
        }
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --primary".to_string(),
            );
        };
        return Ok(Command::Layout {
            action: LayoutAction::Primary { monitor },
            yes,
        });
    }

    if monitor_explicit && placement.is_none() {
        return Err("-m, --monitor needs a direction flag or --primary\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5".to_string());
    }

    if let Some((direction, reference)) = placement {
        let Some(monitor) = monitor else {
            return Err(
                "missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
                    .to_string(),
            );
        };
        return Ok(Command::Layout {
            action: LayoutAction::Place {
                monitor,
                direction,
                reference,
            },
            yes,
        });
    }

    Ok(Command::Layout {
        action: LayoutAction::Show,
        yes,
    })
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
            modes: Vec::new(),
            is_primary,
            manufacturer: "TEST".to_string(),
            serial: format!("SERIAL{number}"),
            fingerprint: format!("finger{number}"),
            manufactured_week: 1,
            manufactured_year: 2024,
            native_width: 1920,
            native_height: 1080,
            native_refresh: 60,
            physical_size_cm: None,
            gamma: None,
            dpi_physical: None,
            gamut: None,
            hdr: None,
            bits_per_pel: 0,
            log_pixels: 0,
            orientation: 0,
            connector: None,
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

    #[test]
    fn rotation_angle_maps_orientation_values() {
        assert_eq!(rotation_angle(0), "0");
        assert_eq!(rotation_angle(1), "90");
        assert_eq!(rotation_angle(2), "180");
        assert_eq!(rotation_angle(3), "270");
        assert_eq!(rotation_angle(4), "4");
    }

    #[test]
    fn grid_renders_all_columns_aligned() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 1920, 0, false)];
        let out = grid(&monitors);
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "#  NAME  ID       RELATIVE TO  RESOLUTION  REFRESH  ROT"
        );
        assert_eq!(lines.next().unwrap(), "─".repeat(55));
        assert_eq!(
            lines.next().unwrap(),
            "1  M1    finger1  (primary)    1920x1080   60Hz     0  "
        );
        assert_eq!(
            lines.next().unwrap(),
            "2  M2    finger2  right of 1   1920x1080   60Hz     0  "
        );
        assert!(lines.next().is_none());
    }

    #[test]
    fn grid_shows_fingerprint_for_each_monitor() {
        let monitors = vec![monitor(1, 0, 0, true), monitor(2, 1920, 0, false)];
        let out = grid(&monitors);
        assert!(out.contains("finger1"), "missing fingerprint: {out}");
        assert!(out.contains("finger2"), "missing fingerprint: {out}");
    }

    #[test]
    fn grid_marks_rotated_monitor_with_angle() {
        let mut rotated = monitor(2, 1920, 0, false);
        rotated.orientation = 1;
        let monitors = vec![monitor(1, 0, 0, true), rotated];
        let out = grid(&monitors);
        assert!(
            out.lines().any(|l| l.trim_end().ends_with("90")),
            "missing rotation angle: {out}"
        );
    }

    const SERIAL_A: &str = "ABC12345678";
    const SERIAL_B: &str = "DEF45678901";

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn layout_no_args_is_show() {
        assert_eq!(
            parse(&["layout"]),
            Ok(Command::Layout {
                action: LayoutAction::Show,
                yes: false
            })
        );
    }

    #[test]
    fn layout_place_left_of_with_reference() {
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--left-of", SERIAL_B]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                    direction: Direction::Left,
                    reference: MonitorTarget::Id(SERIAL_B.to_string()),
                },
                yes: false,
            })
        );
    }

    #[test]
    fn layout_place_with_explicit_reference() {
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--above", SERIAL_B]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                    direction: Direction::Above,
                    reference: MonitorTarget::Id(SERIAL_B.to_string()),
                },
                yes: false,
            })
        );
    }

    #[test]
    fn layout_direction_flags_cover_all_four() {
        for (flag, direction) in [
            ("--left-of", Direction::Left),
            ("--right-of", Direction::Right),
            ("--above", Direction::Above),
            ("--below", Direction::Below),
        ] {
            assert_eq!(
                parse(&["layout", "-m", SERIAL_A, flag, SERIAL_B]),
                Ok(Command::Layout {
                    action: LayoutAction::Place {
                        monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                        direction,
                        reference: MonitorTarget::Id(SERIAL_B.to_string()),
                    },
                    yes: false,
                }),
                "flag '{}'",
                flag
            );
        }
    }

    #[test]
    fn layout_missing_value_for_direction_is_error() {
        for flag in ["--left-of", "--right-of", "--above", "--below"] {
            assert_eq!(
                parse(&["layout", "-m", SERIAL_A, flag]),
                Err(format!(
                    "{flag} needs a value. a monitor ID\ne.g. {flag} b2c3d4e5"
                )),
                "flag '{}'",
                flag
            );
            assert_eq!(
                parse(&["layout", "-m", SERIAL_A, flag, "--primary"]),
                Err(format!(
                    "{flag} needs a value. a monitor ID\ne.g. {flag} b2c3d4e5"
                )),
                "flag '{}'",
                flag
            );
        }
    }

    #[test]
    fn layout_second_direction_flag_is_error() {
        assert_eq!(
            parse(&[
                "layout",
                "-m",
                SERIAL_A,
                "--left-of",
                SERIAL_B,
                "--right-of",
                SERIAL_A
            ]),
            Err(
                "use only one direction flag\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
                    .to_string()
            )
        );
    }

    #[test]
    fn layout_primary_with_direction_is_error() {
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--primary", "--left-of", SERIAL_B]),
            Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m a1b2c3d4 --primary"
                    .to_string()
            )
        );
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--left-of", SERIAL_B, "--primary"]),
            Err(
                "use --primary or a direction flag, not both\ne.g. rmod layout -m a1b2c3d4 --primary"
                    .to_string()
            )
        );
    }

    #[test]
    fn layout_primary_with_monitor() {
        for args in [
            &["layout", "-m", SERIAL_A, "--primary"][..],
            &["layout", "--primary", "-m", SERIAL_A][..],
        ] {
            assert_eq!(
                parse(args),
                Ok(Command::Layout {
                    action: LayoutAction::Primary {
                        monitor: MonitorTarget::Id(SERIAL_A.to_string())
                    },
                    yes: false
                })
            );
        }
    }

    #[test]
    fn layout_primary_without_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "--primary"]),
            Err("missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --primary".to_string())
        );
    }

    #[test]
    fn layout_direction_without_monitor_is_error() {
        assert_eq!(
            parse(&["layout", "--left-of", SERIAL_B]),
            Err(
                "missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
                    .to_string()
            )
        );
    }

    #[test]
    fn layout_yes_flag() {
        for args in [
            &["layout", "-y", "--left-of", SERIAL_B, "-m", SERIAL_A][..],
            &["layout", "--left-of", SERIAL_B, "-y", "-m", SERIAL_A][..],
        ] {
            assert_eq!(
                parse(args),
                Ok(Command::Layout {
                    action: LayoutAction::Place {
                        monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                        direction: Direction::Left,
                        reference: MonitorTarget::Id(SERIAL_B.to_string()),
                    },
                    yes: true,
                })
            );
        }
    }

    #[test]
    fn layout_monitor_without_action_is_error() {
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A]),
            Err("-m, --monitor needs a direction flag or --primary\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5".to_string())
        );
    }

    #[test]
    fn layout_missing_value_for_monitor_flag() {
        assert_eq!(
            parse(&["layout", "-m", "--left-of", SERIAL_B]),
            Err("-m, --monitor needs a value. a monitor ID\ne.g. -m a1b2c3d4".to_string())
        );
    }

    #[test]
    fn layout_help_flag() {
        assert_eq!(
            parse(&["layout", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Layout)
            })
        );
        assert_eq!(
            parse(&["layout", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Layout)
            })
        );
    }

    #[test]
    fn layout_version_flag() {
        assert_eq!(parse(&["layout", "--version"]), Ok(Command::Version));
    }

    #[test]
    fn layout_unknown_argument_is_error() {
        assert_eq!(
            parse(&["layout", "foo"]),
            Err("unexpected argument foo for layout. use --left-of, --right-of, --above, --below, or --primary".to_string())
        );
    }

    #[test]
    fn layout_any_string_is_id() {
        assert_eq!(
            parse(&["layout", "-m", "x", "--left-of", SERIAL_B]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: MonitorTarget::Id("x".to_string()),
                    direction: Direction::Left,
                    reference: MonitorTarget::Id(SERIAL_B.to_string()),
                },
                yes: false,
            })
        );
        assert_eq!(
            parse(&["layout", "-m", "2", "--left-of", SERIAL_B]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: MonitorTarget::Index(2),
                    direction: Direction::Left,
                    reference: MonitorTarget::Id(SERIAL_B.to_string()),
                },
                yes: false,
            })
        );
        assert!(parse(&["layout", "-m", "0", "--left-of", SERIAL_B]).is_err());
    }

    #[test]
    fn layout_monitor_primary_keyword() {
        assert_eq!(
            parse(&["layout", "-m", "primary", "--primary"]),
            Ok(Command::Layout {
                action: LayoutAction::Primary {
                    monitor: MonitorTarget::Primary
                },
                yes: false
            })
        );
    }

    #[test]
    fn layout_reference_primary_keyword() {
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--left-of", "primary"]),
            Ok(Command::Layout {
                action: LayoutAction::Place {
                    monitor: MonitorTarget::Id(SERIAL_A.to_string()),
                    direction: Direction::Left,
                    reference: MonitorTarget::Primary,
                },
                yes: false,
            })
        );
    }

    #[test]
    fn layout_keywords_are_case_insensitive() {
        assert_eq!(
            parse(&["layout", "-m", "PRIMARY", "--primary"]),
            parse(&["layout", "-m", "primary", "--primary"])
        );
        assert_eq!(
            parse(&["layout", "-m", SERIAL_A, "--left-of", "PRIMARY"]),
            parse(&["layout", "-m", SERIAL_A, "--left-of", "primary"])
        );
    }

    #[test]
    fn layout_all_is_rejected() {
        assert!(
            parse(&["layout", "-m", "all", "--primary"]).is_err()
                && parse(&["layout", "-m", "all", "--primary"])
                    .unwrap_err()
                    .contains("not 'all'"),
            "expected -m all rejection, got: {:?}",
            parse(&["layout", "-m", "all", "--primary"])
        );
        assert!(
            parse(&["layout", "-m", SERIAL_A, "--left-of", "all"]).is_err()
                && parse(&["layout", "-m", SERIAL_A, "--left-of", "all"])
                    .unwrap_err()
                    .contains("not 'all'"),
            "expected --left-of all rejection, got: {:?}",
            parse(&["layout", "-m", SERIAL_A, "--left-of", "all"])
        );
    }
}
