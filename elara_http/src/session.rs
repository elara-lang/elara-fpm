//! File-backed session store (MVP).

#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use crate::cookies::CookieOptions;
use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_context_userdata,
    elara_is_nil, elara_return, elara_val_nil, elara_val_string, value_as_string,
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

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            store_dir: std::env::temp_dir().join("elara-http-sessions"),
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
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    /// MVP: string values only (JSON-serializable subset).
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
    let mut out = String::from("v1\n");
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
    if ver != "v1" {
        return None;
    }
    let mut data = BTreeMap::new();
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

pub fn session_userdata_ptr(session: &mut Session) -> *mut c_void {
    session as *mut Session as *mut c_void
}

unsafe fn session_from_ctx<'a>(ctx: *mut elara_CallContext) -> Option<&'a mut Session> {
    unsafe {
        let ptr = elara_context_userdata(ctx) as *mut Session;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

pub unsafe extern "C" fn native_session_get(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let sess = match session_from_ctx(ctx) {
            Some(v) => v,
            None => return elara_Status::ErrRuntime,
        };
        if sess.destroyed || elara_argc(ctx) < 1 {
            return elara_Status::ErrRuntime;
        }
        let key = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        match sess.data.get(&key) {
            Some(v) => {
                let val = elara_val_string(s, v.as_ptr() as *const std::os::raw::c_char, v.len());
                elara_return(ctx, val);
            }
            None => elara_return(ctx, elara_val_nil()),
        }
        elara_Status::Ok
    }
}

pub unsafe extern "C" fn native_session_set(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let sess = match session_from_ctx(ctx) {
            Some(v) => v,
            None => return elara_Status::ErrRuntime,
        };
        if sess.destroyed || elara_argc(ctx) < 2 {
            return elara_Status::ErrRuntime;
        }
        let key = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        let val = elara_arg(ctx, 1);
        if elara_is_nil(val) {
            sess.data.remove(&key);
        } else {
            let value = match value_as_string(s, val) {
                Ok(v) => v,
                Err(_) => return elara_Status::ErrRuntime,
            };
            sess.data.insert(key, value);
        }
        elara_return(ctx, elara_val_nil());
        elara_Status::Ok
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
        s.data.insert("k".into(), "v".into());
        store.save(&s).unwrap();
        let loaded = store.load(&s.id).expect("load");
        assert_eq!(loaded.data.get("k").map(String::as_str), Some("v"));
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
}
