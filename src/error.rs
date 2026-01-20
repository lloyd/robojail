use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("jail '{0}' not found")]
    JailNotFound(String),

    #[error("jail '{0}' already exists")]
    JailExists(String),

    #[error("jail '{0}' is currently running (use --force to destroy)")]
    JailRunning(String),

    #[error("not a git repository: {0}")]
    NotGitRepo(PathBuf),

    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("invalid jail name '{0}': must be 1-64 alphanumeric characters, dashes, or underscores")]
    InvalidJailName(String),

    #[error("user namespaces are not available on this system\n\
             hint: check that /proc/sys/kernel/unprivileged_userns_clone is set to 1")]
    NamespacesUnavailable,

    #[error("sandbox setup failed: {0}")]
    SandboxSetup(String),

    #[error("mount failed for {path}: {reason}")]
    MountFailed { path: PathBuf, reason: String },

    #[error("failed to create worktree: {0}")]
    WorktreeCreation(String),

    #[error("failed to remove worktree: {0}")]
    WorktreeRemoval(String),

    #[error("git command failed: {0}")]
    GitCommand(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("state file corrupted: {0}")]
    StateCorrupted(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("system error: {0}")]
    Nix(#[from] nix::Error),
}
