//! Cleartext HTTP/1.1 development server (`-S`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::body::BodyLimits;
use crate::helpers::host_error_page;
use crate::response::FinalResponse;
use crate::routing::{Route, load_script_source, resolve_route, serve_static};
use crate::script_api::{RequestData, run_script_with_limits};

/// Configuration for the built-in development server.
#[derive(Debug, Clone)]
pub struct DevServerConfig {
    pub listen: SocketAddr,
    pub docroot: PathBuf,
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
    eprintln!("elara-http development server started at http://{actual}/");
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
                if let Err(e) = handle_connection(stream, &cfg.docroot) {
                    eprintln!("request error: {e}");
                }
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, docroot: &Path) -> Result<(), String> {
    let mut buf = vec![0u8; 64 * 1024];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let raw = String::from_utf8_lossy(&buf[..n]);
    let (req, _) = parse_http_request(&raw)?;
    let response = dispatch_request(docroot, &req);
    write_http_response(&mut stream, &response)?;
    let _ = stream.flush();
    Ok(())
}

/// Dispatch one request against the document root (script or static).
pub fn dispatch_request(docroot: &Path, req: &RequestData) -> FinalResponse {
    match resolve_route(docroot, &req.path) {
        Route::Error(e) => e,
        Route::Static { path, content_type } => serve_static(&path, &content_type),
        Route::Script(path) => match load_script_source(&path) {
            Err(e) => e,
            Ok(source) => match run_script_with_limits(req, &source, &BodyLimits::default()) {
                Ok((resp, err)) => {
                    if err.is_some() {
                        // run_script already maps failures to a 500 page when Err in call;
                        // if Some(err) with 500 page, return it; else keep resp.
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
            },
        },
    }
}

fn parse_http_request(raw: &str) -> Result<(RequestData, usize), String> {
    let header_end = raw
        .find("\r\n\r\n")
        .ok_or_else(|| "incomplete HTTP headers".to_string())?;
    let head = &raw[..header_end];
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or("empty request")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_string();
    let target = parts.next().ok_or("missing target")?;
    let _version = parts.next().ok_or("missing version")?;

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

    let body_start = header_end + 4;
    let mut body = String::new();
    if let Some(cl) = headers.get("content-length") {
        let len: usize = cl.parse().unwrap_or(0);
        let end = (body_start + len).min(raw.len());
        body = raw[body_start..end].to_string();
    }

    Ok((
        RequestData {
            method,
            path,
            query,
            headers,
            body,
            ..RequestData::default()
        },
        body_start,
    ))
}

fn write_http_response(stream: &mut TcpStream, resp: &FinalResponse) -> Result<(), String> {
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
    out.push_str(&format!("Content-Length: {}\r\n", resp.body.len()));
    out.push_str("Connection: close\r\n");
    for (k, v) in &resp.headers {
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
    pub docroot: PathBuf,
}

pub fn parse_cli(args: &[String]) -> Result<CliArgs, String> {
    let mut listen = None;
    let mut docroot = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut i = 1usize; // skip argv[0]
    while i < args.len() {
        match args[i].as_str() {
            "-S" => {
                i += 1;
                let addr = args.get(i).ok_or("-S requires addr:port")?;
                listen = Some(parse_listen_addr(addr)?);
            }
            "-t" => {
                i += 1;
                let dir = args.get(i).ok_or("-t requires a directory")?;
                docroot = PathBuf::from(dir);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(CliArgs { listen, docroot })
}
