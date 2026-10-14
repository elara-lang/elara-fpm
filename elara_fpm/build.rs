use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .expect("elara_fpm package must live under the repo root");
    let script = repo_root.join(".make/elara-env.sh");

    let output = Command::new("bash")
        .arg(&script)
        .arg("--print")
        .output()
        .unwrap_or_else(|err| panic!("failed to run {}: {err}", script.display()));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Elara discovery failed:\n{stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut include = None;
    let mut libdir = None;
    for line in stdout.lines() {
        if let Some(v) = line.strip_prefix("ELARA_INCLUDE=") {
            include = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("ELARA_LIBDIR=") {
            libdir = Some(v.to_string());
        }
    }

    let include = include.expect("ELARA_INCLUDE missing from elara-env.sh --print");
    let libdir = libdir.expect("ELARA_LIBDIR missing from elara-env.sh --print");

    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-env-changed=ELARA_ROOT");
    println!("cargo:rerun-if-env-changed=ELARA_PREFIX");
    println!("cargo:rustc-link-search=native={libdir}");
    println!("cargo:rustc-link-lib=dylib=elara");
    println!("cargo:rustc-env=ELARA_INCLUDE={include}");
}
