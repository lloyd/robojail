use crate::config::Config;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailInfo {
    pub id: Uuid,
    pub name: String,
    pub repo_path: PathBuf,
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Entrypoint command to run (first element is resolved path, rest are args)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub jails: HashMap<String, JailInfo>,
}

impl State {
    /// Load state from file, or create empty state
    pub fn load() -> Result<Self> {
        let state_path = Self::state_path()?;

        if state_path.exists() {
            let content = fs::read_to_string(&state_path)?;
            let state: State = serde_json::from_str(&content).map_err(|e| {
                Error::StateCorrupted(format!("invalid JSON: {e}"))
            })?;
            Ok(state)
        } else {
            Ok(State::default())
        }
    }

    /// Save state to file atomically
    pub fn save(&self) -> Result<()> {
        let state_path = Self::state_path()?;

        // Ensure parent directory exists
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to temp file first, then rename (atomic)
        let temp_path = state_path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&temp_path, content)?;
        fs::rename(&temp_path, &state_path)?;

        Ok(())
    }

    /// Get state file path
    fn state_path() -> Result<PathBuf> {
        Ok(Config::state_dir()?.join("jails.json"))
    }

    /// Add a new jail
    pub fn add_jail(&mut self, info: JailInfo) -> Result<()> {
        if self.jails.contains_key(&info.name) {
            return Err(Error::JailExists(info.name.clone()));
        }
        self.jails.insert(info.name.clone(), info);
        self.save()
    }

    /// Remove a jail by name
    pub fn remove_jail(&mut self, name: &str) -> Result<JailInfo> {
        let info = self.jails.remove(name)
            .ok_or_else(|| Error::JailNotFound(name.to_string()))?;
        self.save()?;
        Ok(info)
    }

    /// Get a jail by name
    pub fn get_jail(&self, name: &str) -> Result<&JailInfo> {
        self.jails.get(name)
            .ok_or_else(|| Error::JailNotFound(name.to_string()))
    }

    /// Get a mutable reference to a jail
    pub fn get_jail_mut(&mut self, name: &str) -> Result<&mut JailInfo> {
        self.jails.get_mut(name)
            .ok_or_else(|| Error::JailNotFound(name.to_string()))
    }

    /// Update jail PID
    pub fn set_pid(&mut self, name: &str, pid: Option<u32>) -> Result<()> {
        let jail = self.get_jail_mut(name)?;
        jail.pid = pid;
        self.save()
    }

    /// Check if a PID is still alive
    pub fn is_pid_alive(pid: u32) -> bool {
        // Check if process exists by sending signal 0
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    /// Get all jails as a list
    pub fn list_jails(&self) -> Vec<&JailInfo> {
        let mut jails: Vec<_> = self.jails.values().collect();
        jails.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        jails
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jail_info_serialize() {
        let info = JailInfo {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            repo_path: PathBuf::from("/home/user/repo"),
            worktree_path: PathBuf::from("/home/user/.local/share/robojail/jails/test"),
            branch_name: "robojail/test-abc123".to_string(),
            created_at: Utc::now(),
            pid: None,
            entrypoint: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: JailInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.name, parsed.name);
    }

    #[test]
    fn test_state_add_remove() {
        let mut state = State::default();
        let info = JailInfo {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            repo_path: PathBuf::from("/repo"),
            worktree_path: PathBuf::from("/jail"),
            branch_name: "robojail/test".to_string(),
            created_at: Utc::now(),
            pid: None,
            entrypoint: None,
        };

        // Can't actually save in tests without mocking, but we can test logic
        state.jails.insert(info.name.clone(), info);
        assert!(state.get_jail("test").is_ok());
        assert!(state.get_jail("nonexistent").is_err());
    }
}
