use crate::config::Config;
use crate::error::{Error, Result};
use crate::state::{JailInfo, State};
use crate::validation::{validate_git_repo, validate_jail_name};
use chrono::Utc;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

/// Create a new jail from a git repository
pub fn create(name: &str, repo: &Path, branch: Option<&str>, _config: &Config) -> Result<()> {
    // Validate inputs
    validate_jail_name(name)?;
    validate_git_repo(repo)?;

    // Load state
    let mut state = State::load()?;

    // Check if jail already exists
    if state.jails.contains_key(name) {
        return Err(Error::JailExists(name.to_string()));
    }

    // Generate unique branch name
    let short_uuid = &Uuid::new_v4().to_string()[..8];
    let branch_name = format!("robojail/{}-{}", name, short_uuid);

    // Determine base ref
    let base_ref = branch.unwrap_or("HEAD");

    // Create jail directory
    let jails_dir = Config::jails_dir()?;
    let jail_path = jails_dir.join(name);
    std::fs::create_dir_all(&jail_path)?;

    // Create git worktree
    let output = Command::new("git")
        .args([
            "-C",
            repo.to_str().ok_or_else(|| Error::Config("invalid repo path".to_string()))?,
            "worktree",
            "add",
            "-b",
            &branch_name,
            jail_path.to_str().ok_or_else(|| Error::Config("invalid jail path".to_string()))?,
            base_ref,
        ])
        .output()?;

    if !output.status.success() {
        // Clean up directory on failure
        let _ = std::fs::remove_dir_all(&jail_path);

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::WorktreeCreation(stderr.to_string()));
    }

    // Create jail info
    let info = JailInfo {
        id: Uuid::new_v4(),
        name: name.to_string(),
        repo_path: repo.canonicalize()?,
        worktree_path: jail_path.clone(),
        branch_name,
        created_at: Utc::now(),
        pid: None,
    };

    // Add to state
    state.add_jail(info)?;

    println!("Created jail '{}' at {}", name, jail_path.display());
    println!("Branch: robojail/{}-{}", name, short_uuid);

    Ok(())
}
