//! Per-request VM sandbox defaults.

use crate::ffi::{elara_Config, elara_State, elara_default_config, elara_open_with_config};

/// Capability bits (mirror `elara.h`).
pub const ELARA_CAP_MATH: u32 = 1 << 0;
pub const ELARA_CAP_STRING: u32 = 1 << 1;
pub const ELARA_CAP_TABLE: u32 = 1 << 2;
pub const ELARA_CAP_BIT: u32 = 1 << 3;
pub const ELARA_CAP_FIBER: u32 = 1 << 4;
pub const ELARA_CAP_SYS_ENV: u32 = 1 << 5;
pub const ELARA_CAP_STDIO: u32 = 1 << 6;

/// Safe default caps: pure computation only — **no** `ELARA_CAP_SYS_ENV`.
pub const ELARA_CAP_SAFE_DEFAULT: u32 =
    ELARA_CAP_MATH | ELARA_CAP_STRING | ELARA_CAP_TABLE | ELARA_CAP_BIT | ELARA_CAP_FIBER;

/// Default gas budget per request (finite; operators may raise).
pub const DEFAULT_GAS_LIMIT: u64 = 10_000_000;

/// Default memory budget per request in bytes (finite; operators may raise).
pub const DEFAULT_MEM_LIMIT_BYTES: usize = 32 * 1024 * 1024;

/// Operator-overridable sandbox settings for one request.
#[derive(Debug, Clone, Copy)]
pub struct SandboxConfig {
    pub gas_limit: u64,
    pub mem_limit_bytes: usize,
    pub capabilities: u32,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            gas_limit: DEFAULT_GAS_LIMIT,
            mem_limit_bytes: DEFAULT_MEM_LIMIT_BYTES,
            capabilities: ELARA_CAP_SAFE_DEFAULT,
        }
    }
}

impl SandboxConfig {
    pub fn to_elara_config(self) -> elara_Config {
        let mut cfg = unsafe { elara_default_config() };
        cfg.gas_limit = self.gas_limit;
        cfg.mem_limit_bytes = self.mem_limit_bytes;
        cfg.capabilities = self.capabilities;
        cfg
    }
}

/// Open a fresh VM state for one request with sandbox quotas applied.
pub fn open_request_state(sandbox: &SandboxConfig) -> Result<*mut elara_State, String> {
    let cfg = sandbox.to_elara_config();
    let state = unsafe { elara_open_with_config(&cfg) };
    if state.is_null() {
        return Err("elara_open_with_config returned null".into());
    }
    Ok(state)
}
