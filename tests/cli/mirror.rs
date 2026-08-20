use super::common::{rmod, stderr, stdout, strip_ansi};

#[test]
fn mirror_help() {
    let out = rmod(&["mirror", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = strip_ansi(&stdout(&out));
    assert!(text.contains("rmod mirror"));
    assert!(text.contains("Clone all displays"));
}

#[test]
fn mirror_mirrors_two_monitors_to_common_mode() {
    let out = rmod(&["mirror", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("applied 3840x2160 @ 60Hz to RMOD Fake Monitor 1 [:1]"),
        "got: {text}"
    );
    assert!(
        text.contains("applied 3840x2160 @ 60Hz to RMOD Fake Monitor 2 [:2]"),
        "got: {text}"
    );
}
