//! Command dispatch and display-change reporting.
//!
//! [`run`] executes a parsed [`Command`]: the per-command runners in the
//! `ls`, `caps`, `max` and `set` submodules apply the change via
//! [`crate::sys::windows`], report the outcome, and run the shared
//! keep-or-revert confirmation flow.

mod caps;
mod ls;
mod main;
mod max;
mod set;

// Import via `crate::cli`, not `crate::cli::help`, so the `pub use help::{...}` re-exports in `crate::cli` stay referenced (direct imports would dead-code them and trip clippy).
use crate::cli::{
    Command, Confirm, HelpTopic, Target, caps as caps_help, confirm_keep, help, ls as ls_help,
    main_help, max as max_help, set as set_help, version,
};
use crate::sys::windows::{self, Change, Mode};

use caps::run_caps;
use ls::run_list;
use main::run_main;
use max::run_max;
use set::run_set;

const CONFIRM_TIMEOUT_SECS: u64 = 5;

/// Runs a parsed command and returns the process exit code (0 success, 2
/// error).
pub fn run(command: Command) -> i32 {
    match command {
        Command::Help { topic: None } => {
            println!("{}", help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::List),
        } => {
            println!("{}", ls_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Max),
        } => {
            println!("{}", max_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Caps),
        } => {
            println!("{}", caps_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Set),
        } => {
            println!("{}", set_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Main),
        } => {
            println!("{}", main_help());
            0
        }
        Command::Version => {
            println!("{}", version());
            0
        }
        Command::List => run_list(),
        Command::Caps { target } => run_caps(target),
        Command::Max { target, yes } => run_max(target, yes),
        Command::Main { target, yes } => run_main(target, yes),
        Command::Set {
            width,
            height,
            refresh,
            orientation,
            target,
            yes,
        } => run_set(width, height, refresh, orientation, target, yes),
    }
}

/// Maps a command target to the monitor number [`crate::sys::windows`]
/// expects; the primary display is `None`.
fn monitor_of(target: Target) -> Option<u32> {
    match target {
        Target::Primary => None,
        Target::Index(n) => Some(n),
        Target::All => unreachable!(),
    }
}

/// Describes a display-change outcome.
///
/// Produces the "already at", "already rotated", "applied" and "rotated"
/// lines, optionally prefixed with the display label; a `, rotated {a}°`
/// suffix is appended when the change carried an orientation request.
/// `mode_requested` selects between "already rotated" and the plain
/// "already at" line when the change reports the current orientation.
fn describe_outcome(change: &Change, display: Option<&str>, mode_requested: bool) -> String {
    let rotated = match (change.orientation, change.previous_orientation) {
        (Some(angle), Some(previous)) => angle != previous,
        _ => false,
    };
    if change.mode == change.previous && !rotated {
        if !mode_requested && let Some(angle) = change.orientation {
            return match display {
                Some(name) => format!("{name} is already rotated {angle}°"),
                None => format!("already rotated {angle}°"),
            };
        }
        let mut message = match display {
            Some(name) => format!(
                "{name} is already at {}x{} @ {}Hz",
                change.mode.width, change.mode.height, change.mode.refresh
            ),
            None => format!(
                "already at {}x{} @ {}Hz",
                change.mode.width, change.mode.height, change.mode.refresh
            ),
        };
        if let Some(angle) = change.orientation {
            message.push_str(&format!(", rotated {angle}°"));
        }
        return message;
    }
    if change.mode == change.previous {
        let angle = change
            .orientation
            .expect("an orientation-only change has an angle");
        return match display {
            Some(name) => format!("rotated {name} to {angle}°"),
            None => format!("rotated {angle}°"),
        };
    }
    let mut message = match display {
        Some(name) => format!(
            "applied {}x{} @ {}Hz to {name}",
            change.mode.width, change.mode.height, change.mode.refresh
        ),
        None => format!(
            "applied {}x{} @ {}Hz",
            change.mode.width, change.mode.height, change.mode.refresh
        ),
    };
    if let Some(angle) = change.orientation {
        message.push_str(&format!(", rotated {angle}°"));
    }
    message
}

/// Describes a revert to a previous mode, optionally prefixed with the
/// display label; a `, rotated {prev}°` suffix is appended when the change
/// carried an orientation request.
fn describe_revert(
    mode: &Mode,
    previous_orientation: Option<u32>,
    display: Option<&str>,
) -> String {
    let mut message = match display {
        Some(name) => format!(
            "reverted {name} to {}x{} @ {}Hz",
            mode.width, mode.height, mode.refresh
        ),
        None => format!(
            "reverted to {}x{} @ {}Hz",
            mode.width, mode.height, mode.refresh
        ),
    };
    if let Some(prev) = previous_orientation {
        message.push_str(&format!(", rotated {prev}°"));
    }
    message
}

/// Runs the keep-or-revert confirmation for a single display; `yes` skips
/// the prompt. Reverts to the previous mode and prints the revert line.
fn confirm_or_revert(monitor: Option<u32>, change: Change, yes: bool) -> i32 {
    if yes {
        return 0;
    }
    match confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)) {
        Confirm::Keep => 0,
        Confirm::Revert => {
            match windows::revert(monitor, change.previous, change.previous_orientation) {
                Ok(mode) => {
                    println!(
                        "{}",
                        describe_revert(&mode, change.previous_orientation, None)
                    );
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
    }
}

/// Runs the keep-or-revert confirmation for a batch of displays; an empty
/// batch or `yes` skips the prompt. Reverts every change to its previous
/// mode, printing one revert line per display.
fn confirm_or_revert_all(applied: Vec<Change>, yes: bool) -> i32 {
    if applied.is_empty() || yes {
        return 0;
    }
    match confirm_keep(std::time::Duration::from_secs(CONFIRM_TIMEOUT_SECS)) {
        Confirm::Keep => 0,
        Confirm::Revert => {
            let mut failed = false;
            for change in applied {
                match windows::revert(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::{Change, Mode};

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

    #[test]
    fn unchanged_without_display_uses_already_at() {
        assert_eq!(
            describe_outcome(
                &change(mode(1920, 1080, 60), mode(1920, 1080, 60), None, None),
                None,
                true
            ),
            "already at 1920x1080 @ 60Hz"
        );
    }

    #[test]
    fn unchanged_with_display_uses_is_already_at() {
        assert_eq!(
            describe_outcome(
                &change(mode(1920, 1080, 60), mode(1920, 1080, 60), None, None),
                Some("AOC 24G2 [:1]"),
                true
            ),
            "AOC 24G2 [:1] is already at 1920x1080 @ 60Hz"
        );
    }

    #[test]
    fn unchanged_without_display_uses_already_rotated() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(90)
                ),
                None,
                false
            ),
            "already rotated 90°"
        );
    }

    #[test]
    fn unchanged_with_display_uses_is_already_rotated() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(90)
                ),
                Some("AOC 24G2 [:1]"),
                false
            ),
            "AOC 24G2 [:1] is already rotated 90°"
        );
    }

    #[test]
    fn unchanged_with_mode_request_appends_rotation_suffix() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(90)
                ),
                None,
                true
            ),
            "already at 1920x1080 @ 60Hz, rotated 90°"
        );
    }

    #[test]
    fn applied_without_display_uses_applied() {
        let previous = mode(1280, 720, 60);
        let applied = mode(1920, 1080, 144);
        assert_eq!(
            describe_outcome(&change(applied, previous, None, None), None, true),
            "applied 1920x1080 @ 144Hz"
        );
    }

    #[test]
    fn applied_with_display_uses_applied_to() {
        let previous = mode(1280, 720, 60);
        let applied = mode(1920, 1080, 144);
        assert_eq!(
            describe_outcome(
                &change(applied, previous, None, None),
                Some("AOC 24G2 [:1]"),
                true
            ),
            "applied 1920x1080 @ 144Hz to AOC 24G2 [:1]"
        );
    }

    #[test]
    fn applied_orientation_only_without_display_uses_rotated() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(0)
                ),
                None,
                true
            ),
            "rotated 90°"
        );
    }

    #[test]
    fn applied_orientation_only_with_display_uses_rotated_to() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(0)
                ),
                Some("AOC 24G2 [:1]"),
                true
            ),
            "rotated AOC 24G2 [:1] to 90°"
        );
    }

    #[test]
    fn applied_with_rotation_request_appends_rotation_suffix() {
        let previous = mode(1280, 720, 60);
        let applied = mode(1920, 1080, 144);
        assert_eq!(
            describe_outcome(&change(applied, previous, Some(90), Some(0)), None, true),
            "applied 1920x1080 @ 144Hz, rotated 90°"
        );
    }

    #[test]
    fn revert_without_display_uses_reverted_to() {
        assert_eq!(
            describe_revert(&mode(1920, 1080, 60), None, None),
            "reverted to 1920x1080 @ 60Hz"
        );
    }

    #[test]
    fn revert_with_display_uses_reverted_display_to() {
        assert_eq!(
            describe_revert(&mode(1920, 1080, 60), None, Some("AOC 24G2 [:1]")),
            "reverted AOC 24G2 [:1] to 1920x1080 @ 60Hz"
        );
    }

    #[test]
    fn revert_with_previous_orientation_appends_rotation_suffix() {
        assert_eq!(
            describe_revert(&mode(1920, 1080, 60), Some(90), None),
            "reverted to 1920x1080 @ 60Hz, rotated 90°"
        );
    }

    #[test]
    fn unchanged_with_display_and_mode_request_appends_rotation_suffix() {
        assert_eq!(
            describe_outcome(
                &change(
                    mode(1920, 1080, 60),
                    mode(1920, 1080, 60),
                    Some(90),
                    Some(90)
                ),
                Some("AOC 24G2 [:1]"),
                true
            ),
            "AOC 24G2 [:1] is already at 1920x1080 @ 60Hz, rotated 90°"
        );
    }

    #[test]
    fn applied_with_display_and_rotation_request_appends_rotation_suffix() {
        let previous = mode(1280, 720, 60);
        let applied = mode(1920, 1080, 144);
        assert_eq!(
            describe_outcome(
                &change(applied, previous, Some(90), Some(0)),
                Some("AOC 24G2 [:1]"),
                true
            ),
            "applied 1920x1080 @ 144Hz to AOC 24G2 [:1], rotated 90°"
        );
    }

    #[test]
    fn revert_with_display_and_previous_orientation_appends_rotation_suffix() {
        assert_eq!(
            describe_revert(&mode(1920, 1080, 60), Some(90), Some("AOC 24G2 [:1]")),
            "reverted AOC 24G2 [:1] to 1920x1080 @ 60Hz, rotated 90°"
        );
    }
}
