use super::common::{SERIAL_B, rmod, stderr, stdout, strip_ansi};

#[test]
fn sleep_prints_slept_per_line() {
    let out = rmod(&["sleep"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("slept RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("slept RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("asleep"));
}

#[test]
fn wake_prints_woke_per_line() {
    let out = rmod(&["wake"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("woke RMOD Fake Monitor 1 [:1]"));
    assert!(text.contains("woke RMOD Fake Monitor 2 [:2]"));
    assert!(!text.contains("awake"));
}

#[test]
fn sleep_rejects_monitor_flag() {
    let out = rmod(&["sleep", "-m", SERIAL_B]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for sleep"));
}

#[test]
fn sleep_rejects_yes_flag() {
    let out = rmod(&["sleep", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("not valid for sleep"));
}

#[test]
fn sleep_help_page() {
    let out = rmod(&["sleep", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod sleep"));
    assert!(text.contains("Put every monitor to sleep"));
}

#[test]
fn wake_help_page() {
    let out = rmod(&["wake", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod wake"));
    assert!(text.contains("Wake every monitor"));
}
