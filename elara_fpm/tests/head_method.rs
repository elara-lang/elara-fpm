use elara_fpm::{
    DevServerConfig, ProcessConfig, bind_listener, dispatch_request, serve_dev_connection,
};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn header_value(raw: &str, name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    for line in raw.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case(&lower)
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

#[test]
fn head_static_matches_get_status_type_and_length() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-head-st-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let css = b"body{color:red}";
    fs::write(dir.join("file.css"), css).unwrap();

    let mut get_req = elara_fpm::RequestData {
        method: "GET".into(),
        path: "/file.css".into(),
        ..elara_fpm::RequestData::default()
    };
    let get = dispatch_request(&dir, &get_req);
    assert_eq!(get.status, 200);
    assert_eq!(get.body, css);

    get_req.method = "HEAD".into();
    let head = dispatch_request(&dir, &get_req);
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    assert_eq!(
        head.headers.get("Content-Type").map(String::as_str),
        get.headers.get("Content-Type").map(String::as_str)
    );
    assert_eq!(
        head.headers.get("Content-Length").map(String::as_str),
        Some(css.len().to_string().as_str())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn head_missing_path_matches_get_status() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-head-404-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let get = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "GET".into(),
            path: "/nope.css".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    let head = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "HEAD".into(),
            path: "/nope.css".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    assert_eq!(head.status, get.status);
    assert!(head.body.is_empty());
    assert_eq!(
        head.headers.get("Content-Length").map(String::as_str),
        Some(get.body.len().to_string().as_str())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn head_script_strips_body_keeps_length_and_get_returns_body() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-head-scr-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("hello-head")"#).unwrap();

    let get = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "GET".into(),
            path: "/hi.ela".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    assert_eq!(get.status, 200);
    assert_eq!(get.body, b"hello-head");

    let head = dispatch_request(
        &dir,
        &elara_fpm::RequestData {
            method: "HEAD".into(),
            path: "/hi.ela".into(),
            ..elara_fpm::RequestData::default()
        },
    );
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    assert_eq!(
        head.headers.get("Content-Length").map(String::as_str),
        Some(get.body.len().to_string().as_str())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn head_over_wire_has_zero_body_bytes() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-head-wire-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let css = b"body{color:red}";
    fs::write(dir.join("file.css"), css).unwrap();
    let cfg = DevServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        docroot: dir.clone(),
        process: ProcessConfig::default(),
    };
    let listener = bind_listener(&cfg).unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let _ = serve_dev_connection(stream, &docroot);
    });
    thread::sleep(Duration::from_millis(20));
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client
        .write_all(b"HEAD /file.css HTTP/1.1\r\nHost: app.local\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    assert!(text.starts_with("HTTP/1.1 200"), "{text}");
    assert_eq!(
        header_value(&text, "Content-Length").as_deref(),
        Some(css.len().to_string().as_str())
    );
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("x");
    assert!(body.is_empty(), "body={body:?}");
    let _ = fs::remove_dir_all(&dir);
}
