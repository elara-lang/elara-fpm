//! Mutable HTTP response buffer shared with Elara natives.

#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::os::raw::c_void;

use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_context_userdata,
    elara_is_int, elara_return, elara_to_int, elara_val_nil, value_as_string,
};

/// Host-side response accumulated while a script runs.
#[derive(Debug, Clone, Default)]
pub struct ResponseBuffer {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    /// Separate from `headers` so multiple `Set-Cookie` lines are preserved.
    pub set_cookies: Vec<String>,
    pub body: Vec<u8>,
    pub finalized: bool,
}

impl ResponseBuffer {
    pub fn new() -> Self {
        Self {
            status: 200,
            headers: BTreeMap::new(),
            set_cookies: Vec::new(),
            body: Vec::new(),
            finalized: false,
        }
    }

    pub fn finalize(&mut self) -> FinalResponse {
        self.finalized = true;
        FinalResponse {
            status: self.status,
            headers: self.headers.clone(),
            set_cookies: self.set_cookies.clone(),
            body: self.body.clone(),
        }
    }
}

/// Immutable snapshot after the script finishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub set_cookies: Vec<String>,
    pub body: Vec<u8>,
}

pub unsafe extern "C" fn native_res_set_status(
    _s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        if elara_argc(ctx) < 1 {
            return elara_Status::ErrRuntime;
        }
        let code = crate::ffi::elara_to_int(elara_arg(ctx, 0));
        if !(100..=599).contains(&code) {
            return elara_Status::ErrRuntime;
        }
        buf.status = code as u16;
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

pub unsafe extern "C" fn native_res_header(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        if elara_argc(ctx) < 2 {
            return elara_Status::ErrRuntime;
        }
        let name = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        let value = match value_as_string(s, elara_arg(ctx, 1)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        buf.headers.insert(name, value);
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

pub unsafe extern "C" fn native_res_write(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        if elara_argc(ctx) < 1 {
            return elara_Status::ErrRuntime;
        }
        let chunk = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        buf.body.extend_from_slice(chunk.as_bytes());
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

fn redirect_status_allowed(code: i64) -> bool {
    matches!(code, 301 | 302 | 303 | 307 | 308)
}

/// `res.redirect(url)` or `res.redirect(url, status)`.
pub unsafe extern "C" fn native_res_redirect(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        let argc = elara_argc(ctx);
        if argc < 1 {
            return elara_Status::ErrRuntime;
        }
        let url = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        let status = if argc >= 2 {
            let arg = elara_arg(ctx, 1);
            if !elara_is_int(arg) {
                return elara_Status::ErrRuntime;
            }
            let code = elara_to_int(arg);
            if !redirect_status_allowed(code) {
                return elara_Status::ErrRuntime;
            }
            code as u16
        } else {
            302
        };
        buf.status = status;
        buf.headers.insert("Location".into(), url);
        buf.body.clear();
        buf.finalized = true;
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

pub(crate) unsafe fn response_from_ctx<'a>(
    ctx: *mut elara_CallContext,
) -> Option<&'a mut ResponseBuffer> {
    unsafe {
        let ptr = elara_context_userdata(ctx) as *mut ResponseBuffer;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

pub fn userdata_ptr(buf: &mut ResponseBuffer) -> *mut c_void {
    buf as *mut ResponseBuffer as *mut c_void
}
