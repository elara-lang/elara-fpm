use elara_fpm::{encode_test_request, extract_stdout};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn binary_f_unix_serves_one_fixture() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-fcgi-ux-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("fcgi-unix-hi")"#).unwrap();
    let sock = dir.join("worker.sock");
    let sock_s = sock.to_str().expect("utf8 path").to_string();

    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let mut child = Command::new(bin)
        .arg("-F")
        .arg(&sock_s)
        .arg("-t")
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn elara-fpm -F unix");

    let ready = (0..100).any(|_| {
        thread::sleep(Duration::from_millis(20));
        UnixStream::connect(&sock).is_ok()
    });
    assert!(ready, "FastCGI worker did not listen on {sock_s}");

    let mut params = BTreeMap::new();
    params.insert("REQUEST_METHOD".into(), "GET".into());
    params.insert("PATH_INFO".into(), "/hi.ela".into());
    let req = encode_test_request(&params, b"");
    let mut client = UnixStream::connect(&sock).expect("connect unix worker");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&req).unwrap();
    let mut buf = Vec::new();
    client.read_to_end(&mut buf).unwrap();
    let stdout = extract_stdout(&buf);
    let text = String::from_utf8_lossy(&stdout);
    assert!(text.contains("Status: 200 "), "{text}");
    assert!(text.contains("fcgi-unix-hi"), "{text}");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn binary_f_non_socket_path_fails() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-fcgi-ns-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let not_sock = dir.join("regular-file");
    fs::write(&not_sock, b"not a socket").unwrap();

    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let output = Command::new(bin)
        .arg("-F")
        .arg(&not_sock)
        .arg("-t")
        .arg(&dir)
        .output()
        .expect("run elara-fpm -F non-socket");
    assert!(
        !output.status.success(),
        "expected failure for non-socket path"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a socket"), "stderr={stderr}");
    let _ = fs::remove_dir_all(&dir);
}
