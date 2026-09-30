use elara_http::{FinalResponse, format_cgi_response, handle_cgi_request};
use std::collections::BTreeMap;
use std::fs;

#[test]
fn cgi_stdout_includes_status_headers_and_body() {
    let mut headers = BTreeMap::new();
    headers.insert("X-Custom".into(), "yes".into());
    headers.insert("Content-Type".into(), "text/plain".into());
    let resp = FinalResponse {
        status: 201,
        headers,
        set_cookies: vec![],
        body: b"ok".to_vec(),
    };
    let out = String::from_utf8(format_cgi_response(&resp)).unwrap();
    assert!(out.starts_with("Status: 201 "), "{out}");
    assert!(out.contains("X-Custom: yes\r\n"), "{out}");
    assert!(out.contains("\r\n\r\nok"), "{out}");
}

#[test]
fn handle_cgi_runs_script_and_emits_custom_header() {
    let dir = std::env::temp_dir().join(format!("elara-http-cgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("app.ela"),
        r#"
res.header("X-From-Script", "1")
res.write("body")
"#,
    )
    .unwrap();

    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/app.ela".into());
    let resp = handle_cgi_request(&dir, &env, b"");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"body");
    assert_eq!(
        resp.headers.get("X-From-Script").map(String::as_str),
        Some("1")
    );
    let out = String::from_utf8(format_cgi_response(&resp)).unwrap();
    assert!(out.contains("Status: 200 "), "{out}");
    assert!(out.contains("X-From-Script: 1\r\n"), "{out}");
}
