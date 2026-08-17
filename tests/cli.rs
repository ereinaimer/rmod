use std::process::Command;

const SERIAL_A: &str = "ABC12345678"; // RMOD Fake Monitor 1 (primary)
const SERIAL_B: &str = "DEF45678901"; // RMOD Fake Monitor 2

fn rmod(args: &[&str]) -> std::process::Output {
    rmod_env(args, &[])
}

fn rmod_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rmod"));
    cmd.args(args).env("RMOD_SYS_FAKE", "1");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("failed to run rmod")
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn strip_ansi(s: &str) -> String {
    s.replace("\x1b[92m", "")
        .replace("\x1b[4m", "")
        .replace("\x1b[0m", "")
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn no_args_prints_help() {
    let out = rmod(&[]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod [COMMAND] [OPTIONS]"));
    assert!(text.contains("Commands:"));
    assert!(text.contains("list     List displays and their current settings"));
    assert!(text.contains("set      Apply resolution, refresh rate, and orientation"));
    assert!(text.contains("layout   Show the monitor arrangement or move monitors"));
    assert!(text.contains("monitor  Attach, detach, sleep, or wake monitors"));
    assert!(text.contains("temp     Set or show the display color temperature"));
    assert!(text.contains("--help     Print help"));
    assert!(text.contains("--version  Print version"));
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
    assert_eq!(rmod(&["-h"]).status.code(), Some(2));
    assert!(rmod(&["--help"]).status.success());
}

#[test]
fn version_flags_exit_zero() {
    let out = rmod(&["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("rmod"));
}

#[test]
fn subcommand_help_flags_exit_zero() {
    assert_eq!(rmod(&["ls", "-h"]).status.code(), Some(2));
    assert!(rmod(&["ls", "--help"]).status.success());
    assert_eq!(rmod(&["set", "-p", "1080", "-h"]).status.code(), Some(2));
    assert!(rmod(&["set", "-p", "4k", "--help"]).status.success());
    assert_eq!(rmod(&["layout", "-h"]).status.code(), Some(2));
    assert!(rmod(&["layout", "--help"]).status.success());
    assert_eq!(rmod(&["temp", "-h"]).status.code(), Some(2));
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

#[test]
fn list_is_alias_for_ls() {
    assert_eq!(stdout(&rmod(&["list"])), stdout(&rmod(&["ls"])));
    let out = rmod(&["list", "--help"]);
    assert!(out.status.success());
    assert!(
        strip_ansi(&stdout(&out)).contains("Alias: ls"),
        "list help must mention the ls alias"
    );
}

#[test]
fn list_lists_displays() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("RMOD Fake Monitor 1"));
    assert!(stdout.contains("RMOD Fake Monitor 2"));
}

#[test]
fn list_shows_full_edid_block() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for line in [
        "Primary:         true",
        "Manufacturer:    RM1",
        "Current:         1920x1080 @ 60Hz",
        "Native:          1920x1080 @ 60Hz",
        "Physical:        27.0\" (59.8 cm × 33.6 cm)",
        "DPI:             82×82 physical / 96 logical",
        "Color Depth:     32-bit (RGB 8:8:8)",
        "Orientation:     Landscape",
        "Connector:       Internal",
        "Manufactured:    Week 12, 2023",
        "Gamma:           2.2",
        "HDR:             HDR10 (not active)",
        "Color Gamut:     sRGB 100% / DCI-P3 74%",
        "Supported:",
    ] {
        assert!(text.contains(line), "missing line '{line}' in:\n{text}");
    }
}

#[test]
fn list_shows_second_monitor_color_and_gamut() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for line in [
        "Physical:        24.0\" (53.1 cm × 29.9 cm)",
        "DPI:             92×92 physical / 144 logical",
        "Color Depth:     30-bit (RGB 10:10:10)",
        "Orientation:     Landscape",
        "Connector:       DisplayPort",
        "Gamma:           2.4",
        "HDR:             Not supported",
        "Color Gamut:     sRGB 100% / DCI-P3 100%",
    ] {
        assert!(text.contains(line), "missing line '{line}' in:\n{text}");
    }
}

#[test]
fn list_values_align_at_column_19() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for value in [
        "true",
        "27.0\" (59.8 cm × 33.6 cm)",
        "82×82 physical / 96 logical",
        "32-bit (RGB 8:8:8)",
        "Landscape",
        "Internal",
        "2.2",
        "HDR10 (not active)",
        "sRGB 100% / DCI-P3 74%",
    ] {
        let line = text
            .lines()
            .find(|l| l.contains(value))
            .unwrap_or_else(|| panic!("no line with '{value}' in:\n{text}"));
        assert_eq!(
            line.find(value),
            Some(19),
            "value '{value}' must start at column 19, line: '{line}'"
        );
    }
}

#[test]
fn list_marks_primary_display() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert_eq!(text.matches("Primary:         true").count(), 1);
    assert_eq!(text.matches("Primary:         false").count(), 1);
}

#[test]
fn list_shows_supported_modes_grouped_by_resolution() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for line in [
        "1280x720  @ 60Hz",
        "1920x1080 @ 60Hz, 144Hz",
        "2560x1440 @ 60Hz, 144Hz",
        "3840x2160 @ 60Hz, 144Hz",
    ] {
        assert!(text.contains(line), "missing mode line '{line}' in:\n{text}");
    }
}

#[test]
fn list_lists_monitors_in_stable_order() {
    let out = rmod(&["list"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    // Sort key is the EDID fingerprint (a1b2c3d4 < b2c3d4e5 in the fake world).
    let pos_a = text.find("RMOD Fake Monitor 1").expect("monitor 1 present");
    let pos_b = text.find("RMOD Fake Monitor 2").expect("monitor 2 present");
    assert!(pos_a < pos_b, "monitors must sort by fingerprint");
}

#[test]
fn list_rejects_old_caps_flag() {
    let out = rmod(&["list", "--caps"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unexpected argument --caps for list"));
}

#[test]
fn list_rejects_old_monitor_flag() {
    let out = rmod(&["list", "-m", SERIAL_A]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unexpected argument -m for list"));
}

#[test]
fn list_unknown_argument_exits_2() {
    let out = rmod(&["list", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

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
fn flag_with_trailing_argument_exits_2() {
    let out = rmod(&["--help", "extra"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error:"));
}

#[test]
fn ls_shows_fake_environment() {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("RMOD Fake Monitor"),
        "expected fake monitor names: {stdout}"
    );
    assert!(
        stdout.contains("1920x1080"),
        "expected fake resolution: {stdout}"
    );
}

#[test]
fn caps_is_unknown_command() {
    let out = rmod(&["caps"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown command"));
}

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

fn current_mode() -> (String, String, String) {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("Current:"))
        .expect("no Current: line");
    let value = line
        .split_once(':')
        .map(|(_, v)| v.trim())
        .expect("Current: value");
    let (res, refresh) = value.split_once('@').expect("resolution @ refresh");
    let (width, height) = res.trim().split_once('x').expect("WxH");
    let refresh = refresh.trim().trim_end_matches("Hz");
    (width.trim().into(), height.trim().into(), refresh.into())
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
fn layout_show_lists_positions() {
    let out = rmod(&["layout"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("RELATIVE TO"),
        "missing RELATIVE TO header: {stdout}"
    );
    assert!(
        stdout.contains("RMOD Fake Monitor"),
        "expected fake monitor names: {stdout}"
    );
    assert!(
        stdout.contains("(primary)"),
        "missing primary marker: {stdout}"
    );
    assert!(
        stdout.contains("right of 1"),
        "missing relative position: {stdout}"
    );
}

#[test]
fn layout_places_monitor_left_of_primary() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--left-of", SERIAL_A, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed"),
        "missing placement line: {stdout}"
    );
    assert!(
        stdout.contains("to the left of"),
        "missing direction wording: {stdout}"
    );
}

#[test]
fn layout_places_monitor_below_explicit_reference() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--below", SERIAL_A, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("below"),
        "missing direction wording: {}",
        stdout(&out)
    );
}

#[test]
fn layout_noop_placement_reports_already() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--right-of", SERIAL_A, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains(
            "RMOD Fake Monitor 2 [:2] is already to the right of RMOD Fake Monitor 1 [:1]"
        ),
        "expected already-there message: {stdout}"
    );
    assert!(
        !stdout.contains("placed"),
        "no placement line expected: {stdout}"
    );
    assert!(
        !stdout.contains("keep changes"),
        "no prompt expected: {stdout}"
    );
}

#[test]
fn layout_places_monitor_right_of_explicit_reference() {
    let out = rmod(&["layout", "-m", SERIAL_A, "--right-of", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed") && stdout.contains("to the right of"),
        "missing placement line: {stdout}"
    );
}

#[test]
fn layout_places_monitor_above_explicit_reference() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--above", SERIAL_A, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(
        stdout.contains("placed") && stdout.contains("above"),
        "missing placement line: {stdout}"
    );
}

#[test]
fn layout_primary_promotes() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--primary", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("is now the main display"));
    assert!(!stdout(&out).contains("keep changes"));
    assert!(!stdout(&out).contains("applied"));
    let noop = rmod(&["layout", "-m", SERIAL_A, "--primary", "-y"]);
    assert!(noop.status.success(), "stderr: {}", stderr(&noop));
    assert!(stdout(&noop).contains("already the main display"));
}

#[test]
fn layout_primary_keyword_promotes_primary_is_noop() {
    let out = rmod(&["layout", "-m", "primary", "--primary", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("already the main display"));
}

#[test]
fn layout_primary_keyword_as_reference() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--left-of", "primary", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    assert!(stdout.contains("placed"), "missing placement line: {stdout}");
    assert!(
        stdout.contains("to the left of"),
        "missing direction wording: {stdout}"
    );
}

#[test]
fn layout_all_is_rejected() {
    for args in [
        &["layout", "-m", "all", "--primary"][..],
        &["layout", "-m", SERIAL_B, "--left-of", "all", "-y"][..],
    ] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains("not 'all'"),
            "args: {args:?}: {}",
            stderr(&out)
        );
    }
}

#[test]
fn layout_self_reference_is_error() {
    let out = rmod(&["layout", "-m", SERIAL_A, "--left-of", SERIAL_A, "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: cannot place monitor 1 relative to itself"));
}

#[test]
fn layout_missing_monitor_is_error() {
    let out = rmod(&["layout", "--left-of", SERIAL_A]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out)
            .contains("missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5")
    );
}

#[test]
fn layout_monitor_without_action_is_error() {
    let out = rmod(&["layout", "-m", SERIAL_B]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: -m, --monitor needs a direction flag or --primary"));
}

#[test]
fn layout_missing_value_for_direction_is_error() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--left-of"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: --left-of needs a value"));
}

#[test]
fn layout_primary_with_direction_is_error() {
    let out = rmod(&["layout", "-m", SERIAL_B, "--primary", "--left-of", SERIAL_A]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: use --primary or a direction flag, not both"));
}

#[test]
fn layout_unknown_argument_is_error() {
    let out = rmod(&["layout", "-m", SERIAL_B, "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error: unexpected argument foo for layout"));
}

#[test]
fn layout_help_flag() {
    assert_eq!(rmod(&["layout", "-h"]).status.code(), Some(2));
    let out = rmod(&["layout", "--help"]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod layout [OPTIONS]"));
    assert!(text.contains("-y, --yes"), "layout page must advertise -y");
    assert!(
        text.contains("--primary"),
        "layout page must advertise --primary"
    );
    assert_eq!(rmod(&["layout", "-m", SERIAL_B, "-h"]).status.code(), Some(2));
    assert!(rmod(&["layout", "-m", SERIAL_B, "--help"]).status.success());
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
fn main_command_removed_shows_hint() {
    let out = rmod(&["main", "2"]);
    assert_eq!(out.status.code(), Some(2));
    let err = stderr(&out);
    assert!(err.contains("unknown command main"), "stderr: {err}");
    assert!(err.contains("layout"), "missing migration hint: {err}");
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
    let out = rmod(&["set", "-w", "1920", "-h", "1080", "-m", SERIAL_B, "-o", "90"]);
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
    assert!(text.contains("set RMOD Fake Monitor 1 [:1]"), "stdout: {text}");
    assert!(text.contains("set RMOD Fake Monitor 2 [:2]"), "stdout: {text}");
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

#[test]
fn monitor_help_exits_zero() {
    assert_eq!(rmod(&["monitor", "-h"]).status.code(), Some(2));
    assert!(rmod(&["monitor", "--help"]).status.success());
    assert!(rmod(&["monitor", "detach", "--help"]).status.success());
    let text = strip_ansi(&stdout(&rmod(&["monitor", "--help"])));
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
    assert!(stdout(&out).contains(
        "skipped RMOD Fake Monitor 1 [:1], the primary display cannot be detached"
    ));
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
fn temp_no_args_shows_primary() {
    let out = rmod(&["temp"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("RMOD Fake Monitor 1 [:1] is currently approx 6500K"),
        "expected show line: {text}"
    );
    assert!(!text.contains("RMOD Fake Monitor 2"), "primary only: {text}");
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
fn temp_invalid_value_is_error() {
    let out = rmod(&["temp", "bogus"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid temperature"));
}

#[test]
fn temp_help_flag() {
    assert_eq!(rmod(&["temp", "-h"]).status.code(), Some(2));
    let out = rmod(&["temp", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("Set or show the display color temperature"));
    assert!(text.contains("Presets:"));
    assert!(text.contains("candle"));
    assert!(!text.contains("-y, --yes"), "temp page must not advertise -y");
}

#[test]
fn serial_targeting_is_case_insensitive() {
    let out = rmod(&["set", "--max", "-m", "abc12345678", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("3840x2160"),
        "got: {}",
        stdout(&out)
    );
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
    let detach = rmod(&["monitor", "detach", "-m", SERIAL_B, "-y"]);
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
        &["monitor", "detach", "-m", "NOPE"][..],
        &["temp", "-m", "NOPE", "3400"][..],
    ] {
        let out = rmod(args);
        assert_eq!(out.status.code(), Some(2), "args: {args:?}");
        assert!(
            stderr(&out).contains("not found. run rmod list to see connected displays"),
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
