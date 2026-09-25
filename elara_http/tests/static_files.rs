use elara_http::{Route, resolve_route, serve_static};
use std::fs;

#[test]
fn css_file_returns_bytes_and_content_type() {
    let dir = std::env::temp_dir().join(format!("elara-http-st-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("style.css"), b"body{color:red}").unwrap();

    match resolve_route(&dir, "/style.css") {
        Route::Static { path, content_type } => {
            assert!(content_type.starts_with("text/css"));
            let resp = serve_static(&path, &content_type);
            assert_eq!(resp.status, 200);
            assert_eq!(resp.body, b"body{color:red}");
            assert_eq!(
                resp.headers.get("Content-Type").map(String::as_str),
                Some(content_type.as_str())
            );
        }
        other => panic!("expected Static, got {other:?}"),
    }
}

#[test]
fn unknown_extension_is_octet_stream() {
    let dir = std::env::temp_dir().join(format!("elara-http-st2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("blob.xyz"), [0u8, 1, 2]).unwrap();
    match resolve_route(&dir, "/blob.xyz") {
        Route::Static { content_type, .. } => {
            assert_eq!(content_type, "application/octet-stream");
        }
        other => panic!("expected Static, got {other:?}"),
    }
}
