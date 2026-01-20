//! Linux namespace creation and configuration
//!
//! This module handles creating user, mount, PID, and IPC namespaces
//! for unprivileged sandboxing.

use crate::error::{Error, Result};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getgid, getuid};
use std::fs;
use std::io::Write;

/// Set up user namespace with UID/GID mapping
///
/// This must be called first, before any other namespace operations.
/// It creates a user namespace where the current user is mapped to root (UID 0).
pub fn setup_user_namespace() -> Result<()> {
    let uid = getuid();
    let gid = getgid();

    // Create user namespace
    unshare(CloneFlags::CLONE_NEWUSER).map_err(|e| {
        if e == nix::Error::EPERM {
            Error::NamespacesUnavailable
        } else {
            Error::SandboxSetup(format!("failed to create user namespace: {e}"))
        }
    })?;

    // Write UID mapping: map our UID to 0 inside the namespace
    // Format: <inside_uid> <outside_uid> <count>
    let uid_map = format!("0 {} 1", uid);
    write_to_proc_file("/proc/self/uid_map", &uid_map)?;

    // CRITICAL: Deny setgroups before writing gid_map
    // This is a security requirement to prevent privilege escalation
    write_to_proc_file("/proc/self/setgroups", "deny")?;

    // Write GID mapping
    let gid_map = format!("0 {} 1", gid);
    write_to_proc_file("/proc/self/gid_map", &gid_map)?;

    Ok(())
}

/// Set up mount and IPC namespaces
///
/// Must be called after setup_user_namespace().
///
/// Note: We skip CLONE_NEWPID because mounting /proc for a new PID namespace
/// requires being PID 1 in that namespace (which requires an additional fork).
/// For simplicity, we rely on mount namespace isolation instead.
pub fn setup_other_namespaces(share_net: bool) -> Result<()> {
    let mut flags = CloneFlags::CLONE_NEWNS   // Mount namespace
                  | CloneFlags::CLONE_NEWIPC  // IPC namespace
                  | CloneFlags::CLONE_NEWUTS; // UTS namespace (hostname)

    if !share_net {
        flags |= CloneFlags::CLONE_NEWNET; // Network namespace
    }

    unshare(flags).map_err(|e| {
        Error::SandboxSetup(format!("failed to create namespaces: {e}"))
    })?;

    // Set hostname inside UTS namespace
    nix::unistd::sethostname("robojail").ok();

    Ok(())
}

/// Helper to write to a /proc file
fn write_to_proc_file(path: &str, content: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| Error::SandboxSetup(format!("failed to open {path}: {e}")))?;

    file.write_all(content.as_bytes())
        .map_err(|e| Error::SandboxSetup(format!("failed to write to {path}: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // Namespace tests require actual namespace support and are tested in integration tests
}
