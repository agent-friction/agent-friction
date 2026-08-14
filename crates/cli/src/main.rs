use std::process::ExitCode;

use agent_friction_cli::cli::Cli;
use agent_friction_cli::run;
use clap::Parser;


fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("agent-friction: {e:#}"); ExitCode::FAILURE }
    }
}
