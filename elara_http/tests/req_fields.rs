use elara_http::{RequestData, run_script};

#[test]
fn script_reads_request_fields_and_case_insensitive_headers() {
    let req = RequestData {
        method: "post".into(),
        path: "/api".into(),
        query: "a=1".into(),
        body: "raw-body".into(),
        ..RequestData::default()
    }
    .with_header("Content-Type", "text/plain");

    let source = r#"
res.write(req.method)
res.write("|")
res.write(req.path)
res.write("|")
res.write(req.query)
res.write("|")
res.write(req.headers["content-type"])
res.write("|")
res.write(req.body)
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    let body = String::from_utf8(response.body).expect("utf8");
    assert_eq!(body, "POST|/api|a=1|text/plain|raw-body");
}
