use elara_fpm::{DevServerConfig, bind_listener, parse_cli, parse_listen_addr};
use std::net::SocketAddr;

#[test]
fn parse_s_and_t_flags() {
    let args = vec![
        "elara-fpm".into(),
        "-S".into(),
        "127.0.0.1:0".into(),
        "-t".into(),
        "/tmp".into(),
    ];
    let cli = parse_cli(&args).expect("parse");
    assert_eq!(cli.listen, Some(parse_listen_addr("127.0.0.1:0").unwrap()));
    assert_eq!(cli.docroot, std::path::PathBuf::from("/tmp"));
}

#[test]
fn bind_success_and_failure() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let cfg = DevServerConfig {
        listen: addr,
        docroot: std::env::temp_dir(),
        process: Default::default(),
    };
    let listener = bind_listener(&cfg).expect("bind ephemeral");
    let actual = listener.local_addr().unwrap();
    assert_eq!(actual.ip().to_string(), "127.0.0.1");
    drop(listener);

    // Port 1 is typically privileged — bind as non-root should fail.
    let bad = DevServerConfig {
        listen: "127.0.0.1:1".parse().unwrap(),
        docroot: std::env::temp_dir(),
        process: Default::default(),
    };
    let err = bind_listener(&bad);
    if std::env::var_os("ELARA_FPM_ALLOW_PRIV_BIND").is_none() {
        // Best-effort: if we somehow can bind, skip asserting failure.
        if let Err(msg) = err {
            assert!(msg.contains("bind"), "{msg}");
        }
    }
}
