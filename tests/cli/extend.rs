use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn extend_help() {
    let out = rmod(&["extend", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod extend"));
    assert!(text.contains("Restore extended desktop"));
}

#[test]
fn extend_restores_layout() {
    let out = rmod(&["extend", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("applied") || text.contains("already") || text.contains("placed"),
        "expected placement: {text}"
    );
    assert!(
        text.contains("right of") || text.contains("placed"),
        "expected the second monitor arranged: {text}"
    );
}

#[test]
fn extend_without_yes_reports_already_extended_without_prompt() {
    let out = rmod(&["extend"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("already right of"),
        "fake env is pre-extended: {text}"
    );
    assert!(
        text.contains("already extended"),
        "empty placement batch must report already extended: {text}"
    );
}
