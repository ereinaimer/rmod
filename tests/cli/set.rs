use super::common::{SERIAL_A, SERIAL_B, current_mode, rmod, stderr, stdout, strip_ansi};

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
    let out = rmod(&["set", "--max", "-m", SERIAL_A]);
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
fn set_single_monitor_output_includes_display_name() {
    let out = rmod(&["set", "--max", "-m", SERIAL_B, "-y"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 2 [:2]"), "stdout: {text}");
    assert!(text.contains("applied"), "stdout: {text}");
}

#[test]
fn set_max_unknown_serial_is_error() {
    let out = rmod(&["set", "--max", "-m", "NOPE"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor with id 'NOPE' not found"));
}

#[test]
fn set_targets_monitor_by_fingerprint() {
    let out = rmod(&["set", "-r", "144", "-m", "b2c3d4e5", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("RMOD Fake Monitor 2 [:2]"),
        "fingerprint must resolve to monitor 2: {}",
        stdout(&out)
    );
    let out = rmod(&["set", "-r", "144", "-m", "a1b2c3d4", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("RMOD Fake Monitor 1 [:1]"),
        "fingerprint must resolve to monitor 1: {}",
        stdout(&out)
    );
}

#[test]
fn set_max_unknown_serial_yes_flag() {
    let out = rmod(&["set", "--max", "-m", "NOPE", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor with id 'NOPE' not found"));
}

#[test]
fn set_max_zero_monitor_is_error() {
    let out = rmod(&["set", "--max", "-m", "0"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor numbers start at 1"));
}

#[test]
fn set_nonexistent_monitor_is_error() {
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-r", "60", "-m", "NOPE"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor with id 'NOPE' not found"));
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
    assert!(stderr(&out).contains("-o, --orientation needs a value"));
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
    let out = rmod(&["set", "-p", "1080", "-m", SERIAL_B]);
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
    let out = rmod(&[
        "set", "-w", "1920", "-h", "1080", "-m", SERIAL_B, "-o", "90",
    ]);
    if !out.status.success() {
        let err = stderr(&out);
        assert!(!err.contains("unknown command"));
        assert!(!err.contains("unexpected argument"));
    }
}

#[test]
fn set_orientation_zero_is_accepted() {
    let out = rmod(&["set", "-p", "1080", "-o", "0", "-y"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
}

#[test]
fn monitor_keywords_are_case_insensitive() {
    let out = rmod(&["set", "-m", "PRIMARY", "--max", "-y"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 1 [:1]"));

    let out = rmod(&["set", "-m", "ALL", "-p", "1080", "-y"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 1"), "stdout: {text}");
    assert!(text.contains("RMOD Fake Monitor 2"), "stdout: {text}");

    let out = rmod(&["temp", "-m", "ALL", "3400"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("set RMOD Fake Monitor 1 [:1]"),
        "stdout: {text}"
    );
    assert!(
        text.contains("set RMOD Fake Monitor 2 [:2]"),
        "stdout: {text}"
    );
}

#[test]
fn set_flags_in_any_order_target_monitor() {
    let out = rmod(&["set", "-y", "-m", "2", "-p", "1080"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("RMOD Fake Monitor 2 [:2]"),
        "monitor 2 must be targeted: {}",
        stdout(&out)
    );
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
