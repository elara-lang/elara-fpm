use elara_http::{
    encode_test_request, extract_stdout, format_cgi_response, handle_cgi_request,
    handle_fastcgi_connection,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

#[test]
fn fastcgi_matches_cgi_golden_body() {
    let dir = std::env::temp_dir().join(format!("elara-http-fcgip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("golden.ela"),
        r#"
res.header("X-Demo", "1")
res.write("golden")
"#,
    )
    .unwrap();

    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/golden.ela".into());
    let cgi = handle_cgi_request(&dir, &env, b"");
    let cgi_out = format_cgi_response(&cgi);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let docroot = dir.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        handle_fastcgi_connection(&mut stream, &docroot).unwrap();
    });
    thread::sleep(Duration::from_millis(20));
    let req = encode_test_request(&env, b"");
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&req).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let fcgi_out = extract_stdout(&buf);
    server.join().unwrap();

    // Session cookies differ per request; compare status/body and custom header.
    assert_eq!(cgi.status, 200);
    assert_eq!(cgi.body, b"golden");
    assert_eq!(cgi.headers.get("X-Demo").map(String::as_str), Some("1"));
    let text = String::from_utf8_lossy(&fcgi_out);
    assert!(text.contains("Status: 200 "), "{text}");
    assert!(text.contains("X-Demo: 1\r\n"), "{text}");
    assert!(text.ends_with("golden"), "{text}");
    let _ = cgi_out;
}
