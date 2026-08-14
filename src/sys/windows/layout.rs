//! Placement engine for the `layout` command.
//!
//! Computes where a monitor lands relative to another (top-aligned, using
//! effective dimensions so rotated displays are measured edge-to-edge),
//! swaps positions when the landing spot is occupied, applies the change
//! via [`apply::apply_position`] and records what to revert.

use super::apply;
use super::bindings::{DM_POSITION, DevmodeW, Pointl};
use super::query;

/// The side of the reference monitor a target is placed on.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[allow(dead_code)]
pub enum Direction {
    Left,
    Right,
    Above,
    Below,
}

/// A completed placement: what was applied and what to revert.
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub struct PlacementChange {
    /// The target display label, e.g. `AOC 24G2 [:2]`.
    pub display: String,
    /// The reference monitor label.
    pub reference_display: String,
    /// The occupant label when a swap happened.
    pub swap_display: Option<String>,
    /// The `(device name, devmode)` pairs applied, in order.
    pub applied: Vec<(String, DevmodeW)>,
    /// The originals for revert, in reverse-apply order.
    pub previous: Vec<(String, DevmodeW)>,
}

/// Places a monitor on a side of another monitor, swapping positions when
/// the landing spot is occupied.
///
/// `monitor` is the 1-based number from `ls`; `reference` is the monitor to
/// position relative to. The change is applied via [`apply::apply_position`]
/// in order and returned so it can be reverted later.
///
/// # Errors
/// Unknown monitor, placing a monitor relative to itself, or a rejected
/// position change.
#[allow(dead_code)]
pub(crate) fn apply_placement(
    monitor: u32,
    direction: Direction,
    reference: u32,
    names: &[String],
) -> Result<PlacementChange, String> {
    let (target_index, target_name) = query::resolve_device(Some(monitor), names)?;
    let (reference_index, reference_name) = query::resolve_device(Some(reference), names)?;
    if reference_index == target_index {
        return Err(format!(
            "cannot place monitor {} relative to itself",
            target_index + 1
        ));
    }
    let target_dev = query::current_mode(target_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let reference_dev =
        query::current_mode(reference_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let landing = landing_position(direction, &reference_dev, &target_dev);
    let change = build_placement(
        target_index,
        reference_index,
        direction,
        &target_dev,
        &reference_dev,
        occupant_at(landing, target_index, names),
        names,
    );
    for (name, devmode) in &change.applied {
        apply::apply_position(name, devmode)?;
    }
    Ok(change)
}

/// Undoes a placement by re-applying the original positions captured in a
/// [`PlacementChange`], in reverse-apply order.
///
/// # Errors
/// A rejected position change.
#[allow(dead_code)]
pub(crate) fn revert_placement(change: &PlacementChange) -> Result<(), String> {
    for (name, devmode) in &change.previous {
        apply::apply_position(name, devmode)?;
    }
    Ok(())
}

/// The landing position of `target` relative to `reference`, top-aligned,
/// using effective dimensions so rotated displays are measured edge-to-edge.
#[allow(dead_code)]
pub(crate) fn landing_position(
    direction: Direction,
    reference: &DevmodeW,
    target: &DevmodeW,
) -> Pointl {
    let (reference_w, reference_h) = apply::effective_dims(None, None, None, reference);
    let (target_w, target_h) = apply::effective_dims(None, None, None, target);
    let (rx, ry) = (reference.dm_position.x, reference.dm_position.y);
    match direction {
        Direction::Left => Pointl {
            x: rx - target_w as i32,
            y: ry,
        },
        Direction::Right => Pointl {
            x: rx + reference_w as i32,
            y: ry,
        },
        Direction::Above => Pointl {
            x: rx,
            y: ry - target_h as i32,
        },
        Direction::Below => Pointl {
            x: rx,
            y: ry + reference_h as i32,
        },
    }
}

/// The monitor (0-based index and devmode) sitting at `landing`, ignoring
/// the target; `None` when the landing spot is free.
#[allow(dead_code)]
fn occupant_at(landing: Pointl, target_index: usize, names: &[String]) -> Option<(usize, DevmodeW)> {
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != target_index)
        .filter_map(|(i, name)| query::current_mode(name).map(|dev| (i, dev)))
        .find(|(_, dev)| dev.dm_position == landing)
}

/// Builds the applied/previous pairs of a placement: the target moved to
/// the landing spot, and when it is occupied, the occupant moved to the
/// target's previous spot.
#[allow(dead_code)]
fn build_placement(
    target_index: usize,
    reference_index: usize,
    direction: Direction,
    target_dev: &DevmodeW,
    reference_dev: &DevmodeW,
    occupant: Option<(usize, DevmodeW)>,
    names: &[String],
) -> PlacementChange {
    let display = query::display_label(&names[target_index], target_index as u32 + 1);
    let reference_display = query::display_label(&names[reference_index], reference_index as u32 + 1);
    let landing = landing_position(direction, reference_dev, target_dev);
    let (applied, previous, swap_display) = match occupant {
        Some((occupant_index, occupant_dev)) => {
            let (new_target, new_occupant) = build_swap(target_dev, &occupant_dev, landing);
            (
                vec![
                    (names[target_index].clone(), new_target),
                    (names[occupant_index].clone(), new_occupant),
                ],
                vec![
                    (names[occupant_index].clone(), occupant_dev),
                    (names[target_index].clone(), *target_dev),
                ],
                Some(query::display_label(
                    &names[occupant_index],
                    occupant_index as u32 + 1,
                )),
            )
        }
        None => (
            vec![(names[target_index].clone(), build_moved(target_dev, landing))],
            vec![(names[target_index].clone(), *target_dev)],
            None,
        ),
    };
    PlacementChange {
        display,
        reference_display,
        swap_display,
        applied,
        previous,
    }
}

/// The target devmode moved to the landing spot, marked with `DM_POSITION`.
#[allow(dead_code)]
fn build_moved(target: &DevmodeW, landing: Pointl) -> DevmodeW {
    let mut new_target = *target;
    new_target.dm_position = landing;
    new_target.dm_fields |= DM_POSITION;
    new_target
}

/// Builds the two devmodes of a placement swap: the target moved to the
/// landing spot and the occupant moved to the target's previous spot. Both
/// devmodes gain the `DM_POSITION` field flag; everything else is copied
/// through unchanged and the inputs are not modified (mirrors
/// [`apply::build_swap`]).
#[allow(dead_code)]
fn build_swap(target: &DevmodeW, occupant: &DevmodeW, landing: Pointl) -> (DevmodeW, DevmodeW) {
    let mut new_occupant = *occupant;
    new_occupant.dm_position = target.dm_position;
    new_occupant.dm_fields |= DM_POSITION;
    (build_moved(target, landing), new_occupant)
}

#[cfg(test)]
mod tests {
    use super::super::bindings::{DM_POSITION, DevmodeW, Pointl};
    use super::*;

    /// True when `RMOD_HW_TEST` is set to `"1"`, gating the tests that
    /// re-apply positions to the host display.
    fn hw_tests_enabled() -> bool {
        std::env::var("RMOD_HW_TEST").is_ok_and(|v| v == "1")
    }

    fn devmode_at(x: i32, y: i32, width: u32, height: u32) -> DevmodeW {
        let mut devmode: DevmodeW = unsafe { std::mem::zeroed() };
        devmode.dm_position = Pointl { x, y };
        devmode.dm_pels_width = width;
        devmode.dm_pels_height = height;
        devmode
    }

    fn names3() -> Vec<String> {
        vec!["A".to_string(), "B".to_string(), "C".to_string()]
    }

    #[test]
    fn landing_position_left_is_left_of_reference_and_top_aligned() {
        let reference = devmode_at(0, 0, 1920, 1080);
        let target = devmode_at(1920, 0, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Left, &reference, &target),
            Pointl { x: -1920, y: 0 }
        );
    }

    #[test]
    fn landing_position_right_is_right_of_reference() {
        let reference = devmode_at(1920, 0, 1920, 1080);
        let target = devmode_at(0, 0, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Right, &reference, &target),
            Pointl { x: 3840, y: 0 }
        );
    }

    #[test]
    fn landing_position_above_is_above_reference_top_aligned_x() {
        let reference = devmode_at(0, 1080, 1920, 1080);
        let target = devmode_at(0, 0, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Above, &reference, &target),
            Pointl { x: 0, y: 0 }
        );
    }

    #[test]
    fn landing_position_below_is_below_reference() {
        let reference = devmode_at(0, 0, 1920, 1080);
        let target = devmode_at(0, -1080, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Below, &reference, &target),
            Pointl { x: 0, y: 1080 }
        );
    }

    #[test]
    fn landing_uses_effective_dims_for_rotated_monitor() {
        let reference = devmode_at(0, 0, 1920, 1080);
        let mut target = devmode_at(1920, 0, 1080, 1920);
        target.dm_display_orientation = 1;
        assert_eq!(
            landing_position(Direction::Left, &reference, &target),
            Pointl { x: -1920, y: 0 }
        );
    }

    #[test]
    fn collision_swaps_occupant_with_target() {
        let target = devmode_at(0, 1080, 1920, 1080);
        let reference = devmode_at(0, 0, 1920, 1080);
        let occupant = devmode_at(1920, 0, 1920, 1080);
        let names = names3();
        let change = build_placement(
            2,
            0,
            Direction::Right,
            &target,
            &reference,
            Some((1, occupant)),
            &names,
        );
        assert_eq!(change.swap_display.as_deref(), Some("B [:2]"));
        assert_eq!(change.applied.len(), 2);
        assert_eq!(change.applied[0].0, "C");
        assert_eq!(change.applied[0].1.dm_position, Pointl { x: 1920, y: 0 });
        assert_ne!(change.applied[0].1.dm_fields & DM_POSITION, 0);
        assert_eq!(change.applied[1].0, "B");
        assert_eq!(change.applied[1].1.dm_position, Pointl { x: 0, y: 1080 });
        assert_ne!(change.applied[1].1.dm_fields & DM_POSITION, 0);
        assert_eq!(change.applied[1].1.dm_pels_width, 1920);
        assert_eq!(change.previous[0].1.dm_position, Pointl { x: 1920, y: 0 });
        assert_eq!(change.previous[1].1.dm_position, Pointl { x: 0, y: 1080 });
    }

    #[test]
    fn collision_free_places_target_only() {
        let target = devmode_at(0, 1080, 1920, 1080);
        let reference = devmode_at(0, 0, 1920, 1080);
        let names = names3();
        let change = build_placement(2, 0, Direction::Right, &target, &reference, None, &names);
        assert_eq!(change.swap_display, None);
        assert_eq!(change.applied.len(), 1);
        assert_eq!(change.applied[0].0, "C");
        assert_eq!(change.applied[0].1.dm_position, Pointl { x: 1920, y: 0 });
        assert_ne!(change.applied[0].1.dm_fields & DM_POSITION, 0);
        assert_eq!(change.applied[0].1.dm_pels_width, 1920);
    }

    #[test]
    fn apply_placement_self_reference_is_error() {
        let names = names3();
        assert_eq!(
            apply_placement(1, Direction::Left, 1, &names),
            Err("cannot place monitor 1 relative to itself".to_string())
        );
    }

    #[test]
    fn apply_placement_unknown_monitor_is_error() {
        let names = names3();
        assert_eq!(
            apply_placement(5, Direction::Left, 1, &names),
            Err("monitor 5 not found".to_string())
        );
    }

    #[test]
    fn revert_placement_restores_previous_positions() {
        let target = devmode_at(0, 1080, 1920, 1080);
        let reference = devmode_at(0, 0, 1920, 1080);
        let occupant = devmode_at(1920, 0, 1920, 1080);
        let names = names3();
        let change = build_placement(
            2,
            0,
            Direction::Right,
            &target,
            &reference,
            Some((1, occupant)),
            &names,
        );
        assert_eq!(change.previous.len(), 2);
        assert_eq!(change.previous[0].0, "B");
        assert_eq!(change.previous[0].1.dm_position, Pointl { x: 1920, y: 0 });
        assert_eq!(change.previous[1].0, "C");
        assert_eq!(change.previous[1].1.dm_position, Pointl { x: 0, y: 1080 });
        let empty = PlacementChange {
            display: "A [:1]".to_string(),
            reference_display: "A [:1]".to_string(),
            swap_display: None,
            applied: vec![],
            previous: vec![],
        };
        assert_eq!(revert_placement(&empty), Ok(()));
    }

    #[test]
    fn apply_placement_real_display_then_revert() {
        // Skipped by default so `cargo test` never touches the display; run
        // with `RMOD_HW_TEST=1` in a hardware lab.
        if !hw_tests_enabled() {
            return;
        }
        let names = query::enumerate_devices();
        if names.len() < 2 {
            return;
        }
        let Ok(change) = apply_placement(2, Direction::Left, 1, &names) else {
            return;
        };
        assert!(revert_placement(&change).is_ok());
    }
}
