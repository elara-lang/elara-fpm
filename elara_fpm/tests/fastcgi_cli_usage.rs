use elara_fpm::parse_cli;
use std::process::Command;

#[test]
fn parse_rejects_s_with_f() {
    let args = vec![
        "elara-fpm".into(),
        "-S".into(),
        "127.0.0.1:8080".into(),
        "-F".into(),
        "127.0.0.1:9000".into(),
        "-t".into(),
        "/tmp".into(),
    ];
    let err = parse_cli(&args).expect_err("S+F must fail");
    assert!(err.contains("-S") && err.contains("-F"), "{err}");
}

#[test]
fn binary_s_with_f_exits_2() {
    let bin = env!("CARGO_BIN_EXE_elara-fpm");
    let output = Command::new(bin)
        .args(["-S", "127.0.0.1:8080", "-F", "127.0.0.1:9000", "-t", "/tmp"])
        .output()
        .expect("run elara-fpm -S -F");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("-S") && stderr.contains("-F"),
        "stderr={stderr}"
    );
}
