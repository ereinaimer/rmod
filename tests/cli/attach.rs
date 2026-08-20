use super::common::{SERIAL_A, SERIAL_B, rmod, stderr, stdout, strip_ansi};

#[test]
fn detach_second_monitor() {
    let out = rmod(&["detach", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn attach_already_attached_is_unchanged() {
    let out = rmod(&["attach", "-m", SERIAL_B, "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn detach_primary_is_error() {
    let out = rmod(&["detach", "-m", SERIAL_A, "-y"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("cannot detach the primary display"));
}

#[test]
fn detach_without_monitor_is_error() {
    for args in [&["detach"][..], &["attach"][..]] {
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
fn unknown_monitor_is_error() {
    let out = rmod(&["detach", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("monitor 99 not found"));
}

#[test]
fn detach_all_skips_primary_and_detaches_secondary() {
    let out = rmod(&["detach", "-m", "all", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stderr(&out).is_empty(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out)
            .contains("skipped RMOD Fake Monitor 1 [:1], the primary display cannot be detached")
    );
    assert!(stdout(&out).contains("detached RMOD Fake Monitor 2 [:2]"));
}

#[test]
fn attach_all_is_unchanged() {
    let out = rmod(&["attach", "-m", "all", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("RMOD Fake Monitor 1 [:1] is already attached"));
    assert!(text.contains("RMOD Fake Monitor 2 [:2] is already attached"));
}

#[test]
fn attach_help_page() {
    let out = rmod(&["attach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod attach"));
    assert!(text.contains("Re-attach a monitor to the desktop"));
    assert!(!text.contains("Aliases"), "got: {text}");
    assert!(!text.contains("enable, on"), "got: {text}");
}

#[test]
fn detach_help_page() {
    let out = rmod(&["detach", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod detach"));
    assert!(text.contains("Detach a monitor from the desktop"));
    assert!(!text.contains("Aliases"), "got: {text}");
    assert!(!text.contains("disable, off"), "got: {text}");
}
