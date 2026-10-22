//! Process-wide operator configuration (CLI flags).

use crate::body::BodyLimits;
use crate::sandbox::SandboxConfig;
use crate::session::SessionConfig;

/// Shared operator knobs for `-S`, CGI, and `-F` in one process.
#[derive(Debug, Clone, Default)]
pub struct ProcessConfig {
    pub sandbox: SandboxConfig,
    pub body_limits: BodyLimits,
    pub session: SessionConfig,
}
