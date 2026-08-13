//! `set` command: applies a resolution, refresh and orientation policy to
//! the targeted display(s).
//!
//! [`run_set`] reports the outcome per display, then runs the shared
//! keep-or-revert confirmation flow for the changes it applied.

use crate::cli::Target;
use crate::sys::windows::{self, ApplyOutcome, Refresh};

use super::{confirm_or_revert, confirm_or_revert_all, describe_outcome, monitor_of};

/// Applies a resolution, refresh and orientation policy to the targeted
/// display(s).
pub(super) fn run_set(
    width: Option<u32>,
    height: Option<u32>,
    refresh: Refresh,
    orientation: Option<u32>,
    target: Target,
    yes: bool,
) -> i32 {
    let mode_requested = width.is_some() || height.is_some() || refresh != Refresh::Keep;
    match target {
        Target::Primary | Target::Index(_) => {
            let monitor = monitor_of(target);
            match windows::set(monitor, width, height, refresh, orientation) {
                Ok(ApplyOutcome::Unchanged(change)) => {
                    println!("{}", describe_outcome(&change, None, mode_requested));
                    0
                }
                Ok(ApplyOutcome::Applied(change)) => {
                    println!("{}", describe_outcome(&change, None, mode_requested));
                    confirm_or_revert(monitor, change, yes)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        Target::All => match windows::set_all(width, height, refresh, orientation) {
            Ok(outcomes) => {
                let mut applied = Vec::new();
                for outcome in outcomes {
                    match outcome {
                        ApplyOutcome::Unchanged(change) => println!(
                            "{}",
                            describe_outcome(&change, Some(&change.display), mode_requested)
                        ),
                        ApplyOutcome::Applied(change) => {
                            println!(
                                "{}",
                                describe_outcome(&change, Some(&change.display), mode_requested)
                            );
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
