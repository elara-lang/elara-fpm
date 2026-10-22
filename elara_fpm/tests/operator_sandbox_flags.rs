use elara_fpm::{
    ELARA_CAP_SYS_ENV, ProcessConfig, RequestData, SandboxConfig, parse_cli, run_script_sandboxed,
};

#[test]
fn parse_gas_mem_and_allow_sys_env() {
    let args = vec![
        "elara-fpm".into(),
        "--gas".into(),
        "42".into(),
        "--mem".into(),
        "1024".into(),
        "--allow-sys-env".into(),
        "-t".into(),
        "/tmp".into(),
    ];
    let cli = parse_cli(&args).expect("parse");
    assert_eq!(cli.process.sandbox.gas_limit, 42);
    assert_eq!(cli.process.sandbox.mem_limit_bytes, 1024);
    assert_ne!(cli.process.sandbox.capabilities & ELARA_CAP_SYS_ENV, 0);
}

#[test]
fn gas_one_fails_busy_script() {
    let process = ProcessConfig {
        sandbox: SandboxConfig {
            gas_limit: 1,
            ..SandboxConfig::default()
        },
        ..ProcessConfig::default()
    };
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (resp, err) = run_script_sandboxed(
        &req,
        "while true { }\nres.write(\"x\")",
        &process.body_limits,
        None,
        &process.sandbox,
    )
    .expect("run");
    assert_eq!(resp.status, 500);
    let msg = err.expect("script error");
    assert!(msg.to_ascii_lowercase().contains("gas"), "{msg}");
}

#[test]
fn allow_sys_env_enables_sys_import() {
    let mut process = ProcessConfig::default();
    process.sandbox.capabilities |= ELARA_CAP_SYS_ENV;
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (resp, err) = run_script_sandboxed(
        &req,
        r#"import "sys" as sys
res.write("ok")"#,
        &process.body_limits,
        None,
        &process.sandbox,
    )
    .expect("run");
    assert!(err.is_none(), "{err:?}");
    assert_eq!(String::from_utf8_lossy(&resp.body), "ok");
}
