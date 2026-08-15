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

use super::bindings::encode_wide;

/// A Windows GUID (16 bytes, packed).
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const IID_NULL: Guid = Guid {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};
const IID_IDISPATCH: Guid = Guid {
    data1: 0x00020400,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

/// `CLSID_WbemScriptingLocator` — the `WbemScripting.SWbemLocator` object.
const CLSID_WBEMSCRIPTING_LOCATOR: Guid = Guid {
    data1: 0x76A64158,
    data2: 0xCB41,
    data3: 0x11D1,
    data4: [0x8B, 0x02, 0x00, 0x60, 0x08, 0x06, 0xD9, 0xB6],
};

const COINIT_MULTITHREADED: u32 = 0;
const CLSCTX_INPROC_SERVER: u32 = 0x3;
const S_OK: i32 = 0;
const S_FALSE: i32 = 1;

/// `RPC_C_AUTHN_LEVEL_PKT_PRIVACY` for `CoInitializeSecurity`.
const RPC_C_AUTHN_LEVEL_PKT_PRIVACY: u32 = 6;
/// `RPC_C_IMP_LEVEL_IMPERSONATE`, needed so WMI calls can impersonate the
/// caller.
const RPC_C_IMP_LEVEL_IMPERSONATE: u32 = 3;
/// `EOAC_NONE`.
const EOAC_NONE: u32 = 0;

const DISPATCH_METHOD: u16 = 0x1;
const DISPATCH_PROPERTYGET: u16 = 0x2;
const DISPATCH_PROPERTYPUT: u16 = 0x4;
const DISPID_PROPERTYPUT: i32 = -3;
const DISP_E_EXCEPTION: i32 = 0x80020009u32 as i32;

/// `VT_EMPTY`.
const VT_EMPTY: u16 = 0;
/// `VT_I4`.
const VT_I4: u16 = 3;
/// `VT_BSTR`.
const VT_BSTR: u16 = 8;
/// `VT_DISPATCH`.
const VT_DISPATCH: u16 = 9;
/// `VT_UNKNOWN`.
const VT_UNKNOWN: u16 = 0x0D;
/// `VT_UI1`.
const VT_UI1: u16 = 0x11;
/// `VT_UI4`.
const VT_UI4: u16 = 0x13;

/// The WMI class exposing `WmiSetBrightness`.
const CLASS_METHODS: &str = "WmiMonitorBrightnessMethods";
/// The WMI class exposing the current brightness.
const CLASS_STATE: &str = "WmiMonitorBrightness";
/// The method that writes a brightness level (0-100).
const METHOD_SET: &str = "WmiSetBrightness";
/// The key property common to every `root\wmi` monitor instance.
const KEY_INSTANCE_NAME: &str = "InstanceName";

/// `HKEY_LOCAL_MACHINE`.
const HKEY_LOCAL_MACHINE: *mut c_void = 0x8000_0002usize as *mut c_void;
/// `KEY_READ` access rights for registry keys.
const KEY_READ: u32 = 0x20019;
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

/// Minimal `VARIANT` covering the scalar and object types used here.
///
/// On x64 a `VARIANT` is `vt` + 3 reserved words + a 16-byte union, so the
/// data payload must be 16 bytes to match what COM reads and writes.
#[repr(C)]
#[derive(Clone, Copy)]
struct Variant {
    vt: u16,
    w_reserved1: u16,
    w_reserved2: u16,
    w_reserved3: u16,
    data: [u64; 2],
}

impl Variant {
    fn empty() -> Variant {
        Variant {
            vt: VT_EMPTY,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [0, 0],
        }
    }

    fn bstr(s: &str) -> Variant {
        Variant {
            vt: VT_BSTR,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [sys_alloc(&encode_wide(s)) as u64, 0],
        }
    }

    fn i4(value: i32) -> Variant {
        Variant {
            vt: VT_I4,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u32 as u64, 0],
        }
    }

    fn ui1(value: u8) -> Variant {
        Variant {
            vt: VT_UI1,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u64, 0],
        }
    }

    fn ui4(value: u32) -> Variant {
        Variant {
            vt: VT_UI4,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u64, 0],
        }
    }

    fn dispatch(object: *mut c_void) -> Variant {
        Variant {
            vt: VT_DISPATCH,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [object as u64, 0],
        }
    }

    /// The object pointer when this is a `VT_DISPATCH` or `VT_UNKNOWN`,
    /// else `None`.
    fn object(&self) -> Option<*mut c_void> {
        if (self.vt == VT_DISPATCH || self.vt == VT_UNKNOWN) && self.data[0] != 0 {
            Some(self.data[0] as *mut c_void)
        } else {
            None
        }
    }
}

/// `DISPPARAMS` for `IDispatch::Invoke`.
#[repr(C)]
struct DispParams {
    rgvarg: *mut Variant,
    rgdispid_named_args: *mut i32,
    c_args: u32,
    c_named_args: u32,
}

/// `EXCEPINFO` for `IDispatch::Invoke`; only `scode` is consulted.
#[repr(C)]
struct ExcepInfo {
    w_code: u16,
    w_reserved: u16,
    bstr_source: *mut u16,
    bstr_description: *mut u16,
    bstr_help_file: *mut u16,
    dw_help_context: u32,
    pv_reserved: *mut c_void,
    hr_error: i32,
    scode: i32,
}

impl ExcepInfo {
    fn new() -> ExcepInfo {
        ExcepInfo {
            w_code: 0,
            w_reserved: 0,
            bstr_source: ptr::null_mut(),
            bstr_description: ptr::null_mut(),
            bstr_help_file: ptr::null_mut(),
            dw_help_context: 0,
            pv_reserved: ptr::null_mut(),
            hr_error: 0,
            scode: 0,
        }
    }
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pv_reserved: usize, dw_co_init: u32) -> i32;
    fn CoUninitialize();
    fn CoInitializeSecurity(
        p_void: *mut c_void,
        c_auth_svc: i32,
        as_auth_svc: *mut c_void,
        p_reserved1: *mut c_void,
        dw_authn_level: u32,
        dw_imp_level: u32,
        p_auth_list: *mut c_void,
        dw_capabilities: u32,
        p_reserved3: *mut c_void,
    ) -> i32;
    fn CoCreateInstance(
        rclsid: *const Guid,
        p_unk_outer: usize,
        dw_cls_context: u32,
        riid: *const Guid,
        ppv: *mut *mut c_void,
    ) -> i32;
}
#[link(name = "oleaut32")]
unsafe extern "system" {
    fn SysAllocString(psz: *const u16) -> *mut u16;
    fn VariantClear(pvar: *mut Variant) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(
        h_key: *mut c_void,
        lp_sub_key: *const u16,
        ul_options: u32,
        sam_desired: u32,
        phk_result: *mut *mut c_void,
    ) -> i32;
    fn RegEnumKeyExW(
        h_key: *mut c_void,
        dw_index: u32,
        lp_name: *mut u16,
        lpcch_name: *mut u32,
        lp_reserved: *mut u32,
        lp_class: *mut u16,
        lpcch_class: *mut u32,
        lpft_last_write_time: *mut c_void,
    ) -> i32;
    fn RegCloseKey(h_key: *mut c_void) -> i32;
}

/// The `IDispatch` vtable, laid out as a `repr(C)` struct of function
/// pointers so method slots are reached by field access rather than by
/// transmuting a pointer into a function pointer (which the optimizer can
/// treat as undefined behavior and miscompile).
#[repr(C)]
struct IDispatchVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut c_void,
        *const Guid,
        *mut *mut u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *const Guid,
        u32,
        u16,
        *const DispParams,
        *mut Variant,
        *mut ExcepInfo,
        *mut u32,
    ) -> i32,
}

/// The `IDispatch` vtable of a COM object. Every COM interface pointer
/// points to an object whose first member is the vtable pointer.
unsafe fn idispatch_vtbl(com: *const c_void) -> &'static IDispatchVtbl {
    unsafe { &**(com as *const *const IDispatchVtbl) }
}

/// Releases a COM interface through its vtable `Release` slot.
unsafe fn release(com: *mut c_void) {
    unsafe {
        let vtbl = idispatch_vtbl(com);
        (vtbl.release)(com);
    }
}

/// Releases a dispatch object, if non-null.
fn release_disp(object: *mut c_void) {
    if !object.is_null() {
        unsafe { release(object) };
    }
}

/// Allocates a `BSTR` copy of a null-terminated wide string.
fn sys_alloc(wide: &[u16]) -> *mut u16 {
    unsafe { SysAllocString(wide.as_ptr()) }
}

/// Runs `f` with a null-terminated wide copy of `s`.
fn with_wide<R>(s: &str, f: impl FnOnce(*const u16) -> R) -> R {
    f(encode_wide(s).as_ptr())
}

/// Looks up a member's `DISPID` on an automation object.
fn dispatch_id(disp: *mut c_void, name: &str) -> Result<i32, String> {
    fn dispatch_id_inner(disp: *mut c_void, name: &str) -> Result<i32, String> {
        let vtbl = unsafe { idispatch_vtbl(disp) };
        let mut id = 0i32;
        let hr = with_wide(name, |wide| {
            let mut names = [wide as *mut u16];
            unsafe { (vtbl.get_ids_of_names)(disp, &IID_NULL, names.as_mut_ptr(), 1, 0, &mut id) }
        });
        if hr != S_OK {
            return Err(format!("member \"{name}\" not found: 0x{hr:08x}"));
        }
        Ok(id)
    }
    dispatch_id_inner(disp, name)
}

/// Runs `IDispatch::Invoke` and returns the result variant.
fn dispatch_invoke(
    disp: *mut c_void,
    member: i32,
    name: &str,
    flags: u16,
    args: &mut [Variant],
    named: &[(i32, usize)],
) -> Result<Variant, String> {
    fn dispatch_invoke_inner(
        disp: *mut c_void,
        member: i32,
        name: &str,
        flags: u16,
        args: &mut [Variant],
        named: &[(i32, usize)],
    ) -> Result<Variant, String> {
        let vtbl = unsafe { idispatch_vtbl(disp) };

        let mut reversed = args.to_vec();
        reversed.reverse();
        let mut named_disps = vec![0i32; args.len()];
        for (dispid, forward_index) in named {
            named_disps[args.len() - 1 - forward_index] = *dispid;
        }
        let params = DispParams {
            rgvarg: reversed.as_mut_ptr(),
            rgdispid_named_args: if named.is_empty() {
                ptr::null_mut()
            } else {
                named_disps.as_mut_ptr()
            },
            c_args: args.len() as u32,
            c_named_args: named.len() as u32,
        };
        let mut result = Variant::empty();
        let mut excep = ExcepInfo::new();
        // Pin the `DISPPARAMS`, result, and `EXCEPINFO` in real memory before
        // the call. Under `opt-level = "z"` with whole-program LTO, LLVM can
        // otherwise keep these values in registers and emit an `Invoke` that
        // reaches oleaut32 with a valid HRESULT but no marshaled arguments or
        // result (`ConnectServer` then returns VT_EMPTY or the stub reports
        // bad data, 0x800706F7). The reads force the values to be spilled, so
        // the pointers `Invoke` receives point at real, initialized memory.
        std::hint::black_box(&params);
        std::hint::black_box(&mut result);
        std::hint::black_box(&mut excep);
        let hr = unsafe {
            (vtbl.invoke)(
                disp,
                member,
                &IID_NULL,
                0,
                flags,
                &params,
                &mut result,
                &mut excep,
                ptr::null_mut(),
            )
        };
        for arg in args {
            if arg.vt == VT_BSTR {
                unsafe { VariantClear(arg) };
            }
        }
        if hr != S_OK {
            if hr == DISP_E_EXCEPTION && excep.scode != 0 {
                return Err(format!("{name}: 0x{:08x}", excep.scode));
            }
            return Err(format!("{name}: 0x{hr:08x}"));
        }
        Ok(result)
    }
    dispatch_invoke_inner(disp, member, name, flags, args, named)
}

/// Calls a method by name with positional args.
fn call(disp: *mut c_void, name: &str, args: &mut [Variant]) -> Result<Variant, String> {
    let member = dispatch_id(disp, name)?;
    dispatch_invoke(disp, member, name, DISPATCH_METHOD, args, &[])
}

/// Reads a property by name.
fn get_prop(disp: *mut c_void, name: &str) -> Result<Variant, String> {
    let member = dispatch_id(disp, name)?;
    dispatch_invoke(disp, member, name, DISPATCH_PROPERTYGET, &mut [], &[])
}

/// Writes a property by name (single named argument).
fn put_prop(disp: *mut c_void, name: &str, value: &Variant) -> Result<(), String> {
    let member = dispatch_id(disp, name)?;
    let mut args = [*value];
    dispatch_invoke(disp, member, name, DISPATCH_PROPERTYPUT, &mut args, &[(DISPID_PROPERTYPUT, 0)])
        .map(|_| ())
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

/// Every display PnP device instance as `DISPLAY\<model>\<instance>`, from
/// the `Enum\DISPLAY` registry key.
fn device_instances() -> Vec<String> {
    let mut instances = Vec::new();
    for model in registry_keys(HKEY_LOCAL_MACHINE, KEY_DISPLAY) {
        let subpath = format!("{KEY_DISPLAY}\\{model}");
        for instance in registry_keys(HKEY_LOCAL_MACHINE, &subpath) {
            instances.push(format!("DISPLAY\\{model}\\{instance}"));
        }
    }
    instances
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
        result.object().ok_or_else(|| format!("Get({path}) returned no object"))
    }

    /// The instance of `class` for the `index`-th (0-based) display, or
    /// `Ok(None)` when the display has no such WMI instance.
    ///
    /// The `InstanceName` is the PnP device instance from the registry plus
    /// the monitor ordinal; the correct ordinal is not known up front, so
    /// each candidate path is tried until one resolves.
    fn resolve_instance(&self, class: &str, index: usize) -> Result<Option<*mut c_void>, String> {
        let devices = device_instances();
        let Some(device) = devices.get(index) else {
            return Ok(None);
        };
        for ordinal in 0..4 {
            let instance_name = format!("{device}_{ordinal}");
            let path = instance_path(class, &instance_name);
            if let Ok(object) = self.get_object(&path) {
                return Ok(Some(object));
            }
        }
        Ok(None)
    }

    /// The current brightness of the `index`-th (0-based) display, or `None`.
    fn current(&self, index: usize) -> Option<u32> {
        let instance = self.resolve_instance(CLASS_STATE, index).ok()??;
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

    /// Sets the brightness of the `index`-th (0-based) display.
    ///
    /// `Ok(None)` means the display has no WMI brightness instance.
    fn set_brightness(&self, index: usize, value: u32) -> Result<Option<bool>, String> {
        let Some(instance) = self.resolve_instance(CLASS_METHODS, index)? else {
            return Ok(None);
        };
        let current = self.current(index);
        if current.is_some_and(|current| current == value) {
            unsafe { release(instance) };
            return Ok(Some(true));
        }
        let result = self.exec_method(instance, value);
        unsafe { release(instance) };
        match result {
            Ok(()) => Ok(Some(false)),
            Err(e) => Err(e),
        }
    }

    /// Calls `WmiSetBrightness` on a methods instance with the given value.
    fn exec_method(&self, instance: *mut c_void, value: u32) -> Result<(), String> {
        let methods = get_prop(instance, "Methods_")?;
        let methods = methods.object().ok_or("Methods_ not found")?;

        let mut args = [Variant::bstr(METHOD_SET)];
        let method = call(methods, "Item", &mut args)?;
        let method = method.object().ok_or("WmiSetBrightness method not found")?;

        let in_params_class = get_prop(method, "InParameters")?;
        let in_params_class = in_params_class
            .object()
            .ok_or("InParameters not found")?;

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
            return Err(format!("the system brightness change failed: 0x{status:08x}"));
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

/// Sets the brightness of the `index`-th (0-based) display through WMI.
///
/// `Ok(None)` means the display has no WMI brightness instance.
pub(crate) fn set(index: usize, value: u32) -> Result<Option<bool>, String> {
    let connection = Connection::new()?;
    connection.set_brightness(index, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_ui1_encodes_vt_and_value() {
        let variant = Variant::ui1(42);
        assert_eq!(variant.vt, VT_UI1);
        assert_eq!(variant.data[0], 42);
    }

    #[test]
    fn variant_ui4_encodes_vt_and_value() {
        let variant = Variant::ui4(1);
        assert_eq!(variant.vt, VT_UI4);
        assert_eq!(variant.data[0], 1);
    }

    #[test]
    fn variant_matches_x64_variant_size() {
        assert_eq!(std::mem::size_of::<Variant>(), 24);
    }

    #[test]
    fn brightness_constants_name_the_right_classes() {
        assert_eq!(CLASS_METHODS, "WmiMonitorBrightnessMethods");
        assert_eq!(CLASS_STATE, "WmiMonitorBrightness");
        assert_eq!(METHOD_SET, "WmiSetBrightness");
    }

    #[test]
    fn instance_path_escapes_backslashes() {
        let path = instance_path(CLASS_METHODS, "DISPLAY\\LEN9059\\4&201f0991&1&UID8388688_0");
        assert_eq!(
            path,
            "WmiMonitorBrightnessMethods.InstanceName=\"DISPLAY\\\\LEN9059\\\\4&201f0991&1&UID8388688_0\""
        );
    }

    #[test]
    #[ignore]
    fn diagnose_connect_only() {
        eprintln!("[diag] attempting Connection::new...");
        match Connection::new() {
            Ok(_) => eprintln!("[diag] Connection::new OK"),
            Err(e) => eprintln!("[diag] Connection::new FAILED: {e}"),
        }
        eprintln!("[diag] attempting full pipeline (no-op slider @100)...");
        match super::super::brightness::set_brightness(
            None,
            100,
            Some(super::super::brightness::BrightnessBackend::Slider),
        ) {
            Ok(o) => eprintln!("[diag] pipeline OK: unchanged={}", o.unchanged),
            Err(e) => eprintln!("[diag] pipeline FAILED: {e}"),
        }
    }
}