use elara_fpm::{RequestData, run_script};

#[test]
fn req_args_parses_query_last_wins() {
    let req = RequestData {
        method: "GET".into(),
        path: "/echo.ela".into(),
        query: "name=ada&name=bev&q=a%2Bb".into(),
        ..RequestData::default()
    };
    let source = r#"
res.write(req.query)
res.write("|")
res.write(req.args["name"])
res.write("|")
res.write(req.args["q"])
res.write("|")
res.write(req.form["name"] or "")
"#;
    let (resp, err) = run_script(&req, source).expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(
        String::from_utf8_lossy(&resp.body),
        "name=ada&name=bev&q=a%2Bb|bev|a+b|"
    );
}
