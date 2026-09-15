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
//! - `req` — table: `method`, `path`, `query`, `headers` (lowercased keys), `body`,
//!   plus `form` / `json` / `files` when body parsers run
//! - `res` — table of closures over host natives mutating a [`ResponseBuffer`]
//!
//! Header lookups are case-insensitive (keys stored lowercased). Multi-value: first wins.

use std::collections::BTreeMap;

use crate::body::{BodyLimits, enrich_request};
use crate::cookies::{native_res_cookie, parse_cookie_header};
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
    /// Parsed `application/x-www-form-urlencoded` (and multipart text fields). Last key wins.
    pub form: BTreeMap<String, String>,
    /// Parsed `Cookie` header. Duplicate names: last value wins.
    pub cookies: BTreeMap<String, String>,
    /// Elara source fragment for `req.json` (e.g. `nil` or a table literal); set by JSON parser.
    pub json_elara: String,
    /// Uploaded files metadata for `req.files` (filled by multipart).
    pub files: Vec<UploadedFile>,
}

/// One multipart file part exposed on `req.files`.
#[derive(Debug, Clone, Default)]
pub struct UploadedFile {
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub path: String,
    pub size: u64,
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

fn string_map_literal(map: &BTreeMap<String, String>) -> String {
    headers_literal(map)
}

fn files_literal(files: &[UploadedFile]) -> String {
    let mut src = String::from("{\n");
    for (i, f) in files.iter().enumerate() {
        src.push_str(&format!(
            "  [{i}] = {{\n    name: {},\n    filename: {},\n    content_type: {},\n    path: {},\n    size: {},\n  }},\n",
            elara_string_lit(&f.name),
            elara_string_lit(&f.filename),
            elara_string_lit(&f.content_type),
            elara_string_lit(&f.path),
            f.size,
        ));
    }
    src.push('}');
    src
}

pub(crate) fn wrap_script(request: &RequestData, source: &str) -> String {
    let json_expr = if request.json_elara.is_empty() {
        "nil".to_string()
    } else {
        request.json_elara.clone()
    };
    // Body is passed as a call argument (`__req_body`) because Elara double-quoted
    // strings interpolate `{…}` and cannot safely embed raw JSON text.
    format!(
        r#"
fn __elara_http_run(__res_set_status, __res_header, __res_write, __res_cookie, __req_body) {{
  let req = {{
    method: {method},
    path: {path},
    query: {query},
    headers: {headers},
    body: __req_body,
    form: {form},
    cookies: {cookies},
    json: {json},
    files: {files},
  }}
  let res = {{
    set_status: fn(code) {{ __res_set_status(code) }},
    header: fn(name, value) {{ __res_header(name, value) }},
    write: fn(chunk) {{ __res_write(chunk) }},
    cookie: fn(name, value) {{
      __res_cookie(name, value, "/", 0, 0, "", -1)
    }},
    cookie_ex: fn(name, value, path, http_only, secure, same_site, max_age) {{
      __res_cookie(name, value, path, http_only, secure, same_site, max_age)
    }},
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
        form = string_map_literal(&request.form),
        cookies = string_map_literal(&request.cookies),
        json = json_expr,
        files = files_literal(&request.files),
        source = source,
    )
}

/// Run `source` with `req` / `res` bound; returns the finalized response and optional script error.
pub fn run_script(
    request: &RequestData,
    source: &str,
) -> Result<(FinalResponse, Option<String>), String> {
    run_script_with_limits(request, source, &BodyLimits::default())
}

/// Like [`run_script`] with explicit body/upload limits.
pub fn run_script_with_limits(
    request: &RequestData,
    source: &str,
    limits: &BodyLimits,
) -> Result<(FinalResponse, Option<String>), String> {
    let mut prepared = request.clone();
    if prepared.cookies.is_empty()
        && let Some(raw) = prepared.headers.get("cookie")
    {
        prepared.cookies = parse_cookie_header(raw);
    }
    if let Err(e) = enrich_request(&mut prepared, limits) {
        return Err(e.to_string());
    }
    let outcome = unsafe {
        let state = elara_open();
        if state.is_null() {
            crate::multipart::cleanup_uploads(&prepared);
            return Err("elara_open returned null".into());
        }
        let mut bindings = ScriptBindings::new();
        let result = run_script_on(state, &prepared, &mut bindings, source);
        let response = bindings.finalize();
        elara_close(state);
        match result {
            Ok(()) => Ok((response, None)),
            Err(e) => Ok((response, Some(e))),
        }
    };
    crate::multipart::cleanup_uploads(&prepared);
    outcome
}

/// Like [`run_script`] but uses an existing state and bindings (for adapter reuse).
/// Caller MUST run body enrichment first when structured `form`/`json`/`files` are needed.
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
        let body_val = crate::ffi::elara_val_string(
            state,
            request.body.as_ptr() as *const std::os::raw::c_char,
            request.body.len(),
        );
        let args: [elara_Value; 5] = [
            elara_val_native_fn(state, Some(native_res_set_status), ud),
            elara_val_native_fn(state, Some(native_res_header), ud),
            elara_val_native_fn(state, Some(native_res_write), ud),
            elara_val_native_fn(state, Some(native_res_cookie), ud),
            body_val,
        ];
        let mut results = [elara_val_nil()];
        let mut nret: u16 = 0;
        let status = elara_call(
            state,
            callable,
            args.as_ptr(),
            5,
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
