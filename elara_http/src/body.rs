//! Request body parsers (urlencoded, JSON, multipart) and size limits.

use std::collections::BTreeMap;

use crate::script_api::RequestData;

/// Operator-configurable body/upload limits.
#[derive(Debug, Clone)]
pub struct BodyLimits {
    pub max_body_bytes: usize,
    pub max_upload_bytes: usize,
    pub upload_dir: String,
}

impl Default for BodyLimits {
    fn default() -> Self {
        Self {
            max_body_bytes: 1_048_576,
            max_upload_bytes: 1_048_576,
            upload_dir: std::env::temp_dir()
                .join("elara-http-uploads")
                .to_string_lossy()
                .into_owned(),
        }
    }
}

/// Errors from body parsing / size policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyError {
    TooLarge { limit: usize, got: usize },
    InvalidJson(String),
    InvalidMultipart(String),
    Io(String),
}

impl std::fmt::Display for BodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyError::TooLarge { limit, got } => {
                write!(f, "request body too large ({got} bytes; limit {limit})")
            }
            BodyError::InvalidJson(msg) => write!(f, "invalid JSON body: {msg}"),
            BodyError::InvalidMultipart(msg) => write!(f, "invalid multipart body: {msg}"),
            BodyError::Io(msg) => write!(f, "body I/O error: {msg}"),
        }
    }
}

/// Decode `application/x-www-form-urlencoded` into a map.
/// Duplicate keys: **last value wins**.
pub fn parse_urlencoded(input: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in input.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_val) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        let key = percent_decode(raw_key);
        let val = percent_decode(raw_val);
        out.insert(key, val);
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = hex_nibble(bytes[i + 1]);
                let l = hex_nibble(bytes[i + 2]);
                if let (Some(hi), Some(lo)) = (h, l) {
                    out.push((hi << 4) | lo);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn content_type_mime(headers: &BTreeMap<String, String>) -> String {
    headers
        .get("content-type")
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default()
}

/// Fill structured fields on `request` from `body` / Content-Type (urlencoded only in this step).
pub fn enrich_request(request: &mut RequestData, limits: &BodyLimits) -> Result<(), BodyError> {
    if request.body.len() > limits.max_body_bytes {
        return Err(BodyError::TooLarge {
            limit: limits.max_body_bytes,
            got: request.body.len(),
        });
    }
    let mime = content_type_mime(&request.headers);
    if mime == "application/x-www-form-urlencoded" {
        request.form = parse_urlencoded(&request.body);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoded_last_wins_and_decodes() {
        let m = parse_urlencoded("a=1&b=hello+%57orld&a=2");
        assert_eq!(m.get("a").map(String::as_str), Some("2"));
        assert_eq!(m.get("b").map(String::as_str), Some("hello World"));
    }
}
