mod cli;
mod config;
mod error;
mod jail;
mod sandbox;
mod state;
mod validation;

use clap::Parser;
use cli::{Cli, Command};
use error::Result;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::load()?;

    match cli.command {
        Command::Create { name, repo, branch, entrypoint } => {
            jail::create(&name, &repo, branch.as_deref(), entrypoint.as_deref(), &config)?;
        }
        Command::List { json } => {
            jail::list(json)?;
        }
        Command::Enter { name } => {
            jail::enter(&name, &config)?;
        }
        Command::Destroy { name, force } => {
            jail::destroy(&name, force)?;
        }
        Command::Run { name, command } => {
            let code = jail::run(&name, &command, &config)?;
            std::process::exit(code);
        }
        Command::Status { name, json, diff } => {
            jail::status(&name, json, diff)?;
        }
    }

    Ok(())
}
