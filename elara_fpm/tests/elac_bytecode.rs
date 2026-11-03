use elara_fpm::{BodyLimits, SandboxConfig};
use elara_fpm::{
    RequestData, compile_wrapped_script_to_bytecode, dispatch_request, run_bytecode_sandboxed,
};
use std::fs;

#[test]
fn compiled_elac_returns_body() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-elac-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let req = RequestData {
        method: "GET".into(),
        path: "/app.elac".into(),
        ..RequestData::default()
    };
    let bytes =
        compile_wrapped_script_to_bytecode(&req, r#"res.write(req.method)"#).expect("compile");
    fs::write(dir.join("app.elac"), &bytes).unwrap();

    let resp = dispatch_request(&dir, &req);
    assert_eq!(
        resp.status,
        200,
        "{:?}",
        String::from_utf8_lossy(&resp.body)
    );
    assert_eq!(resp.body, b"GET");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn truncated_elac_is_500() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (resp, err) = run_bytecode_sandboxed(
        &req,
        b"not-valid-bytecode",
        &BodyLimits::default(),
        None,
        &SandboxConfig::default(),
    )
    .expect("run");
    assert!(err.is_some() || resp.status == 500);
    assert_eq!(resp.status, 500, "truncated bytecode must be a host 500");
}
