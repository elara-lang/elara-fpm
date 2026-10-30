//! Minimal Elara C ABI bindings used by the host.

#![allow(non_camel_case_types)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::missing_safety_doc)]

use std::os::raw::{c_char, c_void};

pub type elara_Value = u64;

#[repr(C)]
pub struct elara_State {
    _private: [u8; 0],
}

#[repr(C)]
pub struct elara_Config {
    pub alloc_fn: *mut c_void,
    pub alloc_ud: *mut c_void,
    pub mem_limit_bytes: usize,
    pub gas_limit: u64,
    pub pause_budget_us: u32,
    pub capabilities: u32,
    pub hash_seed: u64,
}

#[repr(C)]
pub struct elara_CallContext {
    _private: [u8; 0],
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum elara_Status {
    Ok = 0,
    Yield = 1,
    ErrSyntax = 2,
    ErrRuntime = 3,
    ErrOom = 4,
    ErrGas = 5,
    ErrVerify = 6,
    ErrModule = 7,
}

pub type elara_NativeFn =
    Option<unsafe extern "C" fn(*mut elara_State, *mut elara_CallContext) -> elara_Status>;

unsafe extern "C" {
    pub fn elara_default_config() -> elara_Config;
    pub fn elara_open() -> *mut elara_State;
    pub fn elara_open_with_config(cfg: *const elara_Config) -> *mut elara_State;
    pub fn elara_close(s: *mut elara_State);
    pub fn elara_eval(
        s: *mut elara_State,
        name: *const c_char,
        source: *const c_char,
        out_result: *mut elara_Value,
    ) -> elara_Status;
    pub fn elara_is_int(v: elara_Value) -> bool;
    pub fn elara_is_nil(v: elara_Value) -> bool;
    pub fn elara_is_float(v: elara_Value) -> bool;
    pub fn elara_is_string(v: elara_Value) -> bool;
    pub fn elara_to_int(v: elara_Value) -> i64;
    pub fn elara_to_string(
        s: *mut elara_State,
        v: elara_Value,
        out_len: *mut usize,
    ) -> *const c_char;
    pub fn elara_last_error(s: *const elara_State) -> *const c_char;
    pub fn elara_val_nil() -> elara_Value;
    pub fn elara_val_bool(b: bool) -> elara_Value;
    pub fn elara_val_int(s: *mut elara_State, i: i64) -> elara_Value;
    pub fn elara_val_string(s: *mut elara_State, str: *const c_char, len: usize) -> elara_Value;
    pub fn elara_val_native_fn(
        s: *mut elara_State,
        fn_: elara_NativeFn,
        userdata: *mut c_void,
    ) -> elara_Value;
    pub fn elara_set_global(s: *mut elara_State, name: *const c_char, val: elara_Value);
    pub fn elara_get_global(s: *mut elara_State, name: *const c_char) -> elara_Value;
    pub fn elara_argc(ctx: *const elara_CallContext) -> u16;
    pub fn elara_arg(ctx: *const elara_CallContext, index: u16) -> elara_Value;
    pub fn elara_return(ctx: *mut elara_CallContext, val: elara_Value);
    pub fn elara_context_userdata(ctx: *const elara_CallContext) -> *mut c_void;
    pub fn elara_call(
        s: *mut elara_State,
        callable: elara_Value,
        args: *const elara_Value,
        num_args: u16,
        results: *mut elara_Value,
        max_results: u16,
        out_num_results: *mut u16,
    ) -> elara_Status;
}

pub fn last_error_message(s: *mut elara_State) -> String {
    unsafe {
        let err = elara_last_error(s);
        if err.is_null() {
            "unknown Elara error".into()
        } else {
            std::ffi::CStr::from_ptr(err).to_string_lossy().into_owned()
        }
    }
}

pub fn eval(s: *mut elara_State, name: &str, source: &str) -> Result<elara_Value, String> {
    let name_c = std::ffi::CString::new(name).map_err(|_| "name contains NUL".to_string())?;
    let source_c = std::ffi::CString::new(source).map_err(|_| "source contains NUL".to_string())?;
    unsafe {
        let mut result = elara_val_nil();
        let status = elara_eval(s, name_c.as_ptr(), source_c.as_ptr(), &mut result);
        if status != elara_Status::Ok {
            return Err(last_error_message(s));
        }
        Ok(result)
    }
}

pub fn set_global_str(s: *mut elara_State, name: &str, value: &str) -> Result<(), String> {
    let name_c = std::ffi::CString::new(name).map_err(|_| "name contains NUL".to_string())?;
    unsafe {
        let val = elara_val_string(s, value.as_ptr() as *const c_char, value.len());
        elara_set_global(s, name_c.as_ptr(), val);
    }
    Ok(())
}

pub fn set_global_native(
    s: *mut elara_State,
    name: &str,
    fn_: elara_NativeFn,
    userdata: *mut c_void,
) -> Result<(), String> {
    let name_c = std::ffi::CString::new(name).map_err(|_| "name contains NUL".to_string())?;
    unsafe {
        let val = elara_val_native_fn(s, fn_, userdata);
        elara_set_global(s, name_c.as_ptr(), val);
    }
    Ok(())
}

pub fn value_as_string(s: *mut elara_State, v: elara_Value) -> Result<String, String> {
    unsafe {
        if !elara_is_string(v) {
            return Err("value is not a string".into());
        }
        let mut len = 0usize;
        let ptr = elara_to_string(s, v, &mut len);
        if ptr.is_null() {
            return Ok(String::new());
        }
        let slice = std::slice::from_raw_parts(ptr as *const u8, len);
        String::from_utf8(slice.to_vec()).map_err(|e| e.to_string())
    }
}
