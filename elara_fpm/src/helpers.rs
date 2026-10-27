//! Small response helpers (escape, error pages).

#![allow(clippy::missing_safety_doc)]

use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_return,
    elara_val_string, value_as_string,
};
use crate::response::FinalResponse;

/// Escape `& < > " '` for HTML text/attribute contexts.
pub fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

pub unsafe extern "C" fn native_html_escape(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        if elara_argc(ctx) < 1 {
            return elara_Status::ErrRuntime;
        }
        let raw = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        let escaped = html_escape(&raw);
        let val = elara_val_string(
            s,
            escaped.as_ptr() as *const std::os::raw::c_char,
            escaped.len(),
        );
        elara_return(ctx, val);
        elara_Status::Ok
    }
}

/// For `HEAD`, keep status/headers, set `Content-Length` to the would-be body size, clear body.
pub fn apply_head_method(method: &str, mut resp: FinalResponse) -> FinalResponse {
    if method.eq_ignore_ascii_case("HEAD") {
        let len = resp.body.len();
        resp.headers
            .insert("Content-Length".into(), len.to_string());
        resp.body.clear();
    }
    resp
}

/// Built-in plain-text status page (no internal paths).
pub fn host_error_page(status: u16, message: &str) -> FinalResponse {
    let mut headers = std::collections::BTreeMap::new();
    headers.insert("Content-Type".into(), "text/plain; charset=utf-8".into());
    let body = format!("{status} {message}\n");
    FinalResponse {
        status,
        headers,
        set_cookies: Vec::new(),
        body: body.into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_covers_required_set() {
        assert_eq!(
            html_escape(r#"<a href="x">&'y"</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;y&quot;&lt;/a&gt;"
        );
        assert_eq!(html_escape("plain"), "plain");
    }
}
