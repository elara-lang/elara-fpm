//! File-backed session store.

#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use crate::cookies::CookieOptions;
use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_context_userdata,
    elara_return, elara_val_nil, value_as_string,
};

/// Operator configuration for sessions.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub store_dir: PathBuf,
    pub cookie_name: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: String,
    pub max_age: Option<i64>,
}

/// Default file store: `$TMPDIR/elara/.sessions` (typically `/tmp/elara/.sessions`).
pub fn default_session_store_dir() -> PathBuf {
    std::env::temp_dir().join("elara").join(".sessions")
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            store_dir: default_session_store_dir(),
            cookie_name: "ELARASESSID".into(),
            path: "/".into(),
            http_only: true,
            secure: false,
            same_site: "Lax".into(),
            max_age: None,
        }
    }
}

/// In-memory session for one request.
///
/// Values are stored as compact JSON text (null / bool / int / string / array / object).
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub data: BTreeMap<String, String>,
    pub is_new: bool,
    pub destroyed: bool,
}

impl Session {
    pub fn empty(id: String) -> Self {
        Self {
            id,
            data: BTreeMap::new(),
            is_new: true,
            destroyed: false,
        }
    }
}

/// File store: one file per session id under `store_dir`.
#[derive(Debug, Clone)]
pub struct FileSessionStore {
    dir: PathBuf,
}

impl FileSessionStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self { dir })
    }

    pub fn generate_id() -> Result<String, String> {
        let mut buf = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut buf))
            .map_err(|e| e.to_string())?;
        Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn path_for(&self, id: &str) -> Result<PathBuf, String> {
        if !is_safe_session_id(id) {
            return Err("invalid session id".into());
        }
        Ok(self.dir.join(id))
    }

    /// Load an existing session, or `None` if missing/corrupt.
    pub fn load(&self, id: &str) -> Option<Session> {
        let path = self.path_for(id).ok()?;
        let raw = fs::read_to_string(path).ok()?;
        let data = decode_session_file(&raw)?;
        Some(Session {
            id: id.to_string(),
            data,
            is_new: false,
            destroyed: false,
        })
    }

    pub fn save(&self, session: &Session) -> Result<(), String> {
        if session.destroyed {
            return self.destroy(&session.id);
        }
        let path = self.path_for(&session.id)?;
        let tmp = path.with_extension("tmp");
        let body = encode_session_file(&session.data);
        {
            let mut f = File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn destroy(&self, id: &str) -> Result<(), String> {
        let path = self.path_for(id)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Load by cookie id or create a new empty session.
    pub fn load_or_create(&self, cookie_id: Option<&str>) -> Result<Session, String> {
        if let Some(id) = cookie_id
            && let Some(s) = self.load(id)
        {
            return Ok(s);
        }
        let id = Self::generate_id()?;
        Ok(Session::empty(id))
    }
}

fn is_safe_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn encode_session_file(data: &BTreeMap<String, String>) -> String {
    let mut out = String::from("v2\n");
    for (k, v) in data {
        out.push_str(&percent_encode(k));
        out.push('=');
        out.push_str(&percent_encode(v));
        out.push('\n');
    }
    out
}

fn decode_session_file(raw: &str) -> Option<BTreeMap<String, String>> {
    let mut lines = raw.lines();
    let ver = lines.next()?.trim();
    let mut data = BTreeMap::new();
    match ver {
        "v2" => {
            for line in lines {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let (k, v) = line.split_once('=')?;
                data.insert(percent_decode(k), percent_decode(v));
            }
            Some(data)
        }
        "v1" => {
            for line in lines {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let (k, v) = line.split_once('=')?;
                let plain = percent_decode(v);
                data.insert(percent_decode(k), json_quote(&plain));
            }
            Some(data)
        }
        _ => None,
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = from_hex(bytes[i + 1]);
            let lo = from_hex(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn json_quote(s: &str) -> String {
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

/// Parse a JSON object into key → raw JSON value text.
pub fn json_object_to_raw_map(input: &str) -> Result<BTreeMap<String, String>, String> {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    fn skip_ws(bytes: &[u8], i: &mut usize) {
        while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
            *i += 1;
        }
    }
    skip_ws(bytes, &mut i);
    if i >= bytes.len() || bytes[i] != b'{' {
        return Err("expected JSON object".into());
    }
    i += 1;
    let mut out = BTreeMap::new();
    loop {
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b'}' {
            break;
        }
        if i >= bytes.len() || bytes[i] != b'"' {
            return Err("expected string key".into());
        }
        let key = parse_json_string(bytes, &mut i)?;
        skip_ws(bytes, &mut i);
        if i >= bytes.len() || bytes[i] != b':' {
            return Err("expected :".into());
        }
        i += 1;
        skip_ws(bytes, &mut i);
        let start = i;
        skip_json_value(bytes, &mut i)?;
        let raw = std::str::from_utf8(&bytes[start..i])
            .map_err(|_| "invalid utf8 in json value")?
            .to_string();
        out.insert(key, raw);
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
            continue;
        }
        if i < bytes.len() && bytes[i] == b'}' {
            break;
        }
        return Err("expected , or }".into());
    }
    Ok(out)
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, String> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err("expected string".into());
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        match bytes[*i] {
            b'"' => {
                *i += 1;
                return Ok(out);
            }
            b'\\' => {
                *i += 1;
                if *i >= bytes.len() {
                    return Err("truncated escape".into());
                }
                match bytes[*i] {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => return Err("unicode escapes not supported in session keys".into()),
                    _ => return Err("bad escape".into()),
                }
                *i += 1;
            }
            b => {
                out.push(b as char);
                *i += 1;
            }
        }
    }
    Err("unterminated string".into())
}

fn skip_json_value(bytes: &[u8], i: &mut usize) -> Result<(), String> {
    if *i >= bytes.len() {
        return Err("unexpected end".into());
    }
    match bytes[*i] {
        b'n' => {
            if bytes.get(*i..*i + 4) == Some(b"null") {
                *i += 4;
                Ok(())
            } else {
                Err("bad null".into())
            }
        }
        b't' => {
            if bytes.get(*i..*i + 4) == Some(b"true") {
                *i += 4;
                Ok(())
            } else {
                Err("bad true".into())
            }
        }
        b'f' => {
            if bytes.get(*i..*i + 5) == Some(b"false") {
                *i += 5;
                Ok(())
            } else {
                Err("bad false".into())
            }
        }
        b'"' => {
            parse_json_string(bytes, i)?;
            Ok(())
        }
        b'{' => {
            *i += 1;
            loop {
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < bytes.len() && bytes[*i] == b'}' {
                    *i += 1;
                    return Ok(());
                }
                parse_json_string(bytes, i)?;
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i >= bytes.len() || bytes[*i] != b':' {
                    return Err("expected :".into());
                }
                *i += 1;
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                skip_json_value(bytes, i)?;
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < bytes.len() && bytes[*i] == b',' {
                    *i += 1;
                    continue;
                }
                if *i < bytes.len() && bytes[*i] == b'}' {
                    *i += 1;
                    return Ok(());
                }
                return Err("expected , or }".into());
            }
        }
        b'[' => {
            *i += 1;
            loop {
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < bytes.len() && bytes[*i] == b']' {
                    *i += 1;
                    return Ok(());
                }
                skip_json_value(bytes, i)?;
                while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
                    *i += 1;
                }
                if *i < bytes.len() && bytes[*i] == b',' {
                    *i += 1;
                    continue;
                }
                if *i < bytes.len() && bytes[*i] == b']' {
                    *i += 1;
                    return Ok(());
                }
                return Err("expected , or ]".into());
            }
        }
        b'-' | b'0'..=b'9' => {
            *i += 1;
            while *i < bytes.len()
                && (bytes[*i].is_ascii_digit()
                    || matches!(bytes[*i], b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                *i += 1;
            }
            Ok(())
        }
        _ => Err("unexpected json value".into()),
    }
}

pub fn session_userdata_ptr(session: &mut Session) -> *mut c_void {
    session as *mut Session as *mut c_void
}

unsafe fn session_from_ctx<'a>(ctx: *mut elara_CallContext) -> Option<&'a mut Session> {
    unsafe {
        let ptr = elara_context_userdata(ctx) as *mut Session;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

/// Replace session data from a JSON object produced by the Elara encoder.
pub unsafe extern "C" fn native_session_commit(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let sess = match session_from_ctx(ctx) {
            Some(v) => v,
            None => return elara_Status::ErrRuntime,
        };
        if sess.destroyed || elara_argc(ctx) < 1 {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        let raw = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        match json_object_to_raw_map(&raw) {
            Ok(map) => {
                sess.data = map;
                elara_return(ctx, elara_val_nil());
                elara_Status::Ok
            }
            Err(_) => elara_Status::ErrRuntime,
        }
    }
}

/// Fail a `session.set` that could not be serialized (leaves prior data untouched).
pub unsafe extern "C" fn native_session_reject(
    _s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        if session_from_ctx(ctx).is_none() {
            return elara_Status::ErrRuntime;
        }
        elara_Status::ErrRuntime
    }
}

pub unsafe extern "C" fn native_session_destroy(
    _s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let sess = match session_from_ctx(ctx) {
            Some(v) => v,
            None => return elara_Status::ErrRuntime,
        };
        sess.destroyed = true;
        sess.data.clear();
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
    }
}

/// Build the session id `Set-Cookie` line (or expiry line when destroyed).
pub fn session_set_cookie_line(cfg: &SessionConfig, session: &Session) -> String {
    let opts = CookieOptions {
        path: Some(cfg.path.clone()),
        http_only: cfg.http_only,
        secure: cfg.secure,
        same_site: Some(cfg.same_site.clone()),
        max_age: if session.destroyed {
            Some(0)
        } else {
            cfg.max_age
        },
    };
    let value = if session.destroyed {
        ""
    } else {
        session.id.as_str()
    };
    opts.format_set_cookie(&cfg.cookie_name, value)
}

/// Build an Elara table literal for loaded session JSON values.
pub fn session_data_literal(data: &BTreeMap<String, String>) -> Result<String, String> {
    if data.is_empty() {
        return Ok("{}".into());
    }
    let mut parts = Vec::new();
    for (k, v) in data {
        let expr = crate::body::json_to_elara(v).map_err(|e| e.to_string())?;
        let mut key = String::from("\"");
        for ch in k.chars() {
            match ch {
                '\\' => key.push_str("\\\\"),
                '"' => key.push_str("\\\""),
                '\n' => key.push_str("\\n"),
                '\r' => key.push_str("\\r"),
                '\t' => key.push_str("\\t"),
                c => key.push(c),
            }
        }
        key.push('"');
        parts.push(format!("[{key}] = {expr}"));
    }
    Ok(format!("{{\n  {}\n}}", parts.join(",\n  ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_round_trip_and_destroy() {
        let dir = std::env::temp_dir().join(format!("elara-sess-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = FileSessionStore::open(&dir).unwrap();
        let mut s = store.load_or_create(None).unwrap();
        assert!(s.is_new);
        s.data.insert("k".into(), json_quote("v"));
        store.save(&s).unwrap();
        let loaded = store.load(&s.id).expect("load");
        assert_eq!(loaded.data.get("k").map(String::as_str), Some("\"v\""));
        store.destroy(&s.id).unwrap();
        assert!(store.load(&s.id).is_none());
    }

    #[test]
    fn rejects_unsafe_session_ids() {
        let dir = std::env::temp_dir().join(format!("elara-sess-bad-{}", std::process::id()));
        let store = FileSessionStore::open(&dir).unwrap();
        assert!(store.path_for("../etc/passwd").is_err());
        assert!(store.path_for("abc/def").is_err());
    }

    #[test]
    fn json_object_to_raw_map_extracts_values() {
        let m = json_object_to_raw_map(r#"{"i":42,"b":true,"t":{"x":1}}"#).unwrap();
        assert_eq!(m.get("i").map(String::as_str), Some("42"));
        assert_eq!(m.get("b").map(String::as_str), Some("true"));
        assert_eq!(m.get("t").map(String::as_str), Some(r#"{"x":1}"#));
    }
}
