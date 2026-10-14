use elara_fpm::{RequestData, run_script};

#[test]
fn script_sets_status_headers_and_body_chunks() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };

    let source = r#"
res.set_status(201)
res.header("X-Test", "yes")
res.write("hel")
res.write("lo")
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    assert_eq!(response.status, 201);
    assert_eq!(
        response.headers.get("X-Test").map(String::as_str),
        Some("yes")
    );
    assert_eq!(response.body, b"hello");
}
