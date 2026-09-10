//! Elara HTTP host library (SAPI).

pub mod body;
pub mod ffi;
pub mod response;
pub mod script_api;

use crate::ffi::{elara_Status, elara_close, elara_eval, elara_is_int, elara_open, elara_to_int};

pub use body::{BodyError, BodyLimits, parse_urlencoded};
pub use response::{FinalResponse, ResponseBuffer};
pub use script_api::{
    RequestData, ScriptBindings, UploadedFile, run_script, run_script_on, run_script_with_limits,
    smoke_inject_req_res,
};

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
