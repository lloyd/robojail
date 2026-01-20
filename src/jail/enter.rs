use crate::config::Config;
use crate::error::Result;
use crate::sandbox::create_jail_sandbox;
use crate::state::State;

/// Enter a jail interactively
pub fn enter(name: &str, config: &Config) -> Result<()> {
    let mut state = State::load()?;
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

    // Update PID in state (we use our PID as a marker that we're running)
    // The actual sandbox runs in a child process
    state.set_pid(name, Some(std::process::id()))?;

    // Create and enter sandbox
    let sandbox = create_jail_sandbox(&worktree_path, config, entrypoint.as_deref());

    // Determine what to run
    let exit_code = if let Some(ref ep) = entrypoint {
        let display_cmd = ep.join(" ");
        println!("Running '{}' in jail '{}'...", display_cmd, name);
        sandbox.run(ep)?
    } else {
        println!("Entering jail '{}'...", name);
        sandbox.enter(&config.default_shell)?
    };

    // Clear PID on exit
    let mut state = State::load()?;
    state.set_pid(name, None)?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}
