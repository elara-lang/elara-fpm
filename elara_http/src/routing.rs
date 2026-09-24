//! Document-root path resolution and static/script routing.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::helpers::host_error_page;
use crate::response::FinalResponse;

/// Outcome of resolving a URL path under a document root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Execute this `.ela` / `.elac` script.
    Script(PathBuf),
    /// Serve this regular file as static content.
    Static { path: PathBuf, content_type: String },
    /// Host-generated error (404/403/400).
    Error(FinalResponse),
}

/// Resolve `url_path` under `docroot` without escaping the root.
pub fn resolve_under_root(docroot: &Path, url_path: &str) -> Result<PathBuf, FinalResponse> {
    let root =
        fs::canonicalize(docroot).map_err(|_| host_error_page(500, "Internal Server Error"))?;
    let relative = url_path_to_relative(url_path)?;
    let joined = root.join(&relative);
    // If the target does not exist yet, canonicalize parents + push file name.
    let resolved = if joined.exists() {
        fs::canonicalize(&joined).map_err(|_| host_error_page(404, "Not Found"))?
    } else {
        // Walk existing prefix; reject `..` already stripped in relative.
        let mut cur = root.clone();
        for comp in Path::new(&relative).components() {
            match comp {
                Component::Normal(s) => cur.push(s),
                Component::CurDir => {}
                _ => return Err(host_error_page(400, "Bad Request")),
            }
        }
        cur
    };
    if !resolved.starts_with(&root) {
        return Err(host_error_page(404, "Not Found"));
    }
    Ok(resolved)
}

fn url_path_to_relative(url_path: &str) -> Result<PathBuf, FinalResponse> {
    let path = url_path.split('?').next().unwrap_or(url_path);
    let path = if let Some(stripped) = path.strip_prefix('/') {
        stripped
    } else {
        path
    };
    if path.is_empty() {
        return Ok(PathBuf::new());
    }
    let mut out = PathBuf::new();
    for comp in Path::new(path).components() {
        match comp {
            Component::Normal(s) => {
                let name = s.to_string_lossy();
                if name.starts_with('.') {
                    return Err(host_error_page(404, "Not Found"));
                }
                out.push(s);
            }
            Component::CurDir => {}
            Component::ParentDir => return Err(host_error_page(400, "Bad Request")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(host_error_page(400, "Bad Request"));
            }
        }
    }
    Ok(out)
}

/// Map a file extension to a Content-Type.
pub fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn is_script_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ela") | Some("elac")
    )
}

/// Full route resolution including index scripts and static files.
pub fn resolve_route(docroot: &Path, url_path: &str) -> Route {
    let path = match resolve_under_root(docroot, url_path) {
        Ok(p) => p,
        Err(e) => return Route::Error(e),
    };

    if path.is_dir() {
        for name in ["index.ela", "index.elac"] {
            let idx = path.join(name);
            if idx.is_file() {
                return Route::Script(idx);
            }
        }
        return Route::Error(host_error_page(404, "Not Found"));
    }

    if !path.is_file() {
        return Route::Error(host_error_page(404, "Not Found"));
    }

    if is_script_path(&path) {
        return Route::Script(path);
    }

    Route::Static {
        content_type: content_type_for(&path).to_string(),
        path,
    }
}

/// Read a static file into a finalized response.
pub fn serve_static(path: &Path, content_type: &str) -> FinalResponse {
    match fs::read(path) {
        Ok(body) => {
            let mut headers = std::collections::BTreeMap::new();
            headers.insert("Content-Type".into(), content_type.to_string());
            FinalResponse {
                status: 200,
                headers,
                set_cookies: Vec::new(),
                body,
            }
        }
        Err(_) => host_error_page(404, "Not Found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_dir_escape() {
        let dir = std::env::temp_dir().join(format!("elara-route-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let err = resolve_under_root(&dir, "/../../etc/passwd").unwrap_err();
        assert_eq!(err.status, 400);
    }

    #[test]
    fn confines_existing_file() {
        let dir = std::env::temp_dir().join(format!("elara-route2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ok.txt");
        fs::write(&file, b"hi").unwrap();
        let resolved = resolve_under_root(&dir, "/ok.txt").unwrap();
        assert_eq!(resolved, fs::canonicalize(&file).unwrap());
    }
}
