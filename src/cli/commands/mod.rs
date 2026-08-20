//! Command dispatch and display-change reporting.
//!
//! [`run`] executes a parsed [`Command`]: the per-command runners in the
//! `layout`, `ls` and `set` submodules apply the change via
//! [`crate::sys::windows`], report the outcome, and run the shared
//! keep-or-revert confirmation flow.

pub(crate) mod completions;
pub(crate) mod flow;
pub(crate) mod layout;
pub(crate) mod ls;
pub(crate) mod monitor;
pub(crate) mod set;
pub(crate) mod temp;
pub(crate) mod view;

use crate::cli::{
    Command, HelpTopic, MonitorAction, MonitorTarget, ViewAction, completions, help,
    layout as layout_help, ls, monitor as monitor_help, monitor_attach, monitor_brightness,
    monitor_contrast, monitor_detach, set as set_help, temp as temp_help, version, view,
    view_extend_help, view_mirror_help, view_project_help, view_single_help,
};
use crate::sys::windows::{AttachAction, AttachChange, Change, Mode};

use completions::run_completions;
use flow::{
    confirm_or_revert, confirm_or_revert_all, confirm_or_revert_attach,
    confirm_or_revert_attach_all, confirm_or_revert_project,
};
use layout::run_layout;
use ls::{run_list, run_list_short};
use monitor::run_monitor;
use set::run_set;
use temp::run_temp;
use view::run_view;

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
            println!("{}", ls());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Set),
        } => {
            println!("{}", set_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Layout),
        } => {
            println!("{}", layout_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Monitor { action }),
        } => {
            let page = match action {
                Some(MonitorAction::Disable) => monitor_detach(),
                Some(MonitorAction::Enable) => monitor_attach(),
                Some(MonitorAction::Brightness { .. }) => monitor_brightness(),
                Some(MonitorAction::Contrast { .. }) => monitor_contrast(),
                _ => monitor_help(),
            };
            println!("{page}");
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Temp),
        } => {
            println!("{}", temp_help());
            0
        }
        Command::Help {
            topic: Some(HelpTopic::View { action }),
        } => {
            let page = match action {
                Some(ViewAction::Mirror) => view_mirror_help(),
                Some(ViewAction::Extend) => view_extend_help(),
                Some(ViewAction::Project) => view_project_help(),
                Some(ViewAction::Single { .. }) => view_single_help(),
                _ => view(),
            };
            println!("{page}");
            0
        }
        Command::Help {
            topic: Some(HelpTopic::Completions),
        } => {
            println!("{}", completions());
            0
        }
        Command::Version => {
            println!("{}", version());
            0
        }
        Command::List { short, all } => {
            if short {
                run_list_short()
            } else {
                run_list(all)
            }
        }
        Command::Layout { action, yes } => run_layout(action, yes),
        Command::Set {
            spec,
            monitor,
            orientation,
            yes,
        } => run_set(spec, monitor, orientation, yes),
        Command::Monitor {
            action,
            monitor,
            yes,
        } => run_monitor(action, monitor, yes),
        Command::Temp { action, monitor } => run_temp(action, monitor),
        Command::View { action, yes } => run_view(action, yes),
        Command::Completions { help } => run_completions(help),
    }
}

/// Resolves a MonitorTarget to a monitor index (1-based) for backend calls;
/// the primary display is `None`. An unknown monitor id is a hard error.
/// Uses `enumerate_all_devices` to include detached monitors.
///
/// # Errors
/// Returns `Err` when an id matches no display (attached or detached).
pub fn resolve_target_all(target: &MonitorTarget) -> Result<Option<u32>, String> {
    match target {
        MonitorTarget::Primary => Ok(None),
        MonitorTarget::All => unreachable!(),
        MonitorTarget::Index(n) => crate::sys::windows::resolve_device(
            Some(*n),
            &crate::sys::windows::enumerate_all_devices(),
        )
        .map(|(index, _)| Some(index as u32 + 1)),
        MonitorTarget::Id(id) => crate::sys::windows::resolve_by_id_all(id)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "monitor with id '{id}' not found. connected: {}",
                    crate::sys::windows::connected_displays_list()
                )
            }),
    }
}

/// Resolves a MonitorTarget to a monitor index (1-based) for backend calls;
/// the primary display is `None`. An unknown monitor id is a hard error.
/// Uses `enumerate_devices` (attached displays only).
///
/// # Errors
/// Returns `Err` when an id matches no attached display.
fn resolve_target(target: &MonitorTarget) -> Result<Option<u32>, String> {
    match target {
        MonitorTarget::Primary => Ok(None),
        MonitorTarget::All => unreachable!(),
        MonitorTarget::Index(n) => {
            crate::sys::windows::resolve_device(Some(*n), &crate::sys::windows::enumerate_devices())
                .map(|(index, _)| Some(index as u32 + 1))
        }
        MonitorTarget::Id(id) => {
            crate::sys::windows::resolve_by_id(id)
                .map(Some)
                .ok_or_else(|| {
                    format!(
                        "monitor with id '{id}' not found. connected: {}",
                        crate::sys::windows::connected_displays_list()
                    )
                })
        }
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

/// Describes an attach/detach outcome: "detached {display}",
/// "attached {display}", or the already-detached/attached variants.
fn describe_attach(change: &AttachChange) -> String {
    match change.action {
        AttachAction::Disable => {
            if change.previous.dm_pels_width == 0 {
                format!("{} is already detached", change.display)
            } else {
                format!("detached {}", change.display)
            }
        }
        AttachAction::Enable => {
            if change.previous.dm_pels_width > 0 {
                format!("{} is already attached", change.display)
            } else {
                format!("attached {}", change.display)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::windows::bindings::DevmodeW;
    use crate::sys::windows::{AttachAction, AttachChange, Change, Mode};

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

    #[test]
    fn describe_attach_detached_when_applied() {
        assert_eq!(
            describe_attach(&attach_change(AttachAction::Disable, 1920)),
            "detached Generic PnP Monitor [:2]"
        );
    }

    #[test]
    fn describe_attach_already_detached_when_width_zero() {
        assert_eq!(
            describe_attach(&attach_change(AttachAction::Disable, 0)),
            "Generic PnP Monitor [:2] is already detached"
        );
    }

    #[test]
    fn describe_attach_attached_when_applied() {
        assert_eq!(
            describe_attach(&attach_change(AttachAction::Enable, 0)),
            "attached Generic PnP Monitor [:2]"
        );
    }

    #[test]
    fn describe_attach_already_attached_when_width_positive() {
        assert_eq!(
            describe_attach(&attach_change(AttachAction::Enable, 1920)),
            "Generic PnP Monitor [:2] is already attached"
        );
    }
}
