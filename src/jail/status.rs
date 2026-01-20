use crate::error::{Error, Result};
use crate::state::State;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
struct StatusOutput {
    name: String,
    modified: Vec<String>,
    added: Vec<String>,
    deleted: Vec<String>,
    stats: DiffStats,
}

#[derive(Serialize)]
struct DiffStats {
    insertions: u32,
    deletions: u32,
    files_changed: u32,
}

/// Show git status of a jail (external supervisor view)
pub fn status(name: &str, json: bool, show_diff: bool) -> Result<()> {
    let state = State::load()?;
    let jail = state.get_jail(name)?;

    // Check that worktree exists
    if !jail.worktree_path.exists() {
        return Err(Error::JailNotFound(format!(
            "{} (worktree missing at {})",
            name,
            jail.worktree_path.display()
        )));
    }

    // Get git status
    let status_output = Command::new("git")
        .args([
            "-C",
            jail.worktree_path.to_str().unwrap_or("."),
            "status",
            "--porcelain",
        ])
        .output()
        .map_err(|e| Error::GitCommand(format!("failed to run git status: {e}")))?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(Error::GitCommand(format!("git status failed: {stderr}")));
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);

    // Parse status into categories
    let mut modified = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for line in status_str.lines() {
        if line.len() < 3 {
            continue;
        }

        let status_code = &line[0..2];
        let file = line[3..].trim();

        match status_code.trim() {
            "M" | "MM" | "AM" | " M" => modified.push(file.to_string()),
            "A" | "??" => added.push(file.to_string()),
            "D" | " D" => deleted.push(file.to_string()),
            "R" => {
                // Renamed: old -> new
                if let Some((_, new)) = file.split_once(" -> ") {
                    modified.push(new.to_string());
                } else {
                    modified.push(file.to_string());
                }
            }
            _ => {
                // Other status codes - treat as modified
                if !file.is_empty() {
                    modified.push(file.to_string());
                }
            }
        }
    }

    // Get diff stats
    let diff_stat_output = Command::new("git")
        .args([
            "-C",
            jail.worktree_path.to_str().unwrap_or("."),
            "diff",
            "--stat",
            "--stat-width=1000",
        ])
        .output()
        .ok();

    let (insertions, deletions, files_changed) = if let Some(output) = diff_stat_output {
        if output.status.success() {
            parse_diff_stats(&String::from_utf8_lossy(&output.stdout))
        } else {
            (0, 0, 0)
        }
    } else {
        (0, 0, 0)
    };

    if json {
        let output = StatusOutput {
            name: name.to_string(),
            modified,
            added,
            deleted,
            stats: DiffStats {
                insertions,
                deletions,
                files_changed,
            },
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        // Human-readable output
        let total_files = modified.len() + added.len() + deleted.len();

        if total_files == 0 {
            println!("Jail '{}': No changes", name);
        } else {
            println!(
                "Jail '{}': {} file(s) changed (+{}, -{})",
                name, files_changed, insertions, deletions
            );

            if !modified.is_empty() {
                println!("\nModified:");
                for file in &modified {
                    println!("  M {}", file);
                }
            }

            if !added.is_empty() {
                println!("\nAdded:");
                for file in &added {
                    println!("  A {}", file);
                }
            }

            if !deleted.is_empty() {
                println!("\nDeleted:");
                for file in &deleted {
                    println!("  D {}", file);
                }
            }
        }

        // Show diff if requested
        if show_diff {
            println!("\n--- Diff ---\n");

            let diff_output = Command::new("git")
                .args(["-C", jail.worktree_path.to_str().unwrap_or("."), "diff"])
                .output();

            if let Ok(output) = diff_output {
                if output.status.success() {
                    print!("{}", String::from_utf8_lossy(&output.stdout));
                }
            }

            // Also show untracked file contents
            if !added.is_empty() {
                println!("\n--- New Files ---\n");
                for file in &added {
                    let file_path = jail.worktree_path.join(file);
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        println!("=== {} ===", file);
                        println!("{}", content);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Parse the summary line from git diff --stat
fn parse_diff_stats(output: &str) -> (u32, u32, u32) {
    // Look for a line like: " 3 files changed, 42 insertions(+), 10 deletions(-)"
    let lines: Vec<&str> = output.lines().collect();

    if let Some(summary) = lines.last() {
        let mut files = 0u32;
        let mut insertions = 0u32;
        let mut deletions = 0u32;

        // Parse "X files changed"
        if let Some(idx) = summary.find("file") {
            if let Some(num_str) = summary[..idx].split_whitespace().last() {
                files = num_str.parse().unwrap_or(0);
            }
        }

        // Parse "X insertions(+)"
        if let Some(idx) = summary.find("insertion") {
            let before = &summary[..idx];
            if let Some(num_str) = before.split_whitespace().last() {
                insertions = num_str.parse().unwrap_or(0);
            }
        }

        // Parse "X deletions(-)"
        if let Some(idx) = summary.find("deletion") {
            let before = &summary[..idx];
            if let Some(num_str) = before.split(',').last().and_then(|s| s.split_whitespace().last())
            {
                deletions = num_str.parse().unwrap_or(0);
            }
        }

        (insertions, deletions, files)
    } else {
        (0, 0, 0)
    }
}
