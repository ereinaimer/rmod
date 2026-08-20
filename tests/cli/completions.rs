use super::common::{rmod, stderr, stdout};

#[test]
fn completions_outputs_powershell_script() {
    let out = rmod(&["completions"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("Register-ArgumentCompleter"),
        "missing Register-ArgumentCompleter: {text}"
    );
    assert!(
        text.contains("-CommandName 'rmod'"),
        "missing -CommandName 'rmod': {text}"
    );
    for verb in [
        "list",
        "set",
        "layout",
        "brightness",
        "contrast",
        "temp",
        "attach",
        "detach",
        "sleep",
        "wake",
        "mirror",
        "extend",
        "project",
        "single",
        "completions",
    ] {
        assert!(
            text.contains(&format!("'{verb}', '{verb}'")),
            "missing root verb {verb}: {text}"
        );
    }
    assert!(
        !text.contains("rmod;view"),
        "legacy rmod;view chain still present: {text}"
    );
    assert!(
        !text.contains("rmod;monitor"),
        "legacy rmod;monitor chain still present: {text}"
    );
}

#[test]
fn completions_help() {
    assert!(rmod(&["completions", "-h"]).status.success());
    let out = rmod(&["completions", "--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("rmod completions"));
    assert!(text.contains("PowerShell tab-completion script"));
}

#[test]
fn completions_unknown_argument_errors() {
    let out = rmod(&["completions", "foo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("unexpected argument foo for completions"));
}
