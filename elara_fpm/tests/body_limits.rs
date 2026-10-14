use elara_fpm::{BodyLimits, RequestData, run_script_with_limits};

#[test]
fn oversized_body_is_rejected() {
    let limits = BodyLimits {
        max_body_bytes: 8,
        ..BodyLimits::default()
    };
    let req = RequestData {
        method: "POST".into(),
        path: "/".into(),
        body: "0123456789".into(),
        ..RequestData::default()
    }
    .with_header("Content-Type", "application/x-www-form-urlencoded");

    let err = run_script_with_limits(&req, r#"res.write("no")"#, &limits).expect_err("reject");
    assert!(err.contains("too large"), "got {err}");
}

#[test]
fn oversized_upload_is_rejected() {
    let boundary = "----LimBound";
    let big = "x".repeat(32);
    let body = format!(
        "\
--{boundary}\r\n\
Content-Disposition: form-data; name=\"doc\"; filename=\"big.bin\"\r\n\
Content-Type: application/octet-stream\r\n\
\r\n\
{big}\r\n\
--{boundary}--\r\n"
    );
    let upload_dir = std::env::temp_dir().join(format!("elara-fpm-lim-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&upload_dir);
    let limits = BodyLimits {
        max_body_bytes: 10_000,
        max_upload_bytes: 8,
        upload_dir: upload_dir.to_string_lossy().into_owned(),
    };
    let req = RequestData {
        method: "POST".into(),
        path: "/".into(),
        body,
        ..RequestData::default()
    }
    .with_header(
        "Content-Type",
        format!("multipart/form-data; boundary={boundary}"),
    );

    let err = run_script_with_limits(&req, r#"res.write("no")"#, &limits).expect_err("reject");
    assert!(err.contains("too large"), "got {err}");
}
