use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn contrast_monitor_flag_before_value_reset() {
    let out = rmod(&["contrast", "-m", "2", "reset"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] contrast to 100% via gamma"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn contrast_sets_primary_via_ddc() {
    let out = rmod(&["contrast", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] contrast to 60% via ddc"));
}

#[test]
fn contrast_already_at_is_noop() {
    let out = rmod(&["contrast", "75"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 1 [:1] is already at 75%"));
    assert!(!stdout(&out).contains("via"));
}

#[test]
fn contrast_second_monitor_falls_back_to_gamma() {
    let out = rmod(&["contrast", "60", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] contrast to 60% via gamma"));
}

#[test]
fn contrast_all_targets_every_display() {
    let out = rmod(&["contrast", "60", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 1 [:1] contrast to 60% via ddc"));
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] contrast to 60% via gamma"));
}

#[test]
fn contrast_via_short_flag() {
    let out = rmod(&["contrast", "60", "-v", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn contrast_via_long_flag() {
    let out = rmod(&["contrast", "60", "--via", "ddc"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via ddc"));
}

#[test]
fn contrast_forced_unsupported_backend_is_error() {
    let out = rmod(&["contrast", "60", "-m", "2", "--via", "ddc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not support ddc contrast control"));
}

#[test]
fn contrast_unknown_backend_is_error() {
    let out = rmod(&["contrast", "60", "--via", "slider"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown backend slider. use ddc or gamma"));
}

#[test]
fn contrast_out_of_range_is_error() {
    let out = rmod(&["contrast", "131"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid contrast 131. use a number between 0 and 130"));
}

#[test]
fn contrast_missing_value_is_error() {
    let out = rmod(&["contrast"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("contrast needs a value"));
}

#[test]
fn contrast_rejects_yes_flag() {
    let out = rmod(&["contrast", "60", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("-y, --yes is not valid for contrast"));
}

#[test]
fn contrast_zero_is_valid() {
    let out = rmod(&["contrast", "0"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] contrast to 0% via ddc"));
}

#[test]
fn contrast_overdrive_warns_clipping() {
    let out = rmod(&["contrast", "130", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] contrast to 130% via gamma"));
    assert!(text.contains("contrast boost clips shadows and highlights"));
}

#[test]
fn contrast_unknown_monitor_is_error() {
    let out = rmod(&["contrast", "60", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn contrast_help_flag() {
    let out = rmod(&["contrast", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod contrast <VALUE> [OPTIONS]"));
    assert!(text.contains("Set the display contrast (0-130, 100 = neutral)"));
}
