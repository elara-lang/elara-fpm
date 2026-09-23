use std::path::PathBuf;

use elara_http::{BodyLimits, RequestData, run_script_with_limits};

fn multipart_body(boundary: &str) -> String {
    format!(
        "\
--{boundary}\r\n\
Content-Disposition: form-data; name=\"doc\"; filename=\"note.txt\"\r\n\
Content-Type: text/plain\r\n\
\r\n\
file-bytes\r\n\
--{boundary}--\r\n"
    )
}

#[test]
fn upload_temps_removed_after_script_failure() {
    let boundary = "----ElaraTeardown9";
    let upload_dir = std::env::temp_dir().join(format!("elara-http-td-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&upload_dir);
    std::fs::create_dir_all(&upload_dir).unwrap();

    let limits = BodyLimits {
        upload_dir: upload_dir.to_string_lossy().into_owned(),
        ..BodyLimits::default()
    };

    let req = RequestData {
        method: "POST".into(),
        path: "/upload".into(),
        body: multipart_body(boundary),
        ..RequestData::default()
    }
    .with_header(
        "Content-Type",
        format!("multipart/form-data; boundary={boundary}"),
    );

    // Capture path then fail the script.
    let (response, err) = run_script_with_limits(
        &req,
        r#"
let p = req.files[0].path
res.write(p)
missing_fn()
"#,
        &limits,
    )
    .expect("host run");
    assert!(err.is_some(), "expected script failure");
    assert_eq!(response.status, 500);

    // After failure teardown, upload dir should have no leftover files.
    let leftovers: Vec<PathBuf> = std::fs::read_dir(&upload_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "upload temps must be removed on failure: {leftovers:?}"
    );
}
