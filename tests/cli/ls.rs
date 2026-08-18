use super::common::{SERIAL_A, rmod, stderr, stdout, strip_ansi};

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
        assert!(
            text.contains(line),
            "missing mode line '{line}' in:\n{text}"
        );
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
fn list_short_compact_output() {
    let out = rmod(&["list", "--short"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let stdout = stdout(&out);
    // Expected format: # N: Name [fingerprint]  WxH@Hz  (primary?)
    assert!(
        stdout.contains("1: RMOD Fake Monitor 1 [a1b2c3d4]  1920x1080@60Hz  (primary)"),
        "missing primary monitor line: {stdout}"
    );
    assert!(
        stdout.contains("2: RMOD Fake Monitor 2 [b2c3d4e5]  1920x1080@60Hz"),
        "missing second monitor line: {stdout}"
    );
    // Should have exactly 2 lines
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected exactly 2 lines, got: {stdout}");
}
