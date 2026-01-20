use crate::config::Config;
use crate::error::Result;
use crate::sandbox::create_jail_sandbox;
use crate::state::State;

/// Run a command inside a jail
pub fn run(name: &str, command: &[String], config: &Config) -> Result<i32> {
    let state = State::load()?;
    let jail = state.get_jail(name)?;

    // Check that worktree still exists
    if !jail.worktree_path.exists() {
        eprintln!(
            "warning: worktree directory missing at {}",
            jail.worktree_path.display()
        );
    }

    let worktree_path = jail.worktree_path.clone();
    let entrypoint = jail.entrypoint.clone();

    // Create sandbox and run command
    // We pass entrypoint so it gets bind-mounted even for explicit commands
    let sandbox = create_jail_sandbox(&worktree_path, config, entrypoint.as_deref());
    sandbox.run(command)
}

