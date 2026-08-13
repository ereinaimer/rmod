//! `max` command: applies the best supported mode to the targeted
//! display(s).
//!
//! [`run_max`] reports the outcome per display, then runs the shared
//! keep-or-revert confirmation flow for the changes it applied.

use crate::cli::Target;
use crate::sys::windows::{self, ApplyOutcome};

use super::{confirm_or_revert, confirm_or_revert_all, describe_outcome, monitor_of};

/// Applies the best supported mode to the targeted display(s).
pub(super) fn run_max(target: Target, yes: bool) -> i32 {
    match target {
        Target::Primary | Target::Index(_) => {
            let monitor = monitor_of(target);
            match windows::max(monitor) {
                Ok(ApplyOutcome::Unchanged(change)) => {
                    println!("{}", describe_outcome(&change, None, true));
                    0
                }
                Ok(ApplyOutcome::Applied(change)) => {
                    println!("{}", describe_outcome(&change, None, true));
                    confirm_or_revert(monitor, change, yes)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        Target::All => match windows::max_all() {
            Ok(outcomes) => {
                let mut applied = Vec::new();
                for outcome in outcomes {
                    match outcome {
                        ApplyOutcome::Unchanged(change) => {
                            println!("{}", describe_outcome(&change, Some(&change.display), true))
                        }
                        ApplyOutcome::Applied(change) => {
                            println!("{}", describe_outcome(&change, Some(&change.display), true));
                            applied.push(change);
                        }
                    }
                }
                confirm_or_revert_all(applied, yes)
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
    }
}
