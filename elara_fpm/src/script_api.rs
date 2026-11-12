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
    elara_State, elara_Status, elara_Value, elara_call, elara_close, elara_compile_to_bytecode,
    elara_free_buffer, elara_load_bytecode, elara_val_native_fn, elara_val_nil, eval,
    last_error_message,
};
use crate::helpers::native_html_escape;
use crate::response::{
    FinalResponse, ResponseBuffer, native_res_header, native_res_redirect, native_res_set_status,
    native_res_write, userdata_ptr,
};
use crate::sandbox::{SandboxConfig, open_request_state};
use crate::session::{
    FileSessionStore, Session, SessionConfig, native_session_commit, native_session_destroy,
    native_session_reject, session_data_literal, session_set_cookie_line, session_userdata_ptr,
};
use crate::template::native_res_render;

/// Inbound request fields exposed on `req`.
#[derive(Debug, Clone, Default)]
pub struct RequestData {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
    /// Host for this request (`Host` / `HTTP_HOST` / `SERVER_NAME`).
    pub host: String,
    /// `http` or `https` for this request.
    pub scheme: String,
    /// Client address without port.
    pub remote: String,
    /// Parsed query string (`req.args`). Last key wins. Does not alter `query`.
    pub args: BTreeMap<String, String>,
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
    pub session: Option<Session>,
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
            session: None,
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

pub(crate) fn wrap_script(
    request: &RequestData,
    source: &str,
    session: Option<&Session>,
) -> Result<String, String> {
    let json_expr = if request.json_elara.is_empty() {
        "nil".to_string()
    } else {
        request.json_elara.clone()
    };
    let enc_helpers = include_str!("session_elara.inc.ela");
    let session_block = if let Some(sess) = session {
        let data_lit = session_data_literal(&sess.data)?;
        format!(
            r#"
  let session_data = {data_lit}
  let session = {{
    get: fn(k) {{
      return session_data[k]
    }},
    set: fn(k, v) {{
      let j = __fpm_sess_enc(v, 0)
      if j == nil {{
        __session_reject()
      }}
      session_data[k] = v
    }},
    destroy: fn() {{
      session_data = {{}}
      __session_destroy()
    }},
  }}
  defer {{
    let blob = __fpm_sess_enc_map(session_data)
    if blob != nil {{
      __session_commit(blob)
    }}
  }}
"#
        )
    } else {
        String::new()
    };
    let params = if session.is_some() {
        "__res_set_status, __res_header, __res_write, __res_cookie, __res_redirect, __res_render, __html_escape, __session_commit, __session_destroy, __session_reject, __req_body"
    } else {
        "__res_set_status, __res_header, __res_write, __res_cookie, __res_redirect, __res_render, __html_escape, __req_body"
    };
    // Body is passed as a call argument (`__req_body`) because Elara double-quoted
    // strings interpolate `{…}` and cannot safely embed raw JSON text.
    Ok(format!(
        r#"
fn __elara_fpm_run({params}) {{
{enc_helpers}
  let req = {{
    method: {method},
    path: {path},
    query: {query},
    headers: {headers},
    body: __req_body,
    host: {host},
    scheme: {scheme},
    remote: {remote},
    args: {args},
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
    redirect: __res_redirect,
    render: fn(path, ctx) {{
      let c = ctx
      if c == nil {{
        c = {{}}
      }}
      let j = __fpm_sess_enc(c, 0)
      if j == nil {{
        __res_render(path, "")
      }} else {{
        __res_render(path, j)
      }}
    }},
  }}
  let html_escape = __html_escape
{session_block}
{source}
  return nil
}}
return __elara_fpm_run
"#,
        method = elara_string_lit(&request.method.to_ascii_uppercase()),
        path = elara_string_lit(&request.path),
        query = elara_string_lit(&request.query),
        headers = headers_literal(&request.headers),
        host = elara_string_lit(&request.host),
        scheme = elara_string_lit(&request.scheme),
        remote = elara_string_lit(&request.remote),
        args = string_map_literal(&request.args),
        form = string_map_literal(&request.form),
        cookies = string_map_literal(&request.cookies),
        json = json_expr,
        files = files_literal(&request.files),
        source = source,
    ))
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
    run_script_with_session(request, source, limits, None)
}

/// Run a script with optional file-backed sessions and default sandbox.
pub fn run_script_with_session(
    request: &RequestData,
    source: &str,
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
) -> Result<(FinalResponse, Option<String>), String> {
    run_script_sandboxed(
        request,
        source,
        limits,
        session_cfg,
        &SandboxConfig::default(),
    )
}

/// Run a script with explicit sandbox configuration.
pub fn run_script_sandboxed(
    request: &RequestData,
    source: &str,
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
) -> Result<(FinalResponse, Option<String>), String> {
    run_script_sandboxed_with_templates(request, source, limits, session_cfg, sandbox, None)
}

/// Like [`run_script_sandboxed`] with an explicit template directory.
pub fn run_script_sandboxed_with_templates(
    request: &RequestData,
    source: &str,
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
    template_dir: Option<&std::path::Path>,
) -> Result<(FinalResponse, Option<String>), String> {
    if let Some(dir) = template_dir {
        crate::template::set_template_dir(dir.to_path_buf());
    }
    let mut prepared = request.clone();
    prepared.args = crate::body::parse_urlencoded(&prepared.query);
    if prepared.cookies.is_empty()
        && let Some(raw) = prepared.headers.get("cookie")
    {
        prepared.cookies = parse_cookie_header(raw);
    }
    if let Err(e) = enrich_request(&mut prepared, limits) {
        return Err(e.to_string());
    }

    let store = if let Some(cfg) = session_cfg {
        Some(FileSessionStore::open(&cfg.store_dir)?)
    } else {
        None
    };

    let outcome = unsafe {
        let state = match open_request_state(sandbox) {
            Ok(s) => s,
            Err(e) => {
                crate::multipart::cleanup_uploads(&prepared);
                return Err(e);
            }
        };
        let mut bindings = ScriptBindings::new();
        if let (Some(cfg), Some(store)) = (session_cfg, store.as_ref()) {
            let cookie_id = prepared.cookies.get(&cfg.cookie_name).map(String::as_str);
            bindings.session = Some(store.load_or_create(cookie_id)?);
        }
        let result = run_script_on(state, &prepared, &mut bindings, source);
        let script_ok = result.is_ok();
        if let (Some(cfg), Some(store), Some(session)) =
            (session_cfg, store.as_ref(), bindings.session.as_ref())
        {
            if script_ok {
                let _ = store.save(session);
            }
            let issue_cookie = session.is_new || session.destroyed || script_ok;
            if issue_cookie {
                bindings
                    .response
                    .set_cookies
                    .push(session_set_cookie_line(cfg, session));
            }
        }
        let response = bindings.finalize();
        elara_close(state);
        match result {
            Ok(()) => Ok((response, None)),
            Err(e) => Ok((
                crate::helpers::host_error_page(500, "Internal Server Error"),
                Some(e),
            )),
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
    let wrapped = wrap_script(request, source, bindings.session.as_ref())?;
    let callable = eval(state, "elara_fpm_wrap", &wrapped)?;
    call_wrapped_run(state, request, bindings, callable)
}

/// Load a `.elac` chunk, run it to obtain the host entry function, then call that function.
pub fn run_bytecode_on(
    state: *mut elara_State,
    request: &RequestData,
    bindings: &mut ScriptBindings,
    bytes: &[u8],
) -> Result<(), String> {
    unsafe {
        let mut chunk = elara_val_nil();
        let status = elara_load_bytecode(state, bytes.as_ptr(), bytes.len(), &mut chunk);
        if status != elara_Status::Ok {
            return Err(last_error_message(state));
        }
        let mut results = [elara_val_nil()];
        let mut nret: u16 = 0;
        let status = elara_call(
            state,
            chunk,
            std::ptr::null(),
            0,
            results.as_mut_ptr(),
            1,
            &mut nret,
        );
        if status != elara_Status::Ok {
            return Err(last_error_message(state));
        }
        call_wrapped_run(state, request, bindings, results[0])
    }
}

fn call_wrapped_run(
    state: *mut elara_State,
    request: &RequestData,
    bindings: &mut ScriptBindings,
    callable: elara_Value,
) -> Result<(), String> {
    let ud = userdata_ptr(&mut bindings.response);
    unsafe {
        let body_val = crate::ffi::elara_val_string(
            state,
            request.body.as_ptr() as *const std::os::raw::c_char,
            request.body.len(),
        );
        if let Some(sess) = bindings.session.as_mut() {
            let sud = session_userdata_ptr(sess);
            let args: [elara_Value; 11] = [
                elara_val_native_fn(state, Some(native_res_set_status), ud),
                elara_val_native_fn(state, Some(native_res_header), ud),
                elara_val_native_fn(state, Some(native_res_write), ud),
                elara_val_native_fn(state, Some(native_res_cookie), ud),
                elara_val_native_fn(state, Some(native_res_redirect), ud),
                elara_val_native_fn(state, Some(native_res_render), ud),
                elara_val_native_fn(state, Some(native_html_escape), std::ptr::null_mut()),
                elara_val_native_fn(state, Some(native_session_commit), sud),
                elara_val_native_fn(state, Some(native_session_destroy), sud),
                elara_val_native_fn(state, Some(native_session_reject), sud),
                body_val,
            ];
            let mut results = [elara_val_nil()];
            let mut nret: u16 = 0;
            let status = elara_call(
                state,
                callable,
                args.as_ptr(),
                11,
                results.as_mut_ptr(),
                1,
                &mut nret,
            );
            if status != elara_Status::Ok {
                return Err(last_error_message(state));
            }
        } else {
            let args: [elara_Value; 8] = [
                elara_val_native_fn(state, Some(native_res_set_status), ud),
                elara_val_native_fn(state, Some(native_res_header), ud),
                elara_val_native_fn(state, Some(native_res_write), ud),
                elara_val_native_fn(state, Some(native_res_cookie), ud),
                elara_val_native_fn(state, Some(native_res_redirect), ud),
                elara_val_native_fn(state, Some(native_res_render), ud),
                elara_val_native_fn(state, Some(native_html_escape), std::ptr::null_mut()),
                body_val,
            ];
            let mut results = [elara_val_nil()];
            let mut nret: u16 = 0;
            let status = elara_call(
                state,
                callable,
                args.as_ptr(),
                8,
                results.as_mut_ptr(),
                1,
                &mut nret,
            );
            if status != elara_Status::Ok {
                return Err(last_error_message(state));
            }
        }
    }
    Ok(())
}

/// Compile wrapped host script source to `.elac` bytes (for fixtures and tests).
pub fn compile_wrapped_script_to_bytecode(
    request: &RequestData,
    source: &str,
) -> Result<Vec<u8>, String> {
    let wrapped = wrap_script(request, source, None)?;
    unsafe {
        let state = open_request_state(&SandboxConfig::default())?;
        let name = std::ffi::CString::new("fixture").map_err(|e| e.to_string())?;
        let src = std::ffi::CString::new(wrapped).map_err(|e| e.to_string())?;
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut len: usize = 0;
        let status =
            elara_compile_to_bytecode(state, name.as_ptr(), src.as_ptr(), &mut buf, &mut len);
        if status != elara_Status::Ok {
            let err = last_error_message(state);
            elara_close(state);
            return Err(err);
        }
        let bytes = std::slice::from_raw_parts(buf, len).to_vec();
        elara_free_buffer(state, buf, len);
        elara_close(state);
        Ok(bytes)
    }
}

/// Run `.ela` source or `.elac` bytecode from a document-root path.
pub fn run_script_path_sandboxed(
    path: &std::path::Path,
    request: &RequestData,
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
) -> Result<(FinalResponse, Option<String>), String> {
    run_script_path_sandboxed_with_templates(path, request, limits, session_cfg, sandbox, None)
}

/// Like [`run_script_path_sandboxed`] with an explicit template directory.
pub fn run_script_path_sandboxed_with_templates(
    path: &std::path::Path,
    request: &RequestData,
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
    template_dir: Option<&std::path::Path>,
) -> Result<(FinalResponse, Option<String>), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "elac" {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        run_bytecode_sandboxed_with_templates(
            request,
            &bytes,
            limits,
            session_cfg,
            sandbox,
            template_dir,
        )
    } else {
        let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        run_script_sandboxed_with_templates(
            request,
            &source,
            limits,
            session_cfg,
            sandbox,
            template_dir,
        )
    }
}

/// Like [`run_script_sandboxed`] for precompiled bytecode.
pub fn run_bytecode_sandboxed(
    request: &RequestData,
    bytes: &[u8],
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
) -> Result<(FinalResponse, Option<String>), String> {
    run_bytecode_sandboxed_with_templates(request, bytes, limits, session_cfg, sandbox, None)
}

/// Like [`run_bytecode_sandboxed`] with an explicit template directory.
pub fn run_bytecode_sandboxed_with_templates(
    request: &RequestData,
    bytes: &[u8],
    limits: &BodyLimits,
    session_cfg: Option<&SessionConfig>,
    sandbox: &SandboxConfig,
    template_dir: Option<&std::path::Path>,
) -> Result<(FinalResponse, Option<String>), String> {
    if let Some(dir) = template_dir {
        crate::template::set_template_dir(dir.to_path_buf());
    }
    let mut prepared = request.clone();
    prepared.args = crate::body::parse_urlencoded(&prepared.query);
    if prepared.cookies.is_empty()
        && let Some(raw) = prepared.headers.get("cookie")
    {
        prepared.cookies = parse_cookie_header(raw);
    }
    if let Err(e) = enrich_request(&mut prepared, limits) {
        return Err(e.to_string());
    }

    let store = if let Some(cfg) = session_cfg {
        Some(FileSessionStore::open(&cfg.store_dir)?)
    } else {
        None
    };

    let outcome = unsafe {
        let state = match open_request_state(sandbox) {
            Ok(s) => s,
            Err(e) => {
                crate::multipart::cleanup_uploads(&prepared);
                return Err(e);
            }
        };
        let mut bindings = ScriptBindings::new();
        if let (Some(cfg), Some(store)) = (session_cfg, store.as_ref()) {
            let cookie_id = prepared.cookies.get(&cfg.cookie_name).map(String::as_str);
            bindings.session = Some(store.load_or_create(cookie_id)?);
        }
        let result = run_bytecode_on(state, &prepared, &mut bindings, bytes);
        let script_ok = result.is_ok();
        if let (Some(cfg), Some(store), Some(session)) =
            (session_cfg, store.as_ref(), bindings.session.as_ref())
        {
            if script_ok {
                let _ = store.save(session);
            }
            let issue_cookie = session.is_new || session.destroyed || script_ok;
            if issue_cookie {
                bindings
                    .response
                    .set_cookies
                    .push(session_set_cookie_line(cfg, session));
            }
        }
        let response = bindings.finalize();
        elara_close(state);
        match result {
            Ok(()) => Ok((response, None)),
            Err(e) => Ok((
                crate::helpers::host_error_page(500, "Internal Server Error"),
                Some(e),
            )),
        }
    };
    crate::multipart::cleanup_uploads(&prepared);
    outcome
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
