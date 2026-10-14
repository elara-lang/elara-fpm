use elara_fpm::{RequestData, run_script};

#[test]
fn cookie_ex_emits_set_cookie_attributes() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };

    let source = r#"
res.cookie_ex("sid", "abc", "/app", 1, 1, "Lax", 3600)
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    assert_eq!(response.set_cookies.len(), 1);
    let line = &response.set_cookies[0];
    assert!(line.starts_with("sid=abc"), "{line}");
    assert!(line.contains("Path=/app"), "{line}");
    assert!(line.contains("HttpOnly"), "{line}");
    assert!(line.contains("Secure"), "{line}");
    assert!(line.contains("SameSite=Lax"), "{line}");
    assert!(line.contains("Max-Age=3600"), "{line}");
}
