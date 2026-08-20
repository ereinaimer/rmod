use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn no_args_prints_help() {
    let out = rmod(&[]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod [COMMAND] [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("    list         List displays"));
    assert!(text.contains("    set          Set resolution/refresh/orientation"));
    assert!(text.contains("    layout       Show/arrange monitor layout"));
    assert!(text.contains("    brightness   Set backlight (0-100, min/max/boost)"));
    assert!(text.contains("    contrast     Set contrast (0-130, 100=neutral)"));
    assert!(text.contains("    temp         Set/show color temperature"));
    assert!(text.contains("    attach       Attach a monitor"));
    assert!(text.contains("    detach       Detach a monitor"));
    assert!(text.contains("    sleep        Put monitors to sleep"));
    assert!(text.contains("    wake         Wake monitors"));
    assert!(text.contains("    mirror       Mirror displays"));
    assert!(text.contains("    extend       Extend desktop (auto-arrange)"));
    assert!(text.contains("    project      Project to external (disable primary)"));
    assert!(text.contains("    single       Single display only"));
    assert!(text.contains("    completions  Output PowerShell completions"));
    assert!(text.contains("-h, --help     Print help"));
    assert!(text.contains("-V, --version  Print version"));
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
    assert!(
        !text.contains("monitor      "),
        "old monitor row must not appear at top level"
    );
    assert!(
        !text.contains("view         "),
        "old view row must not appear at top level"
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
fn short_version_flag_prints_version() {
    let out = rmod(&["-V"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn top_help_ignores_trailing_topic() {
    let out = rmod(&["--help", "set"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod [COMMAND] [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("set          Set resolution/refresh/orientation"));
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
    for verb in [
        "brightness",
        "contrast",
        "attach",
        "detach",
        "sleep",
        "wake",
        "mirror",
        "extend",
        "project",
        "single",
    ] {
        assert!(rmod(&[verb, "-h"]).status.success(), "rmod {verb} -h");
        assert!(
            rmod(&[verb, "--help"]).status.success(),
            "rmod {verb} --help"
        );
    }
}

#[test]
fn new_verb_help_pages_render() {
    let out = rmod(&["brightness", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod brightness"));
    assert!(text.contains("rmod brightness 60"));
    let out = rmod(&["sleep", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(strip_ansi(&stdout(&out)).contains("rmod sleep"));
    let out = rmod(&["single", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod single"));
    assert!(text.contains("rmod single -m 2"));
    let out = rmod(&["attach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        !strip_ansi(&stdout(&out)).contains("Aliases"),
        "attach page must not carry the old aliases section"
    );
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
