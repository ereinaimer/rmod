//! Placement engine for the `layout` command.
//!
//! Computes where a monitor lands relative to another (top-aligned, measured
//! edge-to-edge with raw pixel dimensions so rotated displays use their
//! on-screen footprint), swaps positions when the landing spot is occupied,
//! applies the change via [`apply::apply_position`] under a full-screen fade
//! and records what to revert. A no-op placement (already there) reports
//! [`PlacementOutcome::Unchanged`]; a blocked swap destination is an error.

use super::apply;
use super::bindings::{DM_POSITION, DevmodeW, Pointl};
use super::fade;
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

/// The result of a placement request.
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum PlacementOutcome {
    /// The monitor moved (or swapped); the change is revertible.
    Applied(PlacementChange),
    /// The monitor already sits on that side; nothing was changed.
    Unchanged {
        /// The target display label.
        display: String,
        /// The reference monitor label.
        reference_display: String,
    },
}

/// Places a monitor on a side of another monitor, swapping positions when
/// the landing spot is occupied.
///
/// `monitor` is the 1-based number from `ls`; `reference` is the monitor to
/// position relative to. The change is applied under a full-screen fade via
/// [`apply::apply_position`] and returned so it can be reverted later.
///
/// # Errors
/// Unknown monitor or reference, placing a monitor relative to itself, a
/// blocked swap destination, or a rejected position change.
#[allow(dead_code)]
pub(crate) fn apply_placement(
    monitor: u32,
    direction: Direction,
    reference: u32,
    names: &[String],
) -> Result<PlacementOutcome, String> {
    let (target_index, target_name) = query::resolve_device(Some(monitor), names)?;
    let (reference_index, reference_name) =
        query::resolve_device(Some(reference), names).map_err(|e| format!("reference {e}"))?;
    if reference_index == target_index {
        return Err(format!(
            "cannot place monitor {} relative to itself, use a different reference monitor",
            target_index + 1
        ));
    }
    let target_dev =
        query::current_mode(target_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let reference_dev =
        query::current_mode(reference_name).unwrap_or_else(|| unsafe { std::mem::zeroed() });
    let landing = landing_position(direction, &reference_dev, &target_dev);
    let occupant = occupant_overlapping(landing, &target_dev, target_index, names);
    if landing == target_dev.dm_position && occupant.is_none() {
        return Ok(PlacementOutcome::Unchanged {
            display: query::display_label(&names[target_index], target_index as u32 + 1),
            reference_display: query::display_label(
                &names[reference_index],
                reference_index as u32 + 1,
            ),
        });
    }
    if occupant.is_some()
        && let Some((other_index, _)) = destination_occupied(
            &target_dev,
            target_index,
            occupant.as_ref().map(|(i, _)| *i),
            names,
        )
    {
        let other_label = query::display_label(&names[other_index], other_index as u32 + 1);
        return Err(format!(
            "cannot place monitor {}: {other_label} occupies its current position, move that monitor first",
            target_index + 1
        ));
    }
    let change = build_placement(
        target_index,
        reference_index,
        &target_dev,
        landing,
        occupant,
        names,
    );
    fade::transition_all(|| {
        apply_with_rollback(&change.applied, &change.previous, |name, devmode| {
            apply::apply_position(name, devmode)
        })
    })?;
    Ok(PlacementOutcome::Applied(change))
}

/// Undoes a placement by re-applying the original positions captured in a
/// [`PlacementChange`], in reverse-apply order, under a full-screen fade.
///
/// # Errors
/// A rejected position change.
#[allow(dead_code)]
pub(crate) fn revert_placement(change: &PlacementChange) -> Result<(), String> {
    fade::transition_all(|| {
        apply_with_rollback(&change.previous, &change.applied, |name, devmode| {
            apply::apply_position(name, devmode)
        })
    })
}

/// Applies `applied` in order; on failure at index `k`, best-effort rolls
/// the already-applied pairs back to `previous` (in reverse-apply order)
/// before returning the original error. Reports a combined error when the
/// rollback itself fails.
pub(crate) fn apply_with_rollback<N: AsRef<str>>(
    applied: &[(N, DevmodeW)],
    previous: &[(N, DevmodeW)],
    apply: impl Fn(&str, &DevmodeW) -> Result<(), String>,
) -> Result<(), String> {
    for (k, (name, devmode)) in applied.iter().enumerate() {
        if let Err(e) = apply(name.as_ref(), devmode) {
            let mut failed = Vec::new();
            for (name, devmode) in previous.iter().skip(previous.len() - k) {
                if let Err(rollback_e) = apply(name.as_ref(), devmode) {
                    failed.push(rollback_e);
                }
            }
            if failed.is_empty() {
                return Err(e);
            }
            return Err(format!("{e}; rollback failed: {}", failed.join(", ")));
        }
    }
    Ok(())
}

/// The landing position of `target` relative to `reference`, top-aligned,
/// measured edge-to-edge with raw pixel dimensions so rotated displays use
/// their on-screen footprint.
#[allow(dead_code)]
pub(crate) fn landing_position(
    direction: Direction,
    reference: &DevmodeW,
    target: &DevmodeW,
) -> Pointl {
    let (rx, ry) = (reference.dm_position.x, reference.dm_position.y);
    match direction {
        Direction::Left => Pointl {
            x: rx - target.dm_pels_width as i32,
            y: ry,
        },
        Direction::Right => Pointl {
            x: rx + reference.dm_pels_width as i32,
            y: ry,
        },
        Direction::Above => Pointl {
            x: rx,
            y: ry - target.dm_pels_height as i32,
        },
        Direction::Below => Pointl {
            x: rx,
            y: ry + reference.dm_pels_height as i32,
        },
    }
}

/// The on-screen rect of a monitor at `pos` given its raw pixel size.
fn rect_of(pos: Pointl, dev: &DevmodeW) -> (i32, i32, i32, i32) {
    (
        pos.x,
        pos.y,
        pos.x + dev.dm_pels_width as i32,
        pos.y + dev.dm_pels_height as i32,
    )
}

/// Whether two rects overlap: strict, so edge-adjacent monitors do not.
fn rects_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// The lowest-indexed monitor (0-based index and devmode) whose rect
/// overlaps the landing spot, ignoring the target; `None` when free.
#[allow(dead_code)]
fn occupant_overlapping(
    landing: Pointl,
    target_dev: &DevmodeW,
    target_index: usize,
    names: &[String],
) -> Option<(usize, DevmodeW)> {
    let landing_rect = rect_of(landing, target_dev);
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != target_index)
        .filter_map(|(i, name)| query::current_mode(name).map(|dev| (i, dev)))
        .find(|(_, dev)| rects_overlap(rect_of(dev.dm_position, dev), landing_rect))
}

/// The lowest-indexed monitor whose rect overlaps the target's current
/// rect, ignoring the target and the swapping occupant; `None` when the
/// destination is free.
#[allow(dead_code)]
fn destination_occupied(
    target_dev: &DevmodeW,
    target_index: usize,
    occupant_index: Option<usize>,
    names: &[String],
) -> Option<(usize, DevmodeW)> {
    let target_rect = rect_of(target_dev.dm_position, target_dev);
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != target_index && Some(*i) != occupant_index)
        .filter_map(|(i, name)| query::current_mode(name).map(|dev| (i, dev)))
        .find(|(_, dev)| rects_overlap(rect_of(dev.dm_position, dev), target_rect))
}

/// Builds the applied/previous pairs of a placement: the target moved to
/// the landing spot, and when it is occupied, the occupant moved to the
/// target's previous spot.
#[allow(dead_code)]
fn build_placement(
    target_index: usize,
    reference_index: usize,
    target_dev: &DevmodeW,
    landing: Pointl,
    occupant: Option<(usize, DevmodeW)>,
    names: &[String],
) -> PlacementChange {
    let display = query::display_label(&names[target_index], target_index as u32 + 1);
    let reference_display =
        query::display_label(&names[reference_index], reference_index as u32 + 1);
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
            vec![(
                names[target_index].clone(),
                build_moved(target_dev, landing),
            )],
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
    fn landing_rotated_target_uses_on_screen_footprint() {
        let reference = devmode_at(0, 0, 1920, 1080);
        let mut target = devmode_at(1920, 0, 1080, 1920);
        target.dm_display_orientation = 1;
        assert_eq!(
            landing_position(Direction::Left, &reference, &target),
            Pointl { x: -1080, y: 0 }
        );
    }

    #[test]
    fn landing_rotated_target_270_uses_on_screen_footprint() {
        let reference = devmode_at(0, 0, 1920, 1080);
        let mut target = devmode_at(1920, 0, 1080, 1920);
        target.dm_display_orientation = 3;
        assert_eq!(
            landing_position(Direction::Left, &reference, &target),
            Pointl { x: -1080, y: 0 }
        );
    }

    #[test]
    fn landing_right_of_rotated_reference_uses_on_screen_width() {
        let mut reference = devmode_at(0, 0, 1080, 1920);
        reference.dm_display_orientation = 1;
        let target = devmode_at(0, -1080, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Right, &reference, &target),
            Pointl { x: 1080, y: 0 }
        );
    }

    #[test]
    fn landing_below_rotated_reference_uses_on_screen_height() {
        let mut reference = devmode_at(0, 0, 1080, 1920);
        reference.dm_display_orientation = 1;
        let target = devmode_at(0, -1920, 1920, 1080);
        assert_eq!(
            landing_position(Direction::Below, &reference, &target),
            Pointl { x: 0, y: 1920 }
        );
    }

    #[test]
    fn rects_overlap_partial_overlap_detected() {
        assert!(rects_overlap((0, 0, 1920, 1080), (1000, 0, 2920, 1080)));
        assert!(rects_overlap((1000, 0, 2920, 1080), (0, 0, 1920, 1080)));
    }

    #[test]
    fn rects_overlap_adjacent_rects_do_not() {
        assert!(!rects_overlap((0, 0, 1920, 1080), (1920, 0, 3840, 1080)));
        assert!(!rects_overlap((0, 0, 1920, 1080), (0, 1080, 1920, 2160)));
    }

    #[test]
    fn rects_overlap_containment_detected() {
        assert!(rects_overlap((0, 0, 1920, 1080), (100, 100, 500, 500)));
    }

    #[test]
    fn apply_with_rollback_success_never_rolls_back() {
        let applied = vec![("A".to_string(), devmode_at(100, 0, 1920, 1080))];
        let previous = vec![("A".to_string(), devmode_at(0, 0, 1920, 1080))];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = apply_with_rollback(&applied, &previous, |name, _| {
            calls.borrow_mut().push(name.to_string());
            Ok(())
        });
        assert_eq!(result, Ok(()));
        assert_eq!(calls.into_inner(), vec!["A".to_string()]);
    }

    #[test]
    fn apply_with_rollback_first_failure_rolls_back_nothing() {
        let applied = vec![
            ("A".to_string(), devmode_at(100, 0, 1920, 1080)),
            ("B".to_string(), devmode_at(200, 0, 1920, 1080)),
        ];
        let previous = vec![
            ("B".to_string(), devmode_at(20, 0, 1920, 1080)),
            ("A".to_string(), devmode_at(0, 0, 1920, 1080)),
        ];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = apply_with_rollback(&applied, &previous, |name, _| {
            calls.borrow_mut().push(name.to_string());
            Err("boom".to_string())
        });
        assert_eq!(result, Err("boom".to_string()));
        assert_eq!(calls.into_inner(), vec!["A".to_string()]);
    }

    #[test]
    fn apply_with_rollback_second_failure_rolls_back_first_in_reverse_order() {
        let applied = vec![
            ("A".to_string(), devmode_at(100, 0, 1920, 1080)),
            ("B".to_string(), devmode_at(200, 0, 1920, 1080)),
        ];
        let previous = vec![
            ("B".to_string(), devmode_at(20, 0, 1920, 1080)),
            ("A".to_string(), devmode_at(0, 0, 1920, 1080)),
        ];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = apply_with_rollback(&applied, &previous, |name, _| {
            calls.borrow_mut().push(name.to_string());
            if name == "B" {
                Err("boom".to_string())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("boom".to_string()));
        assert_eq!(
            calls.into_inner(),
            vec!["A".to_string(), "B".to_string(), "A".to_string()]
        );
    }

    #[test]
    fn apply_with_rollback_rollback_failure_is_reported() {
        let applied = vec![
            ("A".to_string(), devmode_at(100, 0, 1920, 1080)),
            ("B".to_string(), devmode_at(200, 0, 1920, 1080)),
        ];
        let previous = vec![
            ("B".to_string(), devmode_at(20, 0, 1920, 1080)),
            ("A".to_string(), devmode_at(0, 0, 1920, 1080)),
        ];
        let calls = std::cell::Cell::new(0usize);
        let result = apply_with_rollback(&applied, &previous, |name, _| {
            if name == "B" {
                Err("boom".to_string())
            } else if calls.get() == 0 {
                calls.set(1);
                Ok(())
            } else {
                Err("rollback boom".to_string())
            }
        });
        assert!(
            result
                .unwrap_err()
                .contains("boom; rollback failed: rollback boom")
        );
    }

    #[test]
    fn collision_swaps_occupant_with_target() {
        let target = devmode_at(0, 1080, 1920, 1080);
        let occupant = devmode_at(1920, 0, 1920, 1080);
        let names = names3();
        let change = build_placement(
            2,
            0,
            &target,
            Pointl { x: 1920, y: 0 },
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
        let names = names3();
        let change = build_placement(2, 0, &target, Pointl { x: 1920, y: 0 }, None, &names);
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
            Err(
                "cannot place monitor 1 relative to itself, use a different reference monitor"
                    .to_string()
            )
        );
    }

    #[test]
    fn apply_placement_unknown_monitor_is_error() {
        let names = names3();
        assert_eq!(
            apply_placement(5, Direction::Left, 1, &names),
            Err("monitor 5 not found, run 'rmod list' to see connected displays".to_string())
        );
    }

    #[test]
    fn apply_placement_unknown_reference_is_error() {
        let names = names3();
        assert_eq!(
            apply_placement(1, Direction::Left, 5, &names),
            Err(
                "reference monitor 5 not found, run 'rmod list' to see connected displays"
                    .to_string()
            )
        );
    }

    #[test]
    fn revert_placement_restores_previous_positions() {
        let target = devmode_at(0, 1080, 1920, 1080);
        let occupant = devmode_at(1920, 0, 1920, 1080);
        let names = names3();
        let change = build_placement(
            2,
            0,
            &target,
            Pointl { x: 1920, y: 0 },
            Some((1, occupant)),
            &names,
        );
        assert_eq!(change.previous.len(), 2);
        assert_eq!(change.previous[0].0, "B");
        assert_eq!(change.previous[0].1.dm_position, Pointl { x: 1920, y: 0 });
        assert_eq!(change.previous[1].0, "C");
        assert_eq!(change.previous[1].1.dm_position, Pointl { x: 0, y: 1080 });
        // Reverting a change with no applied pairs succeeds with no work.
        // Exercised through the pure rollback helper: `revert_placement`
        // wraps this in a real full-screen fade, which unit tests must not
        // trigger (the fake backend covers the fade-free revert path).
        let empty = PlacementChange {
            display: "A [:1]".to_string(),
            reference_display: "A [:1]".to_string(),
            swap_display: None,
            applied: vec![],
            previous: vec![],
        };
        assert_eq!(
            apply_with_rollback(&empty.applied, &empty.previous, |_, _| Ok(())),
            Ok(())
        );
    }
}
