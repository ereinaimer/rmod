use super::common::{rmod, stderr, stdout};

#[test]
fn view_mirror_help() {
    let out = rmod(&["view", "mirror", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("view mirror"));
    assert!(text.contains("Clone all displays"));
}

#[test]
fn view_extend_help() {
    let out = rmod(&["view", "extend", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("view extend"));
    assert!(text.contains("Restore extended desktop"));
}

#[test]
fn view_project_help() {
    let out = rmod(&["view", "project", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("view project"));
    assert!(text.contains("Second screen only"));
}

#[test]
fn view_single_help() {
    let out = rmod(&["view", "single", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("view single"));
    assert!(text.contains("PC screen only"));
}

#[test]
fn view_mirror_noop_when_single_monitor() {
    // In fake environment, we have 2 monitors, so this should apply
    let out = rmod(&["view", "mirror", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    // Should show applied changes for both monitors
    assert!(
        text.contains("applied") || text.contains("already"),
        "expected applied or already: {text}"
    );
}

#[test]
fn view_extend_auto_arranges() {
    let out = rmod(&["view", "extend", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("applied") || text.contains("already") || text.contains("placed"),
        "expected placement: {text}"
    );
}

#[test]
fn view_project_disables_primary() {
    let out = rmod(&["view", "project", "-y"]);
    // Should succeed if there's an external monitor
    assert!(
        out.status.success() || out.status.code() == Some(2),
        "stderr: {}",
        stderr(&out)
    );
    let text = stdout(&out);
    if out.status.success() {
        assert!(
            text.contains("detached") || text.contains("already"),
            "expected detached: {text}"
        );
    }
}

#[test]
fn view_single_enables_only_specified() {
    let out = rmod(&["view", "single", "-m", "2", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("detached") || text.contains("attached") || text.contains("already"),
        "expected attach/detach: {text}"
    );
}

#[test]
fn view_yes_flag_before_subcommand_mirror() {
    let out = rmod(&["view", "-y", "mirror"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("applied") || text.contains("already"),
        "expected applied or already: {text}"
    );
}

#[test]
fn view_monitor_flag_before_subcommand_single() {
    let out = rmod(&["view", "-m", "2", "single", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("detached") || text.contains("attached") || text.contains("already"),
        "expected attach/detach: {text}"
    );
}

#[test]
fn view_single_invalid_monitor_errors() {
    let out = rmod(&["view", "single", "-m", "99", "-y"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("not found"),
        "stderr: {}",
        stderr(&out)
    );
}

#[test]
fn view_unknown_subcommand_errors() {
    let out = rmod(&["view", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unknown"), "stderr: {}", stderr(&out));
}

#[test]
fn view_missing_subcommand_errors() {
    let out = rmod(&["view"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error"), "stderr: {}", stderr(&out));
}

#[test]
fn view_monitor_flag_without_subcommand_errors() {
    let out = rmod(&["view", "-m", "2"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("needs a subcommand"),
        "stderr: {}",
        stderr(&out)
    );
}
