use super::bindings::{encode_wide, DEVMODEW, EnumDisplaySettingsW};

#[derive(Debug, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

pub(crate) fn enumerate_modes(name: &str) -> Vec<Mode> {
    let name_wide = encode_wide(name);
    let mut modes = Vec::new();
    let mut index = 0u32;
    loop {
        let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
        let ok = unsafe { EnumDisplaySettingsW(name_wide.as_ptr(), index, &mut mode) };
        if ok == 0 {
            break;
        }
        modes.push(Mode {
            width: mode.dm_pels_width,
            height: mode.dm_pels_height,
            refresh: mode.dm_display_frequency,
        });
        index += 1;
    }
    modes
}

pub(crate) fn normalize_modes(mut modes: Vec<Mode>) -> Vec<Mode> {
    modes.sort_by_key(|m| (m.width, m.height, m.refresh));
    modes.dedup_by(|a, b| a.width == b.width && a.height == b.height && a.refresh == b.refresh);
    modes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_modes_dedupes_and_sorts() {
        let modes = vec![
            Mode { width: 1920, height: 1080, refresh: 144 },
            Mode { width: 3840, height: 2160, refresh: 60 },
            Mode { width: 1920, height: 1080, refresh: 144 },
            Mode { width: 1920, height: 1080, refresh: 60 },
        ];
        assert_eq!(
            normalize_modes(modes),
            vec![
                Mode { width: 1920, height: 1080, refresh: 60 },
                Mode { width: 1920, height: 1080, refresh: 144 },
                Mode { width: 3840, height: 2160, refresh: 60 },
            ]
        );
    }

    #[test]
    fn normalize_modes_empty() {
        assert_eq!(normalize_modes(Vec::new()), Vec::new());
    }
}