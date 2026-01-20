//! Security hardening for sandboxed processes
//!
//! Applies various security restrictions including:
//! - PR_SET_NO_NEW_PRIVS to prevent privilege escalation
//! - Creating a new session to prevent TIOCSTI injection
//! - Dropping capabilities

use crate::error::{Error, Result};
use nix::unistd::setsid;

/// Apply security restrictions to the current process
pub fn apply_security_restrictions() -> Result<()> {
    // Set PR_SET_NO_NEW_PRIVS
    // This prevents the process from gaining new privileges via setuid binaries
    set_no_new_privs()?;

    // Create a new session
    // This prevents TIOCSTI attacks where a sandboxed process could inject
    // input into the controlling terminal
    create_new_session()?;

    // Note: We don't drop capabilities here because we need them for mounts.
    // Capabilities are implicitly limited by the user namespace - we only have
    // capabilities within our namespace, not on the host.

    Ok(())
}

/// Set PR_SET_NO_NEW_PRIVS
fn set_no_new_privs() -> Result<()> {
    // PR_SET_NO_NEW_PRIVS = 38
    const PR_SET_NO_NEW_PRIVS: libc::c_int = 38;

    let result = unsafe { libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };

    if result != 0 {
        return Err(Error::SandboxSetup(
            "failed to set PR_SET_NO_NEW_PRIVS".to_string(),
        ));
    }

    Ok(())
}

/// Create a new session (setsid)
///
/// This detaches from the controlling terminal, preventing TIOCSTI attacks.
fn create_new_session() -> Result<()> {
    // setsid() creates a new session and process group
    // This may fail if we're already a session leader, which is fine
    match setsid() {
        Ok(_) => Ok(()),
        Err(nix::Error::EPERM) => {
            // Already a session leader, that's fine
            Ok(())
        }
        Err(e) => Err(Error::SandboxSetup(format!("setsid failed: {e}"))),
    }
}

/// Get the current process capabilities (for debugging)
#[allow(dead_code)]
pub fn get_caps_info() -> String {
    // Read /proc/self/status for capability info
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let cap_lines: Vec<&str> = status
            .lines()
            .filter(|line| line.starts_with("Cap"))
            .collect();
        cap_lines.join("\n")
    } else {
        "could not read capability info".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_new_privs() {
        // This should succeed even without namespaces
        assert!(set_no_new_privs().is_ok());
    }
}
