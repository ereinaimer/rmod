//! `main` command: promotes a display to the main (primary) display.
//!
//! [`run_main`] applies the position swap via [`crate::sys::windows::make_main`],
//! reports the outcome, and runs the shared keep-or-revert confirmation flow.

use crate::cli::{Confirm, confirm_keep};
use crate::sys::windows::{MainChange, MainOutcome, make_main, revert_main};

use super::CONFIRM_TIMEOUT_SECS;

/// Promotes the targeted monitor to the main display.
pub(super) fn run_main(monitor: u32, yes: bool) -> i32 {
    let names = crate::sys::windows::enumerate_devices();
    match make_main(monitor, &names) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::MainChange;

    fn main_change() -> MainChange<'static> {
        MainChange {
            monitor: 2,
            display: "RMOD Fake Monitor 2".to_string(),
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
}
