use elara_fpm::{RequestData, run_script};

#[test]
fn multiple_cookies_are_separate_set_cookie_lines() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };

    let source = r#"
res.cookie("a", "1")
res.cookie("b", "2")
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    assert_eq!(response.set_cookies.len(), 2);
    assert!(response.set_cookies[0].starts_with("a=1"));
    assert!(response.set_cookies[1].starts_with("b=2"));
    assert!(response.set_cookies[0].contains("Path=/"));
}
