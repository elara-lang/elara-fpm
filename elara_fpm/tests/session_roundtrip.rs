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
fn session_get_set_round_trips_across_requests() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = cfg(dir);
    let limits = BodyLimits::default();

    let req1 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (r1, e1) = run_script_with_session(
        &req1,
        r#"
session.set("n", "1")
res.write("ok")
"#,
        &limits,
        Some(&cfg),
    )
    .expect("run1");
    assert!(e1.is_none(), "{e1:?}");
    let sid = sid_from_set_cookie(&r1.set_cookies[0]).to_string();

    let req2 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    }
    .with_header("Cookie", format!("ELARASESSID={sid}"));
    let (r2, e2) = run_script_with_session(
        &req2,
        r#"
let n = session.get("n")
res.write(n)
"#,
        &limits,
        Some(&cfg),
    )
    .expect("run2");
    assert!(e2.is_none(), "{e2:?}");
    assert_eq!(r2.body, b"1");
}

#[test]
fn session_destroy_expires_cookie() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = cfg(dir);
    let limits = BodyLimits::default();

    let req1 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (r1, _) =
        run_script_with_session(&req1, r#"session.set("k", "v")"#, &limits, Some(&cfg)).unwrap();
    let sid = sid_from_set_cookie(&r1.set_cookies[0]).to_string();

    let req2 = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    }
    .with_header("Cookie", format!("ELARASESSID={sid}"));
    let (r2, e2) =
        run_script_with_session(&req2, r#"session.destroy()"#, &limits, Some(&cfg)).unwrap();
    assert!(e2.is_none(), "{e2:?}");
    assert!(
        r2.set_cookies[0].contains("Max-Age=0"),
        "{:?}",
        r2.set_cookies
    );
}
