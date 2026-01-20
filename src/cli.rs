use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "robojail")]
#[command(author, version, about = "Sandboxed environments for AI coding assistants")]
#[command(long_about = "RoboJail creates isolated development environments where AI tools can safely \
modify code without affecting your host system. Each jail is a git worktree with \
read-only access to system binaries and full access to project files.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new jail from a git repository
    #[command(visible_alias = "new")]
    Create {
        /// Name for the jail (alphanumeric, dash, underscore)
        #[arg(short, long)]
        name: String,

        /// Path to the git repository
        #[arg(short, long)]
        repo: PathBuf,

        /// Branch to base the worktree on (defaults to HEAD)
        #[arg(short, long)]
        branch: Option<String>,

        /// Entrypoint program to run (e.g., 'claude', '/usr/bin/python')
        /// The binary will be auto-detected and bind-mounted into the jail
        #[arg(short, long)]
        entrypoint: Option<String>,
    },

    /// List all jails
    #[command(visible_alias = "ls")]
    List {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Enter a jail interactively
    #[command(visible_alias = "shell")]
    Enter {
        /// Name of the jail to enter
        name: String,
    },

    /// Destroy a jail and clean up its worktree
    #[command(visible_alias = "rm")]
    Destroy {
        /// Name of the jail to destroy
        name: String,

        /// Force destruction even if jail is running or has unsaved changes
        #[arg(short, long)]
        force: bool,
    },

    /// Run a command inside a jail
    #[command(visible_alias = "exec")]
    Run {
        /// Name of the jail
        name: String,

        /// Command to run (with arguments)
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Show git status of a jail (external supervisor)
    Status {
        /// Name of the jail
        name: String,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Show full diff output
        #[arg(short, long)]
        diff: bool,
    },
}
