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
