//! `multipart/form-data` parser writing file parts to a temp directory.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::body::{BodyError, BodyLimits};
use crate::script_api::{RequestData, UploadedFile};

/// Parse multipart body into `form` + `files`, writing uploads under `limits.upload_dir`.
pub fn parse_multipart(request: &mut RequestData, limits: &BodyLimits) -> Result<(), BodyError> {
    let boundary = boundary_from_content_type(
        request
            .headers
            .get("content-type")
            .map(String::as_str)
            .unwrap_or(""),
    )
    .ok_or_else(|| BodyError::InvalidMultipart("missing boundary".into()))?;

    fs::create_dir_all(&limits.upload_dir).map_err(|e| BodyError::Io(e.to_string()))?;

    let raw = request.body.as_bytes();
    let delim = format!("--{boundary}");
    let delim_bytes = delim.as_bytes();

    // Optional leading preamble before first delimiter.
    let mut pos = find_subslice(raw, delim_bytes)
        .ok_or_else(|| BodyError::InvalidMultipart("boundary not found".into()))?;

    loop {
        if !raw[pos..].starts_with(delim_bytes) {
            return Err(BodyError::InvalidMultipart("expected boundary".into()));
        }
        pos += delim_bytes.len();
        if raw.get(pos..pos + 2) == Some(b"--") {
            break; // closing delimiter
        }
        if raw.get(pos..pos + 2) == Some(b"\r\n") {
            pos += 2;
        } else if raw.get(pos..pos + 1) == Some(b"\n") {
            pos += 1;
        } else {
            return Err(BodyError::InvalidMultipart(
                "expected CRLF after boundary".into(),
            ));
        }

        let header_end = find_headers_end(&raw[pos..])
            .ok_or_else(|| BodyError::InvalidMultipart("unterminated part headers".into()))?;
        let headers_blob = &raw[pos..pos + header_end];
        pos += header_end;
        // Skip blank line after headers (\r\n\r\n or \n\n already included in header_end).

        let next_bound = find_subslice(&raw[pos..], delim_bytes)
            .ok_or_else(|| BodyError::InvalidMultipart("part not terminated".into()))?;
        let mut body = &raw[pos..pos + next_bound];
        // Trim trailing CRLF before next boundary.
        if body.ends_with(b"\r\n") {
            body = &body[..body.len() - 2];
        } else if body.ends_with(b"\n") {
            body = &body[..body.len() - 1];
        }
        pos += next_bound;

        let (name, filename, part_ct) = parse_part_headers(headers_blob)?;
        if let Some(filename) = filename {
            if body.len() > limits.max_upload_bytes {
                return Err(BodyError::TooLarge {
                    limit: limits.max_upload_bytes,
                    got: body.len(),
                });
            }
            let path = write_upload(&limits.upload_dir, &filename, body)?;
            request.files.push(UploadedFile {
                name,
                filename: sanitize_filename(&filename),
                content_type: part_ct.unwrap_or_default(),
                path: path.to_string_lossy().into_owned(),
                size: body.len() as u64,
            });
        } else {
            let value = String::from_utf8_lossy(body).into_owned();
            request.form.insert(name, value);
        }
    }
    Ok(())
}

/// Remove temp files listed on `request.files`.
pub fn cleanup_uploads(request: &RequestData) {
    for f in &request.files {
        let _ = fs::remove_file(&f.path);
    }
}

fn boundary_from_content_type(ct: &str) -> Option<String> {
    for part in ct.split(';').skip(1) {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("boundary=") {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn find_headers_end(data: &[u8]) -> Option<usize> {
    data.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
        .or_else(|| data.windows(2).position(|w| w == b"\n\n").map(|i| i + 2))
}

fn parse_part_headers(
    headers: &[u8],
) -> Result<(String, Option<String>, Option<String>), BodyError> {
    let text = String::from_utf8_lossy(headers);
    let mut name = None;
    let mut filename = None;
    let mut content_type = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("content-disposition:") {
            for seg in line.split(';').skip(1) {
                let seg = seg.trim();
                if let Some(v) = seg.strip_prefix("name=") {
                    name = Some(v.trim_matches('"').to_string());
                } else if let Some(v) = seg.strip_prefix("filename=") {
                    filename = Some(v.trim_matches('"').to_string());
                }
            }
        } else if lower.starts_with("content-type:") {
            content_type = Some(
                line.split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default(),
            );
        }
    }
    let name = name.ok_or_else(|| BodyError::InvalidMultipart("part missing name".into()))?;
    Ok((name, filename, content_type))
}

fn sanitize_filename(name: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_upload(dir: &str, filename: &str, data: &[u8]) -> Result<PathBuf, BodyError> {
    let safe = sanitize_filename(filename);
    let mut path = PathBuf::from(dir);
    let unique = format!(
        "up-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        safe
    );
    path.push(unique);
    let mut f = fs::File::create(&path).map_err(|e| BodyError::Io(e.to_string()))?;
    f.write_all(data)
        .map_err(|e| BodyError::Io(e.to_string()))?;
    Ok(path)
}
