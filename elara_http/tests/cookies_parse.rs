use elara_http::{RequestData, run_script};

#[test]
fn cookie_header_fills_req_cookies() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    }
    .with_header("Cookie", "a=1; b=2; a=9");

    let source = r#"
res.write(req.cookies["a"])
res.write("|")
res.write(req.cookies["b"])
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    let body = String::from_utf8(response.body).expect("utf8");
    assert_eq!(body, "9|2");
}
