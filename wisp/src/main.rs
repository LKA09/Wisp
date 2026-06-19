mod agent;
mod cli;
mod config;
mod error;
mod git;
mod instructions;
mod language;
mod policy;
mod session;
mod workflow;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wisp",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Wisp in the current directory
    Init {
        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },
    /// Check environment and configuration
    Doctor,
    /// Summon agents to complete a task
    Summon {
        /// The task description
        task: String,
        /// Actually invoke Claude and Codex CLIs (default: dry-run)
        #[arg(long)]
        execute_agents: bool,
        /// Allow running with uncommitted changes
        #[arg(long)]
        allow_dirty: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => cli::print_intro(),
        Some(Commands::Init { force }) => cli::init(force),
        Some(Commands::Doctor) => cli::doctor(),
        Some(Commands::Summon {
            task,
            execute_agents,
            allow_dirty,
        }) => {
            cli::summon(&task, execute_agents, allow_dirty);
        }
    }
}
