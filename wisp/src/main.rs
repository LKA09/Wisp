mod agent;
mod cli;
mod config;
mod display;
mod error;
mod git;
mod input;
mod instructions;
mod language;
mod policy;
mod session;
mod workflow;

use crate::agent::PermissionMode;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wisp",
    version,
    about = "Local personal coding agent orchestrator"
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
        /// Permission mode for agent CLI execution
        #[arg(long, value_enum, default_value = "interactive")]
        permission: PermissionMode,
    },
    /// Run a single agent directly
    Ask {
        /// The agent name, for example claude or codex
        agent: String,
        /// The task description
        task: String,
        /// Actually invoke the agent CLI (default: dry-run)
        #[arg(long)]
        execute_agents: bool,
        /// Allow running with uncommitted changes
        #[arg(long)]
        allow_dirty: bool,
        /// Permission mode for agent CLI execution
        #[arg(long, value_enum, default_value = "interactive")]
        permission: PermissionMode,
    },
}

fn main() {
    // Enable ANSI color output on Windows (needed for cmd.exe / older PowerShell).
    // Windows Terminal and PowerShell 7+ already support ANSI by default.
    #[cfg(windows)]
    enable_ansi_windows();

    let cli = Cli::parse();

    match cli.command {
        None => cli::interactive(),
        Some(Commands::Init { force }) => cli::init(force),
        Some(Commands::Doctor) => cli::doctor(),
        Some(Commands::Summon {
            task,
            execute_agents,
            allow_dirty,
            permission,
        }) => {
            cli::summon(&task, execute_agents, allow_dirty, permission);
        }
        Some(Commands::Ask {
            agent,
            task,
            execute_agents,
            allow_dirty,
            permission,
        }) => {
            cli::ask(&agent, &task, execute_agents, allow_dirty, permission);
        }
    }
}

#[cfg(windows)]
fn enable_ansi_windows() {
    // Set ENABLE_VIRTUAL_TERMINAL_PROCESSING on the Windows console.
    // Best-effort; fails silently when output is piped or on old Windows.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
    }

    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() {
            return;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}
