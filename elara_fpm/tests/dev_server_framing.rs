use elara_fpm::{BodyLimits, bind_listener, serve_dev_connection};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn spawn_one(docroot: std::path::PathBuf) -> std::net::SocketAddr {
    let cfg = elara_fpm::DevServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        docroot: docroot.clone(),
    };
    let listener = bind_listener(&cfg).expect("bind");
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let _ = serve_dev_connection(stream, &docroot);
    });
    thread::sleep(Duration::from_millis(20));
    addr
}

fn read_status(buf: &[u8]) -> u16 {
    let text = String::from_utf8_lossy(buf);
    text.lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn response_body(buf: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(buf);
    if let Some(i) = text.find("\r\n\r\n") {
        buf[i + 4..].to_vec()
    } else {
        Vec::new()
    }
}

fn read_one_response(stream: &mut TcpStream) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        let n = stream.read(&mut tmp).expect("read headers");
        assert!(n > 0, "eof before response headers");
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]);
    let mut content_len = None;
    for line in head.lines() {
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_len = Some(v.trim().parse::<usize>().expect("content-length"));
        }
    }
    let len = content_len.expect("response Content-Length");
    let body_start = header_end + 4;
    while buf.len() < body_start + len {
        let n = stream.read(&mut tmp).expect("read body");
        assert!(n > 0, "eof before response body");
        buf.extend_from_slice(&tmp[..n]);
    }
    buf.truncate(body_start + len);
    buf
}

fn response_has_connection_close(buf: &[u8]) -> bool {
    String::from_utf8_lossy(buf).lines().any(|l| {
        l.to_ascii_lowercase().starts_with("connection:")
            && l.to_ascii_lowercase().contains("close")
    })
}

#[test]
fn post_body_arrives_intact_across_socket_reads() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-frame-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("echo.ela"), r#"res.write(req.body)"#).unwrap();

    let addr = spawn_one(dir.clone());
    let body = "x".repeat(12_000);
    let header = format!(
        "POST /echo.ela HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );

    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    // First write: headers + a small prefix so Content-Length body spans reads.
    client.write_all(header.as_bytes()).unwrap();
    client.write_all(&body.as_bytes()[..100]).unwrap();
    thread::sleep(Duration::from_millis(50));
    client.write_all(&body.as_bytes()[100..]).unwrap();

    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    assert_eq!(read_status(&buf), 200);
    assert_eq!(response_body(&buf), body.as_bytes());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn content_length_over_limit_is_413() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-frame413-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("x.ela"), r#"res.write("no")"#).unwrap();

    let limit = BodyLimits::default().max_body_bytes;
    let addr = spawn_one(dir.clone());
    let req = format!(
        "POST /x.ela HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        limit + 1
    );
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    assert_eq!(read_status(&buf), 413);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn chunked_transfer_encoding_is_501() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-frame501-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("x.ela"), r#"res.write("no")"#).unwrap();

    let addr = spawn_one(dir.clone());
    let req = "POST /x.ela HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n0\r\n\r\n";
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    assert_eq!(read_status(&buf), 501);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_content_length_means_empty_body() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-frame0-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("len.ela"), r#"res.write(req.body)"#).unwrap();

    // Bind ourselves to prove POST without Content-Length sees empty body.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
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
    // Trailing bytes after headers must not be treated as body without Content-Length.
    let req = "POST /len.ela HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\nIGNORED";
    client.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    assert_eq!(read_status(&buf), 200);
    assert_eq!(response_body(&buf), b"");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn two_http11_requests_on_one_connection() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-ka-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.ela"),
        r#"res.write("A-")
res.write(req.body)"#,
    )
    .unwrap();
    fs::write(
        dir.join("b.ela"),
        r#"res.write("B-")
res.write(req.body)"#,
    )
    .unwrap();

    let addr = spawn_one(dir.clone());
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let req1 = "POST /a.ela HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\none";
    let req2 = "POST /b.ela HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nConnection: close\r\n\r\ntwo";
    client.write_all(req1.as_bytes()).unwrap();
    client.write_all(req2.as_bytes()).unwrap();

    let r1 = read_one_response(&mut client);
    assert_eq!(read_status(&r1), 200);
    assert_eq!(response_body(&r1), b"A-one");
    assert!(
        !response_has_connection_close(&r1),
        "{}",
        String::from_utf8_lossy(&r1)
    );

    let r2 = read_one_response(&mut client);
    assert_eq!(read_status(&r2), 200);
    assert_eq!(response_body(&r2), b"B-two");
    assert!(
        response_has_connection_close(&r2),
        "{}",
        String::from_utf8_lossy(&r2)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn connection_close_ends_after_one_response() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-close-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("hi")"#).unwrap();

    let addr = spawn_one(dir.clone());
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let req = "GET /hi.ela HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    client.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    assert_eq!(read_status(&buf), 200);
    assert_eq!(response_body(&buf), b"hi");
    assert!(response_has_connection_close(&buf));
    // Peer closed: further write/read should fail or return empty.
    let mut more = [0u8; 8];
    let n = client.read(&mut more).unwrap_or(0);
    assert_eq!(n, 0);
    let _ = fs::remove_dir_all(&dir);
}
