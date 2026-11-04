use elara_fpm::serve_dev_connection;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::FromRawFd;
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

fn find_access_lines(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|l| is_access_line(l))
        .map(|l| l.to_string())
        .collect()
}

/// Redirect fd 2 into a pipe for the duration of `f`, then restore.
fn capture_stderr<R>(f: impl FnOnce() -> R) -> (R, Vec<u8>) {
    unsafe extern "C" {
        fn pipe(fds: *mut i32) -> i32;
        fn dup(fd: i32) -> i32;
        fn dup2(oldfd: i32, newfd: i32) -> i32;
        fn close(fd: i32) -> i32;
    }

    let mut fds = [0i32; 2];
    assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
    let (read_fd, write_fd) = (fds[0], fds[1]);
    let old_stderr = unsafe { dup(2) };
    assert!(old_stderr >= 0);
    assert_eq!(unsafe { dup2(write_fd, 2) }, 2);
    unsafe {
        close(write_fd);
    }

    let result = f();

    // dup2 closes the pipe write end currently on fd 2, so the reader sees EOF.
    assert_eq!(unsafe { dup2(old_stderr, 2) }, 2);
    unsafe {
        close(old_stderr);
    }

    let mut reader = unsafe { std::fs::File::from_raw_fd(read_fd) };
    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf);
    (result, buf)
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
    let lines = find_access_lines(&stderr);
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
    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let captured2 = Arc::clone(&captured);

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let (_, lines) = capture_stderr(|| {
            let _ = serve_dev_connection(stream, &docroot);
        });
        *captured2.lock().unwrap() = lines;
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
    let raw = captured.lock().unwrap().clone();
    let stderr = String::from_utf8_lossy(&raw);
    let lines = find_access_lines(&stderr);
    assert_eq!(lines.len(), 1, "stderr={stderr}");
    assert!(
        lines[0].starts_with("GET /hi.ela 200 "),
        "line={} stderr={stderr}",
        lines[0]
    );
    assert!(!stderr.contains("token=nope"), "query leaked: {stderr}");
    let _ = fs::remove_dir_all(&dir);
}
