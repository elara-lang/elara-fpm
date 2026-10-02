//! Elara HTTP host library (SAPI).

pub mod body;
pub mod cgi;
pub mod cookies;
pub mod fastcgi;
pub mod ffi;
pub mod helpers;
pub mod multipart;
pub mod response;
pub mod routing;
pub mod sandbox;
pub mod script_api;
pub mod server;
pub mod session;

use crate::ffi::{elara_Status, elara_close, elara_eval, elara_is_int, elara_open, elara_to_int};

pub use body::{BodyError, BodyLimits, parse_urlencoded};
pub use cgi::{
    cgi_env_from_os, format_cgi_response, handle_cgi_request, read_cgi_stdin, request_from_cgi_env,
    run_cgi_once, write_cgi_stdout,
};
pub use cookies::{CookieOptions, parse_cookie_header};
pub use fastcgi::{
    FastCgiConfig, encode_test_request, extract_stdout, handle_fastcgi_connection,
    run_fastcgi_worker,
};
pub use helpers::{host_error_page, html_escape};
pub use response::{FinalResponse, ResponseBuffer};
pub use routing::{
    Route, content_type_for, load_script_source, resolve_route, resolve_under_root, serve_static,
};
pub use sandbox::{
    DEFAULT_GAS_LIMIT, DEFAULT_MEM_LIMIT_BYTES, ELARA_CAP_SAFE_DEFAULT, ELARA_CAP_SYS_ENV,
    SandboxConfig, open_request_state,
};
pub use script_api::{
    RequestData, ScriptBindings, UploadedFile, run_script, run_script_on, run_script_sandboxed,
    run_script_with_limits, run_script_with_session, smoke_inject_req_res,
};
pub use server::{
    CliArgs, DevServerConfig, bind_listener, dispatch_request, parse_cli, parse_listen_addr,
    print_listen_banner, run_dev_server,
};
pub use session::{FileSessionStore, Session, SessionConfig};

/// Open a state, evaluate `return 1+1`, assert the integer result is 2, then close.
pub fn smoke_eval_one_plus_one() -> Result<(), String> {
    unsafe {
        let s = elara_open();
        if s.is_null() {
            return Err("elara_open returned null".into());
        }

        let mut result = crate::ffi::elara_val_nil();
        let name = c"smoke";
        let source = c"return 1+1";
        let status = elara_eval(s, name.as_ptr(), source.as_ptr(), &mut result);
        if status != elara_Status::Ok {
            let msg = crate::ffi::last_error_message(s);
            elara_close(s);
            return Err(msg);
        }

        if !elara_is_int(result) {
            elara_close(s);
            return Err("result is not an int".into());
        }
        let n = elara_to_int(result);
        elara_close(s);
        if n != 2 {
            return Err(format!("expected 2, got {n}"));
        }
        Ok(())
    }
}
