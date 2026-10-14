use elara_fpm::{RequestData, Route, load_script_source, resolve_route, run_script};
use std::fs;

#[test]
fn ela_file_routes_to_script_and_runs() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-srte-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("app.ela"), r#"res.write("hi")"#).unwrap();

    match resolve_route(&dir, "/app.ela") {
        Route::Script(path) => {
            let src = load_script_source(&path).expect("load");
            let req = RequestData {
                method: "GET".into(),
                path: "/app.ela".into(),
                ..RequestData::default()
            };
            let (resp, err) = run_script(&req, &src).expect("run");
            assert!(err.is_none(), "{err:?}");
            assert_eq!(resp.body, b"hi");
        }
        other => panic!("expected Script, got {other:?}"),
    }
}

#[test]
fn directory_serves_index_ela() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-idx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("app")).unwrap();
    fs::write(dir.join("app/index.ela"), r#"res.write("idx")"#).unwrap();

    match resolve_route(&dir, "/app/") {
        Route::Script(path) => {
            assert!(path.ends_with("index.ela"));
            let src = load_script_source(&path).unwrap();
            let req = RequestData {
                method: "GET".into(),
                path: "/app/".into(),
                ..RequestData::default()
            };
            let (resp, err) = run_script(&req, &src).unwrap();
            assert!(err.is_none(), "{err:?}");
            assert_eq!(resp.body, b"idx");
        }
        other => panic!("expected Script, got {other:?}"),
    }
}

#[test]
fn missing_ela_is_404() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-miss-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    match resolve_route(&dir, "/missing.ela") {
        Route::Error(resp) => assert_eq!(resp.status, 404),
        other => panic!("expected Error, got {other:?}"),
    }
}
