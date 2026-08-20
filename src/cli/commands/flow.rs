//! Keep-or-revert confirmation flows.
//!
//! [`confirm_or_revert`] and [`confirm_or_revert_all`] ask whether to keep
//! an applied display change (or batch), reverting to the previous mode
//! when declined; the `attach` and `placements` variants do the same for
//! attach/detach and layout changes. Each flow has an injectable `_with`
//! variant so the Revert branch is testable without touching the display.

use crate::cli::{Confirm, confirm_keep};
use crate::sys::windows::{self, AttachChange, Change, Mode, PlacementChange};

use super::{CONFIRM_TIMEOUT_SECS, describe_revert};

/// Runs the keep-or-revert confirmation for a single display; `yes` skips
/// the prompt. Reverts to the previous mode and prints the revert line.
///
/// Injectable variant of [`confirm_or_revert`]: the confirm prompt and the
/// revert call are supplied as closures so tests can exercise the Revert
/// branch without touching the display.
fn confirm_or_revert_with<C, R>(
    monitor: Option<u32>,
    change: Change,
    yes: bool,
    confirm: C,
    revert: R,
) -> i32
where
    C: FnOnce() -> Confirm,
    R: FnOnce(Option<u32>, Mode, Option<u32>) -> Result<Mode, String>,
{
    if yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => match revert(monitor, change.previous, change.previous_orientation) {
            Ok(mode) => {
                println!(
                    "{}",
                    describe_revert(&mode, change.previous_orientation, Some(&change.display))
                );
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}

/// Runs the keep-or-revert confirmation for a single display; `yes` skips
/// the prompt. Reverts to the previous mode and prints the revert line.
pub(crate) fn confirm_or_revert(monitor: Option<u32>, change: Change, yes: bool) -> i32 {
    confirm_or_revert_with(
        monitor,
        change,
        yes,
        || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
        windows::revert,
    )
}

/// Runs the keep-or-revert confirmation for a batch of displays; an empty
/// batch or `yes` skips the prompt. Reverts every change to its previous
/// mode, printing one revert line per display.
///
/// Injectable variant of [`confirm_or_revert_all`]: the confirm prompt and
/// the revert call are supplied as closures so tests can exercise the
/// Revert branch without touching the display.
fn confirm_or_revert_all_with<C, R>(applied: Vec<Change>, yes: bool, confirm: C, revert: R) -> i32
where
    C: FnOnce() -> Confirm,
    R: Fn(Option<u32>, Mode, Option<u32>) -> Result<Mode, String>,
{
    if applied.is_empty() || yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => {
            let mut failed = false;
            for change in applied {
                match revert(
                    Some(change.monitor),
                    change.previous,
                    change.previous_orientation,
                ) {
                    Ok(mode) => println!(
                        "{}",
                        describe_revert(&mode, change.previous_orientation, Some(&change.display))
                    ),
                    Err(e) => {
                        eprintln!("error: {e}");
                        failed = true;
                    }
                }
            }
            if failed { 2 } else { 0 }
        }
    }
}

/// Runs the keep-or-revert confirmation for a batch of displays; an empty
/// batch or `yes` skips the prompt. Reverts every change to its previous
/// mode, printing one revert line per display.
pub(crate) fn confirm_or_revert_all(applied: Vec<Change>, yes: bool) -> i32 {
    confirm_or_revert_all_with(
        applied,
        yes,
        || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
        windows::revert,
    )
}

/// Describes an attach/detach revert: "re-attached {display}" or
/// "re-detached {display}" depending on the mode that was restored.
fn describe_attach_revert(change: &AttachChange) -> String {
    if change.previous.dm_pels_width > 0 {
        format!("re-attached {}", change.display)
    } else {
        format!("re-detached {}", change.display)
    }
}

/// Runs the keep-or-revert confirmation for an attach/detach change;
/// `yes` skips the prompt. Reverts by re-applying the previous device mode
/// and prints the revert line.
///
/// Injectable variant of [`confirm_or_revert_attach`]: the confirm prompt
/// and the revert call are supplied as closures so tests can exercise the
/// Revert branch without touching the display.
fn confirm_or_revert_attach_with<C, R>(
    change: AttachChange,
    yes: bool,
    confirm: C,
    revert: R,
) -> i32
where
    C: FnOnce() -> Confirm,
    R: FnOnce(&AttachChange) -> Result<(), String>,
{
    if yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => match revert(&change) {
            Ok(()) => {
                println!("{}", describe_attach_revert(&change));
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}

/// Runs the keep-or-revert confirmation for an attach/detach change;
/// `yes` skips the prompt. Reverts by re-applying the previous device mode
/// and prints the revert line.
pub(crate) fn confirm_or_revert_attach(change: AttachChange, yes: bool) -> i32 {
    confirm_or_revert_attach_with(
        change,
        yes,
        || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
        windows::revert_attach,
    )
}

/// Runs the keep-or-revert confirmation for a batch of attach/detach
/// changes; an empty batch or `yes` skips the prompt. Reverts every change
/// to its previous device mode, printing one revert line per display.
///
/// Injectable variant of [`confirm_or_revert_attach_all`].
fn confirm_or_revert_attach_all_with<C, R>(
    applied: Vec<AttachChange>,
    yes: bool,
    confirm: C,
    revert: R,
) -> i32
where
    C: FnOnce() -> Confirm,
    R: Fn(&AttachChange) -> Result<(), String>,
{
    if applied.is_empty() || yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => {
            let mut failed = false;
            for change in &applied {
                match revert(change) {
                    Ok(()) => println!("{}", describe_attach_revert(change)),
                    Err(e) => {
                        eprintln!("error: {e}");
                        failed = true;
                    }
                }
            }
            if failed { 2 } else { 0 }
        }
    }
}

/// Runs the keep-or-revert confirmation for a batch of attach/detach
/// changes; an empty batch or `yes` skips the prompt. Reverts every change
/// to its previous device mode, printing one revert line per display.
pub(crate) fn confirm_or_revert_attach_all(applied: Vec<AttachChange>, yes: bool) -> i32 {
    confirm_or_revert_attach_all_with(
        applied,
        yes,
        || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
        windows::revert_attach,
    )
}

/// Runs the keep-or-revert confirmation for a batch of placement changes;
/// an empty batch or `yes` skips the prompt. Reverts every change to the
/// previous layout, printing one revert line per change.
///
/// Injectable variant of [`confirm_or_revert_placements`]: the confirm
/// prompt and the revert call are supplied as closures so tests can
/// exercise the Revert branch without touching the display.
fn confirm_or_revert_placements_with<C, R>(
    applied: Vec<PlacementChange>,
    yes: bool,
    confirm: C,
    revert: R,
) -> i32
where
    C: FnOnce() -> Confirm,
    R: Fn(&PlacementChange) -> Result<(), String>,
{
    if applied.is_empty() || yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => {
            let mut failed = false;
            for change in &applied {
                match revert(change) {
                    Ok(()) => println!("reverted to the previous layout"),
                    Err(e) => {
                        eprintln!("error: {e}");
                        failed = true;
                    }
                }
            }
            if failed { 2 } else { 0 }
        }
    }
}

/// Runs the keep-or-revert confirmation for a batch of placement changes;
/// an empty batch or `yes` skips the prompt. Reverts every change to the
/// previous layout, printing one revert line per change.
pub(crate) fn confirm_or_revert_placements(applied: Vec<PlacementChange>, yes: bool) -> i32 {
    confirm_or_revert_placements_with(
        applied,
        yes,
        || confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
        windows::revert_placement,
    )
}

/// Runs the keep-or-revert confirmation for a project-mode change set: an
/// attach/detach batch plus an optional primary promotion captured by
/// [`windows::make_main`]. On revert the promotion is undone first (so the
/// desktop returns to the original primary), then each attach change is
/// reverted. An empty batch and no promotion (or `yes`) skips the prompt.
pub(crate) fn confirm_or_revert_project(
    applied: Vec<AttachChange>,
    main_change: Option<windows::MainChange<'_>>,
    yes: bool,
) -> i32 {
    confirm_or_revert_project_with(applied, main_change, yes, || {
        confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS))
    })
}

/// Injectable variant of [`confirm_or_revert_project`].
fn confirm_or_revert_project_with<C>(
    applied: Vec<AttachChange>,
    main_change: Option<windows::MainChange<'_>>,
    yes: bool,
    confirm: C,
) -> i32
where
    C: FnOnce() -> Confirm,
{
    if (applied.is_empty() && main_change.is_none()) || yes {
        return 0;
    }
    match confirm() {
        Confirm::Keep => 0,
        Confirm::Revert => {
            let mut failed = false;
            if let Some(change) = main_change {
                match windows::revert_main(&change) {
                    Ok(()) => println!("{}", describe_main_revert(&change)),
                    Err(e) => {
                        eprintln!("error: {e}");
                        failed = true;
                    }
                }
            }
            for change in &applied {
                match windows::revert_attach(change) {
                    Ok(()) => println!("{}", describe_attach_revert(change)),
                    Err(e) => {
                        eprintln!("error: {e}");
                        failed = true;
                    }
                }
            }
            if failed { 2 } else { 0 }
        }
    }
}

/// Human-readable revert line for a primary promotion.
fn describe_main_revert(change: &windows::MainChange<'_>) -> String {
    format!("{} reverted to primary display", change.display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::bindings::DevmodeW;
    use crate::sys::windows::{AttachAction, AttachChange, Change, Mode, PlacementChange};

    fn mode(width: u32, height: u32, refresh: u32) -> Mode {
        Mode {
            width,
            height,
            refresh,
        }
    }

    fn change(
        mode: Mode,
        previous: Mode,
        orientation: Option<u32>,
        previous_orientation: Option<u32>,
    ) -> Change {
        Change {
            monitor: 1,
            display: "Generic PnP Monitor [:1]".to_string(),
            mode,
            previous,
            orientation,
            previous_orientation,
        }
    }

    fn attach_change(action: AttachAction, previous_width: u32) -> AttachChange {
        let mut previous: DevmodeW = unsafe { std::mem::zeroed() };
        previous.dm_pels_width = previous_width;
        AttachChange {
            monitor: 2,
            display: "Generic PnP Monitor [:2]".to_string(),
            action,
            previous,
        }
    }

    fn placement_change(display: &str) -> PlacementChange {
        PlacementChange {
            display: display.to_string(),
            reference_display: "Generic PnP Monitor [:1]".to_string(),
            swap_display: None,
            applied: vec![("\\\\.\\DISPLAY2".to_string(), unsafe { std::mem::zeroed() })],
            previous: vec![("\\\\.\\DISPLAY2".to_string(), unsafe { std::mem::zeroed() })],
        }
    }

    #[test]
    fn confirm_or_revert_yes_skips_confirm_and_revert() {
        assert_eq!(
            confirm_or_revert_with(
                Some(1),
                change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_, _, _| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_keep_skips_revert() {
        assert_eq!(
            confirm_or_revert_with(
                Some(1),
                change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
                false,
                || Confirm::Keep,
                |_, _, _| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_revert_calls_revert_with_previous_mode() {
        assert_eq!(
            confirm_or_revert_with(
                Some(2),
                change(mode(1920, 1080, 60), mode(1280, 720, 60), None, Some(90)),
                false,
                || Confirm::Revert,
                |monitor, previous, previous_orientation| {
                    assert_eq!(monitor, Some(2));
                    assert_eq!(previous, mode(1280, 720, 60));
                    assert_eq!(previous_orientation, Some(90));
                    Ok(mode(1920, 1080, 60))
                },
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_revert_error_returns_2() {
        assert_eq!(
            confirm_or_revert_with(
                Some(1),
                change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
                false,
                || Confirm::Revert,
                |_, _, _| Err("boom".to_string()),
            ),
            2
        );
    }

    #[test]
    fn confirm_or_revert_all_empty_skips_confirm_and_revert() {
        assert_eq!(
            confirm_or_revert_all_with(
                Vec::new(),
                false,
                || panic!("confirm must be skipped for an empty batch"),
                |_, _, _| panic!("revert must be skipped for an empty batch"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_all_yes_skips_confirm_and_revert() {
        let applied = vec![
            change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
            change(mode(1920, 1080, 144), mode(1280, 720, 60), None, None),
        ];
        assert_eq!(
            confirm_or_revert_all_with(
                applied,
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_, _, _| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_all_keep_skips_revert() {
        let applied = vec![
            change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
            change(mode(1920, 1080, 144), mode(1280, 720, 60), None, None),
        ];
        assert_eq!(
            confirm_or_revert_all_with(
                applied,
                false,
                || Confirm::Keep,
                |_, _, _| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_all_revert_reverts_every_change() {
        let applied = vec![
            change(mode(1920, 1080, 60), mode(1280, 720, 60), None, Some(90)),
            change(mode(1920, 1080, 144), mode(1024, 768, 60), None, None),
        ];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = confirm_or_revert_all_with(
            applied,
            false,
            || Confirm::Revert,
            |monitor, previous, previous_orientation| {
                calls
                    .borrow_mut()
                    .push((monitor, previous, previous_orientation));
                Ok(mode(1920, 1080, 60))
            },
        );
        assert_eq!(result, 0);
        let calls = calls.into_inner();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], (Some(1), mode(1280, 720, 60), Some(90)));
        assert_eq!(calls[1], (Some(1), mode(1024, 768, 60), None));
    }

    #[test]
    fn confirm_or_revert_all_second_revert_error_returns_2() {
        let applied = vec![
            change(mode(1920, 1080, 60), mode(1280, 720, 60), None, None),
            change(mode(1920, 1080, 144), mode(1024, 768, 60), None, None),
        ];
        let calls = std::cell::Cell::new(0);
        let result = confirm_or_revert_all_with(
            applied,
            false,
            || Confirm::Revert,
            |_, _, _| {
                let n = calls.get() + 1;
                calls.set(n);
                if n == 2 {
                    Err("boom".to_string())
                } else {
                    Ok(mode(1920, 1080, 60))
                }
            },
        );
        assert_eq!(result, 2);
    }

    #[test]
    fn describe_attach_revert_re_attached_when_previous_attached() {
        assert_eq!(
            describe_attach_revert(&attach_change(AttachAction::Disable, 1920)),
            "re-attached Generic PnP Monitor [:2]"
        );
    }

    #[test]
    fn describe_attach_revert_re_detached_when_previous_detached() {
        assert_eq!(
            describe_attach_revert(&attach_change(AttachAction::Enable, 0)),
            "re-detached Generic PnP Monitor [:2]"
        );
    }

    #[test]
    fn confirm_or_revert_attach_yes_skips_confirm_and_revert() {
        assert_eq!(
            confirm_or_revert_attach_with(
                attach_change(AttachAction::Disable, 1920),
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_attach_keep_skips_revert() {
        assert_eq!(
            confirm_or_revert_attach_with(
                attach_change(AttachAction::Disable, 1920),
                false,
                || Confirm::Keep,
                |_| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_attach_revert_calls_revert_with_change() {
        let captured = std::cell::RefCell::new(None);
        let result = confirm_or_revert_attach_with(
            attach_change(AttachAction::Disable, 1920),
            false,
            || Confirm::Revert,
            |change| {
                assert_eq!(change.monitor, 2);
                assert_eq!(change.action, AttachAction::Disable);
                assert_eq!(change.previous.dm_pels_width, 1920);
                captured.borrow_mut().replace(change.display.clone());
                Ok(())
            },
        );
        assert_eq!(result, 0);
        assert_eq!(
            captured.into_inner(),
            Some("Generic PnP Monitor [:2]".to_string())
        );
    }

    #[test]
    fn confirm_or_revert_attach_revert_error_returns_2() {
        assert_eq!(
            confirm_or_revert_attach_with(
                attach_change(AttachAction::Enable, 0),
                false,
                || Confirm::Revert,
                |_| Err("boom".to_string()),
            ),
            2
        );
    }

    #[test]
    fn confirm_or_revert_attach_all_empty_skips_confirm_and_revert() {
        assert_eq!(
            confirm_or_revert_attach_all_with(
                Vec::new(),
                false,
                || panic!("confirm must be skipped for an empty batch"),
                |_| panic!("revert must be skipped for an empty batch"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_attach_all_yes_skips_confirm_and_revert() {
        let applied = vec![
            attach_change(AttachAction::Disable, 1920),
            attach_change(AttachAction::Enable, 0),
        ];
        assert_eq!(
            confirm_or_revert_attach_all_with(
                applied,
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_attach_all_keep_skips_revert() {
        let applied = vec![attach_change(AttachAction::Disable, 1920)];
        assert_eq!(
            confirm_or_revert_attach_all_with(
                applied,
                false,
                || Confirm::Keep,
                |_| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_attach_all_revert_reverts_every_change() {
        let applied = vec![
            attach_change(AttachAction::Disable, 1920),
            attach_change(AttachAction::Enable, 0),
        ];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = confirm_or_revert_attach_all_with(
            applied,
            false,
            || Confirm::Revert,
            |change| {
                calls.borrow_mut().push(change.display.clone());
                Ok(())
            },
        );
        assert_eq!(result, 0);
        let calls = calls.into_inner();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], "Generic PnP Monitor [:2]");
        assert_eq!(calls[1], "Generic PnP Monitor [:2]");
    }

    #[test]
    fn confirm_or_revert_attach_all_second_revert_error_returns_2() {
        let applied = vec![
            attach_change(AttachAction::Disable, 1920),
            attach_change(AttachAction::Enable, 0),
        ];
        let calls = std::cell::Cell::new(0);
        let result = confirm_or_revert_attach_all_with(
            applied,
            false,
            || Confirm::Revert,
            |_| {
                let n = calls.get() + 1;
                calls.set(n);
                if n == 2 {
                    Err("boom".to_string())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, 2);
    }

    #[test]
    fn confirm_or_revert_placements_empty_skips_confirm_and_revert() {
        assert_eq!(
            confirm_or_revert_placements_with(
                Vec::new(),
                false,
                || panic!("confirm must be skipped for an empty batch"),
                |_| panic!("revert must be skipped for an empty batch"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_placements_yes_skips_confirm_and_revert() {
        let applied = vec![
            placement_change("Generic PnP Monitor [:2]"),
            placement_change("Generic PnP Monitor [:3]"),
        ];
        assert_eq!(
            confirm_or_revert_placements_with(
                applied,
                true,
                || panic!("confirm must be skipped when yes is set"),
                |_| panic!("revert must be skipped when yes is set"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_placements_keep_skips_revert() {
        let applied = vec![placement_change("Generic PnP Monitor [:2]")];
        assert_eq!(
            confirm_or_revert_placements_with(
                applied,
                false,
                || Confirm::Keep,
                |_| panic!("revert must be skipped on Keep"),
            ),
            0
        );
    }

    #[test]
    fn confirm_or_revert_placements_revert_reverts_every_change() {
        let applied = vec![
            placement_change("Generic PnP Monitor [:2]"),
            placement_change("Generic PnP Monitor [:3]"),
        ];
        let calls = std::cell::RefCell::new(Vec::new());
        let result = confirm_or_revert_placements_with(
            applied,
            false,
            || Confirm::Revert,
            |change| {
                calls.borrow_mut().push(change.display.clone());
                Ok(())
            },
        );
        assert_eq!(result, 0);
        let calls = calls.into_inner();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], "Generic PnP Monitor [:2]");
        assert_eq!(calls[1], "Generic PnP Monitor [:3]");
    }

    #[test]
    fn confirm_or_revert_placements_second_revert_error_returns_2() {
        let applied = vec![
            placement_change("Generic PnP Monitor [:2]"),
            placement_change("Generic PnP Monitor [:3]"),
        ];
        let calls = std::cell::Cell::new(0);
        let result = confirm_or_revert_placements_with(
            applied,
            false,
            || Confirm::Revert,
            |_| {
                let n = calls.get() + 1;
                calls.set(n);
                if n == 2 {
                    Err("boom".to_string())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, 2);
    }
}
