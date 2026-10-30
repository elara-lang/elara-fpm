use elara_fpm::{BodyLimits, RequestData, SessionConfig, run_script_with_session};
use std::fs;

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
fn typed_values_round_trip_across_requests() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-svt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
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
session.set("i", 42)
session.set("b", true)
session.set("n", nil)
session.set("s", "hello")
session.set("t", {x: 1, y: {true, false}})
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
let i = session.get("i")
let b = session.get("b")
let n = session.get("n")
let s = session.get("s")
let t = session.get("t")
if type(i) != "int" or i != 42 { res.write("bad-i"); return }
if type(b) != "boolean" or b != true { res.write("bad-b"); return }
if n != nil { res.write("bad-n"); return }
if type(s) != "string" or s != "hello" { res.write("bad-s"); return }
if type(t) != "table" or t.x != 1 or t.y[0] != true or t.y[1] != false {
  res.write("bad-t"); return
}
res.write("ok")
"#,
        &limits,
        Some(&cfg),
    )
    .expect("run2");
    assert!(e2.is_none(), "{e2:?}");
    assert_eq!(r2.body, b"ok");
    let _ = fs::remove_dir_all(&cfg.store_dir);
}
