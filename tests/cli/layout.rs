use super::common::{SERIAL_A, SERIAL_B, rmod, stderr, stdout, strip_ansi};

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
        stderr(&out).contains(
            "missing monitor for layout\ne.g. rmod layout -m a1b2c3d4 --left-of b2c3d4e5"
        )
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
fn layout_flags_in_any_order_promote_primary() {
    let out = rmod(&["layout", "-y", "-m", "2", "--primary"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("is now the main display"));
    assert!(!stdout(&out).contains("keep changes"));
}

#[test]
fn layout_help_flag() {
    assert!(rmod(&["layout", "-h"]).status.success());
    let out = rmod(&["layout", "--help"]);
    assert!(out.status.success());
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod layout [OPTIONS]"));
    assert!(text.contains("-y, --yes"), "layout page must advertise -y");
    assert!(
        text.contains("--primary"),
        "layout page must advertise --primary"
    );
    assert!(rmod(&["layout", "-m", SERIAL_B, "-h"]).status.success());
    assert!(rmod(&["layout", "-m", SERIAL_B, "--help"]).status.success());
}
