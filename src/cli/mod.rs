//! Command-line surface: argument parsing and help output.
//!
//! [`parser`] turns argv into a [`Command`]; [`help`] renders the usage
//! pages for the top-level help and per-command topics.

mod help;
mod parser;

pub use help::{caps, help, ls, max, set, version};
pub use parser::{parse, Command, HelpTopic};