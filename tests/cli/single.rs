use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn single_help() {
    let out = rmod(&["single", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod single"));
    assert!(text.contains("PC screen only"));
}

#[test]
fn single_keeps_only_target() {
    let out = rmod(&["single", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("detached") || text.contains("attached") || text.contains("already"),
        "expected attach/detach: {text}"
    );
}

#[test]
fn single_defaults_to_primary() {
    let out = rmod(&["single", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("detached") || text.contains("attached") || text.contains("already"),
        "expected attach/detach: {text}"
    );
}

#[test]
fn single_invalid_monitor_errors() {
    let out = rmod(&["single", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("not found"),
        "stderr: {}",
        stderr(&out)
    );
}
