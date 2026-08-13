use std::process::Command;

fn rmod(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rmod"))
        .args(args)
        .output()
        .expect("failed to run rmod")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn strip_ansi(s: &str) -> String {
    s.replace("\x1b[92m", "").replace("\x1b[0m", "")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn no_args_prints_help() {
    let out = rmod(&[]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("Usage:"));
}

#[test]
fn help_flags_exit_zero() {
    assert!(rmod(&["-h"]).status.success());
    assert!(rmod(&["--help"]).status.success());
}

#[test]
fn version_flags_exit_zero() {
    let out = rmod(&["-V"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn subcommand_help_flags_exit_zero() {
    assert!(rmod(&["ls", "-h"]).status.success());
    assert!(rmod(&["ls", "--help"]).status.success());
    assert!(rmod(&["max", "-h"]).status.success());
    assert!(rmod(&["max", "--help"]).status.success());
    assert!(rmod(&["caps", "-h"]).status.success());
    assert!(rmod(&["caps", "--help"]).status.success());
    assert!(rmod(&["1920x1080@60", "-h"]).status.success());
    assert!(rmod(&["4k", "--help"]).status.success());
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
fn invalid_resolution_exits_2() {
    let out = rmod(&["480"]);
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
    let out = rmod(&["1920x1080@60", "extra"]);
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
    let out = rmod(&["max:4294967296"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn flag_with_trailing_argument_exits_2() {
    let out = rmod(&["-h", "extra"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn caps_lists_supported_modes() {
    let out = rmod(&["caps"]);
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
fn caps_first_monitor_succeeds() {
    assert!(rmod(&["caps:1"]).status.success());
}

#[test]
fn caps_unknown_monitor_exits_2() {
    let out = rmod(&["caps:999"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 999 not found"));
}

#[test]
fn caps_zero_monitor_exits_2() {
    let out = rmod(&["caps:0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 0 not found"));
}

#[test]
fn caps_all_lists_every_monitor() {
    let out = rmod(&["caps:*"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("Generic PnP Monitor"));
    let modes: Vec<&str> = stdout.lines().filter(|l| l.starts_with("  ")).collect();
    assert!(!modes.is_empty(), "no mode rows listed");
    for line in &modes {
        assert!(line.contains('x'));
        assert!(line.contains('@'));
        assert!(line.ends_with("Hz"));
    }
}

#[test]
fn max_help_lists_usage() {
    let out = rmod(&["max", "-h"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("Apply the highest supported resolution"));
    assert!(stdout.contains("rmod max[:N|:*]"));
}

#[test]
fn max_nonexistent_monitor_is_error() {
    let out = rmod(&["max:99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn max_nonexistent_monitor_yes_flag() {
    let out = rmod(&["max:99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn max_zero_monitor_is_error() {
    let out = rmod(&["max:0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 0 not found"));
}

#[test]
fn set_nonexistent_monitor_is_error() {
    let out = rmod(&["1920x1080@60:99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn set_zero_monitor_is_error() {
    let out = rmod(&["1920x1080@60:0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 0 not found"));
}

#[test]
fn set_nonexistent_monitor_yes_flag() {
    let out = rmod(&["1920x1080@60:0", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 0 not found"));
}

#[test]
fn set_unsupported_mode_is_error() {
    let out = rmod(&["9999x9999@1"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(
        err.contains("does not support 9999x9999@1Hz") || err.contains("the display change failed")
    );
}

#[test]
fn set_all_unsupported_mode_is_error() {
    let out = rmod(&["9999x9999@1:*"]);
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
    let mode = format!("{w}x{h}@{r}");
    let out = rmod(&[&mode]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn set_all_already_active_is_noop() {
    let (w, h, r) = current_mode();
    let all = format!("{w}x{h}@{r}:*");
    let out = rmod(&[&all]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("is already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn set_flags_already_active_is_noop() {
    let (w, h, r) = current_mode();
    let out = rmod(&["-w", &w, "-h", &h, "-r", &r]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn set_flags_all_already_active_is_noop() {
    let (w, h, r) = current_mode();
    let out = rmod(&["-w", &w, "-h", &h, "-r", &r, "-m", "*"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("is already at"));
    assert!(!stdout.contains("keep changes"));
    assert!(!stdout.contains("applied"));
}

#[test]
fn set_flags_unsupported_mode_is_error() {
    let out = rmod(&["-w", "9999", "-h", "9999", "-r", "1"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("does not support") || err.contains("the display change failed"));
}

#[test]
fn set_flags_missing_height_is_error() {
    let out = rmod(&["-w", "1920"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("-w requires -h"));
}

#[test]
fn set_flags_nothing_to_set_is_error() {
    let out = rmod(&["-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("nothing to set"));
}

#[test]
fn set_flags_missing_value_is_error() {
    let out = rmod(&["-w"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("missing value for -w"));
}

#[test]
fn set_flags_invalid_refresh_is_error() {
    let out = rmod(&["-r", "abc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid refresh"));
}

#[test]
fn set_flags_monitor_not_found() {
    let out = rmod(&["-w", "1920", "-h", "1080", "-r", "60", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn set_flags_help() {
    let out = rmod(&["-w", "1920", "-h", "1080", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Flags"));
}
