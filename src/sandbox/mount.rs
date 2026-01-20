//! Mount namespace operations
//!
//! Handles bind mounts, tmpfs, proc, and pivot_root for sandbox filesystem setup.

use crate::error::{Error, Result};
use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::unistd::{chdir, pivot_root as nix_pivot_root};
use std::fs;
use std::path::Path;

/// Make all mounts private to prevent propagation
pub fn make_mounts_private() -> Result<()> {
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_PRIVATE | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| Error::MountFailed {
        path: "/".into(),
        reason: format!("failed to make mounts private: {e}"),
    })
}

/// Mount a tmpfs at the given path
pub fn mount_tmpfs(target: &Path) -> Result<()> {
    mount(
        Some("tmpfs"),
        target,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        Some("mode=0755,size=512M"),
    )
    .map_err(|e| Error::MountFailed {
        path: target.to_path_buf(),
        reason: format!("failed to mount tmpfs: {e}"),
    })
}

/// Create a bind mount
///
/// If readonly is true, the mount is remounted read-only in a second step
/// (bind mounts don't respect MS_RDONLY in the initial mount).
pub fn bind_mount(source: &Path, target: &Path, readonly: bool) -> Result<()> {
    // Step 1: Create the bind mount
    mount(
        Some(source),
        target,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| Error::MountFailed {
        path: target.to_path_buf(),
        reason: format!("bind mount failed: {e}"),
    })?;

    // Step 2: If readonly, remount with MS_RDONLY
    if readonly {
        mount(
            None::<&str>,
            target,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|e| Error::MountFailed {
            path: target.to_path_buf(),
            reason: format!("failed to make mount read-only: {e}"),
        })?;
    }

    Ok(())
}

/// Mount proc filesystem (bind mount from host)
///
/// We bind-mount /proc from the host because mounting a new procfs requires
/// being PID 1 in a new PID namespace, which requires an additional fork.
/// The bind-mounted /proc still works for most purposes.
pub fn mount_proc(target: &Path) -> Result<()> {
    // /proc cannot be remounted read-only, so we bind it writable
    bind_mount(Path::new("/proc"), target, false)
}

/// Set up /dev with minimal devices
pub fn setup_dev(target: &Path) -> Result<()> {
    // Mount tmpfs for /dev
    mount(
        Some("tmpfs"),
        target,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
        Some("mode=0755,size=64K"),
    )
    .map_err(|e| Error::MountFailed {
        path: target.to_path_buf(),
        reason: format!("failed to mount dev tmpfs: {e}"),
    })?;

    // Bind mount essential devices from host
    // This is simpler and safer than creating device nodes
    let devices = [
        "null",
        "zero",
        "random",
        "urandom",
        "tty",
    ];

    for device in &devices {
        let src = Path::new("/dev").join(device);
        let dst = target.join(device);

        if src.exists() {
            // Create an empty file to mount over
            fs::write(&dst, "")?;
            bind_mount(&src, &dst, false)?;
        }
    }

    // Create /dev/pts directory for pseudo-terminals
    let pts_path = target.join("pts");
    fs::create_dir_all(&pts_path)?;

    // Mount devpts for pseudo-terminals
    mount(
        Some("devpts"),
        &pts_path,
        Some("devpts"),
        MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
        Some("newinstance,ptmxmode=0666,mode=0620"),
    )
    .ok(); // Ignore errors - devpts might not be available

    // Create /dev/ptmx symlink
    let ptmx_path = target.join("ptmx");
    let _ = std::os::unix::fs::symlink("pts/ptmx", &ptmx_path);

    // Create /dev/fd symlink
    let fd_path = target.join("fd");
    let _ = std::os::unix::fs::symlink("/proc/self/fd", &fd_path);

    // Create /dev/stdin, /dev/stdout, /dev/stderr symlinks
    let _ = std::os::unix::fs::symlink("/proc/self/fd/0", target.join("stdin"));
    let _ = std::os::unix::fs::symlink("/proc/self/fd/1", target.join("stdout"));
    let _ = std::os::unix::fs::symlink("/proc/self/fd/2", target.join("stderr"));

    // Create /dev/shm directory
    let shm_path = target.join("shm");
    fs::create_dir_all(&shm_path)?;
    mount(
        Some("tmpfs"),
        &shm_path,
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        Some("mode=1777,size=64M"),
    )
    .ok(); // Ignore errors

    Ok(())
}

/// Pivot root to the new root filesystem
pub fn pivot_root(new_root: &Path) -> Result<()> {
    // Change to new root
    chdir(new_root).map_err(|e| {
        Error::SandboxSetup(format!("failed to chdir to new root: {e}"))
    })?;

    // Create a directory to hold the old root temporarily
    let old_root = new_root.join(".old_root");
    fs::create_dir_all(&old_root)?;

    // Pivot root
    nix_pivot_root(new_root, &old_root).map_err(|e| {
        Error::SandboxSetup(format!("pivot_root failed: {e}"))
    })?;

    // Now we're in the new root, unmount and remove old root
    chdir("/").map_err(|e| {
        Error::SandboxSetup(format!("failed to chdir to /: {e}"))
    })?;

    // Unmount old root
    umount2("/.old_root", MntFlags::MNT_DETACH).map_err(|e| {
        Error::SandboxSetup(format!("failed to unmount old root: {e}"))
    })?;

    // Remove old root directory
    fs::remove_dir("/.old_root").ok();

    Ok(())
}

#[cfg(test)]
mod tests {
    // Mount tests require root or namespace privileges and are tested in integration tests
}
