use crate::error::{Error, Result};
use std::path::Path;

/// Validate a jail name
pub fn validate_jail_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::InvalidJailName(name.to_string()));
    }

    // Must be alphanumeric, dash, or underscore
    let valid = name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(Error::InvalidJailName(name.to_string()));
    }

    // Must not start with a dash
    if name.starts_with('-') {
        return Err(Error::InvalidJailName(name.to_string()));
    }

    Ok(())
}

/// Validate that a path exists
pub fn validate_path_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::PathNotFound(path.to_path_buf()));
    }
    Ok(())
}

/// Validate that a path is a git repository
pub fn validate_git_repo(path: &Path) -> Result<()> {
    validate_path_exists(path)?;

    let git_dir = path.join(".git");
    if !git_dir.exists() {
        return Err(Error::NotGitRepo(path.to_path_buf()));
    }

    Ok(())
}

/// Check if user namespaces are available
#[allow(dead_code)]
pub fn check_user_namespaces() -> Result<()> {
    // Try to read the sysctl value
    let sysctl_path = Path::new("/proc/sys/kernel/unprivileged_userns_clone");

    if sysctl_path.exists() {
        let value = std::fs::read_to_string(sysctl_path)
            .map_err(|_| Error::NamespacesUnavailable)?;

        if value.trim() != "1" {
            return Err(Error::NamespacesUnavailable);
        }
    }

    // Also try a quick unshare test
    // If the sysctl doesn't exist, the kernel might allow it by default
    // We'll find out when we actually try to create namespaces

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_jail_names() {
        assert!(validate_jail_name("test").is_ok());
        assert!(validate_jail_name("my-jail").is_ok());
        assert!(validate_jail_name("my_jail_123").is_ok());
        assert!(validate_jail_name("AI-Task-1").is_ok());
    }

    #[test]
    fn test_invalid_jail_names() {
        assert!(validate_jail_name("").is_err());
        assert!(validate_jail_name("-starts-with-dash").is_err());
        assert!(validate_jail_name("has spaces").is_err());
        assert!(validate_jail_name("has.dots").is_err());
        assert!(validate_jail_name("has/slashes").is_err());

        // Too long
        let long_name = "a".repeat(65);
        assert!(validate_jail_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_path() {
        assert!(validate_path_exists(Path::new("/")).is_ok());
        assert!(validate_path_exists(Path::new("/nonexistent/path/xyz")).is_err());
    }
}
