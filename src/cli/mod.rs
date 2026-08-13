//! Command-line surface: argument parsing and help output.
//!
//! [`parser`] turns argv into a [`Command`]; [`help`] renders the usage
//! pages for the top-level help and per-command topics.

mod confirm;
mod help;
mod parser;

pub use confirm::{Confirm, confirm_keep};
pub use help::{caps, help, ls, max, set, version};
pub use parser::{Command, HelpTopic, Target, parse};
