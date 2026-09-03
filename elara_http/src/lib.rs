//! Elara HTTP host library (SAPI). FFI bindings grow with the adapters.

#![allow(non_camel_case_types)]

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
    pub fn elara_to_int(v: elara_Value) -> i64;
    pub fn elara_last_error(s: *const elara_State) -> *const c_char;
}

/// Open a state, evaluate `return 1+1`, assert the integer result is 2, then close.
pub fn smoke_eval_one_plus_one() -> Result<(), String> {
    unsafe {
        let s = elara_open();
        if s.is_null() {
            return Err("elara_open returned null".into());
        }

        let mut result: elara_Value = 0;
        let name = c"smoke";
        let source = c"return 1+1";
        let status = elara_eval(s, name.as_ptr(), source.as_ptr(), &mut result);
        if status != elara_Status::Ok {
            let err = elara_last_error(s);
            let msg = if err.is_null() {
                format!("elara_eval failed with status {status:?}")
            } else {
                std::ffi::CStr::from_ptr(err).to_string_lossy().into_owned()
            };
            elara_close(s);
            return Err(msg);
        }

        if !elara_is_int(result) {
            elara_close(s);
            return Err("result is not an int".into());
        }
        let n = elara_to_int(result);
        elara_close(s);
        if n != 2 {
            return Err(format!("expected 2, got {n}"));
        }
        Ok(())
    }
}
