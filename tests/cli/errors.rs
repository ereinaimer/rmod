use super::common::{rmod, stderr};

#[test]
fn invalid_resolution_exits_2() {
    let out = rmod(&["set", "480"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
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
fn caps_is_unknown_command() {
    let out = rmod(&["caps"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
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
fn main_command_removed_errors_generically() {
    let out = rmod(&["main", "2"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(
        err.contains("unknown command main. run rmod --help to list commands"),
        "stderr: {err}"
    );
    assert!(
        !err.contains("layout"),
        "no migration hint may remain: {err}"
    );
}

#[test]
fn legacy_root_commands_are_unknown() {
    for cmd in ["monitor", "view", "main", "disable", "off", "enable", "on"] {
        let out = rmod(&[cmd]);
        assert_eq!(out.status.code(), Some(2), "command {cmd}");
        let err = stderr(&out);
        assert!(
            err.contains(&format!(
                "unknown command {cmd}. run rmod --help to list commands"
            )),
            "command {cmd}: stderr: {err}"
        );
    }
}

#[test]
fn legacy_two_level_forms_are_unknown() {
    for (args, word) in [
        (&["monitor", "brightness", "60"][..], "monitor"),
        (&["view", "mirror"][..], "view"),
        (&["disable", "-m", "2"][..], "disable"),
    ] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        let err = stderr(&out);
        assert!(
            err.contains(&format!(
                "unknown command {word}. run rmod --help to list commands"
            )),
            "args: {args:?}: stderr: {err}"
        );
    }
}
