use elara_http::{RequestData, run_script};

#[test]
fn script_failure_yields_host_500_without_paths() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) =
        run_script(&req, r#"this_identifier_does_not_exist()"#).expect("host run");
    assert!(err.is_some(), "expected script error");
    assert_eq!(response.status, 500);
    assert_eq!(
        response.headers.get("Content-Type").map(String::as_str),
        Some("text/plain; charset=utf-8")
    );
    let body = String::from_utf8_lossy(&response.body);
    assert!(body.contains("500"), "{body}");
    assert!(body.contains("Internal Server Error"), "{body}");
    assert!(!body.contains('/'), "must not leak paths: {body}");
    assert!(!body.contains("elara"), "{body}");
    assert!(!body.contains("session"), "{body}");
}
