// The cli integration tests drive the fake backend through RMOD_SYS_FAKE;
// the fake backend is compiled only with the `fake` feature (or cfg(test)),
// so these tests require `--features fake` (CI uses --all-features).
#[cfg(not(feature = "fake"))]
compile_error!(
    "cli integration tests need the fake backend: run with --features fake (CI uses --all-features)"
);

use std::process::Command;

pub const SERIAL_A: &str = "ABC12345678"; // RMOD Fake Monitor 1 (primary)
pub const SERIAL_B: &str = "DEF45678901"; // RMOD Fake Monitor 2

pub fn rmod(args: &[&str]) -> std::process::Output {
    rmod_env(args, &[])
}

pub fn rmod_env(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rmod"));
    cmd.args(args).env("RMOD_SYS_FAKE", "1");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("failed to run rmod")
}

pub fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn strip_ansi(s: &str) -> String {
    s.replace("\x1b[92m", "")
        .replace("\x1b[4m", "")
        .replace("\x1b[0m", "")
}

pub fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

pub fn current_mode() -> (String, String, String) {
    let out = rmod(&["ls"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("Current:"))
        .expect("no Current: line");
    let value = line
        .split_once(':')
        .map(|(_, v)| v.trim())
        .expect("Current: value");
    let (res, refresh) = value.split_once('@').expect("resolution @ refresh");
    let (width, height) = res.trim().split_once('x').expect("WxH");
    let refresh = refresh.trim().trim_end_matches("Hz");
    (width.trim().into(), height.trim().into(), refresh.into())
}
