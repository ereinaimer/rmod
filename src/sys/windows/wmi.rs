//! WMI-based brightness control through `root\wmi`.
//!
//! Drives the `WmiMonitorBrightnessMethods::WmiSetBrightness` provider, the
//! same one the Windows action-center brightness slider uses, so changes
//! stay in sync with the system UI. COM is called through hand-written
//! vtables with no external crates.
//!
//! The `WbemScripting.SWbemLocator` automation object is used rather than
//! the classic `IWbemLocator` interface: several Windows 11 builds no
//! longer register `CLSID_WbemLocator` (instantiating the replacement
//! "Actual*" locator classes directly yields a broken object), and some
//! machines reject `IWbemLocator::ConnectServer` outright with access
//! denied. The automation path goes through the same provider and works in
//! both cases.
//!
//! Instances are located by explicit object path rather than by querying:
//! on this Windows build the scripting `ExecQuery` result sets return null
//! items no matter how they are enumerated. The WMI `InstanceName` is just
//! the PnP device instance (`DISPLAY\<model>\<instance>` from the
//! `Enum\DISPLAY` registry key) plus the monitor ordinal, so the instance
//! path is constructed from the registry and fetched with `Get`.

use std::ffi::c_void;
use std::ptr;

use super::bindings::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
    RegQueryValueExW, encode_wide, wide_to_string,
};
use super::com::{
    CLSCTX_INPROC_SERVER, CLSID_WBEMSCRIPTING_LOCATOR, COINIT_MULTITHREADED, CoCreateInstance,
    CoInitializeEx, CoInitializeSecurity, CoUninitialize, EOAC_NONE, IID_IDISPATCH,
    RPC_C_AUTHN_LEVEL_PKT_PRIVACY, RPC_C_IMP_LEVEL_IMPERSONATE, S_FALSE, S_OK, SafeArray,
    SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData, VT_ARRAY,
    VT_I4, VT_UI1, VT_VARIANT, Variant, VariantClear, call, get_prop, put_prop, release,
    release_disp,
};

/// The WMI class exposing `WmiSetBrightness`.
const CLASS_METHODS: &str = "WmiMonitorBrightnessMethods";
/// The WMI class exposing the current brightness.
const CLASS_STATE: &str = "WmiMonitorBrightness";
/// The method that writes a brightness level (0-100).
const METHOD_SET: &str = "WmiSetBrightness";
/// The key property common to every `root\wmi` monitor instance.
const KEY_INSTANCE_NAME: &str = "InstanceName";
/// The property listing every brightness level the panel accepts.
const PROP_LEVEL: &str = "Level";

/// `HKEY_LOCAL_MACHINE`.
const HKEY_LOCAL_MACHINE: *mut c_void = 0x8000_0002usize as *mut c_void;
/// `KEY_READ` access rights for registry keys.
const KEY_READ: u32 = 0x20019;
/// `REG_SZ` registry value type.
const REG_SZ: u32 = 1;
/// The registry key holding every display device instance.
const KEY_DISPLAY: &str = "SYSTEM\\CurrentControlSet\\Enum\\DISPLAY";

/// Runs once per process: without `CoInitializeSecurity`, WMI's connection
/// fails with `ERROR_ACCESS_DENIED` because default COM security cannot
/// impersonate the caller.
static SECURITY_INIT: std::sync::Once = std::sync::Once::new();

fn init_security() {
    SECURITY_INIT.call_once(|| unsafe {
        CoInitializeSecurity(
            ptr::null_mut(),
            -1,
            ptr::null_mut(),
            ptr::null_mut(),
            RPC_C_AUTHN_LEVEL_PKT_PRIVACY,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            ptr::null_mut(),
            EOAC_NONE,
            ptr::null_mut(),
        );
    });
}

/// The immediate subkey names of a registry key, in enumeration order.
fn registry_keys(root: *mut c_void, subpath: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut key: *mut c_void = ptr::null_mut();
    let hr = unsafe { RegOpenKeyExW(root, encode_wide(subpath).as_ptr(), 0, KEY_READ, &mut key) };
    if hr != 0 || key.is_null() {
        return keys;
    }
    let mut index = 0u32;
    loop {
        let mut name = [0u16; 256];
        let mut len = 256u32;
        let hr = unsafe {
            RegEnumKeyExW(
                key,
                index,
                name.as_mut_ptr(),
                &mut len,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if hr != 0 {
            break;
        }
        keys.push(String::from_utf16_lossy(&name[..len as usize]));
        index += 1;
    }
    unsafe { RegCloseKey(key) };
    keys
}

/// The monitor-level device ID of the display addressed by `name` (e.g.
/// `MONITOR\LEN9059\{4d36e96e-...}\0002`), or `None`.
fn monitor_device_id(name: &str) -> Option<String> {
    let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let name_wide = encode_wide(name);
    let ok = unsafe { EnumDisplayDevicesW(name_wide.as_ptr(), 0, &mut monitor, 0) };
    if ok == 0 {
        return None;
    }
    let id = wide_to_string(&monitor.device_id);
    if id.is_empty() { None } else { Some(id) }
}

/// Splits a monitor device ID into its model and driver instance tail,
/// e.g. `MONITOR\LEN9059\{4d36e96e-...}\0002` -> `("LEN9059",
/// "{4d36e96e-...}\\0002")`.
fn monitor_parts(device_id: &str) -> Option<(String, String)> {
    let mut parts = device_id.split('\\');
    if parts.next()? != "MONITOR" {
        return None;
    }
    let model = parts.next()?;
    let driver = parts.collect::<Vec<_>>().join("\\");
    if driver.is_empty() {
        return None;
    }
    Some((model.to_string(), driver))
}

/// Reads a REG_SZ value as a string, or `None` when the value is missing
/// or not a string.
fn read_reg_string(root: *mut c_void, subpath: &str, value: &str) -> Option<String> {
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

/// The `DISPLAY\<model>\<instance>` device instances that belong to the
/// display addressed by `name`, ordered with the instance whose `Driver`
/// value matches the monitor device ID tail first (that is the monitor
/// attached to this display), then any remaining instance of the same
/// model. `None` when the display has no monitor device ID to match on.
fn display_device_instances(name: &str) -> Option<Vec<String>> {
    let monitor_id = monitor_device_id(name)?;
    let (model, driver) = monitor_parts(&monitor_id)?;
    let subpath = format!("{KEY_DISPLAY}\\{model}");
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    for instance in registry_keys(HKEY_LOCAL_MACHINE, &subpath) {
        let instance_name = format!("DISPLAY\\{model}\\{instance}");
        let instance_key = format!("{subpath}\\{instance}");
        if read_reg_string(HKEY_LOCAL_MACHINE, &instance_key, "Driver").as_deref()
            == Some(driver.as_str())
        {
            matched.push(instance_name);
        } else {
            rest.push(instance_name);
        }
    }
    matched.extend(rest);
    Some(matched)
}

/// The `Get` object path for a `root\wmi` monitor instance: the class name
/// plus the `InstanceName` key with its backslashes escaped.
fn instance_path(class: &str, instance_name: &str) -> String {
    format!(
        "{class}.{KEY_INSTANCE_NAME}=\"{}\"",
        instance_name.replace('\\', "\\\\")
    )
}

/// A live connection to `root\wmi` through the scripting locator.
struct Connection {
    services: *mut c_void,
    uninit: bool,
}

impl Connection {
    fn new() -> Result<Connection, String> {
        let hr = unsafe { CoInitializeEx(0, COINIT_MULTITHREADED) };
        if hr != S_OK && hr != S_FALSE {
            return Err(format!("cannot initialize COM: 0x{hr:08x}"));
        }
        let uninit = hr == S_OK;
        init_security();
        let mut locator: *mut c_void = ptr::null_mut();
        let hr = unsafe {
            CoCreateInstance(
                &CLSID_WBEMSCRIPTING_LOCATOR,
                0,
                CLSCTX_INPROC_SERVER,
                &IID_IDISPATCH,
                &mut locator,
            )
        };
        if hr != S_OK || locator.is_null() {
            if uninit {
                unsafe { CoUninitialize() };
            }
            return Err(format!("cannot create the WMI locator: 0x{hr:08x}"));
        }
        let mut args = [
            Variant::bstr("."),
            Variant::bstr("root\\wmi"),
            Variant::bstr(""),
            Variant::bstr(""),
            Variant::bstr(""),
            Variant::bstr(""),
            Variant::i4(0),
        ];
        let result = call(locator, "ConnectServer", &mut args);
        let services = match result {
            Ok(result) => result.object(),
            Err(e) => {
                unsafe { release(locator) };
                if uninit {
                    unsafe { CoUninitialize() };
                }
                return Err(format!("cannot connect to root\\wmi: {e}"));
            }
        };
        unsafe { release(locator) };
        match services {
            Some(services) => Ok(Connection { services, uninit }),
            None => {
                if uninit {
                    unsafe { CoUninitialize() };
                }
                Err("cannot connect to root\\wmi: no services returned".to_string())
            }
        }
    }

    /// Fetches an object by explicit path. The returned dispatch pointer is
    /// owned by the caller.
    fn get_object(&self, path: &str) -> Result<*mut c_void, String> {
        let mut args = [Variant::bstr(path)];
        let result = call(self.services, "Get", &mut args)?;
        result
            .object()
            .ok_or_else(|| format!("Get({path}) returned no object"))
    }

    /// Calls `WmiSetBrightness` on a methods instance with the given value.
    fn exec_method(&self, instance: *mut c_void, value: u32) -> Result<(), String> {
        let methods = get_prop(instance, "Methods_")?;
        let methods = methods.object().ok_or("Methods_ not found")?;

        let mut args = [Variant::bstr(METHOD_SET)];
        let method = call(methods, "Item", &mut args)?;
        let method = method.object().ok_or("WmiSetBrightness method not found")?;

        let in_params_class = get_prop(method, "InParameters")?;
        let in_params_class = in_params_class.object().ok_or("InParameters not found")?;

        let spawned = call(in_params_class, "SpawnInstance_", &mut [])?;
        let spawned = spawned.object().ok_or("SpawnInstance_ failed")?;

        let properties = get_prop(spawned, "Properties_")?;
        let properties = properties.object().ok_or("Properties_ not found")?;

        let mut args = [Variant::bstr("Brightness")];
        let brightness = call(properties, "Item", &mut args)?;
        let brightness = brightness.object().ok_or("Brightness property not found")?;
        put_prop(brightness, "Value", &Variant::ui1(value as u8))?;

        let mut args = [Variant::bstr("Timeout")];
        let timeout = call(properties, "Item", &mut args)?;
        let timeout = timeout.object().ok_or("Timeout property not found")?;
        put_prop(timeout, "Value", &Variant::ui4(1))?;

        let mut args = [Variant::bstr(METHOD_SET), Variant::dispatch(spawned)];
        let result = call(instance, "ExecMethod_", &mut args)?;
        let status = match result.object() {
            Some(out) => {
                let mut return_value = get_prop(out, "ReturnValue")?;
                let status = if return_value.vt == VT_I4 {
                    return_value.data[0] as u32 as i32
                } else {
                    0
                };
                unsafe { VariantClear(&mut return_value) };
                unsafe { release(out) };
                status
            }
            None => 0,
        };

        release_disp(timeout);
        release_disp(brightness);
        unsafe { release(properties) };
        unsafe { release(spawned) };
        release_disp(in_params_class);
        unsafe { release(method) };
        unsafe { release(methods) };

        if status != 0 {
            return Err(format!(
                "the system brightness change failed: 0x{status:08x}"
            ));
        }
        Ok(())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if !self.services.is_null() {
            unsafe { release(self.services) };
        }
        if self.uninit {
            unsafe { CoUninitialize() };
        }
    }
}

/// A live `root\wmi` connection plus the pre-enumerated device instances of
/// one display, so the WMI legs of a single command share one connection
/// and one registry enumeration.
pub(crate) struct Session {
    connection: Connection,
    devices: Vec<String>,
}

impl Session {
    /// Connects to `root\wmi` and enumerates the device instances of the
    /// display addressed by `name` (e.g. `\\.\DISPLAY1`). `Err` when the
    /// connection itself fails; `Ok(None)` when the display has no monitor
    /// device ID to match on.
    pub(crate) fn for_display(name: &str) -> Result<Option<Session>, String> {
        let connection = Connection::new()?;
        let Some(devices) = display_device_instances(name) else {
            return Ok(None);
        };
        Ok(Some(Session {
            connection,
            devices,
        }))
    }

    /// The instance of `class` for the display this session was created
    /// for, or `Ok(None)` when the display has no such WMI instance.
    ///
    /// The `InstanceName` is the PnP device instance from the registry plus
    /// the monitor ordinal; the correct ordinal is not known up front, so
    /// each candidate path is tried until one resolves.
    fn resolve_instance(&self, class: &str) -> Result<Option<*mut c_void>, String> {
        for device in &self.devices {
            for ordinal in 0..4 {
                let instance_name = format!("{device}_{ordinal}");
                let path = instance_path(class, &instance_name);
                if let Ok(object) = self.connection.get_object(&path) {
                    return Ok(Some(object));
                }
            }
        }
        Ok(None)
    }

    /// The current brightness of the display, or `None`.
    fn current(&self) -> Option<u32> {
        let instance = self.resolve_instance(CLASS_STATE).ok()??;
        let set = get_prop(instance, "Properties_").ok()?;
        let set = set.object()?;
        let props = call(set, "Item", &mut [Variant::bstr("CurrentBrightness")]).ok()?;
        let props = props.object()?;
        let mut value = get_prop(props, "Value").ok()?;
        let result = if value.vt == VT_UI1 {
            Some(value.data[0] as u8 as u32)
        } else {
            None
        };
        unsafe { VariantClear(&mut value) };
        unsafe { release(props) };
        unsafe { release(set) };
        unsafe { release(instance) };
        result
    }

    /// The smallest positive `Level` entry of the display, or `None` when
    /// the value is unreadable.
    pub(crate) fn min_level(&self) -> Option<u32> {
        let instance = match self.resolve_instance(CLASS_STATE) {
            Ok(Some(instance)) => instance,
            _ => return None,
        };
        let set = get_prop(instance, "Properties_").ok()?;
        let set = set.object()?;
        let levels = call(set, "Item", &mut [Variant::bstr(PROP_LEVEL)]).ok()?;
        let levels = levels.object()?;
        let mut value = get_prop(levels, "Value").ok()?;
        let result = if value.vt == VT_UI1 | VT_ARRAY {
            unsafe { min_positive_ui1(value.data[0] as *mut SafeArray) }
        } else if value.vt == VT_VARIANT | VT_ARRAY {
            unsafe { min_positive_variant(value.data[0] as *mut SafeArray) }
        } else {
            None
        };
        unsafe { VariantClear(&mut value) };
        unsafe { release(levels) };
        unsafe { release(set) };
        unsafe { release(instance) };
        result
    }

    /// Sets the brightness of the display through WMI.
    ///
    /// `Ok(None)` means the display has no WMI brightness instance.
    pub(crate) fn set(&self, value: u32) -> Result<Option<bool>, String> {
        let Some(instance) = self.resolve_instance(CLASS_METHODS)? else {
            return Ok(None);
        };
        let current = self.current();
        if current.is_some_and(|current| current == value) {
            unsafe { release(instance) };
            return Ok(Some(true));
        }
        let result = self.connection.exec_method(instance, value);
        unsafe { release(instance) };
        match result {
            Ok(()) => Ok(Some(false)),
            Err(e) => Err(e),
        }
    }
}

/// Sets the brightness of the display addressed by `name` through WMI.
///
/// `Ok(None)` means the display has no WMI brightness instance.
#[allow(dead_code)]
pub(crate) fn set(name: &str, value: u32) -> Result<Option<bool>, String> {
    let Some(session) = Session::for_display(name)? else {
        return Ok(None);
    };
    session.set(value)
}

/// The smallest positive `Level` entry of the display addressed by `name`,
/// or `None` when the value is unreadable.
///
/// The `Level` array lists every brightness level the panel accepts; the
/// smallest positive entry is the hardware floor `min` writes directly. A
/// zero-only array means the panel can go dark, so there is no floor.
#[allow(dead_code)]
pub(crate) fn min_level(name: &str) -> Option<u32> {
    Session::for_display(name).ok().flatten()?.min_level()
}

/// The smallest positive entry of a `Levels` array, or `None` when every
/// entry is zero (the panel can go dark, so there is no hardware floor).
fn smallest_positive(levels: &[u8]) -> Option<u32> {
    levels.iter().filter(|&&v| v > 0).min().map(|&v| v as u32)
}

/// Copies the payload of a 1-D `VT_UI1` SAFEARRAY, or `None` when the
/// array is not a readable one-dimensional byte array.
unsafe fn safe_array_ui1(psa: *mut SafeArray) -> Option<Vec<u8>> {
    unsafe {
        if psa.is_null() {
            return None;
        }
        let array = &*psa;
        if array.c_dims != 1 || array.cb_elements != 1 {
            return None;
        }
        let mut lower = 0i32;
        let mut upper = 0i32;
        let hr = SafeArrayGetLBound(psa, 1, &mut lower);
        if hr != S_OK {
            return None;
        }
        let hr = SafeArrayGetUBound(psa, 1, &mut upper);
        if hr != S_OK || upper < lower {
            return None;
        }
        let mut data: *mut c_void = ptr::null_mut();
        let hr = SafeArrayAccessData(psa, &mut data);
        if hr != S_OK || data.is_null() {
            return None;
        }
        let count = (upper - lower + 1) as usize;
        let mut levels = Vec::with_capacity(count);
        ptr::copy_nonoverlapping(data as *const u8, levels.as_mut_ptr(), count);
        levels.set_len(count);
        SafeArrayUnaccessData(psa);
        Some(levels)
    }
}

/// The smallest positive element of a 1-D `VT_UI1` SAFEARRAY, or `None`
/// when the array is unreadable or holds no positive element.
unsafe fn min_positive_ui1(psa: *mut SafeArray) -> Option<u32> {
    unsafe { safe_array_ui1(psa) }.and_then(|levels| smallest_positive(&levels))
}

/// The smallest positive element of a 1-D `VT_VARIANT|VT_ARRAY` SAFEARRAY
/// of `VT_UI1` variants (how `WmiMonitorBrightness.Level` arrives through
/// the scripting provider on some panels), or `None` when the array is
/// unreadable or holds no positive element.
unsafe fn min_positive_variant(psa: *mut SafeArray) -> Option<u32> {
    unsafe {
        if psa.is_null() {
            return None;
        }
        let array = &*psa;
        if array.c_dims != 1 || array.cb_elements != std::mem::size_of::<Variant>() as u32 {
            return None;
        }
        let mut lower = 0i32;
        let mut upper = 0i32;
        let hr = SafeArrayGetLBound(psa, 1, &mut lower);
        if hr != S_OK {
            return None;
        }
        let hr = SafeArrayGetUBound(psa, 1, &mut upper);
        if hr != S_OK || upper < lower {
            return None;
        }
        let mut data: *mut c_void = ptr::null_mut();
        let hr = SafeArrayAccessData(psa, &mut data);
        if hr != S_OK || data.is_null() {
            return None;
        }
        let count = (upper - lower + 1) as usize;
        let variants = std::slice::from_raw_parts(data as *const Variant, count);
        let mut levels = Vec::with_capacity(count);
        for variant in variants {
            if variant.vt == VT_UI1 {
                levels.push(variant.data[0] as u8);
            }
        }
        SafeArrayUnaccessData(psa);
        smallest_positive(&levels)
    }
}

#[cfg(test)]
mod tests {
    use super::super::com::SafeArrayBound;
    use super::*;

    /// A hand-built 1-D `VT_UI1` SAFEARRAY over `bytes`, allocated without
    /// oleaut32 so the extraction plumbing is testable in-process. Free with
    /// [`free_synthetic_array`].
    fn synthetic_array(bytes: &[u8]) -> *mut SafeArray {
        let data: Box<[u8]> = bytes.to_vec().into_boxed_slice();
        let array = Box::new(SafeArray {
            c_dims: 1,
            f_features: 0,
            cb_elements: 1,
            c_locks: 0,
            pv_data: Box::into_raw(data).cast(),
            rgsa_bounds: [SafeArrayBound {
                c_elements: bytes.len() as u32,
                l_bound: 0,
            }],
        });
        Box::into_raw(array)
    }

    /// Like [`synthetic_array`] but claiming two dimensions, for the
    /// wrong-dimension rejection test.
    fn synthetic_array_multi_dim(bytes: &[u8]) -> *mut SafeArray {
        let array = synthetic_array(bytes);
        unsafe { (*array).c_dims = 2 };
        array
    }

    /// A hand-built 1-D `VT_VARIANT|VT_ARRAY` SAFEARRAY of `VT_UI1`
    /// variants, mirroring how `WmiMonitorBrightness.Level` arrives
    /// through the scripting provider. Free with
    /// [`free_synthetic_variant_array`].
    fn synthetic_variant_array(values: &[u8]) -> *mut SafeArray {
        let data: Box<[Variant]> = values
            .iter()
            .map(|&v| Variant {
                vt: VT_UI1,
                w_reserved1: 0,
                w_reserved2: 0,
                w_reserved3: 0,
                data: [v as u64, 0],
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let array = Box::new(SafeArray {
            c_dims: 1,
            f_features: 0,
            cb_elements: std::mem::size_of::<Variant>() as u32,
            c_locks: 0,
            pv_data: Box::into_raw(data).cast(),
            rgsa_bounds: [SafeArrayBound {
                c_elements: values.len() as u32,
                l_bound: 0,
            }],
        });
        Box::into_raw(array)
    }

    /// Frees an array from [`synthetic_variant_array`].
    fn free_synthetic_variant_array(array: *mut SafeArray) {
        unsafe {
            let len = (*array).rgsa_bounds[0].c_elements as usize;
            let data = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                (*array).pv_data as *mut Variant,
                len,
            ));
            drop(data);
            drop(Box::from_raw(array));
        }
    }

    /// Frees an array from [`synthetic_array`] or
    /// [`synthetic_array_multi_dim`].
    fn free_synthetic_array(array: *mut SafeArray) {
        unsafe {
            let len = (*array).rgsa_bounds[0].c_elements as usize;
            let data = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                (*array).pv_data as *mut u8,
                len,
            ));
            drop(data);
            drop(Box::from_raw(array));
        }
    }

    #[test]
    fn brightness_constants_name_the_right_classes() {
        assert_eq!(CLASS_METHODS, "WmiMonitorBrightnessMethods");
        assert_eq!(CLASS_STATE, "WmiMonitorBrightness");
        assert_eq!(METHOD_SET, "WmiSetBrightness");
    }

    #[test]
    fn smallest_positive_picks_the_smallest_nonzero_entry() {
        assert_eq!(smallest_positive(&[3, 0, 7, 1]), Some(1));
        assert_eq!(smallest_positive(&[100, 255]), Some(100));
        assert_eq!(smallest_positive(&[1]), Some(1));
    }

    #[test]
    fn smallest_positive_empty_is_none() {
        assert_eq!(smallest_positive(&[]), None);
    }

    #[test]
    fn smallest_positive_all_zero_is_none() {
        assert_eq!(smallest_positive(&[0, 0, 0]), None);
    }

    #[test]
    fn safe_array_ui1_reads_the_payload() {
        let array = synthetic_array(&[3, 0, 7, 1]);
        let levels = unsafe { safe_array_ui1(array) };
        free_synthetic_array(array);
        assert_eq!(levels, Some(vec![3, 0, 7, 1]));
    }

    #[test]
    fn safe_array_ui1_null_is_none() {
        assert_eq!(unsafe { safe_array_ui1(ptr::null_mut()) }, None);
    }

    #[test]
    fn safe_array_ui1_wrong_dimension_is_none() {
        let array = synthetic_array_multi_dim(&[3, 0, 7]);
        let levels = unsafe { safe_array_ui1(array) };
        free_synthetic_array(array);
        assert_eq!(levels, None);
    }

    #[test]
    fn min_positive_ui1_returns_the_smallest_positive_element() {
        let array = synthetic_array(&[10, 0, 3, 8]);
        let floor = unsafe { min_positive_ui1(array) };
        free_synthetic_array(array);
        assert_eq!(floor, Some(3));
    }

    #[test]
    fn min_positive_ui1_zero_only_array_is_none() {
        let array = synthetic_array(&[0, 0]);
        let floor = unsafe { min_positive_ui1(array) };
        free_synthetic_array(array);
        assert_eq!(floor, None);
    }

    #[test]
    fn min_positive_ui1_empty_array_is_none() {
        let array = synthetic_array(&[]);
        let floor = unsafe { min_positive_ui1(array) };
        free_synthetic_array(array);
        assert_eq!(floor, None);
    }

    #[test]
    fn min_positive_variant_returns_the_smallest_positive_element() {
        let array = synthetic_variant_array(&[10, 0, 3, 8]);
        let floor = unsafe { min_positive_variant(array) };
        free_synthetic_variant_array(array);
        assert_eq!(floor, Some(3));
    }

    #[test]
    fn min_positive_variant_zero_only_array_is_none() {
        let array = synthetic_variant_array(&[0, 0]);
        let floor = unsafe { min_positive_variant(array) };
        free_synthetic_variant_array(array);
        assert_eq!(floor, None);
    }

    #[test]
    fn min_positive_variant_empty_array_is_none() {
        let array = synthetic_variant_array(&[]);
        let floor = unsafe { min_positive_variant(array) };
        free_synthetic_variant_array(array);
        assert_eq!(floor, None);
    }

    #[test]
    fn min_positive_variant_wrong_element_size_is_none() {
        let array = synthetic_variant_array(&[3, 7]);
        unsafe { (*array).cb_elements = 8 };
        let floor = unsafe { min_positive_variant(array) };
        free_synthetic_variant_array(array);
        assert_eq!(floor, None);
    }

    #[test]
    fn monitor_parts_splits_model_and_driver_tail() {
        let (model, driver) =
            monitor_parts("MONITOR\\LEN9059\\{4d36e96e-e325-11ce-bfc1-08002be10318}\\0002")
                .unwrap();
        assert_eq!(model, "LEN9059");
        assert_eq!(driver, "{4d36e96e-e325-11ce-bfc1-08002be10318}\\0002");
    }

    #[test]
    fn monitor_parts_rejects_non_monitor_ids() {
        assert_eq!(
            monitor_parts("PCI\\VEN_8086&DEV_9A60&SUBSYS_3E8C17AA&REV_01"),
            None
        );
    }

    #[test]
    fn monitor_parts_rejects_short_ids() {
        assert_eq!(monitor_parts("MONITOR\\LEN9059"), None);
    }

    #[test]
    fn instance_path_escapes_backslashes() {
        let path = instance_path(CLASS_METHODS, "DISPLAY\\LEN9059\\4&201f0991&1&UID8388688_0");
        assert_eq!(
            path,
            "WmiMonitorBrightnessMethods.InstanceName=\"DISPLAY\\\\LEN9059\\\\4&201f0991&1&UID8388688_0\""
        );
    }
}
