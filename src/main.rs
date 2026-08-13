//! rmod — resolution modifier.
//!
//! Lists displays, queries supported modes, and applies resolution/refresh
//! rate changes. Exits with 0 on success and 2 on error.

#![warn(missing_docs)]

mod cli;
mod sys;

const GREEN: &str = "\x1b[92m";
const RESET: &str = "\x1b[0m";

fn main() {
    let code = match cli::parse() {
        Ok(cli::Command::Help { topic: None }) => {
            println!("{}", cli::help());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::List) }) => {
            println!("{}", cli::ls());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::Max) }) => {
            println!("{}", cli::max());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::Caps) }) => {
            println!("{}", cli::caps());
            0
        }
        Ok(cli::Command::Version) => {
            println!("{}", cli::version());
            0
        }
        Ok(cli::Command::List) => match sys::windows::list() {
            Ok(monitors) => {
                let number_width = monitors
                    .iter()
                    .map(|m| m.number.to_string().len())
                    .max()
                    .unwrap_or(1)
                    .max(1);
                let name_width = monitors
                    .iter()
                    .map(|m| m.name.len())
                    .max()
                    .unwrap_or(4)
                    .max(4);
                let res_width = monitors
                    .iter()
                    .map(|m| format!("{}x{}", m.width, m.height).len())
                    .max()
                    .unwrap_or(10)
                    .max(10);
                let header = format!(
                    "{:<number_width$}  {:<7}  {:<name_width$}        {:<res_width$}  {:<7}",
                    "#", "PRIMARY", "NAME", "RESOLUTION", "REFRESH"
                );
                println!("{header}");
                println!("{}", "─".repeat(header.len()));
                for m in &monitors {
                    let primary = if m.is_primary { "*" } else { "" };
                    println!(
                        "{:<number_width$}  {:<7}  {:<name_width$}        {:<res_width$}  {:<7}",
                        m.number,
                        primary,
                        m.name,
                        format!("{}x{}", m.width, m.height),
                        format!("{}Hz", m.refresh)
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        Ok(cli::Command::Caps { monitor }) => match sys::windows::caps(monitor) {
            Ok((mon, modes)) => {
                let primary = if mon.is_primary { " (primary)" } else { "" };
                println!("{}{}:", mon.name, primary);
                let res_width = modes
                    .iter()
                    .map(|m| format!("{}x{}", m.width, m.height).len())
                    .max()
                    .unwrap_or(0);
                for mode in &modes {
                    let active =
                        mode.width == mon.width && mode.height == mon.height && mode.refresh == mon.refresh;
                    let marker = if active {
                        format!("{GREEN}*{RESET} ")
                    } else {
                        "  ".to_string()
                    };
                    println!(
                        "  {marker}{:<res_width$} @ {}Hz",
                        format!("{}x{}", mode.width, mode.height),
                        mode.refresh
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        Ok(cli::Command::Max { monitor }) => match sys::windows::max(monitor) {
            Ok(mode) => {
                println!("applied {}x{} @ {}Hz", mode.width, mode.height, mode.refresh);
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        Ok(_) => {
            eprintln!("error: command not implemented yet");
            2
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    };
    std::process::exit(code);
}