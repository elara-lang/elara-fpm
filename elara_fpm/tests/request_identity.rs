use elara_fpm::{
    DevServerConfig, ProcessConfig, RequestData, bind_listener, handle_cgi_request,
    request_from_cgi_env, run_script, serve_dev_connection,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

#[test]
fn cgi_env_fills_host_scheme_remote() {
    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/".into());
    env.insert("HTTPS".into(), "on".into());
    env.insert("REMOTE_ADDR".into(), "203.0.113.9".into());
    env.insert("HTTP_HOST".into(), "cgi.example".into());
    let req = request_from_cgi_env(&env, b"");
    assert_eq!(req.host, "cgi.example");
    assert_eq!(req.scheme, "https");
    assert_eq!(req.remote, "203.0.113.9");
}

#[test]
fn cgi_script_reads_identity_fields() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-id-cgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("id.ela"),
        r#"res.write(req.scheme)
res.write("|")
res.write(req.remote)"#,
    )
    .unwrap();
    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/id.ela".into());
    env.insert("HTTPS".into(), "on".into());
    env.insert("REMOTE_ADDR".into(), "203.0.113.9".into());
    let resp = handle_cgi_request(&dir, &env, b"");
    assert_eq!(String::from_utf8_lossy(&resp.body), "https|203.0.113.9");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_server_fills_host_scheme_remote() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-id-ds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("id.ela"),
        r#"res.write(req.host)
res.write("|")
res.write(req.scheme)
res.write("|")
res.write(req.remote)"#,
    )
    .unwrap();
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
        .write_all(b"GET /id.ela HTTP/1.1\r\nHost: app.local\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    let parts: Vec<_> = body.split('|').collect();
    assert_eq!(parts.len(), 3, "{body}");
    assert_eq!(parts[0], "app.local");
    assert_eq!(parts[1], "http");
    assert!(!parts[2].is_empty(), "remote empty");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn harness_defaults_do_not_crash_on_empty_identity() {
    let req = RequestData::default();
    let (resp, err) = run_script(
        &req,
        r#"res.write(req.host)
res.write(req.scheme)
res.write(req.remote)"#,
    )
    .expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(resp.status, 200);
}
