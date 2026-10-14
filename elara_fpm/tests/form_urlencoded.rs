use elara_fpm::{RequestData, run_script};

#[test]
fn urlencoded_body_fills_req_form() {
    let req = RequestData {
        method: "POST".into(),
        path: "/submit".into(),
        body: "name=Ada&city=Rio+de+Janeiro&name=Lovelace".into(),
        ..RequestData::default()
    }
    .with_header("Content-Type", "application/x-www-form-urlencoded");

    let source = r#"
res.write(req.form["name"])
res.write("|")
res.write(req.form["city"])
res.write("|")
res.write(req.body)
"#;

    let (response, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    let body = String::from_utf8(response.body).expect("utf8");
    assert_eq!(
        body,
        "Lovelace|Rio de Janeiro|name=Ada&city=Rio+de+Janeiro&name=Lovelace"
    );
}
