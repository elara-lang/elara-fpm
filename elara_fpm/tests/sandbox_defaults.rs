use elara_fpm::ffi::{elara_close, eval};
use elara_fpm::{
    DEFAULT_GAS_LIMIT, DEFAULT_MEM_LIMIT_BYTES, ELARA_CAP_SAFE_DEFAULT, ELARA_CAP_SYS_ENV,
    SandboxConfig, open_request_state,
};

#[test]
fn documented_defaults_exclude_sys_env() {
    let cfg = SandboxConfig::default();
    assert_eq!(cfg.gas_limit, DEFAULT_GAS_LIMIT);
    assert_eq!(cfg.mem_limit_bytes, DEFAULT_MEM_LIMIT_BYTES);
    assert_eq!(cfg.capabilities, ELARA_CAP_SAFE_DEFAULT);
    assert_eq!(cfg.capabilities & ELARA_CAP_SYS_ENV, 0);
}

#[test]
fn default_sandbox_rejects_sys_module() {
    let s = open_request_state(&SandboxConfig::default()).expect("open");
    let err = eval(
        s,
        "cap",
        r#"import "sys" as sys
return 1"#,
    )
    .expect_err("sys import must fail without ELARA_CAP_SYS_ENV");
    assert!(
        err.to_ascii_lowercase().contains("sandbox")
            || err.to_ascii_lowercase().contains("disabled")
            || err.to_ascii_lowercase().contains("capability"),
        "{err}"
    );
    unsafe {
        elara_close(s);
    }
}

#[test]
fn exhausted_gas_fails_without_hanging() {
    let cfg = SandboxConfig {
        gas_limit: 64,
        ..SandboxConfig::default()
    };
    let s = open_request_state(&cfg).expect("open");
    let err = eval(s, "gas", "while true { }\nreturn 1").expect_err("gas");
    assert!(
        err.to_ascii_lowercase().contains("gas"),
        "expected gas error, got {err}"
    );
    unsafe {
        elara_close(s);
    }
}
