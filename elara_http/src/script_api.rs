//! Script-visible `req` / `res` injection.
//!
#![allow(clippy::not_unsafe_ptr_arg_deref)]
//!
//! # Binding shape
//!
//! Elara’s compiler rejects undeclared identifiers, and `elara_set_global` values are
//! **not** visible to `elara_eval` as names. The host therefore wraps each script in a
//! function that receives response natives as parameters and binds well-known locals
//! `req` / `res` (request-scoped; same names adapters document for scripts):
//!
//! - `req` — table: `method`, `path`, `query`, `headers` (lowercased keys), `body`
//! - `res` — table of closures over host natives mutating a [`ResponseBuffer`]
//!
//! Header lookups are case-insensitive (keys stored lowercased). Multi-value: first wins.

use std::collections::BTreeMap;

use crate::ffi::{
    elara_State, elara_Status, elara_Value, elara_call, elara_close, elara_open,
    elara_val_native_fn, elara_val_nil, eval,
};
use crate::response::{
    FinalResponse, ResponseBuffer, native_res_header, native_res_set_status, native_res_write,
    userdata_ptr,
};

/// Inbound request fields exposed on `req`.
#[derive(Debug, Clone, Default)]
pub struct RequestData {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

impl RequestData {
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(name.into().to_ascii_lowercase(), value.into());
        self
    }
}

/// Pair used for one script execution.
pub struct ScriptBindings {
    pub response: ResponseBuffer,
}

impl Default for ScriptBindings {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptBindings {
    pub fn new() -> Self {
        Self {
            response: ResponseBuffer::new(),
        }
    }

    pub fn finalize(mut self) -> FinalResponse {
        self.response.finalize()
    }
}

fn elara_string_lit(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn headers_literal(headers: &BTreeMap<String, String>) -> String {
    let mut src = String::from("{\n");
    for (k, v) in headers {
        src.push_str(&format!(
            "  [{}] = {},\n",
            elara_string_lit(k),
            elara_string_lit(v)
        ));
    }
    src.push('}');
    src
}

fn wrap_script(request: &RequestData, source: &str) -> String {
    format!(
        r#"
fn __elara_http_run(__res_set_status, __res_header, __res_write) {{
  let req = {{
    method: {method},
    path: {path},
    query: {query},
    headers: {headers},
    body: {body},
  }}
  let res = {{
    set_status: fn(code) {{ __res_set_status(code) }},
    header: fn(name, value) {{ __res_header(name, value) }},
    write: fn(chunk) {{ __res_write(chunk) }},
  }}
{source}
  return nil
}}
return __elara_http_run
"#,
        method = elara_string_lit(&request.method.to_ascii_uppercase()),
        path = elara_string_lit(&request.path),
        query = elara_string_lit(&request.query),
        headers = headers_literal(&request.headers),
        body = elara_string_lit(&request.body),
        source = source,
    )
}

/// Run `source` with `req` / `res` bound; returns the finalized response and optional script error.
pub fn run_script(
    request: &RequestData,
    source: &str,
) -> Result<(FinalResponse, Option<String>), String> {
    unsafe {
        let state = elara_open();
        if state.is_null() {
            return Err("elara_open returned null".into());
        }
        let mut bindings = ScriptBindings::new();
        let result = run_script_on(state, request, &mut bindings, source);
        let response = bindings.finalize();
        elara_close(state);
        match result {
            Ok(()) => Ok((response, None)),
            Err(e) => Ok((response, Some(e))),
        }
    }
}

/// Like [`run_script`] but uses an existing state and bindings (for adapter reuse).
pub fn run_script_on(
    state: *mut elara_State,
    request: &RequestData,
    bindings: &mut ScriptBindings,
    source: &str,
) -> Result<(), String> {
    let wrapped = wrap_script(request, source);
    let callable = eval(state, "elara_http_wrap", &wrapped)?;
    let ud = userdata_ptr(&mut bindings.response);
    unsafe {
        let args: [elara_Value; 3] = [
            elara_val_native_fn(state, Some(native_res_set_status), ud),
            elara_val_native_fn(state, Some(native_res_header), ud),
            elara_val_native_fn(state, Some(native_res_write), ud),
        ];
        let mut results = [elara_val_nil()];
        let mut nret: u16 = 0;
        let status = elara_call(
            state,
            callable,
            args.as_ptr(),
            3,
            results.as_mut_ptr(),
            1,
            &mut nret,
        );
        if status != elara_Status::Ok {
            return Err(crate::ffi::last_error_message(state));
        }
    }
    Ok(())
}

/// Smoke helper: inject via wrapper and confirm `res.write` works (bindings alive).
pub fn smoke_inject_req_res() -> Result<(), String> {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) = run_script(&req, r#"res.write("ok")"#)?;
    if let Some(e) = err {
        return Err(e);
    }
    if response.body != b"ok" {
        return Err(format!(
            "expected body ok, got {:?}",
            String::from_utf8_lossy(&response.body)
        ));
    }
    Ok(())
}
