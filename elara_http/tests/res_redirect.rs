use elara_http::{RequestData, run_script};

#[test]
fn redirect_sets_302_and_location() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) = run_script(&req, r#"res.redirect("/x")"#).expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(response.status, 302);
    assert_eq!(
        response.headers.get("Location").map(String::as_str),
        Some("/x")
    );
    assert!(response.body.is_empty());
}

#[test]
fn redirect_accepts_301_and_ignores_later_writes() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) = run_script(
        &req,
        r#"
res.redirect("/gone", 301)
res.write("nope")
"#,
    )
    .expect("run");
    // write after redirect fails the script; response still carries redirect
    assert!(err.is_some(), "expected write-after-redirect error");
    assert_eq!(response.status, 301);
    assert_eq!(
        response.headers.get("Location").map(String::as_str),
        Some("/gone")
    );
    assert!(response.body.is_empty());
}
