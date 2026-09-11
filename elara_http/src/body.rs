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

/// Fill structured fields on `request` from `body` / Content-Type.
pub fn enrich_request(request: &mut RequestData, limits: &BodyLimits) -> Result<(), BodyError> {
    if request.body.len() > limits.max_body_bytes {
        return Err(BodyError::TooLarge {
            limit: limits.max_body_bytes,
            got: request.body.len(),
        });
    }
    let mime = content_type_mime(&request.headers);
    match mime.as_str() {
        "application/x-www-form-urlencoded" => {
            request.form = parse_urlencoded(&request.body);
        }
        "application/json" => {
            request.json_elara = json_to_elara(&request.body)?;
        }
        _ => {}
    }
    Ok(())
}

/// Convert a JSON document into an Elara expression source fragment for `req.json`.
pub fn json_to_elara(input: &str) -> Result<String, BodyError> {
    let mut p = JsonParser {
        bytes: input.as_bytes(),
        i: 0,
    };
    let expr = p.parse_value()?;
    p.skip_ws();
    if p.i != p.bytes.len() {
        return Err(BodyError::InvalidJson(
            "trailing data after JSON value".into(),
        ));
    }
    Ok(expr)
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    i: usize,
}

impl JsonParser<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.bytes.len() && self.bytes[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.i += 1;
        Some(b)
    }

    fn parse_value(&mut self) -> Result<String, BodyError> {
        self.skip_ws();
        match self.peek() {
            Some(b'n') => self.parse_literal(b"null", "nil"),
            Some(b't') => self.parse_literal(b"true", "true"),
            Some(b'f') => self.parse_literal(b"false", "false"),
            Some(b'"') => self.parse_string().map(|s| elara_quote(&s)),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(b) => Err(BodyError::InvalidJson(format!(
                "unexpected byte {:?} at {}",
                b as char, self.i
            ))),
            None => Err(BodyError::InvalidJson("unexpected end of input".into())),
        }
    }

    fn parse_literal(&mut self, lit: &[u8], elara: &str) -> Result<String, BodyError> {
        for &b in lit {
            if self.bump() != Some(b) {
                return Err(BodyError::InvalidJson(format!(
                    "expected literal {}",
                    std::str::from_utf8(lit).unwrap_or("?")
                )));
            }
        }
        Ok(elara.to_string())
    }

    fn parse_string(&mut self) -> Result<String, BodyError> {
        if self.bump() != Some(b'"') {
            return Err(BodyError::InvalidJson("expected string".into()));
        }
        let mut out = String::new();
        loop {
            match self.bump() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.bump() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'/') => out.push('/'),
                    Some(b'b') => out.push('\u{0008}'),
                    Some(b'f') => out.push('\u{000c}'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let mut code = 0u32;
                        for _ in 0..4 {
                            let h = self
                                .bump()
                                .and_then(hex_nibble)
                                .ok_or_else(|| BodyError::InvalidJson("bad \\u escape".into()))?;
                            code = (code << 4) | u32::from(h);
                        }
                        out.push(
                            char::from_u32(code)
                                .ok_or_else(|| BodyError::InvalidJson("bad unicode".into()))?,
                        );
                    }
                    _ => return Err(BodyError::InvalidJson("bad escape".into())),
                },
                Some(b) => out.push(b as char),
                None => return Err(BodyError::InvalidJson("unterminated string".into())),
            }
        }
    }

    fn parse_number(&mut self) -> Result<String, BodyError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        if self.peek() == Some(b'0') {
            self.i += 1;
        } else if matches!(self.peek(), Some(b'1'..=b'9')) {
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        } else {
            return Err(BodyError::InvalidJson("invalid number".into()));
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(BodyError::InvalidJson("invalid fraction".into()));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(BodyError::InvalidJson("invalid exponent".into()));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.i])
            .map_err(|_| BodyError::InvalidJson("number utf8".into()))?;
        Ok(raw.to_string())
    }

    fn parse_array(&mut self) -> Result<String, BodyError> {
        self.bump(); // [
        self.skip_ws();
        let mut parts = Vec::new();
        if self.peek() != Some(b']') {
            loop {
                parts.push(self.parse_value()?);
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.bump();
                        self.skip_ws();
                    }
                    Some(b']') => break,
                    _ => return Err(BodyError::InvalidJson("expected , or ]".into())),
                }
            }
        }
        self.bump(); // ]
        Ok(format!("{{ {} }}", parts.join(", ")))
    }

    fn parse_object(&mut self) -> Result<String, BodyError> {
        self.bump(); // {
        self.skip_ws();
        let mut parts = Vec::new();
        if self.peek() != Some(b'}') {
            loop {
                self.skip_ws();
                let key = self.parse_string()?;
                self.skip_ws();
                if self.bump() != Some(b':') {
                    return Err(BodyError::InvalidJson("expected :".into()));
                }
                let val = self.parse_value()?;
                parts.push(format!("[{}] = {}", elara_quote(&key), val));
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.bump();
                        self.skip_ws();
                    }
                    Some(b'}') => break,
                    _ => return Err(BodyError::InvalidJson("expected , or }}".into())),
                }
            }
        }
        self.bump(); // }
        Ok(format!("{{\n  {}\n}}", parts.join(",\n  ")))
    }
}

fn elara_quote(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoded_last_wins_and_decodes() {
        let m = parse_urlencoded("a=1&b=hello+%57orld&a=2");
        assert_eq!(m.get("a").map(String::as_str), Some("2"));
        assert_eq!(m.get("b").map(String::as_str), Some("hello World"));
    }

    #[test]
    fn json_object_to_elara() {
        let e = json_to_elara(r#"{"n":1,"ok":true}"#).unwrap();
        assert!(e.contains("[\"n\"] = 1"));
        assert!(e.contains("[\"ok\"] = true"));
    }

    #[test]
    fn json_invalid_errors() {
        assert!(matches!(json_to_elara("{"), Err(BodyError::InvalidJson(_))));
    }
}
