//! Confirmation prompt: asks whether to keep applied settings, with a
//! live countdown that reverts on timeout.
//!
//! When stdin is not a terminal the prompt is skipped entirely and
//! [`Confirm::Keep`] is returned immediately, so scripts and piped
//! invocations never hang on an interactive question. When stdout is not
//! a terminal the prompt and countdown are printed as plain lines without
//! ANSI redraw codes.

use std::io::{self, IsTerminal, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

/// Whether the applied settings should be kept or reverted.
#[derive(Debug, PartialEq)]
pub enum Confirm {
    /// Keep the applied settings.
    Keep,
    /// Revert to the previous settings.
    Revert,
}

/// Asks the user whether to keep the change, reverting after `timeout`
/// seconds unless the user answers `y` or `yes`.
///
/// Returns [`Confirm::Keep`] immediately without printing anything when
/// stdin is not a terminal. When stdin is a terminal but stdout is not,
/// the prompt and plain one-per-second countdown lines are printed
/// without ANSI redraw codes. The caller prints the "applied" line itself
/// before calling; this function only prints the prompt and the countdown.
pub fn confirm_keep(timeout: Duration) -> Confirm {
    if !io::stdin().is_terminal() {
        return Confirm::Keep;
    }
    if !io::stdout().is_terminal() {
        return confirm_keep_plain(timeout);
    }
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "keep changes? [N/y]");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        let line = line.trim_end_matches(['\r', '\n']);
        let _ = tx.send(line.to_string());
    });
    let mut remaining = timeout.as_secs();
    let _ = write!(stdout, "{}\r\n", countdown_line(remaining as u32));
    let _ = stdout.flush();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(line) => {
                let _ = write!(stdout, "\x1b[2A\x1b[2K");
                if !line.is_empty() {
                    let _ = write!(stdout, "\x1b[1B\x1b[2K\x1b[1A");
                    let _ = writeln!(stdout, "{line}");
                }
                let _ = stdout.flush();
                return interpret(Some(line));
            }
            Err(RecvTimeoutError::Timeout) => {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    let _ = write!(stdout, "\x1b[1A\x1b[2K");
                    let _ = stdout.flush();
                    return interpret(None);
                }
                let _ = write!(
                    stdout,
                    "\x1b[1A\x1b[2K{}\r\n",
                    countdown_line(remaining as u32)
                );
                let _ = stdout.flush();
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = write!(stdout, "\x1b[1A\x1b[2K");
                let _ = stdout.flush();
                return interpret(None);
            }
        }
    }
}

fn confirm_keep_plain(timeout: Duration) -> Confirm {
    let mut stdout = io::stdout();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        let line = line.trim_end_matches(['\r', '\n']);
        let _ = tx.send(line.to_string());
    });
    run_plain_countdown(&mut stdout, &rx, timeout)
}

fn run_plain_countdown(
    stdout: &mut dyn Write,
    rx: &mpsc::Receiver<String>,
    timeout: Duration,
) -> Confirm {
    let _ = writeln!(stdout, "keep changes? [N/y]");
    let mut remaining = timeout.as_secs();
    let _ = writeln!(stdout, "{}", countdown_line(remaining as u32));
    let _ = stdout.flush();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(line) => {
                if !line.is_empty() {
                    let _ = writeln!(stdout, "{line}");
                }
                let _ = stdout.flush();
                return interpret(Some(line));
            }
            Err(RecvTimeoutError::Timeout) => {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    let _ = stdout.flush();
                    return interpret(None);
                }
                let _ = writeln!(stdout, "{}", countdown_line(remaining as u32));
                let _ = stdout.flush();
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = stdout.flush();
                return interpret(None);
            }
        }
    }
}

fn countdown_line(n: u32) -> String {
    format!("reverting in {n}s")
}

fn interpret(input: Option<String>) -> Confirm {
    match input {
        Some(s) => match s.trim().to_lowercase().as_str() {
            "y" | "yes" => Confirm::Keep,
            _ => Confirm::Revert,
        },
        None => Confirm::Revert,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpret_none_is_revert() {
        assert_eq!(interpret(None), Confirm::Revert);
    }

    #[test]
    fn interpret_y_is_keep() {
        assert_eq!(interpret(Some("y".to_string())), Confirm::Keep);
    }

    #[test]
    fn interpret_capital_y_is_keep() {
        assert_eq!(interpret(Some("Y".to_string())), Confirm::Keep);
    }

    #[test]
    fn interpret_yes_is_keep() {
        assert_eq!(interpret(Some("yes".to_string())), Confirm::Keep);
    }

    #[test]
    fn interpret_capital_yes_is_keep() {
        assert_eq!(interpret(Some("YES".to_string())), Confirm::Keep);
    }

    #[test]
    fn interpret_n_is_revert() {
        assert_eq!(interpret(Some("n".to_string())), Confirm::Revert);
    }

    #[test]
    fn interpret_no_is_revert() {
        assert_eq!(interpret(Some("no".to_string())), Confirm::Revert);
    }

    #[test]
    fn interpret_empty_is_revert() {
        assert_eq!(interpret(Some(String::new())), Confirm::Revert);
    }

    #[test]
    fn interpret_whitespace_is_revert() {
        assert_eq!(interpret(Some("   ".to_string())), Confirm::Revert);
    }

    #[test]
    fn interpret_garbage_is_revert() {
        assert_eq!(interpret(Some("garbage".to_string())), Confirm::Revert);
    }

    #[test]
    fn countdown_line_prints_remaining_seconds() {
        assert_eq!(countdown_line(5), "reverting in 5s");
    }

    #[test]
    fn plain_output_has_no_ansi_and_reverts_when_input_disconnects() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        drop(tx);
        let result = run_plain_countdown(&mut buf, &rx, Duration::from_secs(2));
        assert_eq!(result, Confirm::Revert);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "keep changes? [N/y]\nreverting in 2s\n"
        );
    }

    #[test]
    fn plain_ticks_countdown_and_reverts_on_timeout() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        let result = run_plain_countdown(&mut buf, &rx, Duration::from_secs(2));
        let _ = tx;
        assert_eq!(result, Confirm::Revert);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "keep changes? [N/y]\nreverting in 2s\nreverting in 1s\n"
        );
    }

    #[test]
    fn plain_keeps_and_echoes_on_y() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        tx.send("y".to_string()).unwrap();
        let result = run_plain_countdown(&mut buf, &rx, Duration::from_secs(2));
        assert_eq!(result, Confirm::Keep);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "keep changes? [N/y]\nreverting in 2s\ny\n"
        );
    }

    #[test]
    fn plain_reverts_without_echo_on_empty_input() {
        let (tx, rx) = mpsc::channel();
        let mut buf = Vec::new();
        tx.send(String::new()).unwrap();
        let result = run_plain_countdown(&mut buf, &rx, Duration::from_secs(2));
        assert_eq!(result, Confirm::Revert);
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "keep changes? [N/y]\nreverting in 2s\n"
        );
    }
}
