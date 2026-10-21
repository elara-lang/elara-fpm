use elara_fpm::{DevServerConfig, bind_listener, handle_cgi_request, serve_dev_connection};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

#[test]
fn cgi_query_fills_req_args() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-args-cgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("q.ela"),
        r#"res.write(req.args["name"])
res.write("|")
res.write(req.query)"#,
    )
    .unwrap();
    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/q.ela".into());
    env.insert("QUERY_STRING".into(), "name=ada&name=bev".into());
    let resp = handle_cgi_request(&dir, &env, b"");
    assert_eq!(String::from_utf8_lossy(&resp.body), "bev|name=ada&name=bev");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_server_query_fills_req_args() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-args-ds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("q.ela"),
        r#"res.write(req.args["name"])
res.write("|")
res.write(req.query)"#,
    )
    .unwrap();

    let cfg = DevServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        docroot: dir.clone(),
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
    let req =
        "GET /q.ela?name=ada&name=bev HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    assert_eq!(body, "bev|name=ada&name=bev");
    let _ = fs::remove_dir_all(&dir);
}
