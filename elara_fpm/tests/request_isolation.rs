use elara_fpm::ffi::{elara_close, elara_get_global, elara_is_nil, set_global_str};
use elara_fpm::{RequestData, SandboxConfig, open_request_state, run_script};
use std::ffi::CString;

#[test]
fn sequential_requests_do_not_leak_globals() {
    let sandbox = SandboxConfig::default();
    unsafe {
        let s1 = open_request_state(&sandbox).expect("open1");
        set_global_str(s1, "LEAK", "from-request-1").expect("set");
        elara_close(s1);

        let s2 = open_request_state(&sandbox).expect("open2");
        let name = CString::new("LEAK").unwrap();
        let v = elara_get_global(s2, name.as_ptr());
        assert!(
            elara_is_nil(v),
            "second request state must not see prior globals"
        );
        elara_close(s2);
    }
}

#[test]
fn sequential_script_runs_are_independent() {
    let req = RequestData {
        method: "GET".into(),
        path: "/".into(),
        ..RequestData::default()
    };
    let (r1, e1) = run_script(&req, r#"res.write("one")"#).expect("r1");
    assert!(e1.is_none(), "{e1:?}");
    assert_eq!(r1.body, b"one");

    let (r2, e2) = run_script(&req, r#"res.write("two")"#).expect("r2");
    assert!(e2.is_none(), "{e2:?}");
    assert_eq!(r2.body, b"two");
    assert_ne!(r2.body, b"onetwo");
}
