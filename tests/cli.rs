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