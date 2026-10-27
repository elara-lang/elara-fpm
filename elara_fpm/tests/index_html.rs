use elara_fpm::{Route, dispatch_request, resolve_route};
use std::fs;

#[test]
fn directory_with_only_index_html_is_static() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-ih-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("site")).unwrap();
    let html = b"<h1>hi</h1>";
    fs::write(dir.join("site/index.html"), html).unwrap();
    fs::write(dir.join("index.html"), html).unwrap();

    for url in ["/", "/site/", "/site"] {
        match resolve_route(&dir, url) {
            Route::Static {
                path: p,
                content_type,
            } => {
                assert!(p.ends_with("index.html"), "{url} -> {p:?}");
                assert_eq!(content_type, "text/html; charset=utf-8");
            }
            other => panic!("expected Static for {url}, got {other:?}"),
        }
    }

    let resp = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "GET".into(),
            path: "/".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, html);
    assert_eq!(
        resp.headers.get("Content-Type").map(String::as_str),
        Some("text/html; charset=utf-8")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn index_ela_wins_over_index_html() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-ih2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("app")).unwrap();
    fs::write(dir.join("app/index.ela"), r#"res.write("script")"#).unwrap();
    fs::write(dir.join("app/index.html"), b"<p>html</p>").unwrap();
    match resolve_route(&dir, "/app/") {
        Route::Script(p) => assert!(p.ends_with("index.ela")),
        other => panic!("expected Script, got {other:?}"),
    }
    let resp = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "GET".into(),
            path: "/app/".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    assert_eq!(resp.body, b"script");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_directory_is_404() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-ih3-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("empty")).unwrap();
    match resolve_route(&dir, "/empty/") {
        Route::Error(resp) => assert_eq!(resp.status, 404),
        other => panic!("expected Error, got {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}
