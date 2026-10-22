//! Minimal sequential FastCGI worker.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use crate::cgi::{format_cgi_response, handle_cgi_request_with};
use crate::process::ProcessConfig;

const FCGI_BEGIN_REQUEST: u8 = 1;
const FCGI_ABORT_REQUEST: u8 = 2;
const FCGI_END_REQUEST: u8 = 3;
const FCGI_PARAMS: u8 = 4;
const FCGI_STDIN: u8 = 5;
const FCGI_STDOUT: u8 = 6;
const FCGI_STDERR: u8 = 7;

#[derive(Debug, Clone)]
pub struct FastCgiConfig {
    pub listen: String,
    pub docroot: PathBuf,
    pub process: ProcessConfig,
}

/// Serve FastCGI requests sequentially until the listener fails.
pub fn run_fastcgi_worker(cfg: &FastCgiConfig) -> Result<(), String> {
    if cfg.listen.starts_with('/') {
        run_fastcgi_unix(cfg)
    } else {
        run_fastcgi_tcp(cfg)
    }
}

fn run_fastcgi_tcp(cfg: &FastCgiConfig) -> Result<(), String> {
    let listener =
        TcpListener::bind(&cfg.listen).map_err(|e| format!("bind {}: {e}", cfg.listen))?;
    eprintln!(
        "elara-fpm FastCGI listening on {} (docroot {})",
        cfg.listen,
        cfg.docroot.display()
    );
    for conn in listener.incoming() {
        match conn {
            Ok(mut stream) => {
                if let Err(e) =
                    handle_fastcgi_connection_with(&mut stream, &cfg.docroot, &cfg.process)
                {
                    eprintln!("fastcgi connection error: {e}");
                }
            }
            Err(e) => eprintln!("fastcgi accept error: {e}"),
        }
    }
    Ok(())
}

fn prepare_unix_listen_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.file_type().is_socket() {
        fs::remove_file(path).map_err(|e| format!("replace socket {}: {e}", path.display()))?;
        Ok(())
    } else {
        Err(format!("{} exists and is not a socket", path.display()))
    }
}

fn run_fastcgi_unix(cfg: &FastCgiConfig) -> Result<(), String> {
    let path = Path::new(&cfg.listen);
    prepare_unix_listen_path(path)?;
    let listener = UnixListener::bind(path).map_err(|e| format!("bind {}: {e}", cfg.listen))?;
    eprintln!(
        "elara-fpm FastCGI listening on {} (docroot {})",
        cfg.listen,
        cfg.docroot.display()
    );
    for conn in listener.incoming() {
        match conn {
            Ok(mut stream) => {
                if let Err(e) =
                    handle_fastcgi_connection_with(&mut stream, &cfg.docroot, &cfg.process)
                {
                    eprintln!("fastcgi connection error: {e}");
                }
            }
            Err(e) => eprintln!("fastcgi accept error: {e}"),
        }
    }
    Ok(())
}

/// Handle one FastCGI connection (TCP or Unix).
pub fn handle_fastcgi_connection(
    stream: &mut (impl Read + Write),
    docroot: &Path,
) -> Result<(), String> {
    handle_fastcgi_connection_with(stream, docroot, &ProcessConfig::default())
}

/// Like [`handle_fastcgi_connection`] with process configuration.
pub fn handle_fastcgi_connection_with(
    stream: &mut (impl Read + Write),
    docroot: &Path,
    process: &ProcessConfig,
) -> Result<(), String> {
    let mut params = BTreeMap::new();
    let mut stdin = Vec::new();
    let mut request_id: u16 = 1;
    while let Some((rtype, rid, content)) = read_record(stream)? {
        request_id = rid;
        match rtype {
            FCGI_BEGIN_REQUEST => {}
            FCGI_PARAMS => {
                if !content.is_empty() {
                    parse_name_value_pairs(&content, &mut params)?;
                }
            }
            FCGI_STDIN => {
                if content.is_empty() {
                    break;
                }
                stdin.extend_from_slice(&content);
            }
            FCGI_ABORT_REQUEST => return Ok(()),
            _ => {}
        }
    }
    let resp = handle_cgi_request_with(docroot, &params, &stdin, process);
    let payload = format_cgi_response(&resp);
    write_stream_records(stream, FCGI_STDOUT, request_id, &payload)?;
    write_record(stream, FCGI_STDOUT, request_id, &[])?; // end stdout
    write_end_request(stream, request_id)?;
    Ok(())
}

fn read_record(stream: &mut dyn Read) -> Result<Option<(u8, u16, Vec<u8>)>, String> {
    let mut header = [0u8; 8];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.to_string()),
    }
    let version = header[0];
    if version != 1 {
        return Err(format!("unsupported FastCGI version {version}"));
    }
    let rtype = header[1];
    let rid = u16::from_be_bytes([header[2], header[3]]);
    let clen = u16::from_be_bytes([header[4], header[5]]) as usize;
    let plen = header[6] as usize;
    let mut content = vec![0u8; clen];
    if clen > 0 {
        stream.read_exact(&mut content).map_err(|e| e.to_string())?;
    }
    if plen > 0 {
        let mut pad = vec![0u8; plen];
        stream.read_exact(&mut pad).map_err(|e| e.to_string())?;
    }
    Ok(Some((rtype, rid, content)))
}

fn write_record(stream: &mut dyn Write, rtype: u8, rid: u16, content: &[u8]) -> Result<(), String> {
    let clen = content.len();
    if clen > u16::MAX as usize {
        return Err("FastCGI record too large".into());
    }
    let mut header = [0u8; 8];
    header[0] = 1;
    header[1] = rtype;
    header[2..4].copy_from_slice(&rid.to_be_bytes());
    header[4..6].copy_from_slice(&(clen as u16).to_be_bytes());
    header[6] = 0;
    header[7] = 0;
    stream.write_all(&header).map_err(|e| e.to_string())?;
    if clen > 0 {
        stream.write_all(content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_stream_records(
    stream: &mut dyn Write,
    rtype: u8,
    rid: u16,
    payload: &[u8],
) -> Result<(), String> {
    const CHUNK: usize = 32 * 1024;
    for chunk in payload.chunks(CHUNK) {
        write_record(stream, rtype, rid, chunk)?;
    }
    Ok(())
}

fn write_end_request(stream: &mut dyn Write, rid: u16) -> Result<(), String> {
    // appStatus = 0, protocolStatus = FCGI_REQUEST_COMPLETE (0)
    write_record(stream, FCGI_END_REQUEST, rid, &[0u8; 8])
}

fn parse_name_value_pairs(buf: &[u8], out: &mut BTreeMap<String, String>) -> Result<(), String> {
    let mut i = 0;
    while i < buf.len() {
        let (nlen, ni) = read_fcgi_len(buf, i)?;
        i = ni;
        let (vlen, vi) = read_fcgi_len(buf, i)?;
        i = vi;
        if i + nlen + vlen > buf.len() {
            return Err("truncated FastCGI name-value pair".into());
        }
        let name = String::from_utf8_lossy(&buf[i..i + nlen]).into_owned();
        i += nlen;
        let value = String::from_utf8_lossy(&buf[i..i + vlen]).into_owned();
        i += vlen;
        out.insert(name, value);
    }
    Ok(())
}

fn read_fcgi_len(buf: &[u8], i: usize) -> Result<(usize, usize), String> {
    if i >= buf.len() {
        return Err("truncated FastCGI length".into());
    }
    let b0 = buf[i];
    if b0 >> 7 == 0 {
        Ok((b0 as usize, i + 1))
    } else {
        if i + 4 > buf.len() {
            return Err("truncated FastCGI length".into());
        }
        let n = (((b0 as usize) & 0x7f) << 24)
            | ((buf[i + 1] as usize) << 16)
            | ((buf[i + 2] as usize) << 8)
            | (buf[i + 3] as usize);
        Ok((n, i + 4))
    }
}

/// Build a minimal FastCGI request payload for tests.
pub fn encode_test_request(params: &BTreeMap<String, String>, stdin: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let rid = 1u16;
    // BEGIN_REQUEST body: role=responder(1), flags=0
    let mut begin = [0u8; 8];
    begin[1] = 1; // role responder
    append_record(&mut out, FCGI_BEGIN_REQUEST, rid, &begin);
    let mut nv = Vec::new();
    for (k, v) in params {
        write_nv(&mut nv, k.as_bytes(), v.as_bytes());
    }
    append_record(&mut out, FCGI_PARAMS, rid, &nv);
    append_record(&mut out, FCGI_PARAMS, rid, &[]);
    if !stdin.is_empty() {
        append_record(&mut out, FCGI_STDIN, rid, stdin);
    }
    append_record(&mut out, FCGI_STDIN, rid, &[]);
    out
}

fn write_nv(buf: &mut Vec<u8>, name: &[u8], value: &[u8]) {
    push_len(buf, name.len());
    push_len(buf, value.len());
    buf.extend_from_slice(name);
    buf.extend_from_slice(value);
}

fn push_len(buf: &mut Vec<u8>, n: usize) {
    if n < 128 {
        buf.push(n as u8);
    } else {
        buf.push(((n >> 24) as u8) | 0x80);
        buf.push((n >> 16) as u8);
        buf.push((n >> 8) as u8);
        buf.push(n as u8);
    }
}

fn append_record(out: &mut Vec<u8>, rtype: u8, rid: u16, content: &[u8]) {
    let mut header = [0u8; 8];
    header[0] = 1;
    header[1] = rtype;
    header[2..4].copy_from_slice(&rid.to_be_bytes());
    header[4..6].copy_from_slice(&(content.len() as u16).to_be_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(content);
}

/// Parse STDOUT stream content from a FastCGI response buffer.
pub fn extract_stdout(buf: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let mut stdout = Vec::new();
    while i + 8 <= buf.len() {
        let rtype = buf[i + 1];
        let clen = u16::from_be_bytes([buf[i + 4], buf[i + 5]]) as usize;
        let plen = buf[i + 6] as usize;
        i += 8;
        if i + clen + plen > buf.len() {
            break;
        }
        if rtype == FCGI_STDOUT {
            stdout.extend_from_slice(&buf[i..i + clen]);
        }
        i += clen + plen;
        let _ = (FCGI_STDERR, FCGI_END_REQUEST);
    }
    stdout
}
