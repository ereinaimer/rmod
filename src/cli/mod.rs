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
    completions, help, layout, ls, monitor, monitor_attach, monitor_brightness, monitor_contrast,
    monitor_detach, set, temp, version, view, view_extend_help, view_mirror_help,
    view_project_help, view_single_help,
};
pub use parser::{
    BrightnessBackend, Command, Direction, HelpTopic, LayoutAction, MonitorAction, MonitorTarget,
    SetSpec, TempAction, ViewAction, parse,
};
