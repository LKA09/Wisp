/// Terminal conversation UI for Wisp.
/// Uses ANSI escape codes — works on Windows Terminal, macOS Terminal, and most Linux terminals.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const W: usize = 64;

// ANSI codes
const RST:  &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM:  &str = "\x1b[2m";
const CYAN: &str = "\x1b[96m";   // Claude
const YLW:  &str = "\x1b[93m";   // Codex
const GRN:  &str = "\x1b[92m";   // success
const RED:  &str = "\x1b[91m";   // error
const MAG:  &str = "\x1b[95m";   // Wisp brand
const GRAY: &str = "\x1b[90m";   // dim chrome

// ─── Session header ───────────────────────────────────────────────────────────

pub fn header(task: &str, branch: &str, mode: &str, n_instructions: usize) {
    println!();
    thick_rule();
    println!();
    println!("  {}{}✦ Wisp{}", BOLD, MAG, RST);
    println!("  {}{}{}", BOLD, task, RST);

    let instr = if n_instructions > 0 {
        format!("  ·  {} instruction file(s)", n_instructions)
    } else {
        String::new()
    };
    println!("  {}{}  ·  {}{}{}",  GRAY, branch, mode, instr, RST);
    println!();
    thick_rule();
    println!();
}

// ─── Agent turn ───────────────────────────────────────────────────────────────

pub fn agent_start(agent: &str, role: &str, step: usize, total: usize) {
    let color = agent_color(agent);
    println!(
        "\n  {}{}{}{}{}{} {}╌{} {}{}{}[{}/{}]{}",
        BOLD, color, agent_display(agent), RST,
        DIM, GRAY, RST,
        GRAY,
        DIM, GRAY, RST,
        step, total,
        RST,
    );
    println!("  {}  role: {}{}", GRAY, role, RST);
    thin_rule();
}

pub fn agent_line(line: &str) {
    println!("  │  {}", line);
}

pub fn agent_blank() {
    println!("  │");
}

pub fn agent_end(agent: &str, ok: bool) {
    thin_rule();
    if ok {
        println!("  {}✓{}  {} done\n", GRN, RST, agent_display(agent));
    } else {
        println!("  {}✗{}  {} error\n", RED, RST, agent_display(agent));
    }
}

// ─── Wisp narration (between agent turns) ────────────────────────────────────

pub fn wisp_note(msg: &str) {
    println!("  {}{}wisp →{}  {}", DIM, GRAY, RST, msg);
}

// ─── Final footer ─────────────────────────────────────────────────────────────

pub fn finish(session_path: &str, dry_run: bool) {
    println!();
    thick_rule();
    println!();
    println!("  {}✓{}  Session saved", GRN, RST);
    println!("  {}{}→  {}{}", DIM, GRAY, session_path, RST);
    if dry_run {
        println!();
        println!(
            "  {}{}Pass --execute-agents to invoke real agents.{}",
            DIM, GRAY, RST
        );
    }
    println!();
    thick_rule();
    println!();
}

// ─── Interactive session ──────────────────────────────────────────────────────

pub fn interactive_header() {
    println!();
    thick_rule();
    println!();
    println!("  {}{}✦ Wisp{}  —  local coding agent", BOLD, MAG, RST);
    println!(
        "  {}Claude implements  ·  Codex ships  ·  you stay in control{}",
        GRAY, RST
    );
    println!();
    thick_rule();
    println!();
    println!("  {}Type a task and press Enter — agents run for real.{}", GRAY, RST);
    println!("  {}Prefix with {}~{}{} for dry-run preview.  {}exit{}{} to quit.{}",
        GRAY, RST, BOLD, GRAY, RST, BOLD, GRAY, RST);
}

pub fn interactive_prompt() {
    print!("  {}✦{} ", MAG, RST);
}

pub fn interactive_help() {
    println!();
    println!("  {}Commands:{}", BOLD, RST);
    println!("  {}  <task>{}      run Claude + Codex agents for real", GRAY, RST);
    println!("  {}  ~<task>{}     dry-run — preview prompts only, no agents invoked", GRAY, RST);
    println!("  {}  exit{}        quit Wisp", GRAY, RST);
    println!("  {}  help{}        show this", GRAY, RST);
    println!();
}

pub fn goodbye() {
    println!();
    println!("  {}Bye.{}", GRAY, RST);
    println!();
}

pub fn no_config_hint() {
    println!();
    thick_rule();
    println!();
    println!("  {}{}✦ Wisp{}", BOLD, MAG, RST);
    println!();
    println!("  {}No wisp.toml found in this directory.{}", GRAY, RST);
    println!();
    println!("  Run {}wisp init{} to set up Wisp here.", BOLD, RST);
    println!();
    thick_rule();
    println!();
}

// ─── Thinking spinner ─────────────────────────────────────────────────────────

pub struct ThinkingSpinner {
    running: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ThinkingSpinner {
    pub fn start() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        let thread = std::thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            while r.load(Ordering::Relaxed) {
                use std::io::Write;
                print!("\r  │  \x1b[90m{} thinking...\x1b[0m", frames[i % frames.len()]);
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_millis(80));
                i += 1;
            }
        });
        ThinkingSpinner { running, thread: Some(thread) }
    }

    pub fn stop(&mut self) {
        if self.thread.is_none() { return; }
        self.running.store(false, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        use std::io::Write;
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

pub fn agent_display(agent: &str) -> &'static str {
    match agent {
        "claude" => "Claude",
        "codex"  => "Codex",
        _        => "Agent",
    }
}

fn agent_color(agent: &str) -> &'static str {
    match agent {
        "claude" => CYAN,
        "codex"  => YLW,
        _        => RST,
    }
}

fn thick_rule() {
    println!("{}{}{}", GRAY, "━".repeat(W), RST);
}

fn thin_rule() {
    println!("  {}{}{}", GRAY, "─".repeat(W - 2), RST);
}
