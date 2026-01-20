use crate::error::Result;
use crate::state::State;
use serde::Serialize;

#[derive(Serialize)]
struct JailListEntry {
    name: String,
    repo: String,
    branch: String,
    created: String,
    status: String,
}

/// List all jails
pub fn list(json: bool) -> Result<()> {
    let state = State::load()?;
    let jails = state.list_jails();

    if jails.is_empty() {
        if !json {
            println!("No jails found. Create one with: robojail create --name <name> --repo <path>");
        } else {
            println!("[]");
        }
        return Ok(());
    }

    if json {
        let entries: Vec<JailListEntry> = jails
            .iter()
            .map(|j| {
                let status = match j.pid {
                    Some(pid) if State::is_pid_alive(pid) => "running".to_string(),
                    _ => "stopped".to_string(),
                };

                JailListEntry {
                    name: j.name.clone(),
                    repo: j.repo_path.display().to_string(),
                    branch: j.branch_name.clone(),
                    created: j.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    status,
                }
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        // Table format
        println!(
            "{:<20} {:<30} {:<25} {:<20} {:<10}",
            "NAME", "REPO", "BRANCH", "CREATED", "STATUS"
        );
        println!("{}", "-".repeat(105));

        for jail in jails {
            let status = match jail.pid {
                Some(pid) if State::is_pid_alive(pid) => "running",
                _ => "stopped",
            };

            let repo_display = jail
                .repo_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| jail.repo_path.display().to_string());

            let branch_display = jail
                .branch_name
                .strip_prefix("robojail/")
                .unwrap_or(&jail.branch_name);

            println!(
                "{:<20} {:<30} {:<25} {:<20} {:<10}",
                truncate(&jail.name, 19),
                truncate(&repo_display, 29),
                truncate(branch_display, 24),
                jail.created_at.format("%Y-%m-%d %H:%M"),
                status
            );
        }
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
