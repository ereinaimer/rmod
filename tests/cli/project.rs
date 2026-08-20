use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn project_help() {
    let out = rmod(&["project", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod project"));
    assert!(text.contains("Second screen only"));
}

#[test]
fn project_promotes_external_and_detaches_primary() {
    let out = rmod(&["project", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("is now the main display"),
        "expected external promoted to main: {text}"
    );
    assert!(
        text.contains("detached"),
        "expected the old primary to be detached: {text}"
    );
}
