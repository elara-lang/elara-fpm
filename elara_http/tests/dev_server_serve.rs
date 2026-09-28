use elara_http::{DevServerConfig, RequestData, bind_listener, dispatch_request};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

fn spawn_server(
    docroot: std::path::PathBuf,
    n_requests: usize,
) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
    let cfg = DevServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        docroot: docroot.clone(),
    };
    let listener = bind_listener(&cfg).expect("bind");
    let addr = listener.local_addr().unwrap();
    let ready = Arc::new(Barrier::new(2));
    let ready2 = Arc::clone(&ready);
    let handle = thread::spawn(move || {
        listener.set_nonblocking(false).unwrap();
        ready2.wait();
        for _ in 0..n_requests {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut buf = vec![0u8; 16 * 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                continue;
            }
            let raw = String::from_utf8_lossy(&buf[..n]);
            let req = parse_simple(&raw);
            let resp = dispatch_request(&docroot, &req);
            let mut out = format!(
                "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                resp.status,
                resp.body.len()
            );
            for (k, v) in &resp.headers {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str("\r\n");
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.write_all(&resp.body);
        }
    });
    ready.wait();
    (addr, handle)
}

fn parse_simple(raw: &str) -> RequestData {
    let line = raw.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    RequestData {
        method,
        path,
        query,
        ..RequestData::default()
    }
}

fn http_get(addr: std::net::SocketAddr, path: &str) -> (u16, Vec<u8>, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf);
    let status: u16 = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = if let Some(i) = text.find("\r\n\r\n") {
        buf[i + 4..].to_vec()
    } else {
        Vec::new()
    };
    let ctype = text
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-type:"))
        .unwrap_or("")
        .to_string();
    (status, body, ctype)
}

#[test]
fn serves_ela_and_static_css() {
    let dir = std::env::temp_dir().join(format!("elara-http-ds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hello.ela"), r#"res.write("hello-body")"#).unwrap();
    fs::write(dir.join("style.css"), b"a{color:blue}").unwrap();

    let (addr, handle) = spawn_server(dir, 2);

    let (st, body, _) = http_get(addr, "/hello.ela");
    assert_eq!(st, 200);
    assert_eq!(body, b"hello-body");

    let (st2, body2, ctype) = http_get(addr, "/style.css");
    assert_eq!(st2, 200);
    assert_eq!(body2, b"a{color:blue}");
    assert!(ctype.to_ascii_lowercase().contains("text/css"), "{ctype}");

    handle.join().unwrap();
}
