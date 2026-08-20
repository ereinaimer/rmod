//! COM/ole32/oleaut32 FFI shared by the WMI backend: minimal `VARIANT`,
//! `SAFEARRAY`, `DISPPARAMS`, and `EXCEPINFO` types, the `IDispatch` vtable,
//! and the `call`/`get_prop`/`put_prop` dispatch helpers.

use std::ffi::c_void;
use std::ptr;

use super::bindings::encode_wide;

/// A Windows GUID (16 bytes, packed).
#[repr(C)]
pub(crate) struct Guid {
    pub(crate) data1: u32,
    pub(crate) data2: u16,
    pub(crate) data3: u16,
    pub(crate) data4: [u8; 8],
}

pub(crate) const IID_NULL: Guid = Guid {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};
pub(crate) const IID_IDISPATCH: Guid = Guid {
    data1: 0x00020400,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

/// `CLSID_WbemScriptingLocator` — the `WbemScripting.SWbemLocator` object.
pub(crate) const CLSID_WBEMSCRIPTING_LOCATOR: Guid = Guid {
    data1: 0x76A64158,
    data2: 0xCB41,
    data3: 0x11D1,
    data4: [0x8B, 0x02, 0x00, 0x60, 0x08, 0x06, 0xD9, 0xB6],
};

pub(crate) const COINIT_MULTITHREADED: u32 = 0;
pub(crate) const CLSCTX_INPROC_SERVER: u32 = 0x3;
pub(crate) const S_OK: i32 = 0;
pub(crate) const S_FALSE: i32 = 1;

/// `RPC_C_AUTHN_LEVEL_PKT_PRIVACY` for `CoInitializeSecurity`.
pub(crate) const RPC_C_AUTHN_LEVEL_PKT_PRIVACY: u32 = 6;
/// `RPC_C_IMP_LEVEL_IMPERSONATE`, needed so WMI calls can impersonate the
/// caller.
pub(crate) const RPC_C_IMP_LEVEL_IMPERSONATE: u32 = 3;
/// `EOAC_NONE`.
pub(crate) const EOAC_NONE: u32 = 0;

pub(crate) const DISPATCH_METHOD: u16 = 0x1;
pub(crate) const DISPATCH_PROPERTYGET: u16 = 0x2;
pub(crate) const DISPATCH_PROPERTYPUT: u16 = 0x4;
pub(crate) const DISPID_PROPERTYPUT: i32 = -3;
pub(crate) const DISP_E_EXCEPTION: i32 = 0x80020009u32 as i32;

/// `VT_EMPTY`.
pub(crate) const VT_EMPTY: u16 = 0;
/// `VT_I4`.
pub(crate) const VT_I4: u16 = 3;
/// `VT_BSTR`.
pub(crate) const VT_BSTR: u16 = 8;
/// `VT_DISPATCH`.
pub(crate) const VT_DISPATCH: u16 = 9;
/// `VT_UNKNOWN`.
pub(crate) const VT_UNKNOWN: u16 = 0x0D;
/// `VT_UI1`.
pub(crate) const VT_UI1: u16 = 0x11;
/// `VT_UI4`.
pub(crate) const VT_UI4: u16 = 0x13;
/// `VT_VARIANT`: a `VARIANT` value (used for arrays of variants).
pub(crate) const VT_VARIANT: u16 = 0x0C;
/// `VT_ARRAY`; combined with an element type (e.g. `VT_UI1 | VT_ARRAY`).
pub(crate) const VT_ARRAY: u16 = 0x2000;

/// Minimal `VARIANT` covering the scalar and object types used here.
///
/// On x64 a `VARIANT` is `vt` + 3 reserved words + a 16-byte union, so the
/// data payload must be 16 bytes to match what COM reads and writes.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Variant {
    pub(crate) vt: u16,
    pub(crate) w_reserved1: u16,
    pub(crate) w_reserved2: u16,
    pub(crate) w_reserved3: u16,
    pub(crate) data: [u64; 2],
}

impl Variant {
    pub(crate) fn empty() -> Variant {
        Variant {
            vt: VT_EMPTY,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [0, 0],
        }
    }

    pub(crate) fn bstr(s: &str) -> Variant {
        Variant {
            vt: VT_BSTR,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [sys_alloc(&encode_wide(s)) as u64, 0],
        }
    }

    pub(crate) fn i4(value: i32) -> Variant {
        Variant {
            vt: VT_I4,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u32 as u64, 0],
        }
    }

    pub(crate) fn ui1(value: u8) -> Variant {
        Variant {
            vt: VT_UI1,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u64, 0],
        }
    }

    pub(crate) fn ui4(value: u32) -> Variant {
        Variant {
            vt: VT_UI4,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            data: [value as u64, 0],
        }
    }

    pub(crate) fn dispatch(object: *mut c_void) -> Variant {
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
    pub(crate) fn object(&self) -> Option<*mut c_void> {
        if (self.vt == VT_DISPATCH || self.vt == VT_UNKNOWN) && self.data[0] != 0 {
            Some(self.data[0] as *mut c_void)
        } else {
            None
        }
    }
}

/// A bound of a `SAFEARRAY`: the element count and inclusive lower bound.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct SafeArrayBound {
    pub(crate) c_elements: u32,
    pub(crate) l_bound: i32,
}

/// The `SAFEARRAY` header; only the fields needed to read a 1-D byte array
/// are used, and the trailing bound array is fixed at one entry.
#[repr(C)]
pub(crate) struct SafeArray {
    pub(crate) c_dims: u16,
    pub(crate) f_features: u16,
    pub(crate) cb_elements: u32,
    pub(crate) c_locks: u32,
    pub(crate) pv_data: *mut c_void,
    pub(crate) rgsa_bounds: [SafeArrayBound; 1],
}

/// `DISPPARAMS` for `IDispatch::Invoke`.
#[repr(C)]
pub(crate) struct DispParams {
    pub(crate) rgvarg: *mut Variant,
    pub(crate) rgdispid_named_args: *mut i32,
    pub(crate) c_args: u32,
    pub(crate) c_named_args: u32,
}

/// `EXCEPINFO` for `IDispatch::Invoke`; only `scode` is consulted.
#[repr(C)]
pub(crate) struct ExcepInfo {
    pub(crate) w_code: u16,
    pub(crate) w_reserved: u16,
    pub(crate) bstr_source: *mut u16,
    pub(crate) bstr_description: *mut u16,
    pub(crate) bstr_help_file: *mut u16,
    pub(crate) dw_help_context: u32,
    pub(crate) pv_reserved: *mut c_void,
    pub(crate) hr_error: i32,
    pub(crate) scode: i32,
}

impl ExcepInfo {
    pub(crate) fn new() -> ExcepInfo {
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
    pub(crate) fn CoInitializeEx(pv_reserved: usize, dw_co_init: u32) -> i32;
    pub(crate) fn CoUninitialize();
    pub(crate) fn CoInitializeSecurity(
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
    pub(crate) fn CoCreateInstance(
        rclsid: *const Guid,
        p_unk_outer: usize,
        dw_cls_context: u32,
        riid: *const Guid,
        ppv: *mut *mut c_void,
    ) -> i32;
}
#[link(name = "oleaut32")]
unsafe extern "system" {
    pub(crate) fn SysAllocString(psz: *const u16) -> *mut u16;
    pub(crate) fn VariantClear(pvar: *mut Variant) -> i32;
    pub(crate) fn SafeArrayGetLBound(psa: *mut SafeArray, n_dim: u32, pl_lbound: *mut i32) -> i32;
    pub(crate) fn SafeArrayGetUBound(psa: *mut SafeArray, n_dim: u32, pl_ubound: *mut i32) -> i32;
    pub(crate) fn SafeArrayAccessData(psa: *mut SafeArray, ppv_data: *mut *mut c_void) -> i32;
    pub(crate) fn SafeArrayUnaccessData(psa: *mut SafeArray) -> i32;
}

/// The `IDispatch` vtable, laid out as a `repr(C)` struct of function
/// pointers so method slots are reached by field access rather than by
/// transmuting a pointer into a function pointer (which the optimizer can
/// treat as undefined behavior and miscompile).
#[repr(C)]
pub(crate) struct IDispatchVtbl {
    pub(crate) query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    pub(crate) add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    pub(crate) release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub(crate) get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32,
    pub(crate) get_type_info:
        unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut c_void) -> i32,
    pub(crate) get_ids_of_names: unsafe extern "system" fn(
        *mut c_void,
        *const Guid,
        *mut *mut u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    pub(crate) invoke: unsafe extern "system" fn(
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
pub(crate) unsafe fn idispatch_vtbl(com: *const c_void) -> &'static IDispatchVtbl {
    unsafe { &**(com as *const *const IDispatchVtbl) }
}

/// Releases a COM interface through its vtable `Release` slot.
pub(crate) unsafe fn release(com: *mut c_void) {
    unsafe {
        let vtbl = idispatch_vtbl(com);
        (vtbl.release)(com);
    }
}

/// Releases a dispatch object, if non-null.
pub(crate) fn release_disp(object: *mut c_void) {
    if !object.is_null() {
        unsafe { release(object) };
    }
}

/// Allocates a `BSTR` copy of a null-terminated wide string.
pub(crate) fn sys_alloc(wide: &[u16]) -> *mut u16 {
    unsafe { SysAllocString(wide.as_ptr()) }
}

/// Looks up a member's `DISPID` on an automation object.
pub(crate) fn dispatch_id(disp: *mut c_void, name: &str) -> Result<i32, String> {
    let vtbl = unsafe { idispatch_vtbl(disp) };
    let mut id = 0i32;
    let wide = encode_wide(name);
    let mut names = [wide.as_ptr() as *mut u16];
    let hr = unsafe { (vtbl.get_ids_of_names)(disp, &IID_NULL, names.as_mut_ptr(), 1, 0, &mut id) };
    if hr != S_OK {
        return Err(format!("member \"{name}\" not found: 0x{hr:08x}"));
    }
    Ok(id)
}

/// The `DISPPARAMS` layout for a call: the args reversed into a fixed
/// stack array (COM wants the last argument at index 0 of `rgvarg`) with
/// the named dispid slots aligned to their reversed positions. The base
/// index is where `rgvarg` starts; `c_args` limits what COM reads.
pub(crate) fn reversed_args(
    args: &[Variant],
    named: &[(i32, usize)],
) -> ([Variant; 7], [i32; 7], usize) {
    // The largest call is `ConnectServer` with seven args, so the fixed
    // arrays always cover the argument list.
    const MAX_ARGS: usize = 7;
    debug_assert!(args.len() <= MAX_ARGS, "more than {MAX_ARGS} arguments");
    let mut reversed = [Variant::empty(); MAX_ARGS];
    for (i, arg) in args.iter().enumerate() {
        reversed[MAX_ARGS - 1 - i] = *arg;
    }
    let mut named_disps = [0i32; MAX_ARGS];
    for (dispid, forward_index) in named {
        named_disps[MAX_ARGS - 1 - forward_index] = *dispid;
    }
    (reversed, named_disps, MAX_ARGS - args.len())
}

/// Runs `IDispatch::Invoke` and returns the result variant.
pub(crate) fn dispatch_invoke(
    disp: *mut c_void,
    member: i32,
    name: &str,
    flags: u16,
    args: &mut [Variant],
    named: &[(i32, usize)],
) -> Result<Variant, String> {
    let vtbl = unsafe { idispatch_vtbl(disp) };

    let (mut reversed, mut named_disps, base) = reversed_args(args, named);
    let params = DispParams {
        rgvarg: unsafe { reversed.as_mut_ptr().add(base) },
        rgdispid_named_args: if named.is_empty() {
            ptr::null_mut()
        } else {
            unsafe { named_disps.as_mut_ptr().add(base) }
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

/// Calls a method by name with positional args.
pub(crate) fn call(disp: *mut c_void, name: &str, args: &mut [Variant]) -> Result<Variant, String> {
    let member = dispatch_id(disp, name)?;
    dispatch_invoke(disp, member, name, DISPATCH_METHOD, args, &[])
}

/// Reads a property by name.
pub(crate) fn get_prop(disp: *mut c_void, name: &str) -> Result<Variant, String> {
    let member = dispatch_id(disp, name)?;
    dispatch_invoke(disp, member, name, DISPATCH_PROPERTYGET, &mut [], &[])
}

/// Writes a property by name (single named argument).
pub(crate) fn put_prop(disp: *mut c_void, name: &str, value: &Variant) -> Result<(), String> {
    let member = dispatch_id(disp, name)?;
    let mut args = [*value];
    dispatch_invoke(
        disp,
        member,
        name,
        DISPATCH_PROPERTYPUT,
        &mut args,
        &[(DISPID_PROPERTYPUT, 0)],
    )
    .map(|_| ())
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
    fn safe_array_layout_is_32_bytes_on_x64() {
        assert_eq!(std::mem::size_of::<SafeArray>(), 32);
        assert_eq!(std::mem::size_of::<SafeArrayBound>(), 8);
    }

    #[test]
    fn reversed_args_puts_the_last_argument_first() {
        let args = [Variant::i4(1), Variant::i4(2), Variant::i4(3)];
        let (reversed, _, base) = reversed_args(&args, &[]);
        assert_eq!(base, 4);
        assert_eq!(reversed[4].data[0], 3);
        assert_eq!(reversed[5].data[0], 2);
        assert_eq!(reversed[6].data[0], 1);
        assert_eq!(reversed[0].vt, VT_EMPTY);
    }

    #[test]
    fn reversed_args_aligns_named_dispid_with_its_reversed_slot() {
        let args = [Variant::i4(1), Variant::i4(2)];
        let (_, named_disps, base) = reversed_args(&args, &[(DISPID_PROPERTYPUT, 1)]);
        assert_eq!(base, 5);
        assert_eq!(named_disps[5], DISPID_PROPERTYPUT);
    }

    #[test]
    fn reversed_args_put_prop_layout_is_dispid_at_the_base() {
        let args = [Variant::i4(9)];
        let (_, named_disps, base) = reversed_args(&args, &[(DISPID_PROPERTYPUT, 0)]);
        assert_eq!(base, 6);
        assert_eq!(named_disps[6], DISPID_PROPERTYPUT);
    }

    #[test]
    fn reversed_args_empty_call_has_base_one_past_the_array() {
        let (_, _, base) = reversed_args(&[], &[]);
        assert_eq!(base, 7);
    }
}
