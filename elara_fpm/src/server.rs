//! Cleartext HTTP/1.1 development server (`-S`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::access_log::{elapsed_ms_since, write_access_log};
use crate::helpers::{apply_head_method, host_error_page};
use crate::process::ProcessConfig;
use crate::response::FinalResponse;
use crate::routing::{Route, resolve_route, serve_static};
use crate::sandbox::ELARA_CAP_SYS_ENV;
use crate::script_api::{RequestData, run_script_path_sandboxed_with_templates};
use crate::template::default_template_dir;

/// Configuration for the built-in development server.
#[derive(Debug, Clone)]
pub struct DevServerConfig {
    pub listen: SocketAddr,
    pub docroot: PathBuf,
    pub process: ProcessConfig,
}

/// Parse `addr:port` (e.g. `127.0.0.1:8080`).
pub fn parse_listen_addr(s: &str) -> Result<SocketAddr, String> {
    s.parse::<SocketAddr>()
        .map_err(|e| format!("invalid listen address '{s}': {e}"))
}

/// Bind the listen socket (does not accept yet).
pub fn bind_listener(cfg: &DevServerConfig) -> Result<TcpListener, String> {
    TcpListener::bind(cfg.listen).map_err(|e| format!("bind {}: {e}", cfg.listen))
}

/// Print listen banner to stderr (listen address + document root).
pub fn print_listen_banner(cfg: &DevServerConfig, actual: SocketAddr) {
    eprintln!("elara-fpm development server started at http://{actual}/");
    eprintln!("Document root is {}", cfg.docroot.display());
}

/// Run the development server until the process is killed.
pub fn run_dev_server(cfg: DevServerConfig) -> Result<(), String> {
    let listener = bind_listener(&cfg)?;
    let actual = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?;
    print_listen_banner(&cfg, actual);
    listener
        .set_nonblocking(false)
        .map_err(|e| format!("set_nonblocking: {e}"))?;
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                if let Err(e) = handle_connection(stream, &cfg.docroot, &cfg.process) {
                    eprintln!("request error: {e}");
                }
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    Ok(())
}

/// Handle one client connection for the development server (exported for tests).
pub fn serve_dev_connection(stream: TcpStream, docroot: &Path) -> Result<(), String> {
    serve_dev_connection_with(stream, docroot, &ProcessConfig::default())
}

/// Like [`serve_dev_connection`] with explicit process configuration.
pub fn serve_dev_connection_with(
    stream: TcpStream,
    docroot: &Path,
    process: &ProcessConfig,
) -> Result<(), String> {
    handle_connection(stream, docroot, process)
}

enum FrameError {
    Eof,
    PayloadTooLarge { method: String, path: String },
    NotImplemented { method: String, path: String },
    Bad(String),
}

struct FramedRequest {
    req: RequestData,
    close: bool,
}

fn handle_connection(
    mut stream: TcpStream,
    docroot: &Path,
    process: &ProcessConfig,
) -> Result<(), String> {
    let mut pending = Vec::new();
    loop {
        let started = Instant::now();
        let framed = match read_http_request(
            &mut stream,
            &mut pending,
            process.body_limits.max_body_bytes,
        ) {
            Ok(f) => f,
            Err(FrameError::Eof) => return Ok(()),
            Err(FrameError::PayloadTooLarge { method, path }) => {
                let response = host_error_page(413, "Payload Too Large");
                write_access_log(&method, &path, response.status, elapsed_ms_since(started));
                write_http_response(&mut stream, &response, true)?;
                let _ = stream.flush();
                return Ok(());
            }
            Err(FrameError::NotImplemented { method, path }) => {
                let response = host_error_page(501, "Not Implemented");
                write_access_log(&method, &path, response.status, elapsed_ms_since(started));
                write_http_response(&mut stream, &response, true)?;
                let _ = stream.flush();
                return Ok(());
            }
            Err(FrameError::Bad(e)) => return Err(e),
        };
        let mut framed = framed;
        if framed.req.host.is_empty()
            && let Some(h) = framed.req.headers.get("host")
        {
            framed.req.host = h.clone();
        }
        if framed.req.scheme.is_empty() {
            framed.req.scheme = "http".into();
        }
        if framed.req.remote.is_empty()
            && let Ok(peer) = stream.peer_addr()
        {
            framed.req.remote = peer.ip().to_string();
        }
        let response = dispatch_request_with(docroot, &framed.req, process);
        write_access_log(
            &framed.req.method,
            &framed.req.path,
            response.status,
            elapsed_ms_since(started),
        );
        write_http_response(&mut stream, &response, framed.close)?;
        let _ = stream.flush();
        if framed.close {
            return Ok(());
        }
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn connection_should_close(
    version: &str,
    headers: &std::collections::BTreeMap<String, String>,
) -> bool {
    if let Some(c) = headers.get("connection") {
        let c = c.to_ascii_lowercase();
        if c.split(',').any(|t| t.trim() == "close") {
            return true;
        }
        if c.split(',').any(|t| t.trim() == "keep-alive") {
            return false;
        }
    }
    !version.eq_ignore_ascii_case("HTTP/1.1")
}

fn read_http_request(
    stream: &mut TcpStream,
    pending: &mut Vec<u8>,
    max_body: usize,
) -> Result<FramedRequest, FrameError> {
    let mut buf = std::mem::take(pending);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        let n = stream
            .read(&mut chunk)
            .map_err(|e| FrameError::Bad(e.to_string()))?;
        if n == 0 {
            return if buf.is_empty() {
                Err(FrameError::Eof)
            } else {
                Err(FrameError::Bad("incomplete HTTP headers".into()))
            };
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 64 * 1024 {
            return Err(FrameError::Bad("HTTP headers too large".into()));
        }
    };

    let head = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| FrameError::Bad("HTTP headers are not UTF-8".into()))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(FrameError::Bad("empty request".into()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or(FrameError::Bad("missing method".into()))?
        .to_string();
    let target = parts
        .next()
        .ok_or(FrameError::Bad("missing target".into()))?;
    let version = parts
        .next()
        .ok_or(FrameError::Bad("missing version".into()))?
        .to_string();

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };

    let mut headers = std::collections::BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    if headers.get("transfer-encoding").is_some_and(|v| {
        v.to_ascii_lowercase()
            .split(',')
            .any(|t| t.trim() == "chunked")
    }) {
        return Err(FrameError::NotImplemented {
            method: method.clone(),
            path: path.clone(),
        });
    }

    let body_len = match headers.get("content-length") {
        Some(cl) => cl
            .parse::<usize>()
            .map_err(|_| FrameError::Bad("invalid Content-Length".into()))?,
        None => 0,
    };
    if body_len > max_body {
        return Err(FrameError::PayloadTooLarge { method, path });
    }

    let close = connection_should_close(&version, &headers);
    let body_start = header_end + 4;
    let need = body_start + body_len;
    while buf.len() < need {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| FrameError::Bad(e.to_string()))?;
        if n == 0 {
            return Err(FrameError::Bad("incomplete HTTP body".into()));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = buf[body_start..need].to_vec();
    *pending = buf[need..].to_vec();

    Ok(FramedRequest {
        req: RequestData {
            method,
            path,
            query,
            headers,
            body: String::from_utf8_lossy(&body).into_owned(),
            ..RequestData::default()
        },
        close,
    })
}

/// Dispatch one request against the document root (script or static).
pub fn dispatch_request(docroot: &Path, req: &RequestData) -> FinalResponse {
    dispatch_request_with(docroot, req, &ProcessConfig::default())
}

/// Like [`dispatch_request`] with explicit process configuration.
pub fn dispatch_request_with(
    docroot: &Path,
    req: &RequestData,
    process: &ProcessConfig,
) -> FinalResponse {
    let tmpl = process
        .template_dir
        .clone()
        .unwrap_or_else(|| default_template_dir(docroot));
    let resp = match resolve_route(docroot, &req.path) {
        Route::Error(e) => e,
        Route::Static { path, content_type } => {
            if crate::template::path_is_under_template_dir(&path, &tmpl) {
                host_error_page(404, "Not Found")
            } else {
                serve_static(&path, &content_type)
            }
        }
        Route::Script(path) => {
            if crate::template::path_is_under_template_dir(&path, &tmpl) {
                host_error_page(404, "Not Found")
            } else {
                match run_script_path_sandboxed_with_templates(
                    &path,
                    req,
                    &process.body_limits,
                    Some(&process.session),
                    &process.sandbox,
                    Some(tmpl.as_path()),
                ) {
                    Ok((resp, err)) => {
                        if err.is_some() {
                            if resp.status == 500 {
                                resp
                            } else {
                                host_error_page(500, "Internal Server Error")
                            }
                        } else {
                            resp
                        }
                    }
                    Err(_) => host_error_page(500, "Internal Server Error"),
                }
            }
        }
    };
    apply_head_method(&req.method, resp)
}

fn write_http_response(
    stream: &mut TcpStream,
    resp: &FinalResponse,
    close: bool,
) -> Result<(), String> {
    let reason = match resp.status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "OK",
    };
    let mut out = format!("HTTP/1.1 {} {}\r\n", resp.status, reason);
    let content_length = resp
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| resp.body.len().to_string());
    out.push_str(&format!("Content-Length: {content_length}\r\n"));
    if close {
        out.push_str("Connection: close\r\n");
    }
    for (k, v) in &resp.headers {
        if k.eq_ignore_ascii_case("content-length") {
            continue;
        }
        out.push_str(k);
        out.push_str(": ");
        out.push_str(v);
        out.push_str("\r\n");
    }
    for cookie in &resp.set_cookies {
        out.push_str("Set-Cookie: ");
        out.push_str(cookie);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    stream
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(&resp.body).map_err(|e| e.to_string())?;
    Ok(())
}

/// CLI argument parse result for the host binary.
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub listen: Option<SocketAddr>,
    pub fastcgi: Option<String>,
    pub docroot: PathBuf,
    pub process: ProcessConfig,
}

pub fn parse_cli(args: &[String]) -> Result<CliArgs, String> {
    let mut listen = None;
    let mut fastcgi = None;
    let mut docroot = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut process = ProcessConfig::default();
    let mut i = 1usize; // skip argv[0]
    while i < args.len() {
        match args[i].as_str() {
            "-S" => {
                i += 1;
                let addr = args.get(i).ok_or("-S requires addr:port")?;
                listen = Some(parse_listen_addr(addr)?);
            }
            "-F" => {
                i += 1;
                let addr = args
                    .get(i)
                    .ok_or("-F requires addr:port or an absolute socket path")?;
                fastcgi = Some(addr.clone());
            }
            "-t" => {
                i += 1;
                let dir = args.get(i).ok_or("-t requires a directory")?;
                docroot = PathBuf::from(dir);
            }
            "--gas" => {
                i += 1;
                let raw = args.get(i).ok_or("--gas requires a positive integer")?;
                let n: u64 = raw
                    .parse()
                    .map_err(|_| format!("invalid --gas value: {raw}"))?;
                if n < 1 {
                    return Err("--gas must be >= 1".into());
                }
                process.sandbox.gas_limit = n;
            }
            "--mem" => {
                i += 1;
                let raw = args.get(i).ok_or("--mem requires a positive integer")?;
                let n: usize = raw
                    .parse()
                    .map_err(|_| format!("invalid --mem value: {raw}"))?;
                if n < 1 {
                    return Err("--mem must be >= 1".into());
                }
                process.sandbox.mem_limit_bytes = n;
            }
            "--allow-sys-env" => {
                process.sandbox.capabilities |= ELARA_CAP_SYS_ENV;
            }
            "--max-body" => {
                i += 1;
                let raw = args
                    .get(i)
                    .ok_or("--max-body requires a non-negative integer")?;
                let n: usize = raw
                    .parse()
                    .map_err(|_| format!("invalid --max-body value: {raw}"))?;
                process.body_limits.max_body_bytes = n;
            }
            "--session-dir" => {
                i += 1;
                let dir = args.get(i).ok_or("--session-dir requires a path")?;
                process.session.store_dir = PathBuf::from(dir);
            }
            "--session-secure" => {
                process.session.secure = true;
            }
            "--session-samesite" => {
                i += 1;
                let raw = args
                    .get(i)
                    .ok_or("--session-samesite requires Lax|Strict|None")?;
                match raw.as_str() {
                    "Lax" | "Strict" | "None" => process.session.same_site = raw.clone(),
                    _ => {
                        return Err(format!(
                            "invalid --session-samesite value: {raw} (expected Lax, Strict, or None)"
                        ));
                    }
                }
            }
            "--template-dir" => {
                i += 1;
                let dir = args.get(i).ok_or("--template-dir requires a path")?;
                process.template_dir = Some(PathBuf::from(dir));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    if listen.is_some() && fastcgi.is_some() {
        return Err("-S and -F cannot be used together".into());
    }
    Ok(CliArgs {
        listen,
        fastcgi,
        docroot,
        process,
    })
}
