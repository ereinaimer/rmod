//! `main` command: promotes a display to the main (primary) display.
//!
//! [`run_main`] applies the position swap via [`crate::sys::windows::make_main`],
//! reports the outcome, and runs the shared keep-or-revert confirmation flow.

use crate::cli::{Confirm, Target, confirm_keep};
use crate::sys::windows::{MainOutcome, make_main, revert_main};

use super::CONFIRM_TIMEOUT_SECS;

/// Promotes the targeted monitor to the main display.
pub(super) fn run_main(target: Target, yes: bool) -> i32 {
    let Target::Index(monitor) = target else {
        unreachable!()
    };
    match make_main(monitor) {
        Ok(MainOutcome::Unchanged(display)) => {
            println!("{display} is already the main display");
            0
        }
        Ok(MainOutcome::Applied(change)) => {
            println!("{} is now the main display", change.display);
            if yes {
                return 0;
            }
            match confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)) {
                Confirm::Keep => 0,
                Confirm::Revert => match revert_main(&change) {
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
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}
