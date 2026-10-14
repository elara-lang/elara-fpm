use elara_fpm::handle_cgi_request;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

#[test]
fn cgi_mode_without_s_is_wired() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-cgibin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("cgi-ok")"#).unwrap();
    let mut env = BTreeMap::new();
    env.insert("REQUEST_METHOD".into(), "GET".into());
    env.insert("PATH_INFO".into(), "/hi.ela".into());
    let resp = handle_cgi_request(&dir, &env, b"");
    assert_eq!(resp.body, b"cgi-ok");
}

#[test]
fn binary_cgi_mode_exits_zero_with_fixture() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-cgibin2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("hi.ela"), r#"res.write("cgi-ok")"#).unwrap();

    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let output = Command::new(bin)
        .arg("-t")
        .arg(&dir)
        .env("REQUEST_METHOD", "GET")
        .env("PATH_INFO", "/hi.ela")
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
    assert!(stdout.contains("cgi-ok"), "{stdout}");
}
