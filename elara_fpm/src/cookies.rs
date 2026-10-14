//! Cookie request parsing and `Set-Cookie` emission.

#![allow(clippy::collapsible_if)]
#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;

use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_is_int,
    elara_return, elara_to_int, elara_val_nil, value_as_string,
};
use crate::response::{ResponseBuffer, response_from_ctx};

/// Parse a `Cookie` request header. Duplicate names: **last value wins**.
pub fn parse_cookie_header(header: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for part in header.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, value) = match part.split_once('=') {
            Some((n, v)) => (n.trim(), v.trim()),
            None => (part, ""),
        };
        if !name.is_empty() {
            out.insert(name.to_string(), value.to_string());
        }
    }
    out
}

/// Options for `res.cookie`.
#[derive(Debug, Clone, Default)]
pub struct CookieOptions {
    pub path: Option<String>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
    pub max_age: Option<i64>,
}

impl CookieOptions {
    pub fn format_set_cookie(&self, name: &str, value: &str) -> String {
        let mut line = format!("{name}={value}");
        let path = self.path.as_deref().unwrap_or("/");
        line.push_str("; Path=");
        line.push_str(path);
        if self.http_only {
            line.push_str("; HttpOnly");
        }
        if self.secure {
            line.push_str("; Secure");
        }
        if let Some(ss) = &self.same_site {
            line.push_str("; SameSite=");
            line.push_str(ss);
        }
        if let Some(age) = self.max_age {
            line.push_str("; Max-Age=");
            line.push_str(&age.to_string());
        }
        line
    }
}

/// `res.cookie(name, value)` or `res.cookie(name, value, opts_table)` via flattened natives.
///
/// MVP opts are passed as optional trailing strings/ints through a dedicated native
/// `res.cookie(name, value)` plus attribute setters on a builder, or a single native
/// with optional path/http_only/secure/same_site/max_age args:
/// `__res_cookie(name, value, path, http_only, secure, same_site, max_age)`
/// where booleans are 0/1 ints and empty strings mean unset (except path defaults to `/`).
pub unsafe extern "C" fn native_res_cookie(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            return elara_Status::ErrRuntime;
        }
        let argc = elara_argc(ctx);
        if argc < 2 {
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
        let mut opts = CookieOptions {
            path: Some("/".into()),
            ..CookieOptions::default()
        };
        if argc >= 3 {
            if let Ok(p) = value_as_string(s, elara_arg(ctx, 2)) {
                if !p.is_empty() {
                    opts.path = Some(p);
                }
            }
        }
        if argc >= 4 && elara_is_int(elara_arg(ctx, 3)) {
            opts.http_only = elara_to_int(elara_arg(ctx, 3)) != 0;
        }
        if argc >= 5 && elara_is_int(elara_arg(ctx, 4)) {
            opts.secure = elara_to_int(elara_arg(ctx, 4)) != 0;
        }
        if argc >= 6 {
            if let Ok(ss) = value_as_string(s, elara_arg(ctx, 5)) {
                if !ss.is_empty() {
                    opts.same_site = Some(ss);
                }
            }
        }
        if argc >= 7 && elara_is_int(elara_arg(ctx, 6)) {
            let age = elara_to_int(elara_arg(ctx, 6));
            if age >= 0 {
                opts.max_age = Some(age);
            }
        }
        buf.set_cookies.push(opts.format_set_cookie(&name, &value));
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

/// Merge Set-Cookie lines into header iteration for adapters.
pub fn apply_set_cookies(buf: &ResponseBuffer) -> Vec<(String, String)> {
    buf.set_cookies
        .iter()
        .map(|v| ("Set-Cookie".to_string(), v.clone()))
        .collect()
}
