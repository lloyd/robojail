use crate::error::{Error, Result};
use crate::state::State;
use std::process::Command;

/// Destroy a jail and clean up its worktree
pub fn destroy(name: &str, force: bool) -> Result<()> {
    let mut state = State::load()?;
    let jail = state.get_jail(name)?;

    // Check if running
    if let Some(pid) = jail.pid {
        if State::is_pid_alive(pid) {
            if !force {
                return Err(Error::JailRunning(name.to_string()));
            }

            // Kill the process
            eprintln!("Killing running jail process (PID {})...", pid);
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }

            // Give it a moment to terminate
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Force kill if still alive
            if State::is_pid_alive(pid) {
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }
    }

    let repo_path = jail.repo_path.clone();
    let worktree_path = jail.worktree_path.clone();

    // Try to remove git worktree
    let output = Command::new("git")
        .args([
            "-C",
            repo_path.to_str().unwrap_or("."),
            "worktree",
            "remove",
            worktree_path.to_str().unwrap_or("."),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            // Success
        }
        Ok(out) => {
            // Worktree removal failed, try with --force
            let stderr = String::from_utf8_lossy(&out.stderr);

            if force || stderr.contains("dirty") || stderr.contains("untracked") {
                let force_output = Command::new("git")
                    .args([
                        "-C",
                        repo_path.to_str().unwrap_or("."),
                        "worktree",
                        "remove",
                        "--force",
                        worktree_path.to_str().unwrap_or("."),
                    ])
                    .output();

                if let Ok(out) = force_output {
                    if !out.status.success() {
                        eprintln!(
                            "warning: git worktree remove --force failed: {}",
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                }
            } else {
                return Err(Error::WorktreeRemoval(stderr.to_string()));
            }
        }
        Err(e) => {
            eprintln!("warning: failed to run git worktree remove: {e}");
        }
    }

    // Clean up directory if it still exists
    if worktree_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&worktree_path) {
            eprintln!(
                "warning: failed to remove jail directory {}: {e}",
                worktree_path.display()
            );
        }
    }

    // Prune worktrees
    let _ = Command::new("git")
        .args(["-C", repo_path.to_str().unwrap_or("."), "worktree", "prune"])
        .output();

    // Remove from state
    state.remove_jail(name)?;

    println!("Destroyed jail '{}'", name);

    Ok(())
}
