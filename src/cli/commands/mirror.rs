//! `mirror` command: clone all displays at the same position and the
//! lowest common resolution.

use crate::cli::{Command, HelpTopic};
use crate::sys::windows;
use crate::sys::windows::Monitor;
use crate::sys::windows::caps_all_modes_for_device;

use super::confirm_or_revert_all;

/// Mirror: clone all displays at same position (0,0) with lowest common resolution.
pub(super) fn run_mirror(yes: bool) -> i32 {
    match windows::list_detailed() {
        Ok(monitors) => {
            if monitors.len() <= 1 {
                println!("already mirrored (only one monitor)");
                return 0;
            }

            // Find common resolution across all monitors
            let common_mode = find_common_mode(&monitors);
            let common_mode = match common_mode {
                Some(m) => m,
                None => {
                    eprintln!(
                        "error: no common resolution with a common refresh rate across all monitors"
                    );
                    return 2;
                }
            };

            let mut applied = Vec::new();
            for monitor in &monitors {
                match windows::set(
                    Some(monitor.number),
                    Some(common_mode.width),
                    Some(common_mode.height),
                    crate::sys::windows::apply::Refresh::Fixed(common_mode.refresh),
                    None,
                ) {
                    Ok(windows::ApplyOutcome::Unchanged(change)) => {
                        println!(
                            "{} is already at {}x{} @ {}Hz",
                            change.display,
                            change.mode.width,
                            change.mode.height,
                            change.mode.refresh
                        );
                    }
                    Ok(windows::ApplyOutcome::Applied(change)) => {
                        println!(
                            "applied {}x{} @ {}Hz to {}",
                            change.mode.width,
                            change.mode.height,
                            change.mode.refresh,
                            change.display
                        );
                        applied.push(change);
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            }

            if applied.is_empty() {
                println!("already mirrored");
                0
            } else {
                confirm_or_revert_all(applied, yes)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    }
}

/// Finds a common resolution supported by all monitors.
fn find_common_mode(monitors: &[Monitor]) -> Option<crate::sys::windows::Mode> {
    use crate::sys::windows::Mode;

    let mut common_resolutions: Option<std::collections::HashSet<(u32, u32)>> = None;

    for monitor in monitors {
        let modes = caps_all_modes_for_device(&monitor.device_name);
        let res_set: std::collections::HashSet<(u32, u32)> =
            modes.into_iter().map(|m| (m.width, m.height)).collect();

        if let Some(ref mut common) = common_resolutions {
            common.retain(|(w, h)| res_set.contains(&(*w, *h)));
            if common.is_empty() {
                return None;
            }
        } else {
            common_resolutions = Some(res_set);
        }
    }

    // Pick the highest resolution (by pixel count)
    common_resolutions.and_then(|resolutions| {
        let best_res = resolutions.into_iter().max_by_key(|(w, h)| w * h)?;
        let (width, height) = best_res;

        // Find a common refresh rate for this resolution across all monitors
        let mut common_refresh: Option<std::collections::HashSet<u32>> = None;
        for monitor in monitors {
            let modes = caps_all_modes_for_device(&monitor.device_name);
            let refresh_set: std::collections::HashSet<u32> = modes
                .into_iter()
                .filter(|m| m.width == width && m.height == height)
                .map(|m| m.refresh)
                .collect();

            if let Some(ref mut common) = common_refresh {
                common.retain(|r| refresh_set.contains(r));
                if common.is_empty() {
                    return None;
                }
            } else {
                common_refresh = Some(refresh_set);
            }
        }

        // Pick a sensible refresh rate: prefer 60Hz if available, otherwise highest common
        common_refresh.and_then(|refreshes| {
            let refresh = if refreshes.contains(&60) {
                60
            } else {
                refreshes.into_iter().max()?
            };
            Some(Mode {
                width,
                height,
                refresh,
            })
        })
    })
}

/// Parses `rmod mirror`: only `-y` is accepted; `-m` and any other token
/// are rejected. `name` is the command word embedded in error messages
/// (`mirror` at root, `view` through the old shim).
pub(crate) fn parse_mirror(args: &[impl AsRef<str>], name: &str) -> Result<Command, String> {
    let mut yes = false;
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_ref();
        match arg {
            "-y" | "--yes" => {
                yes = true;
                i += 1;
            }
            "-h" | "--help" => {
                return Ok(Command::Help {
                    topic: Some(HelpTopic::Mirror),
                });
            }
            "--version" => return Ok(Command::Version),
            other => {
                return Err(format!(
                    "unexpected argument {other} for {name}. use -y or --help"
                ));
            }
        }
    }
    Ok(Command::Mirror { yes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        let mut full_args = vec!["rmod"];
        full_args.extend_from_slice(args);
        crate::cli::parser::parse_from(&full_args)
    }

    #[test]
    fn mirror_command() {
        assert_eq!(parse(&["mirror"]), Ok(Command::Mirror { yes: false }));
        assert_eq!(parse(&["mirror", "-y"]), Ok(Command::Mirror { yes: true }));
        assert_eq!(
            parse(&["mirror", "--yes"]),
            Ok(Command::Mirror { yes: true })
        );
    }

    #[test]
    fn mirror_rejects_monitor_flag() {
        assert_eq!(
            parse(&["mirror", "-m", "2"]),
            Err("unexpected argument -m for mirror. use -y or --help".to_string())
        );
    }

    #[test]
    fn mirror_rejects_other_arguments() {
        assert_eq!(
            parse(&["mirror", "foo"]),
            Err("unexpected argument foo for mirror. use -y or --help".to_string())
        );
    }

    #[test]
    fn mirror_help_flag() {
        assert_eq!(
            parse(&["mirror", "-h"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Mirror)
            })
        );
        assert_eq!(
            parse(&["mirror", "--help"]),
            Ok(Command::Help {
                topic: Some(HelpTopic::Mirror)
            })
        );
    }

    #[test]
    fn mirror_version_flag() {
        assert_eq!(parse(&["mirror", "--version"]), Ok(Command::Version));
    }
}
