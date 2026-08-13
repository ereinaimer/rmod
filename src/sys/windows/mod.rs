mod bindings;
mod capabilities;
mod query;

pub use capabilities::Mode;
pub use query::Monitor;

pub fn list() -> Result<Vec<Monitor>, String> {
    let names = query::enumerate_devices();
    let monitors: Vec<Monitor> = names
        .iter()
        .enumerate()
        .map(|(i, name)| query::describe(i, name))
        .collect();
    if monitors.is_empty() {
        return Err("no displays found".into());
    }
    Ok(monitors)
}

pub fn caps(monitor: Option<u32>) -> Result<(Monitor, Vec<Mode>), String> {
    let names = query::enumerate_devices();
    let (index, name) = query::resolve_device(monitor, &names)?;
    let modes = capabilities::enumerate_modes(&name);
    if modes.is_empty() {
        return Err(format!("no supported modes found for monitor {}", index + 1));
    }
    Ok((
        query::describe(index, &name),
        capabilities::normalize_modes(modes),
    ))
}