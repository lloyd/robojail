use crate::error::{Error, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default shell to use inside jails
    pub default_shell: String,

    /// Whether to share network with host
    pub network_enabled: bool,

    /// Additional paths to bind read-only
    pub extra_ro_binds: Vec<PathBuf>,

    /// Additional paths to bind read-write
    pub extra_rw_binds: Vec<PathBuf>,

    /// Paths in home directory to hide (relative to $HOME)
    pub hidden_paths: Vec<String>,

    /// Environment variables to pass through to jail
    pub env_passthrough: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_shell: "/bin/bash".to_string(),
            network_enabled: true,
            extra_ro_binds: vec![],
            extra_rw_binds: vec![],
            hidden_paths: vec![
                ".ssh".to_string(),
                ".gnupg".to_string(),
                ".aws".to_string(),
                ".config/gcloud".to_string(),
                ".kube".to_string(),
                ".docker".to_string(),
                ".npmrc".to_string(),
                ".pypirc".to_string(),
                ".netrc".to_string(),
            ],
            env_passthrough: vec![
                "TERM".to_string(),
                "LANG".to_string(),
                "LC_ALL".to_string(),
                "COLORTERM".to_string(),
            ],
        }
    }
}

impl Config {
    /// Load config from XDG config directory, or use defaults
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "robojail")
            .ok_or_else(|| Error::Config("could not determine config directory".to_string()))?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    /// Get the data directory (for jails)
    pub fn data_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "robojail")
            .ok_or_else(|| Error::Config("could not determine data directory".to_string()))?;
        Ok(dirs.data_dir().to_path_buf())
    }

    /// Get the state directory
    pub fn state_dir() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "robojail")
            .ok_or_else(|| Error::Config("could not determine state directory".to_string()))?;
        Ok(dirs.state_dir()
            .unwrap_or_else(|| dirs.data_dir())
            .to_path_buf())
    }

    /// Get the jails directory
    pub fn jails_dir() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("jails"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_shell, "/bin/bash");
        assert!(config.network_enabled);
        assert!(config.hidden_paths.contains(&".ssh".to_string()));
    }

    #[test]
    fn test_config_parse() {
        let toml_str = r#"
            default_shell = "/bin/zsh"
            network_enabled = false
            hidden_paths = [".ssh", ".gnupg"]
            env_passthrough = ["TERM"]
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.default_shell, "/bin/zsh");
        assert!(!config.network_enabled);
    }
}
