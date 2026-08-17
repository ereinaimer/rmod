//! Shared registry access: enumerate subkey names and read REG_BINARY /
//! REG_SZ values. Used by [`super::query`] for the monitor EDID lookup and
//! by [`super::wmi`] for the display-device instance lookup.
//!
//! The [`enum_subkeys`]/[`read_reg_binary`] pair takes a raw `isize` hive
//! (matching [`super::bindings::HKEY_LOCAL_MACHINE`]); the
//! [`registry_keys`]/[`read_reg_string`] pair takes the `*mut c_void`
//! handles the WMI path works with.

use std::ffi::c_void;
use std::ptr;

use super::bindings::{
    ERROR_SUCCESS, KEY_READ, REG_BINARY, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
    RegQueryValueExW, encode_wide,
};

/// Enumerates the sub-key names of a registry key.
pub(crate) fn enum_subkeys(hive: isize, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let path_wide = encode_wide(path);
        let mut key: *mut c_void = ptr::null_mut();
        if RegOpenKeyExW(
            hive as *mut c_void,
            path_wide.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return out;
        }
        let mut index: u32 = 0;
        loop {
            let mut name_buf = [0u16; 260];
            let mut name_len: u32 = name_buf.len() as u32;
            let hr = RegEnumKeyExW(
                key,
                index,
                name_buf.as_mut_ptr(),
                &mut name_len,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if hr != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&name_buf[..name_len as usize]));
            index += 1;
        }
        RegCloseKey(key);
    }
    out
}

/// The immediate subkey names of a registry key, in enumeration order.
pub(crate) fn registry_keys(root: *mut c_void, subpath: &str) -> Vec<String> {
    enum_subkeys(root as isize, subpath)
}

/// Reads a REG_BINARY value as raw bytes.
pub(crate) fn read_reg_binary(hive: isize, path: &str, value: &str) -> Option<Vec<u8>> {
    unsafe {
        let path_wide = encode_wide(path);
        let value_wide = encode_wide(value);
        let mut key: *mut c_void = ptr::null_mut();
        if RegOpenKeyExW(
            hive as *mut c_void,
            path_wide.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut size: u32 = 0;
        let mut ty: u32 = 0;
        let hr = RegQueryValueExW(
            key,
            value_wide.as_ptr(),
            ptr::null_mut(),
            &mut ty,
            ptr::null_mut(),
            &mut size,
        );
        let data = if hr == ERROR_SUCCESS && ty == REG_BINARY && size > 0 {
            let mut buf = vec![0u8; size as usize];
            let hr = RegQueryValueExW(
                key,
                value_wide.as_ptr(),
                ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            );
            if hr == ERROR_SUCCESS {
                buf.truncate(size as usize);
                Some(buf)
            } else {
                None
            }
        } else {
            None
        };
        RegCloseKey(key);
        data
    }
}

/// Reads a REG_SZ value as a string, or `None` when the value is missing
/// or not a string.
pub(crate) fn read_reg_string(root: *mut c_void, subpath: &str, value: &str) -> Option<String> {
    let mut key: *mut c_void = ptr::null_mut();
    let hr = unsafe { RegOpenKeyExW(root, encode_wide(subpath).as_ptr(), 0, KEY_READ, &mut key) };
    if hr != 0 || key.is_null() {
        return None;
    }
    let mut ty: u32 = 0;
    let mut size: u32 = 0;
    let hr = unsafe {
        RegQueryValueExW(
            key,
            encode_wide(value).as_ptr(),
            ptr::null_mut(),
            &mut ty,
            ptr::null_mut(),
            &mut size,
        )
    };
    let mut out = None;
    if hr == 0 && ty == REG_SZ && size > 0 {
        let mut buf = vec![0u16; (size / 2) as usize];
        let mut ty2: u32 = 0;
        let mut size2 = size;
        let hr = unsafe {
            RegQueryValueExW(
                key,
                encode_wide(value).as_ptr(),
                ptr::null_mut(),
                &mut ty2,
                buf.as_mut_ptr() as *mut u8,
                &mut size2,
            )
        };
        if hr == 0 && ty2 == REG_SZ {
            out = Some(
                String::from_utf16_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_string(),
            );
        }
    }
    unsafe { RegCloseKey(key) };
    out
}
