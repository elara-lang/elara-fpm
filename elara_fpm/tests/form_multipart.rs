use std::path::PathBuf;

use elara_fpm::{BodyLimits, RequestData, run_script_with_limits};

fn multipart_body(boundary: &str) -> String {
    format!(
        "\
--{boundary}\r\n\
Content-Disposition: form-data; name=\"title\"\r\n\
\r\n\
hello\r\n\
--{boundary}\r\n\
Content-Disposition: form-data; name=\"doc\"; filename=\"note.txt\"\r\n\
Content-Type: text/plain\r\n\
\r\n\
file-bytes\r\n\
--{boundary}--\r\n"
    )
}

#[test]
fn multipart_fills_form_and_files_then_cleans_temp() {
    let boundary = "----ElaraBoundary7";
    let upload_dir = std::env::temp_dir().join(format!("elara-fpm-mp-{}", std::process::id()));
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

    let source = r#"
res.write(req.form["title"])
res.write("|")
res.write(req.files[0].filename)
res.write("|")
res.write(req.files[0].path)
"#;

    let (response, err) = run_script_with_limits(&req, source, &limits).expect("run");
    assert!(err.is_none(), "script error: {err:?}");
    let body = String::from_utf8(response.body).expect("utf8");
    let parts: Vec<&str> = body.split('|').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "hello");
    assert_eq!(parts[1], "note.txt");
    let path = PathBuf::from(parts[2]);
    assert!(
        !path.exists(),
        "temp upload should be removed after request: {path:?}"
    );
}
