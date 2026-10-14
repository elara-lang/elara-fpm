use elara_fpm::resolve_under_root;
use std::fs;

#[test]
fn parent_segments_are_rejected() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-pc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let err = resolve_under_root(&dir, "/../../etc/passwd").unwrap_err();
    assert_eq!(err.status, 400);
}

#[test]
fn dotfiles_are_rejected() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-pc2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let err = resolve_under_root(&dir, "/.env").unwrap_err();
    assert_eq!(err.status, 404);
}

#[test]
fn paths_stay_under_docroot() {
    let dir = std::env::temp_dir().join(format!("elara-fpm-pc3-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/a.txt"), b"x").unwrap();
    let resolved = resolve_under_root(&dir, "/sub/a.txt").unwrap();
    assert!(resolved.starts_with(fs::canonicalize(&dir).unwrap()));
}
