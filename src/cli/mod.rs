//! Command-line surface: argument parsing, help output and dispatch.
//!
//! [`parser`] turns argv into a [`Command`]; [`commands`] executes it and
//! renders the output; [`help`] renders the usage pages for the top-level
//! help and per-command topics.

mod commands;
mod confirm;
mod help;
mod parser;

pub use commands::run;
pub use confirm::{Confirm, confirm_keep};
pub use help::{help, layout, ls, monitor, monitor_attach, monitor_brightness, monitor_detach, set, version};
pub use parser::{
    BrightnessBackend, Command, Direction, HelpTopic, LayoutAction, MonitorAction, MonitorTarget,
    SetSpec, parse,
};
