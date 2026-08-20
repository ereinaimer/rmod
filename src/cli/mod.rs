//! Command-line surface: argument parsing, help output and dispatch.
//!
//! [`parser`] turns argv into a [`Command`]; [`commands`] executes it and
//! renders the output; [`help`] renders the usage pages for the top-level
//! help and per-command topics.

mod commands;
mod confirm;
pub(crate) mod flags;
mod help;
mod parser;

pub use commands::run;
pub use confirm::{Confirm, confirm_keep};
pub use help::{
    attach, brightness, contrast, detach, extend, help, layout, ls, mirror, project, set, single,
    sleep, temp, version, wake,
};
pub use parser::{
    BrightnessBackend, Command, Direction, HelpTopic, LayoutAction, MonitorTarget, SetSpec,
    TempAction, parse,
};
