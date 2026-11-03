//! CGI request/response adapter.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use crate::helpers::{apply_head_method, host_error_page};
use crate::process::ProcessConfig;
use crate::response::FinalResponse;
use crate::routing::{Route, resolve_route, serve_static};
use crate::script_api::{RequestData, run_script_path_sandboxed};

/// Build [`RequestData`] from a CGI-style environment map and stdin body bytes.
pub fn request_from_cgi_env(env: &BTreeMap<String, String>, stdin_body: &[u8]) -> RequestData {
    let method = env
        .get("REQUEST_METHOD")
        .cloned()
        .unwrap_or_else(|| "GET".into());
    let path = env
        .get("PATH_INFO")
        .filter(|p| !p.is_empty())
        .cloned()
        .or_else(|| env.get("SCRIPT_NAME").cloned())
        .unwrap_or_else(|| "/".into());
    let query = env.get("QUERY_STRING").cloned().unwrap_or_default();

    let mut headers = BTreeMap::new();
    for (k, v) in env {
        if let Some(rest) = k.strip_prefix("HTTP_") {
            let name = rest.replace('_', "-").to_ascii_lowercase();
            headers.insert(name, v.clone());
        }
    }
    if let Some(ct) = env.get("CONTENT_TYPE") {
        headers.insert("content-type".into(), ct.clone());
    }
    if let Some(cl) = env.get("CONTENT_LENGTH") {
        headers.insert("content-length".into(), cl.clone());
    }

    let body = match env
        .get("CONTENT_LENGTH")
        .and_then(|s| s.parse::<usize>().ok())
    {
        Some(n) => String::from_utf8_lossy(&stdin_body[..n.min(stdin_body.len())]).into_owned(),
        None => String::from_utf8_lossy(stdin_body).into_owned(),
    };

    let host = env
        .get("HTTP_HOST")
        .cloned()
        .or_else(|| env.get("SERVER_NAME").cloned())
        .unwrap_or_default();
    let scheme = {
        let rs = env.get("REQUEST_SCHEME").map(|s| s.as_str()).unwrap_or("");
        let https = env.get("HTTPS").map(|s| s.as_str()).unwrap_or("");
        if rs.eq_ignore_ascii_case("https") || https.eq_ignore_ascii_case("on") || https == "1" {
            "https".into()
        } else {
            "http".into()
        }
    };
    let remote = env.get("REMOTE_ADDR").cloned().unwrap_or_default();

    RequestData {
        method,
        path,
        query,
        headers,
        body,
        host,
        scheme,
        remote,
        ..RequestData::default()
    }
}

/// Format a finalized response as a CGI stdout payload (`Status` + headers + body).
pub fn format_cgi_response(resp: &FinalResponse) -> Vec<u8> {
    let mut out = Vec::new();
    let status_line = format!("Status: {} {}\r\n", resp.status, reason_phrase(resp.status));
    out.extend_from_slice(status_line.as_bytes());
    for (k, v) in &resp.headers {
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    for cookie in &resp.set_cookies {
        out.extend_from_slice(b"Set-Cookie: ");
        out.extend_from_slice(cookie.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&resp.body);
    out
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "OK",
    }
}

/// Resolve and run one CGI request against `docroot`.
pub fn handle_cgi_request(
    docroot: &Path,
    env: &BTreeMap<String, String>,
    stdin_body: &[u8],
) -> FinalResponse {
    handle_cgi_request_with(docroot, env, stdin_body, &ProcessConfig::default())
}

/// Like [`handle_cgi_request`] with process configuration.
pub fn handle_cgi_request_with(
    docroot: &Path,
    env: &BTreeMap<String, String>,
    stdin_body: &[u8],
    process: &ProcessConfig,
) -> FinalResponse {
    let req = request_from_cgi_env(env, stdin_body);
    // Prefer SCRIPT_FILENAME when present and under docroot.
    let resp = if let Some(sf) = env.get("SCRIPT_FILENAME") {
        let sf_path = std::path::Path::new(sf);
        if let (Ok(root), Ok(canon)) = (
            std::fs::canonicalize(docroot),
            std::fs::canonicalize(sf_path),
        ) && canon.starts_with(&root)
            && canon.is_file()
        {
            run_script_file(&canon, &req, process)
        } else {
            match resolve_route(docroot, &req.path) {
                Route::Error(e) => e,
                Route::Static { path, content_type } => serve_static(&path, &content_type),
                Route::Script(path) => run_script_file(&path, &req, process),
            }
        }
    } else {
        match resolve_route(docroot, &req.path) {
            Route::Error(e) => e,
            Route::Static { path, content_type } => serve_static(&path, &content_type),
            Route::Script(path) => run_script_file(&path, &req, process),
        }
    };
    apply_head_method(&req.method, resp)
}

fn run_script_file(path: &Path, req: &RequestData, process: &ProcessConfig) -> FinalResponse {
    match run_script_path_sandboxed(
        path,
        req,
        &process.body_limits,
        Some(&process.session),
        &process.sandbox,
    ) {
        Ok((resp, _)) => resp,
        Err(_) => host_error_page(500, "Internal Server Error"),
    }
}

/// Read all of stdin limited by CONTENT_LENGTH when set.
pub fn read_cgi_stdin(env: &BTreeMap<String, String>, stdin: &mut dyn Read) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(n) = env
        .get("CONTENT_LENGTH")
        .and_then(|s| s.parse::<usize>().ok())
    {
        buf.resize(n, 0);
        let mut read = 0;
        while read < n {
            match stdin.read(&mut buf[read..]) {
                Ok(0) => break,
                Ok(k) => read += k,
                Err(_) => break,
            }
        }
        buf.truncate(read);
    } else {
        let _ = stdin.read_to_end(&mut buf);
    }
    buf
}

/// Write CGI response to stdout.
pub fn write_cgi_stdout(stdout: &mut dyn Write, resp: &FinalResponse) -> std::io::Result<()> {
    stdout.write_all(&format_cgi_response(resp))
}

/// Collect process environment into a string map (CGI variables).
pub fn cgi_env_from_os() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

/// Run one CGI request using OS env + stdin → stdout. Returns process exit code.
pub fn run_cgi_once(docroot: &Path) -> i32 {
    run_cgi_once_with(docroot, &ProcessConfig::default())
}

/// Like [`run_cgi_once`] with process configuration.
pub fn run_cgi_once_with(docroot: &Path, process: &ProcessConfig) -> i32 {
    let env = cgi_env_from_os();
    let body = read_cgi_stdin(&env, &mut std::io::stdin());
    let resp = handle_cgi_request_with(docroot, &env, &body, process);
    match write_cgi_stdout(&mut std::io::stdout(), &resp) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
