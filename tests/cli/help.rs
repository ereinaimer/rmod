use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn no_args_prints_help() {
    let out = rmod(&[]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod [COMMAND] [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("list         List displays and their current settings"));
    assert!(text.contains("set          Apply resolution, refresh rate, and orientation"));
    assert!(text.contains("layout       Show the monitor arrangement or move monitors"));
    assert!(text.contains("monitor      Attach, detach, sleep, or wake monitors"));
    assert!(text.contains("temp         Set or show the display color temperature"));
    assert!(
        text.contains(
            "view         Switch between mirror, extend, project, and single display modes"
        )
    );
    assert!(text.contains("completions  Output PowerShell tab-completion script"));
    assert!(text.contains("-h, --help  Print help"));
    assert!(text.contains("--version   Print version"));
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
    assert!(rmod(&["-h"]).status.success());
    assert!(rmod(&["--help"]).status.success());
}

#[test]
fn version_flags_exit_zero() {
    let out = rmod(&["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn short_version_flag_exits_2() {
    let out = rmod(&["-V"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"), "stderr: {}", stderr(&out));
}

#[test]
fn top_help_ignores_trailing_topic() {
    let out = rmod(&["--help", "set"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod [COMMAND] [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("set          Apply resolution, refresh rate, and orientation"));
}

#[test]
fn set_version_flag_prints_version() {
    let out = rmod(&["set", "--version"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn subcommand_help_flags_exit_zero() {
    assert!(rmod(&["ls", "-h"]).status.success());
    assert!(rmod(&["ls", "--help"]).status.success());
    assert_eq!(rmod(&["set", "-p", "1080", "-h"]).status.code(), Some(2));
    assert!(rmod(&["set", "-p", "4k", "--help"]).status.success());
    assert!(rmod(&["layout", "-h"]).status.success());
    assert!(rmod(&["layout", "--help"]).status.success());
    assert!(rmod(&["temp", "-h"]).status.success());
    assert!(rmod(&["temp", "--help"]).status.success());
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
