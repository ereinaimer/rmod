use std::env;

pub enum Refresh {
    Keep,
    Max,
    Fixed(u32),
}

pub enum HelpTopic {
    List,
    Max,
    Caps,
}

pub enum Command {
    List,
    Max { monitor: Option<u32> },
    Caps { monitor: Option<u32> },
    Set { width: u32, height: u32, refresh: Refresh, monitor: Option<u32> },
    Help { topic: Option<HelpTopic> },
    Version,
}

const PROFILES: &[(&str, u32, u32)] = &[
    ("720", 1280, 720),
    ("1080", 1920, 1080),
    ("1440", 2560, 1440),
    ("4k", 3840, 2160),
    ("8k", 7680, 4320),
];

pub fn parse() -> Result<Command, String> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        return Ok(Command::Help { topic: None });
    };
    match cmd.as_str() {
        "-h" | "--help" => Ok(Command::Help { topic: None }),
        "-V" | "--version" => Ok(Command::Version),
        "ls" => parse_tail("ls", args.next().as_deref(), Command::List, HelpTopic::List),
        "max" => parse_tail("max", args.next().as_deref(), Command::Max { monitor: None }, HelpTopic::Max),
        "caps" => parse_tail("caps", args.next().as_deref(), Command::Caps { monitor: None }, HelpTopic::Caps),
        _ if cmd.starts_with("max:") => {
            let monitor = parse_monitor(&cmd[4..], &cmd)?;
            Ok(Command::Max { monitor: Some(monitor) })
        }
        _ if cmd.starts_with("caps:") => {
            let monitor = parse_monitor(&cmd[5..], &cmd)?;
            Ok(Command::Caps { monitor: Some(monitor) })
        }
        _ => parse_set(&cmd),
    }
}

fn parse_tail(
    name: &str,
    tail: Option<&str>,
    cmd: Command,
    topic: HelpTopic,
) -> Result<Command, String> {
    match tail {
        Some("-h" | "--help") => Ok(Command::Help { topic: Some(topic) }),
        Some(other) => Err(format!("unknown argument '{other}' for '{name}'")),
        None => Ok(cmd),
    }
}

fn parse_set(cmd: &str) -> Result<Command, String> {
    let (spec, monitor) = match cmd.split_once(':') {
        Some((spec, m)) => (spec, Some(parse_monitor(m, cmd)?)),
        None => (cmd, None),
    };
    let (res, refresh) = match spec.split_once('@') {
        Some((res, r)) => (res, Some(parse_refresh(r, cmd)?)),
        None => (spec, None),
    };
    let (width, height) = match res.split_once('x') {
        Some((w, h)) => (
            w.parse().map_err(|_| format!("invalid width in '{cmd}'"))?,
            h.parse().map_err(|_| format!("invalid height in '{cmd}'"))?,
        ),
        None => match PROFILES.iter().find(|(name, _, _)| *name == res) {
            Some((_, w, h)) => (*w, *h),
            None => return Err(format!("unknown profile or invalid resolution '{cmd}'")),
        },
    };
    Ok(Command::Set {
        width,
        height,
        refresh: refresh.unwrap_or(Refresh::Keep),
        monitor,
    })
}

fn parse_refresh(r: &str, cmd: &str) -> Result<Refresh, String> {
    if r == "max" {
        Ok(Refresh::Max)
    } else {
        r.parse()
            .map(Refresh::Fixed)
            .map_err(|_| format!("invalid refresh rate in '{cmd}'"))
    }
}

fn parse_monitor(m: &str, cmd: &str) -> Result<u32, String> {
    m.parse().map_err(|_| format!("invalid monitor id in '{cmd}'"))
}