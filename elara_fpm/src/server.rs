//! Cleartext HTTP/1.1 development server (`-S`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::body::BodyLimits;
use crate::helpers::host_error_page;
use crate::response::FinalResponse;
use crate::routing::{Route, load_script_source, resolve_route, serve_static};
use crate::script_api::{RequestData, run_script_with_session};
use crate::session::SessionConfig;

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
                if let Err(e) = handle_connection(stream, &cfg.docroot) {
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
    handle_connection(stream, docroot)
}

enum FrameError {
    Eof,
    PayloadTooLarge,
    NotImplemented,
    Bad(String),
}

struct FramedRequest {
    req: RequestData,
    close: bool,
}

fn handle_connection(mut stream: TcpStream, docroot: &Path) -> Result<(), String> {
    let limits = BodyLimits::default();
    let mut pending = Vec::new();
    loop {
        let framed = match read_http_request(&mut stream, &mut pending, limits.max_body_bytes) {
            Ok(f) => f,
            Err(FrameError::Eof) => return Ok(()),
            Err(FrameError::PayloadTooLarge) => {
                write_http_response(
                    &mut stream,
                    &host_error_page(413, "Payload Too Large"),
                    true,
                )?;
                let _ = stream.flush();
                return Ok(());
            }
            Err(FrameError::NotImplemented) => {
                write_http_response(&mut stream, &host_error_page(501, "Not Implemented"), true)?;
                let _ = stream.flush();
                return Ok(());
            }
            Err(FrameError::Bad(e)) => return Err(e),
        };
        let response = dispatch_request(docroot, &framed.req);
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
        return Err(FrameError::NotImplemented);
    }

    let body_len = match headers.get("content-length") {
        Some(cl) => cl
            .parse::<usize>()
            .map_err(|_| FrameError::Bad("invalid Content-Length".into()))?,
        None => 0,
    };
    if body_len > max_body {
        return Err(FrameError::PayloadTooLarge);
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
    match resolve_route(docroot, &req.path) {
        Route::Error(e) => e,
        Route::Static { path, content_type } => serve_static(&path, &content_type),
        Route::Script(path) => match load_script_source(&path) {
            Err(e) => e,
            Ok(source) => {
                let session_cfg = SessionConfig::default();
                match run_script_with_session(
                    req,
                    &source,
                    &BodyLimits::default(),
                    Some(&session_cfg),
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
        },
    }
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
    out.push_str(&format!("Content-Length: {}\r\n", resp.body.len()));
    if close {
        out.push_str("Connection: close\r\n");
    }
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
    pub fastcgi: Option<String>,
    pub docroot: PathBuf,
}

pub fn parse_cli(args: &[String]) -> Result<CliArgs, String> {
    let mut listen = None;
    let mut fastcgi = None;
    let mut docroot = std::env::current_dir().map_err(|e| e.to_string())?;
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
    })
}
