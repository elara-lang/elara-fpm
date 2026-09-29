use elara_http::{DevServerConfig, RequestData, bind_listener, dispatch_request};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

#[test]
fn two_sequential_requests_stay_isolated() {
    let dir = std::env::temp_dir().join(format!("elara-http-dsi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.ela"), r#"res.write("A")"#).unwrap();
    fs::write(dir.join("b.ela"), r#"res.write("B")"#).unwrap();

    let cfg = DevServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        docroot: dir.clone(),
    };
    let listener = bind_listener(&cfg).unwrap();
    let addr = listener.local_addr().unwrap();
    let ready = Arc::new(Barrier::new(2));
    let ready2 = Arc::clone(&ready);
    let handle = thread::spawn(move || {
        ready2.wait();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let raw = String::from_utf8_lossy(&buf[..n]);
            let line = raw.lines().next().unwrap();
            let path = line.split_whitespace().nth(1).unwrap().to_string();
            let req = RequestData {
                method: "GET".into(),
                path,
                ..RequestData::default()
            };
            let resp = dispatch_request(&dir, &req);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                resp.body.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&resp.body).unwrap();
        }
    });
    ready.wait();

    let body1 = get_body(addr, "/a.ela");
    let body2 = get_body(addr, "/b.ela");
    assert_eq!(body1, b"A");
    assert_eq!(body2, b"B");
    // No concatenated leakage across requests.
    assert_ne!(body2, b"AB");
    handle.join().unwrap();
}

fn get_body(addr: std::net::SocketAddr, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let i = text.find("\r\n\r\n").unwrap();
    buf[i + 4..].to_vec()
}
