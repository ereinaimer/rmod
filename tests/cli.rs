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