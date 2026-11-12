use elara_fpm::{ProcessConfig, RequestData, clear_template_cache, dispatch_request_with};
use std::fs;
use std::path::PathBuf;

fn setup(tag: &str) -> PathBuf {
    clear_template_cache();
    let dir = std::env::temp_dir().join(format!(
        "elara-fpm-tmpl-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("templates")).unwrap();
    dir
}

#[test]
fn render_escapes_by_default_and_safe_is_raw() {
    let dir = setup("esc");
    fs::write(dir.join("templates/hello.htm"), "<p>Hello {{ name }}</p>").unwrap();
    fs::write(
        dir.join("app.ela"),
        r#"res.render("hello.htm", { name: "<ada>" })"#,
    )
    .unwrap();

    let req = RequestData {
        method: "GET".into(),
        path: "/app.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(
        resp.status,
        200,
        "body={}",
        String::from_utf8_lossy(&resp.body)
    );
    assert_eq!(
        String::from_utf8_lossy(&resp.body),
        "<p>Hello &lt;ada&gt;</p>"
    );

    fs::write(dir.join("templates/raw.htm"), "{{ name | safe }}").unwrap();
    fs::write(
        dir.join("raw.ela"),
        r#"res.render("raw.htm", { name: "<ada>" })"#,
    )
    .unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/raw.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(resp.status, 200);
    assert_eq!(String::from_utf8_lossy(&resp.body), "<ada>");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_key_inserts_empty_string() {
    let dir = setup("miss");
    fs::write(dir.join("templates/m.htm"), "x{{ missing }}y").unwrap();
    fs::write(dir.join("m.ela"), r#"res.render("m.htm", {})"#).unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/m.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(resp.status, 200);
    assert_eq!(String::from_utf8_lossy(&resp.body), "xy");
    let _ = fs::remove_dir_all(&dir);
}
