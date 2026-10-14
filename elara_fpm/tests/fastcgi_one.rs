use elara_fpm::{FastCgiConfig, encode_test_request, extract_stdout, handle_fastcgi_connection};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

#[test]
fn fastcgi_serves_one_script_fixture() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-fcgi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("fcgi-hi")"#).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        handle_fastcgi_connection(&mut stream, &docroot).unwrap();
    });
    thread::sleep(Duration::from_millis(20));

    let mut params = BTreeMap::new();
    params.insert("REQUEST_METHOD".into(), "GET".into());
    params.insert("PATH_INFO".into(), "/hi.ela".into());
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
    assert!(text.contains("fcgi-hi"), "{text}");
    server.join().unwrap();
    let _ = FastCgiConfig {
        listen: addr.to_string(),
        docroot: dir,
    };
}
