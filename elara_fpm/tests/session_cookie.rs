use elara_fpm::{BodyLimits, RequestData, SessionConfig, run_script_with_session};

fn cfg(dir: std::path::PathBuf) -> SessionConfig {
    SessionConfig {
        store_dir: dir,
        cookie_name: "ELARASESSID".into(),
        path: "/".into(),
        http_only: true,
        secure: false,
        same_site: "Lax".into(),
        max_age: None,
    }
}

fn sid_from_set_cookie(line: &str) -> &str {
    line.split(';')
        .next()
        .unwrap()
        .strip_prefix("ELARASESSID=")
        .unwrap()
}

#[test]
fn new_session_sets_session_cookie() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = cfg(dir);
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (response, err) = run_script_with_session(
        &req,
        r#"res.write("ok")"#,
        &BodyLimits::default(),
        Some(&cfg),
    )
    .expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(response.body, b"ok");
    assert_eq!(response.set_cookies.len(), 1);
    let line = &response.set_cookies[0];
    assert!(line.starts_with("ELARASESSID="), "{line}");
    assert!(line.contains("HttpOnly"), "{line}");
    assert!(line.contains("Path=/"), "{line}");
    assert!(line.contains("SameSite=Lax"), "{line}");
}

#[test]
fn existing_session_cookie_is_honored() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = cfg(dir);
    let limits = BodyLimits::default();
    let req1 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (r1, e1) =
        run_script_with_session(&req1, r#"res.write("a")"#, &limits, Some(&cfg)).expect("run1");
    assert!(e1.is_none(), "{e1:?}");
    let sid = sid_from_set_cookie(&r1.set_cookies[0]).to_string();

    let req2 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    }
    .with_header("Cookie", format!("ELARASESSID={sid}"));
    let (r2, e2) =
        run_script_with_session(&req2, r#"res.write("b")"#, &limits, Some(&cfg)).expect("run2");
    assert!(e2.is_none(), "{e2:?}");
    assert_eq!(sid_from_set_cookie(&r2.set_cookies[0]), sid);
}
