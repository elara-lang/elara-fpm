use elara_fpm::{RequestData, html_escape, run_script};

#[test]
fn html_escape_entity_set() {
    assert_eq!(html_escape(r#"&<>"'"#), "&amp;&lt;&gt;&quot;&#39;");
}

#[test]
fn script_html_escape_is_available() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) = run_script(
        &req,
        r#"
let s = html_escape("<a>")
res.write(s)
"#,
    )
    .expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(response.body, b"&lt;a&gt;");
}
