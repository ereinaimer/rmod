use std::process::Command;

fn rmod(args: &[&str]) -> std::process::Output {
    rmod_env(args, &[])
}

fn rmod_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rmod"));
    cmd.args(args).env("RMOD_SYS_FAKE", "1");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("failed to run rmod")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn strip_ansi(s: &str) -> String {
    s.replace("\x1b[92m", "")
        .replace("\x1b[4m", "")
        .replace("\x1b[0m", "")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn no_args_prints_help() {
    let out = rmod(&[]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod <COMMAND> [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("list     List displays and their current settings"));
    assert!(text.contains("set      Apply resolution, refresh rate, and orientation"));
    assert!(text.contains("layout   Show the monitor arrangement or move monitors"));
    assert!(text.contains("monitor  Attach, detach, sleep, or wake monitors"));
    assert!(text.contains("--help     Print help"));
    assert!(text.contains("--version  Print version"));
    assert!(
        !text.contains("-y, --yes"),
        "top-level help must not advertise -y"
    );
    assert!(
        !text.contains("Profiles"),
        "profiles table must not appear at top level"
    );
    assert!(
        !text.contains("Alias"),
        "ls alias must not appear at top level"
    );
}

#[test]
fn help_flags_exit_zero() {
    assert_eq!(rmod(&["-h"]).status.code(), Some(2));
    assert!(rmod(&["--help"]).status.success());
}

#[test]
fn version_flags_exit_zero() {
    let out = rmod(&["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn subcommand_help_flags_exit_zero() {
    assert_eq!(rmod(&["ls", "-h"]).status.code(), Some(2));
    assert!(rmod(&["ls", "--help"]).status.success());
    assert!(rmod(&["ls", "--caps", "--help"]).status.success());
    assert_eq!(rmod(&["set", "-p", "1080", "-h"]).status.code(), Some(2));
    assert!(rmod(&["set", "-p", "4k", "--help"]).status.success());
    assert_eq!(rmod(&["layout", "-h"]).status.code(), Some(2));
    assert!(rmod(&["layout", "--help"]).status.success());
}

#[test]
fn unknown_command_exits_2() {
    let out = rmod(&["foobar"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn unknown_argument_for_command_exits_2() {
    let out = rmod(&["ls", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn list_is_alias_for_ls() {
    assert_eq!(stdout(&rmod(&["list"])), stdout(&rmod(&["ls"])));
    let out = rmod(&["list", "--help"]);
    assert!(out.status.success());
    assert!(
        strip_ansi(&stdout(&out)).contains("Alias: ls"),
        "list help must mention the ls alias"
    );
}

#[test]
fn list_caps_works() {
    let out = rmod(&["list", "--caps", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2"));
}

#[test]
fn list_monitor_without_caps_is_error() {
    let out = rmod(&["list", "-m", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("'-m, --monitor' only works with '--caps'"));
}

#[test]
fn list_unknown_argument_exits_2() {
    let out = rmod(&["list", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn invalid_resolution_exits_2() {
    let out = rmod(&["set", "480"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn ls_lists_displays() {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    let mut lines = stdout.lines();
    let header = lines.next().expect("missing header line");
    assert!(header.starts_with("#  PRIMARY"));
    assert!(header.contains("REFRESH"));
    let sep = lines.next().expect("missing separator line");
    assert_eq!(sep.chars().count(), header.chars().count());
    assert!(sep.chars().all(|c| c == '─'));
    let data: Vec<&str> = lines.collect();
    assert!(!data.is_empty(), "no monitor rows");
    for line in &data {
        assert_eq!(line.len(), header.len(), "misaligned row: '{line}'");
        assert!(line.chars().next().is_some_and(|c| c.is_ascii_digit()));
        assert!(line.contains('x'));
        assert!(line.trim_end().ends_with("Hz"));
    }
}

#[test]
fn ls_marks_primary_display() {
    let out = rmod(&["ls"]);
    assert!(out.status.success());
    let stdout = stdout(&out);
    let data: Vec<&str> = stdout.lines().skip(2).collect();
    let starred = data
        .iter()
        .filter(|l| l.split_whitespace().any(|t| t == "*"))
        .count();
    assert_eq!(starred, 1, "expected exactly one primary marker");
}

#[test]
fn trailing_argument_exits_2() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "extra"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn empty_argument_exits_2() {
    let out = rmod(&[""]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn uppercase_command_exits_2() {
    let out = rmod(&["MAX"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn overflow_monitor_exits_2() {
    let out = rmod(&["set", "--max", "-m", "4294967296"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn flag_with_trailing_argument_exits_2() {
    let out = rmod(&["--help", "extra"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn caps_lists_supported_modes() {
    let out = rmod(&["ls", "--caps"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    let mut lines = stdout.lines();
    let header = lines.next().expect("missing monitor line");
    assert!(!header.trim().is_empty());
    let modes: Vec<&str> = lines.collect();
    assert!(!modes.is_empty(), "no supported modes listed");
    let at_pos = strip_ansi(modes[0])
        .find('@')
        .expect("missing '@' in mode line");
    for line in &modes {
        let clean = strip_ansi(line);
        assert!(line.starts_with("  "), "expected indented mode: '{line}'");
        assert!(line.contains('x'));
        assert!(line.contains('@'));
        assert!(line.ends_with("Hz"));
        assert_eq!(
            clean.find('@'),
            Some(at_pos),
            "misaligned mode line: '{line}'"
        );
    }
    let starred = modes.iter().filter(|l| l.contains('*')).count();
    assert_eq!(starred, 1, "expected exactly one active mode marker");
}

#[test]
fn caps_with_monitor() {
    assert!(rmod(&["ls", "--caps", "-m", "1"]).status.success());
}

#[test]
fn ls_shows_fake_environment() {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("RMOD Fake Monitor"),
        "expected fake monitor names: {stdout}"
    );
    assert!(
        stdout.contains("1920x1080"),
        "expected fake resolution: {stdout}"
    );
}

#[test]
fn caps_all_lists_every_monitor() {
    let out = rmod(&["ls", "--caps", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("RMOD Fake Monitor"));
    let modes: Vec<&str> = stdout.lines().filter(|l| l.starts_with("  ")).collect();
    assert!(!modes.is_empty(), "no mode rows listed");
    for line in &modes {
        assert!(line.contains('x'));
        assert!(line.contains('@'));
        assert!(line.ends_with("Hz"));
    }
}

#[test]
fn caps_unknown_monitor_exits_2() {
    let out = rmod(&["ls", "--caps", "-m", "999"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 999 not found"));
}

#[test]
fn caps_zero_monitor_exits_2() {
    let out = rmod(&["ls", "--caps", "-m", "0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor numbers start at 1"));
}

#[test]
fn ls_m_without_caps_is_error() {
    let out = rmod(&["ls", "-m", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("'-m, --monitor' only works with '--caps'"));
}

#[test]
fn caps_is_unknown_command() {
    let out = rmod(&["caps"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn set_max_primary() {
    let out = rmod(&["set", "--max"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_max_with_monitor() {
    let out = rmod(&["set", "--max", "-m", "1"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_max_with_all() {
    let out = rmod(&["set", "--max", "-m", "all"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_max_nonexistent_monitor_is_error() {
    let out = rmod(&["set", "--max", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn set_max_nonexistent_monitor_yes_flag() {
    let out = rmod(&["set", "--max", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn set_max_zero_monitor_is_error() {
    let out = rmod(&["set", "--max", "-m", "0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor numbers start at 1"));
}

#[test]
fn set_nonexistent_monitor_is_error() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-r", "60", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn set_zero_monitor_is_error() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-r", "60", "-m", "0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor numbers start at 1"));
}

#[test]
fn set_nonexistent_monitor_yes_flag() {
    let out = rmod(&[
        "set", "-w", "1920", "-h", "1080", "-r", "60", "-m", "0", "-y",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor numbers start at 1"));
}

#[test]
fn set_unsupported_mode_is_error() {
    let out = rmod(&["set", "-w", "9999", "-h", "9999", "-r", "1"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("does not support") || err.contains("the display change failed"));
}

#[test]
fn set_all_unsupported_mode_is_error() {
    let out = rmod(&["set", "-w", "9999", "-h", "9999", "-r", "1", "-m", "all"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("does not support") || err.contains("the display change failed"));
}

fn current_mode() -> (String, String, String) {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    let row = stdout
        .lines()
        .find(|l| l.contains('*') && l.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .expect("no primary monitor row");
    let tokens: Vec<&str> = row.split_whitespace().collect();
    let (width, height) = tokens[tokens.len() - 2]
        .split_once('x')
        .map(|(w, h)| (w.to_string(), h.to_string()))
        .expect("resolution column");
    let refresh = tokens
        .last()
        .and_then(|t| t.strip_suffix("Hz"))
        .expect("refresh column");
    (width, height, refresh.to_string())
}

#[test]
fn set_already_active_is_noop() {
    let (w, h, r) = current_mode();
    let out = rmod(&["set", "-w", &w, "-h", &h, "-r", &r]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn set_all_already_active_is_noop() {
    let (w, h, r) = current_mode();
    let out = rmod(&["set", "-w", &w, "-h", &h, "-r", &r, "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("is already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn orientation_invalid_is_error() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-o", "45"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid orientation"));
}

#[test]
fn orientation_missing_value_is_error() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-o"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("'-o, --orientation' needs a value"));
}

#[test]
fn orientation_flag_help() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-o", "90", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Options:"));
    assert!(text.contains("-o, --orientation"));
}

#[test]
fn set_help_flag() {
    let out = rmod(&["set", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Apply resolution, refresh rate, and orientation to a display"));
    assert!(text.contains("rmod set [OPTIONS]"));
    assert!(
        text.contains("Profiles:"),
        "set page must show the profiles table"
    );
    assert!(
        text.contains("1280x720"),
        "set page must list profile resolutions"
    );
    assert!(
        text.contains("Orientations:"),
        "set page must show orientations"
    );
    assert!(text.contains("-y, --yes"), "set page must advertise -y");
}

#[test]
fn layout_show_lists_positions() {
    let out = rmod(&["layout"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("RELATIVE TO"),
        "missing RELATIVE TO header: {stdout}"
    );
    assert!(
        stdout.contains("RMOD Fake Monitor"),
        "expected fake monitor names: {stdout}"
    );
    assert!(
        stdout.contains("(primary)"),
        "missing primary marker: {stdout}"
    );
    assert!(
        stdout.contains("right of 1"),
        "missing relative position: {stdout}"
    );
}

#[test]
fn layout_places_monitor_left_of_primary() {
    let out = rmod(&["layout", "-m", "2", "--left-of", "1", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed"),
        "missing placement line: {stdout}"
    );
    assert!(
        stdout.contains("to the left of"),
        "missing direction wording: {stdout}"
    );
}

#[test]
fn layout_places_monitor_below_explicit_reference() {
    let out = rmod(&["layout", "-m", "2", "--below", "1", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("below"),
        "missing direction wording: {}",
        stdout(&out)
    );
}

#[test]
fn layout_noop_placement_reports_already() {
    let out = rmod(&["layout", "-m", "2", "--right-of", "1", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains(
            "RMOD Fake Monitor 2 [:2] is already to the right of RMOD Fake Monitor 1 [:1]"
        ),
        "expected already-there message: {stdout}"
    );
    assert!(
        !stdout.contains("placed"),
        "no placement line expected: {stdout}"
    );
    assert!(
        !stdout.contains("keep changes"),
        "no prompt expected: {stdout}"
    );
}

#[test]
fn layout_places_monitor_right_of_explicit_reference() {
    let out = rmod(&["layout", "-m", "1", "--right-of", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed") && stdout.contains("to the right of"),
        "missing placement line: {stdout}"
    );
}

#[test]
fn layout_places_monitor_above_explicit_reference() {
    let out = rmod(&["layout", "-m", "2", "--above", "1", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed") && stdout.contains("above"),
        "missing placement line: {stdout}"
    );
}

#[test]
fn layout_primary_promotes() {
    let out = rmod(&["layout", "-m", "2", "--primary", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("is now the main display"));
    assert!(!stdout(&out).contains("keep changes"));
    assert!(!stdout(&out).contains("applied"));
    let noop = rmod(&["layout", "-m", "1", "--primary", "-y"]);
    assert!(noop.status.success(), "stderr: {}", stderr(&noop));
    assert!(stdout(&noop).contains("already the main display"));
}

#[test]
fn layout_self_reference_is_error() {
    let out = rmod(&["layout", "-m", "1", "--left-of", "1", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: cannot place monitor 1 relative to itself"));
}

#[test]
fn layout_missing_monitor_is_error() {
    let out = rmod(&["layout", "--left-of", "1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("missing monitor for 'layout', e.g. 'rmod layout -m 2 --left-of 1'")
    );
}

#[test]
fn layout_monitor_without_action_is_error() {
    let out = rmod(&["layout", "-m", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: '-m, --monitor' needs a direction flag or '--primary'"));
}

#[test]
fn layout_missing_value_for_direction_is_error() {
    let out = rmod(&["layout", "-m", "2", "--left-of"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: '--left-of' needs a value"));
}

#[test]
fn layout_primary_with_direction_is_error() {
    let out = rmod(&["layout", "-m", "2", "--primary", "--left-of", "1"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: use '--primary' or a direction flag, not both"));
}

#[test]
fn layout_unknown_argument_is_error() {
    let out = rmod(&["layout", "-m", "2", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: unexpected argument 'foo' for 'layout'"));
}

#[test]
fn layout_help_flag() {
    assert_eq!(rmod(&["layout", "-h"]).status.code(), Some(2));
    let out = rmod(&["layout", "--help"]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod layout [OPTIONS]"));
    assert!(text.contains("-y, --yes"), "layout page must advertise -y");
    assert!(
        text.contains("--primary"),
        "layout page must advertise --primary"
    );
    assert_eq!(rmod(&["layout", "-m", "2", "-h"]).status.code(), Some(2));
    assert!(rmod(&["layout", "-m", "2", "--help"]).status.success());
}

#[test]
fn old_syntax_max_colon_is_error() {
    let out = rmod(&["max:2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_max_all_is_error() {
    let out = rmod(&["max:*"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_caps_colon_is_error() {
    let out = rmod(&["caps:2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_caps_all_is_error() {
    let out = rmod(&["caps:*"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_main_colon_is_error() {
    let out = rmod(&["main:2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_implicit_set_is_error() {
    let out = rmod(&["1920x1080@60"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_profile_with_monitor_is_error() {
    let out = rmod(&["4k:2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_compact_orientation_is_error() {
    let out = rmod(&["1920x1080:2/90"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_flag_based_is_error() {
    let out = rmod(&["-w", "1920", "-h", "1080", "-r", "60"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_refresh_only_is_error() {
    let out = rmod(&["-r", "144"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn old_syntax_orientation_only_is_error() {
    let out = rmod(&["-o", "90"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

#[test]
fn main_command_removed_shows_hint() {
    let out = rmod(&["main", "2"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("unknown command 'main'"), "stderr: {err}");
    assert!(err.contains("layout"), "missing migration hint: {err}");
}

#[test]
fn set_with_profile() {
    let out = rmod(&["set", "-p", "1080"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_profile_and_refresh() {
    let out = rmod(&["set", "-p", "4k", "-r", "144"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_explicit_resolution() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-r", "60"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_explicit_no_refresh() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_refresh_only() {
    let out = rmod(&["set", "-r", "60"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_max_refresh() {
    let out = rmod(&["set", "-r", "max"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_monitor() {
    let out = rmod(&["set", "-p", "1080", "-m", "2"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_all() {
    let out = rmod(&["set", "-p", "1080", "-m", "all"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_orientation() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-m", "2", "-o", "90"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_with_yes() {
    let out = rmod(&["set", "-p", "1440", "-y"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_all_profiles() {
    for profile in ["720", "1080", "1440", "4k", "8k"] {
        let out = rmod(&["set", "-p", profile]);
        if !out.status.success() {
            let err = stderr(&out);
            assert!(
                !err.contains("unknown command"),
                "profile {}: {}",
                profile,
                err
            );
            assert!(!err.contains("unexpected argument"), "profile {}", profile);
        }
    }
}

#[test]
fn set_optional_spec_orientation() {
    let out = rmod(&["set", "-o", "portrait", "-y"]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn monitor_help_exits_zero() {
    assert_eq!(rmod(&["monitor", "-h"]).status.code(), Some(2));
    assert!(rmod(&["monitor", "--help"]).status.success());
    assert!(rmod(&["monitor", "detach", "--help"]).status.success());
    let text = strip_ansi(&stdout(&rmod(&["monitor", "--help"])));
    assert!(text.contains("Attach, detach, sleep, or wake monitors"));
    assert!(text.contains("-m, --monitor"));
    assert!(text.contains("-y, --yes"));
}

#[test]
fn monitor_detach_second_monitor() {
    let out = rmod(&["monitor", "detach", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_disable_is_alias_for_detach() {
    let out = rmod(&["monitor", "disable", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_off_is_alias_for_detach() {
    let out = rmod(&["monitor", "off", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_detach_primary_is_error() {
    let out = rmod(&["monitor", "detach", "-m", "1", "-y"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("cannot detach the primary display"));
}

#[test]
fn monitor_detach_without_monitor_is_error() {
    for args in [&["monitor", "detach"][..], &["monitor", "attach"][..]] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains("needs '-m, --monitor'"),
            "stderr: {}",
            stderr(&out)
        );
    }
}

#[test]
fn monitor_attach_attached_is_unchanged() {
    let out = rmod(&["monitor", "attach", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_enable_is_alias_for_attach() {
    let out = rmod(&["monitor", "enable", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_on_is_alias_for_attach() {
    let out = rmod(&["monitor", "on", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_sleep_prints_slept_per_line() {
    let out = rmod(&["monitor", "sleep"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("slept RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("slept RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("asleep"));
}

#[test]
fn monitor_wake_prints_woke_per_line() {
    let out = rmod(&["monitor", "wake"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("woke RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("woke RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("awake"));
}

#[test]
fn monitor_detach_help_shows_aliases() {
    let out = rmod(&["monitor", "detach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Aliases:"));
    assert!(text.contains("disable, off"));
}

#[test]
fn monitor_attach_help_shows_aliases() {
    let out = rmod(&["monitor", "attach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Aliases:"));
    assert!(text.contains("enable, on"));
}

#[test]
fn monitor_help_hides_aliases() {
    let out = rmod(&["monitor", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(!text.contains("aliases"), "got: {text}");
    assert!(!text.contains("disable, off"));
    assert!(!text.contains("enable, on"));
}

#[test]
fn monitor_sleep_rejects_monitor_flag() {
    let out = rmod(&["monitor", "sleep", "-m", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for 'monitor sleep'"));
}

#[test]
fn monitor_sleep_rejects_yes_flag() {
    let out = rmod(&["monitor", "sleep", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for 'monitor sleep'"));
}

#[test]
fn monitor_missing_action_is_error() {
    let out = rmod(&["monitor"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("'monitor' needs an action"));
}

#[test]
fn monitor_unknown_action_is_error() {
    let out = rmod(&["monitor", "frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown action 'frobnicate' for 'monitor'"));
}

#[test]
fn monitor_unknown_monitor_is_error() {
    let out = rmod(&["monitor", "detach", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn monitor_detach_all_detaches_secondary_and_reports_primary_error() {
    let out = rmod(&["monitor", "detach", "-m", "all", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
    assert!(stderr(&out).contains("cannot detach the primary display"));
}

#[test]
fn monitor_attach_all_is_unchanged() {
    let out = rmod(&["monitor", "attach", "-m", "all", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 1 [:1] is already attached"));
    assert!(text.contains("RMOD Fake Monitor 2 [:2] is already attached"));
}
