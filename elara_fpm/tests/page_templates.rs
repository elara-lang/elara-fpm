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

#[test]
fn if_for_comment_and_include_with_depth_cap() {
    let dir = setup("ctrl");
    fs::write(dir.join("templates/part.htm"), "P{{ name }}").unwrap();
    fs::write(
        dir.join("templates/ctrl.htm"),
        concat!(
            "{# skip #}",
            "{% if show %}Y{% else %}N{% endif %}",
            "{% for item in items %}[{{ item }}]{% endfor %}",
            "{% include \"part.htm\" %}",
        ),
    )
    .unwrap();
    fs::write(
        dir.join("ctrl.ela"),
        "let items = {\"a\", \"b\"}\nres.render(\"ctrl.htm\", { show: true, items: items, name: \"Z\" })\n",
    )
    .unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/ctrl.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(
        resp.status,
        200,
        "body={}",
        String::from_utf8_lossy(&resp.body)
    );
    assert_eq!(String::from_utf8_lossy(&resp.body), "Y[a][b]PZ");

    // Nested includes: eight levels ok; a ninth nested include is 500.
    for i in 1..=10 {
        let name = format!("d{i}.htm");
        let body = if i == 10 {
            "LEAF".into()
        } else {
            format!("{{% include \"d{}.htm\" %}}", i + 1)
        };
        fs::write(dir.join("templates").join(name), body).unwrap();
    }
    fs::write(dir.join("templates/d9.htm"), "OK8").unwrap();
    clear_template_cache();
    fs::write(dir.join("deep8.ela"), r#"res.render("d1.htm", {})"#).unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/deep8.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(
        resp.status,
        200,
        "eight nested includes ok; body={}",
        String::from_utf8_lossy(&resp.body)
    );
    assert_eq!(String::from_utf8_lossy(&resp.body), "OK8");

    fs::write(dir.join("templates/d9.htm"), "{% include \"d10.htm\" %}").unwrap();
    clear_template_cache();
    fs::write(dir.join("deep9.ela"), r#"res.render("d1.htm", {})"#).unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/deep9.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(resp.status, 500, "ninth nested include must be 500");
    assert!(
        !String::from_utf8_lossy(&resp.body).contains(dir.to_string_lossy().as_ref()),
        "must not leak absolute path"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn confinement_rejects_escape_and_template_urls_are_404() {
    let dir = setup("conf");
    fs::write(dir.join("secret.htm"), "SECRET").unwrap();
    fs::write(dir.join("templates/ok.htm"), "ok").unwrap();
    fs::write(dir.join("escape.ela"), r#"res.render("../secret.htm", {})"#).unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/escape.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(resp.status, 500);
    assert!(
        !String::from_utf8_lossy(&resp.body).contains("SECRET"),
        "escaped template must not render"
    );
    assert!(
        !String::from_utf8_lossy(&resp.body).contains(dir.to_string_lossy().as_ref()),
        "must not leak absolute path"
    );

    let req = RequestData {
        method: "GET".into(),
        path: "/templates/ok.htm".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(resp.status, 404);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn template_file_change_is_picked_up_without_restart() {
    let dir = setup("cache");
    let tmpl = dir.join("templates/v.htm");
    fs::write(&tmpl, "v1").unwrap();
    fs::write(dir.join("v.ela"), r#"res.render("v.htm", {})"#).unwrap();
    let req = RequestData {
        method: "GET".into(),
        path: "/v.ela".into(),
        ..RequestData::default()
    };
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(String::from_utf8_lossy(&resp.body), "v1");

    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&tmpl, "v2").unwrap();
    let resp = dispatch_request_with(&dir, &req, &ProcessConfig::default());
    assert_eq!(
        String::from_utf8_lossy(&resp.body),
        "v2",
        "edited template must apply on the next request"
    );

    let _ = fs::remove_dir_all(&dir);
}
