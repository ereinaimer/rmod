use super::common::{SERIAL_A, SERIAL_B, rmod, stderr, stdout, strip_ansi};

#[test]
fn temp_no_args_shows_primary() {
    let out = rmod(&["temp"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("RMOD Fake Monitor 1 [:1] is currently approx 6500K"),
        "expected show line: {text}"
    );
    assert!(
        !text.contains("RMOD Fake Monitor 2"),
        "primary only: {text}"
    );
}

#[test]
fn temp_sets_kelvin() {
    let out = rmod(&["temp", "3400"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 3400K"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn temp_k_suffix_sets_kelvin() {
    let out = rmod(&["temp", "4000k"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 4000K"));
}

#[test]
fn temp_out_of_range_is_error() {
    for arg in ["500", "999", "6501", "9000", "19200"] {
        let out = rmod(&["temp", arg]);
        assert_eq!(out.status.code(), Some(2), "arg '{arg}'");
        let err = stderr(&out);
        assert!(err.contains("invalid temperature"), "arg '{arg}': {err}");
        assert!(err.contains("1000-6500"), "arg '{arg}': {err}");
    }
}

#[test]
fn temp_range_boundaries_are_accepted() {
    let out = rmod(&["temp", "1000"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 1000K"));
    let out = rmod(&["temp", "6500"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 6500K"));
}

#[test]
fn temp_preset_sets_kelvin() {
    let out = rmod(&["temp", "warm"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 2700K"));
}

#[test]
fn temp_alias_sets_kelvin() {
    let out = rmod(&["temp", "incandescent"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 2700K"));
}

#[test]
fn temp_reset_restores_daylight() {
    let out = rmod(&["temp", "reset"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("reset RMOD Fake Monitor 1 [:1] to 6500K"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn temp_with_monitor_targets_second_display() {
    let out = rmod(&["temp", "-m", SERIAL_B, "4000"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] to 4000K"));
}

#[test]
fn temp_monitor_flag_before_value() {
    let out = rmod(&["temp", "-m", "2", "3400"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] to 3400K"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn temp_value_before_monitor_flag() {
    let out = rmod(&["temp", "3400", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] to 3400K"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn temp_all_sets_every_monitor() {
    let out = rmod(&["temp", "-m", "all", "3000"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 1 [:1] to 3000K"));
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] to 3000K"));
}

#[test]
fn temp_all_shows_every_monitor() {
    let out = rmod(&["temp", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 1 [:1] is currently approx 6500K"));
    assert!(text.contains("RMOD Fake Monitor 2 [:2] is currently approx 6500K"));
}

#[test]
fn temp_unknown_monitor_is_error() {
    let out = rmod(&["temp", "-m", "99", "3000"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn temp_invalid_value_is_error() {
    let out = rmod(&["temp", "bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid temperature"));
}

#[test]
fn temp_help_flag() {
    assert!(rmod(&["temp", "-h"]).status.success());
    let out = rmod(&["temp", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Set or show the display color temperature"));
    assert!(text.contains("Presets:"));
    assert!(text.contains("candle"));
    assert!(
        !text.contains("-y, --yes"),
        "temp page must not advertise -y"
    );
}

#[test]
fn serial_targeting_is_case_insensitive() {
    let out = rmod(&["set", "--max", "-m", "abc12345678", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("3840x2160"), "got: {}", stdout(&out));
    let out = rmod(&["temp", "-m", "def45678901", "3400"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] to 3400K"));
}

#[test]
fn serial_targeting_works_across_all_commands() {
    let set = rmod(&["set", "-p", "1440", "-m", SERIAL_B, "-y"]);
    assert!(set.status.success(), "set: {}", stderr(&set));
    let layout = rmod(&["layout", "-m", SERIAL_B, "--left-of", SERIAL_A, "-y"]);
    assert!(layout.status.success(), "layout: {}", stderr(&layout));
    let detach = rmod(&["detach", "-m", SERIAL_B, "-y"]);
    assert!(detach.status.success(), "detach: {}", stderr(&detach));
    let temp = rmod(&["temp", "-m", SERIAL_B, "3400"]);
    assert!(temp.status.success(), "temp: {}", stderr(&temp));
}

#[test]
fn unknown_serial_errors_for_every_command() {
    for args in [
        &["set", "--max", "-m", "NOPE"][..],
        &["set", "-w", "1920", "-h", "1080", "-m", "NOPE"][..],
        &["layout", "-m", "NOPE", "--primary"][..],
        &["layout", "-m", SERIAL_A, "--left-of", "NOPE"][..],
        &["detach", "-m", "NOPE"][..],
        &["temp", "-m", "NOPE", "3400"][..],
    ] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains("monitor with id 'NOPE' not found. connected:"),
            "args: {args:?}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn primary_keyword_targets_primary() {
    let out = rmod(&["temp", "-m", "primary", "3000"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] to 3000K"));
}
