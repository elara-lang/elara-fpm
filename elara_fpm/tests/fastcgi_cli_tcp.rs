use elara_fpm::{encode_test_request, extract_stdout};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn binary_f_tcp_serves_one_fixture() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-fcgi-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("fcgi-cli-hi")"#).unwrap();

    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let listen = format!("127.0.0.1:{port}");

    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let mut child = Command::new(bin)
        .arg("-F")
        .arg(&listen)
        .arg("-t")
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn elara-fpm -F");

    let ready = (0..100).any(|_| {
        thread::sleep(Duration::from_millis(20));
        TcpStream::connect(&listen).is_ok()
    });
    assert!(ready, "FastCGI worker did not listen on {listen}");

    let mut params = BTreeMap::new();
    params.insert("REQUEST_METHOD".into(), "GET".into());
    params.insert("PATH_INFO".into(), "/hi.ela".into());
    let req = encode_test_request(&params, b"");
    let mut client = TcpStream::connect(&listen).expect("connect worker");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&req).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let stdout = extract_stdout(&buf);
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains("Status: 200 "), "{text}");
    assert!(text.contains("fcgi-cli-hi"), "{text}");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}
