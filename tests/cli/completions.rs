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
    assert!(text.contains("rmod"), "missing rmod command name: {text}");
    assert!(text.contains("list"), "missing list subcommand: {text}");
    assert!(text.contains("set"), "missing set subcommand: {text}");
    assert!(text.contains("layout"), "missing layout subcommand: {text}");
    assert!(
        text.contains("monitor"),
        "missing monitor subcommand: {text}"
    );
    assert!(text.contains("temp"), "missing temp subcommand: {text}");
    assert!(text.contains("view"), "missing view subcommand: {text}");
    assert!(
        text.contains("completions"),
        "missing completions subcommand: {text}"
    );
}

#[test]
fn completions_help() {
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
