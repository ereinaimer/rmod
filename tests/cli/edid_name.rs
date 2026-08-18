use super::common::{rmod, stderr, stdout};

#[test]
fn set_output_uses_edid_name() {
    let out = rmod(&["set", "-m", "a1b2c3d4", "-p", "1080", "-y"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("RMOD Fake Monitor 1"),
        "expected EDID name in set output: {}",
        text
    );
}
