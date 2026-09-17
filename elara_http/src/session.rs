//! File-backed session store (MVP).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

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
