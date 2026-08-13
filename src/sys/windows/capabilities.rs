//! Supported-mode enumeration and normalization.
//!
//! Walks the mode list Windows exposes for a device and produces the
//! deduplicated, ascending-ordered list shown by the caps command.

use super::bindings::{encode_wide, DEVMODEW, EnumDisplaySettingsW};

/// A resolution and refresh-rate combination a display supports.
#[derive(Debug, PartialEq)]
pub struct Mode {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh: u32,
}

/// Enumerates every mode a device reports, unsorted and undeduplicated.
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

/// Sorts ascending by width, height, refresh and removes exact duplicates.
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