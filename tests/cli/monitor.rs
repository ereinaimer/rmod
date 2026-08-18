use super::common::{SERIAL_A, SERIAL_B, rmod, stderr, stdout, strip_ansi};

#[test]
fn monitor_help_exits_zero() {
    assert!(rmod(&["monitor", "-h"]).status.success());
    assert!(rmod(&["monitor", "--help"]).status.success());
    assert!(rmod(&["monitor", "detach", "--help"]).status.success());
    let text = strip_ansi(&stdout(&rmod(&["monitor", "-h"])));
    assert!(text.contains("Attach, detach, sleep, or wake monitors"));
    assert!(text.contains("-m, --monitor"));
    assert!(text.contains("-y, --yes"));
}

#[test]
fn monitor_detach_second_monitor() {
    let out = rmod(&["monitor", "detach", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_disable_is_alias_for_detach() {
    let out = rmod(&["monitor", "disable", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_off_is_alias_for_detach() {
    let out = rmod(&["monitor", "off", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_flags_before_action_detach() {
    let out = rmod(&["monitor", "-m", "2", "detach", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_flags_before_action_sleep_rejected() {
    let out = rmod(&["monitor", "-m", "2", "sleep"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("not valid for monitor sleep"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn monitor_detach_primary_is_error() {
    let out = rmod(&["monitor", "detach", "-m", SERIAL_A, "-y"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("cannot detach the primary display"));
}

#[test]
fn monitor_detach_without_monitor_is_error() {
    for args in [&["monitor", "detach"][..], &["monitor", "attach"][..]] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains("needs -m, --monitor"),
            "stderr: {}",
            stderr(&out)
        );
    }
}

#[test]
fn monitor_attach_attached_is_unchanged() {
    let out = rmod(&["monitor", "attach", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_enable_is_alias_for_attach() {
    let out = rmod(&["monitor", "enable", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_on_is_alias_for_attach() {
    let out = rmod(&["monitor", "on", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_sleep_prints_slept_per_line() {
    let out = rmod(&["monitor", "sleep"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("slept RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("slept RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("asleep"));
}

#[test]
fn monitor_wake_prints_woke_per_line() {
    let out = rmod(&["monitor", "wake"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("woke RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("woke RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("awake"));
}

#[test]
fn monitor_detach_help_shows_aliases() {
    let out = rmod(&["monitor", "detach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Aliases:"));
    assert!(text.contains("disable, off"));
}

#[test]
fn monitor_attach_help_shows_aliases() {
    let out = rmod(&["monitor", "attach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Aliases:"));
    assert!(text.contains("enable, on"));
}

#[test]
fn monitor_help_hides_aliases() {
    let out = rmod(&["monitor", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(!text.contains("aliases"), "got: {text}");
    assert!(!text.contains("disable, off"));
    assert!(!text.contains("enable, on"));
}

#[test]
fn monitor_sleep_rejects_monitor_flag() {
    let out = rmod(&["monitor", "sleep", "-m", SERIAL_B]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for monitor sleep"));
}

#[test]
fn monitor_sleep_rejects_yes_flag() {
    let out = rmod(&["monitor", "sleep", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for monitor sleep"));
}

#[test]
fn monitor_missing_action_is_error() {
    let out = rmod(&["monitor"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor needs an action"));
}

#[test]
fn monitor_unknown_action_is_error() {
    let out = rmod(&["monitor", "frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown action frobnicate for monitor"));
}

#[test]
fn monitor_unknown_monitor_is_error() {
    let out = rmod(&["monitor", "detach", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn monitor_detach_all_skips_primary_and_detaches_secondary() {
    let out = rmod(&["monitor", "detach", "-m", "all", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stderr(&out).is_empty(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("skipped RMOD Fake Monitor 1 [:1], the primary display cannot be detached")
    );
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn monitor_attach_all_is_unchanged() {
    let out = rmod(&["monitor", "attach", "-m", "all", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 1 [:1] is already attached"));
    assert!(text.contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn monitor_brightness_sets_primary_via_ddc() {
    let out = rmod(&["monitor", "brightness", "30"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] brightness to 30% via ddc"));
}

#[test]
fn monitor_brightness_monitor_flag_before_value() {
    let out = rmod(&["monitor", "brightness", "-m", "2", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to 60% via gamma"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn monitor_brightness_already_at_is_noop() {
    let out = rmod(&["monitor", "brightness", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 1 [:1] is already at 60%"));
    assert!(!stdout(&out).contains("via"));
}

#[test]
fn monitor_brightness_second_monitor_falls_back_to_gamma() {
    let out = rmod(&["monitor", "brightness", "30", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to 30% via gamma"));
}

#[test]
fn monitor_brightness_all_targets_every_display() {
    let out = rmod(&["monitor", "brightness", "30", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 1 [:1] brightness to 30% via ddc"));
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] brightness to 30% via gamma"));
}

#[test]
fn monitor_brightness_via_flag() {
    let out = rmod(&["monitor", "brightness", "30", "--via", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn monitor_brightness_via_short_flag() {
    let out = rmod(&["monitor", "brightness", "30", "-v", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn monitor_brightness_forced_unsupported_backend_is_error() {
    let out = rmod(&["monitor", "brightness", "30", "-m", "2", "--via", "ddc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not support ddc brightness control"));
}

#[test]
fn monitor_brightness_unknown_backend_is_error() {
    let out = rmod(&["monitor", "brightness", "30", "--via", "gamma2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown backend gamma2"));
}

#[test]
fn monitor_brightness_out_of_range_is_error() {
    let out = rmod(&["monitor", "brightness", "150"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid brightness 150"));
}

#[test]
fn monitor_brightness_missing_value_is_error() {
    let out = rmod(&["monitor", "brightness"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor brightness needs a value"));
}

#[test]
fn monitor_brightness_rejects_yes_flag() {
    let out = rmod(&["monitor", "brightness", "30", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for monitor brightness"));
}

#[test]
fn monitor_brightness_zero_is_valid() {
    let out = rmod(&["monitor", "brightness", "0", "-m", "1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] brightness to 0% via ddc"));
}

#[test]
fn monitor_brightness_unknown_monitor_is_error() {
    let out = rmod(&["monitor", "brightness", "30", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn monitor_brightness_mode_min_sets_primary() {
    let out = rmod(&["monitor", "brightness", "min"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("set RMOD Fake Monitor 1 [:1] brightness to min (slider 5 + gamma 50%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn monitor_brightness_mode_min_on_second_monitor_is_gamma_only() {
    let out = rmod(&["monitor", "brightness", "min", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] brightness to min (gamma 50%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn monitor_brightness_mode_max_sets_primary() {
    let out = rmod(&["monitor", "brightness", "max", "-m", "1"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("set RMOD Fake Monitor 1 [:1] brightness to max (ddc 100 + gamma 100%)"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn monitor_brightness_mode_boost_applies_and_warns_clipping() {
    let out = rmod(&["monitor", "brightness", "boost"]);
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
fn monitor_brightness_mode_min_all_targets_every_display() {
    let out = rmod(&["monitor", "brightness", "min", "-m", "all"]);
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
fn monitor_brightness_mode_rejects_via_flag() {
    for args in [
        &["monitor", "brightness", "min", "-v", "ddc"][..],
        &["monitor", "brightness", "min", "--via", "ddc"][..],
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
fn monitor_brightness_unknown_mode_word_is_error() {
    let out = rmod(&["monitor", "brightness", "dimm"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid brightness dimm. use a number between 0 and 100"));
}

#[test]
fn monitor_brightness_help_flag() {
    let out = rmod(&["monitor", "brightness", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod monitor brightness <VALUE> [OPTIONS]"));
    assert!(text.contains("--via"));
}

#[test]
fn monitor_help_lists_brightness() {
    let out = rmod(&["monitor", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(
        text.contains("brightness Set the display backlight level (0-100, or min, max, boost)")
    );
}

#[test]
fn monitor_contrast_monitor_flag_before_value_reset() {
    let out = rmod(&["monitor", "contrast", "-m", "2", "reset"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("set RMOD Fake Monitor 2 [:2] contrast to 100% via gamma"),
        "got: {}",
        stdout(&out)
    );
}

#[test]
fn monitor_contrast_sets_primary_via_ddc() {
    let out = rmod(&["monitor", "contrast", "60"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] contrast to 60% via ddc"));
}

#[test]
fn monitor_contrast_already_at_is_noop() {
    let out = rmod(&["monitor", "contrast", "75"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 1 [:1] is already at 75%"));
    assert!(!stdout(&out).contains("via"));
}

#[test]
fn monitor_contrast_second_monitor_falls_back_to_gamma() {
    let out = rmod(&["monitor", "contrast", "60", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 2 [:2] contrast to 60% via gamma"));
}

#[test]
fn monitor_contrast_all_targets_every_display() {
    let out = rmod(&["monitor", "contrast", "60", "-m", "all"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 1 [:1] contrast to 60% via ddc"));
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] contrast to 60% via gamma"));
}

#[test]
fn monitor_contrast_via_short_flag() {
    let out = rmod(&["monitor", "contrast", "60", "-v", "gamma"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via gamma"));
}

#[test]
fn monitor_contrast_via_long_flag() {
    let out = rmod(&["monitor", "contrast", "60", "--via", "ddc"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("via ddc"));
}

#[test]
fn monitor_contrast_forced_unsupported_backend_is_error() {
    let out = rmod(&["monitor", "contrast", "60", "-m", "2", "--via", "ddc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not support ddc contrast control"));
}

#[test]
fn monitor_contrast_unknown_backend_is_error() {
    let out = rmod(&["monitor", "contrast", "60", "--via", "slider"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown backend slider. use ddc or gamma"));
}

#[test]
fn monitor_contrast_out_of_range_is_error() {
    let out = rmod(&["monitor", "contrast", "131"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid contrast 131. use a number between 0 and 130"));
}

#[test]
fn monitor_contrast_missing_value_is_error() {
    let out = rmod(&["monitor", "contrast"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor contrast needs a value"));
}

#[test]
fn monitor_contrast_rejects_yes_flag() {
    let out = rmod(&["monitor", "contrast", "60", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("-y, --yes is not valid for monitor contrast"));
}

#[test]
fn monitor_contrast_zero_is_valid() {
    let out = rmod(&["monitor", "contrast", "0"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("set RMOD Fake Monitor 1 [:1] contrast to 0% via ddc"));
}

#[test]
fn monitor_contrast_overdrive_warns_clipping() {
    let out = rmod(&["monitor", "contrast", "130", "-m", "2"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("set RMOD Fake Monitor 2 [:2] contrast to 130% via gamma"));
    assert!(text.contains("contrast boost clips shadows and highlights"));
}

#[test]
fn monitor_contrast_unknown_monitor_is_error() {
    let out = rmod(&["monitor", "contrast", "60", "-m", "99"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn monitor_contrast_help_flag() {
    let out = rmod(&["monitor", "contrast", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod monitor contrast <VALUE> [OPTIONS]"));
}

#[test]
fn monitor_help_lists_contrast() {
    let out = rmod(&["monitor", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("contrast   Set the display contrast (0-130, 100 = neutral)"));
}
