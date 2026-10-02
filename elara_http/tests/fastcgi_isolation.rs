use elara_http::{encode_test_request, extract_stdout, handle_fastcgi_connection};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn fcgi_get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut params = BTreeMap::new();
    params.insert("REQUEST_METHOD".into(), "GET".into());
    params.insert("PATH_INFO".into(), path.into());
    let req = encode_test_request(&params, b"");
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&req).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&extract_stdout(&buf)).into_owned()
}

#[test]
fn two_fastcgi_requests_stay_isolated() {
    let dir = std::env::temp_dir().join(format!("elara-http-fcgi2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.ela"), r#"res.write("A")"#).unwrap();
    fs::write(dir.join("b.ela"), r#"res.write("B")"#).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            handle_fastcgi_connection(&mut stream, &docroot).unwrap();
        }
    });
    thread::sleep(Duration::from_millis(20));

    let out1 = fcgi_get(addr, "/a.ela");
    let out2 = fcgi_get(addr, "/b.ela");
    assert!(out1.contains("A"), "{out1}");
    assert!(out2.contains("B"), "{out2}");
    assert!(!out2.contains("A"), "{out2}");
    server.join().unwrap();
}
