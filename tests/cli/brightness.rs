use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn brightness_sets_primary_via_ddc() {
    let out = rmod(&["brightness", "30"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] brightness to 30% via ddc"));
}

#[test]
fn brightness_monitor_flag_before_value() {
    let out = rmod(&["brightness", "-m", "2", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to 60% via gamma"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn brightness_already_at_is_noop() {
    let out = rmod(&["brightness", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 1 [:1] is already at 60%"));
    assert!(!stdout(&out).contains("via"));
}

#[test]
fn brightness_second_monitor_falls_back_to_gamma() {
    let out = rmod(&["brightness", "30", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to 30% via gamma"));
}

#[test]
fn brightness_all_targets_every_display() {
    let out = rmod(&["brightness", "30", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 1 [:1] brightness to 30% via ddc"));
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] brightness to 30% via gamma"));
}

#[test]
fn brightness_via_flag() {
    let out = rmod(&["brightness", "30", "--via", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn brightness_via_short_flag() {
    let out = rmod(&["brightness", "30", "-v", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn brightness_forced_unsupported_backend_is_error() {
    let out = rmod(&["brightness", "30", "-m", "2", "--via", "ddc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not support ddc brightness control"));
}

#[test]
fn brightness_unknown_backend_is_error() {
    let out = rmod(&["brightness", "30", "--via", "gamma2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown backend gamma2"));
}

#[test]
fn brightness_out_of_range_is_error() {
    let out = rmod(&["brightness", "150"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid brightness 150"));
}

#[test]
fn brightness_missing_value_is_error() {
    let out = rmod(&["brightness"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("brightness needs a value"));
}

#[test]
fn brightness_rejects_yes_flag() {
    let out = rmod(&["brightness", "30", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for brightness"));
}

#[test]
fn brightness_zero_is_valid() {
    let out = rmod(&["brightness", "0", "-m", "1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] brightness to 0% via ddc"));
}

#[test]
fn brightness_unknown_monitor_is_error() {
    let out = rmod(&["brightness", "30", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn brightness_mode_min_sets_primary() {
    let out = rmod(&["brightness", "min"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("set RMOD Fake Monitor 1 [:1] brightness to min (slider 5 + gamma 50%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn brightness_mode_min_on_second_monitor_is_gamma_only() {
    let out = rmod(&["brightness", "min", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to min (gamma 50%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn brightness_mode_max_sets_primary() {
    let out = rmod(&["brightness", "max", "-m", "1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("set RMOD Fake Monitor 1 [:1] brightness to max (ddc 100 + gamma 100%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn brightness_mode_boost_applies_and_warns_clipping() {
    let out = rmod(&["brightness", "boost"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("set RMOD Fake Monitor 1 [:1] brightness to boost (slider 100 + gamma 130%)"),
        "got: {text}"
    );
    assert!(
        text.contains("boost clips highlights above ~77%"),
        "got: {text}"
    );
}

#[test]
fn brightness_mode_min_all_targets_every_display() {
    let out = rmod(&["brightness", "min", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("set RMOD Fake Monitor 1 [:1] brightness to min (slider 5 + gamma 50%)"),
        "got: {text}"
    );
    assert!(
        text.contains("set RMOD Fake Monitor 2 [:2] brightness to min (gamma 50%)"),
        "got: {text}"
    );
}

#[test]
fn brightness_mode_rejects_via_flag() {
    for args in [
        &["brightness", "min", "-v", "ddc"][..],
        &["brightness", "min", "--via", "ddc"][..],
    ] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains(
                "-v, --via is not valid with min, max, or boost. use a number to choose a backend"
            ),
            "args: {args:?}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn brightness_unknown_mode_word_is_error() {
    let out = rmod(&["brightness", "dimm"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid brightness dimm. use a number between 0 and 100"));
}

#[test]
fn brightness_help_flag() {
    let out = rmod(&["brightness", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod brightness <VALUE> [OPTIONS]"));
    assert!(text.contains("Set the display backlight level (0-100, or min, max, boost)"));
    assert!(text.contains("--via"));
}
