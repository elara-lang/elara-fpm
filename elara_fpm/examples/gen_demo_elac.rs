//! One-shot helper to rebuild demos/hi.elac (not part of normal builds).
use elara_fpm::{RequestData, compile_wrapped_script_to_bytecode};
use std::path::PathBuf;

fn main() {
    let req = RequestData {
        method: "GET".into(),
        path: "/hi.elac".into(),
        ..RequestData::default()
    };
    let bytes =
        compile_wrapped_script_to_bytecode(&req, r#"res.write("elac-ok\n")"#).expect("compile");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../demos/hi.elac");
    std::fs::write(&out, bytes).expect("write hi.elac");
    println!("wrote {}", out.display());
}
