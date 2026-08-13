//! rmod — resolution modifier.
//!
//! Lists displays, queries supported modes, and applies resolution/refresh
//! rate changes. Exits with 0 on success and 2 on error.

#![warn(missing_docs)]

mod cli;
mod sys;

fn main() {
    let code = match cli::parse() {
        Ok(command) => cli::run(command),
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    };
    std::process::exit(code);
}
