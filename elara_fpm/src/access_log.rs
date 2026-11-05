//! One-line access log on stderr after each finished request.

use std::cell::RefCell;
use std::io::{self, Write};
use std::time::Instant;

thread_local! {
    static CAPTURE: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Start capturing access lines on this thread (in addition to stderr).
pub fn begin_access_log_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Take captured access lines for this thread, ending capture.
pub fn take_access_log_capture() -> Vec<String> {
    CAPTURE.with(|c| c.borrow_mut().take().unwrap_or_default())
}

/// Format `<method> <path> <status> <elapsed_ms>` (path has no query).
pub fn format_access_log_line(method: &str, path: &str, status: u16, elapsed_ms: u64) -> String {
    format!("{method} {path} {status} {elapsed_ms}")
}

/// Write one access line to `w`. Write failures are ignored.
pub fn write_access_log_to(
    w: &mut dyn Write,
    method: &str,
    path: &str,
    status: u16,
    elapsed_ms: u64,
) {
    let line = format_access_log_line(method, path, status, elapsed_ms);
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            buf.push(line.clone());
        }
    });
    let _ = writeln!(w, "{line}");
}

/// Write one access line to stderr. Write failures are ignored.
pub fn write_access_log(method: &str, path: &str, status: u16, elapsed_ms: u64) {
    write_access_log_to(&mut io::stderr(), method, path, status, elapsed_ms);
}

/// Elapsed whole-request duration in milliseconds (saturating).
pub fn elapsed_ms_since(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_format_matches_fixture() {
        assert_eq!(
            format_access_log_line("GET", "/hi.ela", 200, 3),
            "GET /hi.ela 200 3"
        );
        assert_eq!(
            format_access_log_line("POST", "/a%20b", 404, 0),
            "POST /a%20b 404 0"
        );
    }

    #[test]
    fn write_failure_is_ignored() {
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("fail"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("fail"))
            }
        }
        write_access_log_to(&mut Fail, "GET", "/", 200, 1);
    }
}
