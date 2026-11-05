use elara_fpm::{
    begin_access_log_capture, encode_test_request, extract_stdout, handle_fastcgi_connection,
    serve_dev_connection, take_access_log_capture,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn is_access_line(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else {
        return false;
    };
    let Some(path) = parts.next() else {
        return false;
    };
    let Some(status) = parts.next() else {
        return false;
    };
    let Some(ms) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    if !path.starts_with('/') {
        return false;
    }
    if status.len() != 3 || !status.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    if !ms.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    matches!(method, "GET" | "POST" | "HEAD" | "PUT" | "DELETE")
}

#[test]
fn cgi_stderr_has_one_access_line() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-alog-cgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("alog-cgi")"#).unwrap();

    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let output = Command::new(bin)
        .arg("-t")
        .arg(&dir)
        .env("REQUEST_METHOD", "GET")
        .env("PATH_INFO", "/hi.ela")
        .env("QUERY_STRING", "secret=1")
        .env_remove("CONTENT_LENGTH")
        .output()
        .expect("run binary");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Status: 200 "), "{stdout}");
    assert!(stdout.contains("alog-cgi"), "{stdout}");
    assert!(!stdout.contains("secret=1"), "{stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<_> = stderr.lines().filter(|l| is_access_line(l)).collect();
    assert_eq!(lines.len(), 1, "stderr={stderr}");
    assert!(
        lines[0].starts_with("GET /hi.ela 200 "),
        "line={} stderr={stderr}",
        lines[0]
    );
    assert!(!stderr.contains("secret=1"), "query leaked: {stderr}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dev_server_stderr_has_one_access_line() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-alog-s-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("alog-s")"#).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured2 = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        begin_access_log_capture();
        let _ = serve_dev_connection(stream, &docroot);
        *captured2.lock().unwrap() = take_access_log_capture();
    });

    thread::sleep(Duration::from_millis(20));
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client
        .write_all(
            b"GET /hi.ela?token=nope HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    assert!(text.contains("200"), "{text}");
    assert!(text.contains("alog-s"), "{text}");

    handle.join().unwrap();
    let lines = captured.lock().unwrap().clone();
    assert_eq!(lines.len(), 1, "lines={lines:?}");
    assert!(
        lines[0].starts_with("GET /hi.ela 200 "),
        "line={}",
        lines[0]
    );
    assert!(
        !lines[0].contains("token=nope"),
        "query leaked: {}",
        lines[0]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fastcgi_stderr_has_one_access_line() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-alog-fcgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("alog-fcgi")"#).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured2 = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        begin_access_log_capture();
        handle_fastcgi_connection(&mut stream, &docroot).unwrap();
        *captured2.lock().unwrap() = take_access_log_capture();
    });

    thread::sleep(Duration::from_millis(20));
    let mut params = BTreeMap::new();
    params.insert("REQUEST_METHOD".into(), "GET".into());
    params.insert("PATH_INFO".into(), "/hi.ela".into());
    params.insert("QUERY_STRING".into(), "hide=1".into());
    let req = encode_test_request(&params, b"");
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&req).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let stdout = extract_stdout(&buf);
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains("Status: 200 "), "{text}");
    assert!(text.contains("alog-fcgi"), "{text}");

    handle.join().unwrap();
    let lines = captured.lock().unwrap().clone();
    assert_eq!(lines.len(), 1, "lines={lines:?}");
    assert!(
        lines[0].starts_with("GET /hi.ela 200 "),
        "line={}",
        lines[0]
    );
    assert!(!lines[0].contains("hide=1"), "query leaked: {}", lines[0]);
    let _ = fs::remove_dir_all(&dir);
}
