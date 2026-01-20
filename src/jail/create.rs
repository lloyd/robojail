use crate::config::Config;
use crate::error::{Error, Result};
use crate::state::{JailInfo, State};
use crate::validation::{validate_git_repo, validate_jail_name};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

/// Parse and resolve an entrypoint string into command + args
/// e.g., "claude --dangerously-skip-permissions" -> ["/path/to/claude", "--dangerously-skip-permissions"]
fn parse_entrypoint(entrypoint: &str) -> Result<Vec<String>> {
    // Simple shell-like parsing (split on whitespace, respect quotes)
    let parts = shell_words::split(entrypoint)
        .map_err(|e| Error::Config(format!("invalid entrypoint syntax: {e}")))?;

    if parts.is_empty() {
        return Err(Error::Config("entrypoint cannot be empty".to_string()));
    }

    // Resolve the command (first part)
    let cmd = &parts[0];
    let resolved_cmd = resolve_command(cmd)?;

    // Build result: resolved command + original args
    let mut result = vec![resolved_cmd.to_string_lossy().to_string()];
    result.extend(parts[1..].iter().cloned());

    Ok(result)
}

/// Resolve a command to an absolute path
fn resolve_command(cmd: &str) -> Result<PathBuf> {
    let path = Path::new(cmd);

    // If it's already an absolute path, just verify it exists
    if path.is_absolute() {
        if path.exists() {
            return Ok(path.to_path_buf());
        } else {
            return Err(Error::Config(format!(
                "entrypoint not found: {}",
                cmd
            )));
        }
    }

    // Otherwise, search PATH using `which`
    let output = Command::new("which")
        .arg(cmd)
        .output()?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout);
        let resolved = PathBuf::from(path_str.trim());
        if resolved.exists() {
            return Ok(resolved);
        }
    }

    Err(Error::Config(format!(
        "entrypoint '{}' not found in PATH",
        cmd
    )))
}

/// Create a new jail from a git repository
pub fn create(name: &str, repo: &Path, branch: Option<&str>, entrypoint: Option<&str>, _config: &Config) -> Result<()> {
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

    // Parse and resolve entrypoint if provided
    let resolved_entrypoint = if let Some(ep) = entrypoint {
        let parsed = parse_entrypoint(ep)?;
        if parsed.len() == 1 {
            println!("Entrypoint: {}", parsed[0]);
        } else {
            println!("Entrypoint: {} {}", parsed[0], parsed[1..].join(" "));
        }
        Some(parsed)
    } else {
        None
    };

    // Create jail info
    let info = JailInfo {
        id: Uuid::new_v4(),
        name: name.to_string(),
        repo_path: repo.canonicalize()?,
        worktree_path: jail_path.clone(),
        branch_name,
        created_at: Utc::now(),
        pid: None,
        entrypoint: resolved_entrypoint,
    };

    // Add to state
    state.add_jail(info)?;

    println!("Created jail '{}' at {}", name, jail_path.display());
    println!("Branch: robojail/{}-{}", name, short_uuid);

    Ok(())
}
