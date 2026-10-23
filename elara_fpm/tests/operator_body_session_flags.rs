use elara_fpm::{
    BodyLimits, ProcessConfig, RequestData, SessionConfig, parse_cli, run_script_sandboxed,
};
use std::fs;

#[test]
fn parse_body_and_session_flags() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sessflag-{}", std::process::id()));
    let args = vec![
        "elara-fpm".into(),
        "--max-body".into(),
        "8".into(),
        "--session-dir".into(),
        dir.to_string_lossy().into_owned(),
        "--session-secure".into(),
        "--session-samesite".into(),
        "Strict".into(),
        "-t".into(),
        "/tmp".into(),
    ];
    let cli = parse_cli(&args).expect("parse");
    assert_eq!(cli.process.body_limits.max_body_bytes, 8);
    assert_eq!(cli.process.session.store_dir, dir);
    assert!(cli.process.session.secure);
    assert_eq!(cli.process.session.same_site, "Strict");
}

#[test]
fn max_body_rejects_oversized() {
    let process = ProcessConfig {
        body_limits: BodyLimits {
            max_body_bytes: 8,
            ..BodyLimits::default()
        },
        ..ProcessConfig::default()
    };
    let req = RequestData {
        method: "POST".into(),
        path: "/".into(),
        body: "0123456789".into(),
        headers: {
            let mut h = std::collections::BTreeMap::new();
            h.insert("content-type".into(), "text/plain".into());
            h
        },
        ..RequestData::default()
    };
    let err = run_script_sandboxed(
        &req,
        r#"res.write("no")"#,
        &process.body_limits,
        None,
        &process.sandbox,
    )
    .expect_err("body too large");
    assert!(err.to_ascii_lowercase().contains("too large"), "{err}");
}

#[test]
fn session_dir_and_secure_cookie() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-sessdir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let process = ProcessConfig {
        session: SessionConfig {
            store_dir: dir.clone(),
            secure: true,
            same_site: "Strict".into(),
            ..SessionConfig::default()
        },
        ..ProcessConfig::default()
    };
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (resp, err) = run_script_sandboxed(
        &req,
        r#"session.set("k", "v")
res.write("ok")"#,
        &process.body_limits,
        Some(&process.session),
        &process.sandbox,
    )
    .expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(String::from_utf8_lossy(&resp.body), "ok");
    let cookie = resp.set_cookies.join(";");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(cookie.contains("SameSite=Strict"), "{cookie}");
    let entries: Vec<_> = fs::read_dir(&dir).unwrap().collect();
    assert!(!entries.is_empty(), "expected session file under {dir:?}");
    let _ = fs::remove_dir_all(&dir);
}
