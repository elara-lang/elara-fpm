use elara_fpm::{BodyLimits, RequestData, run_script, run_script_with_limits};

#[test]
fn json_body_fills_req_json() {
    let req = RequestData {
        method: "POST".into(),
        path: "/api".into(),
        body: r#"{"hello":"world","n":2}"#.into(),
        ..RequestData::default()
    }
    .with_header("Content-Type", "application/json");

    let source = r#"
let n = req.json["n"]
res.write(req.json["hello"])
res.write("|")
res.write("n={n}")
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    let body = String::from_utf8(response.body).expect("utf8");
    assert_eq!(body, "world|n=2");
}

#[test]
fn invalid_json_rejects_before_script() {
    let req = RequestData {
        method: "POST".into(),
        path: "/api".into(),
        body: "{not-json".into(),
        ..RequestData::default()
    }
    .with_header("Content-Type", "application/json");

    let err = run_script_with_limits(&req, r#"res.write("no")"#, &BodyLimits::default())
        .expect_err("should fail");
    assert!(err.contains("invalid JSON"), "got {err}");
}
