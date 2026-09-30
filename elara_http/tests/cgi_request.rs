use elara_http::request_from_cgi_env;
use std::collections::BTreeMap;

#[test]
fn maps_cgi_env_and_body_into_request() {
    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "POST".into());
    env.insert("PATH_INFO".into(), "/app.ela".into());
    env.insert("QUERY_STRING".into(), "x=1".into());
    env.insert("HTTP_USER_AGENT".into(), "cgi-test".into());
    env.insert("CONTENT_TYPE".into(), "text/plain".into());
    env.insert("CONTENT_LENGTH".into(), "5".into());
    let req = request_from_cgi_env(&env, b"helloEXTRA");
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/app.ela");
    assert_eq!(req.query, "x=1");
    assert_eq!(req.body, "hello");
    assert_eq!(
        req.headers.get("user-agent").map(String::as_str),
        Some("cgi-test")
    );
    assert_eq!(
        req.headers.get("content-type").map(String::as_str),
        Some("text/plain")
    );
}
