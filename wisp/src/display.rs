use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

// ─── ANSI helpers ─────────────────────────────────────────────────────────────

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
/// Soft lavender — primary brand accent
const ACCENT: &str = "\x1b[38;2;180;150;255m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const GRAY: &str = "\x1b[90m";
const WHITE: &str = "\x1b[97m";

// ─── Terminal width ───────────────────────────────────────────────────────────

/// Returns the current terminal window width from OS APIs.
/// Refreshed on every call — safe to poll for resize detection.
pub fn term_width() -> usize {
    raw_term_width().clamp(40, 240)
}

fn raw_term_width() -> usize {
    #[cfg(windows)]
    if let Some(w) = windows_term_width() {
        return w;
    }
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(80)
}

#[cfg(windows)]
fn windows_term_width() -> Option<usize> {
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }
    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }
    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        dw_size: Coord,
        dw_cursor_position: Coord,
        w_attributes: u16,
        sr_window: SmallRect,
        dw_maximum_window_size: Coord,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(n: u32) -> *mut std::ffi::c_void;
        fn GetConsoleScreenBufferInfo(
            h: *mut std::ffi::c_void,
            p: *mut ConsoleScreenBufferInfo,
        ) -> i32;
    }

    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;

    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        if h.is_null() || h as usize == usize::MAX {
            return None;
        }
        let mut info: ConsoleScreenBufferInfo = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(h, &mut info) != 0 {
            let w = (info.sr_window.right - info.sr_window.left + 1) as usize;
            if w > 0 { Some(w) } else { None }
        } else {
            None
        }
    }
}

fn heavy_rule() -> String {
    format!("{GRAY}{}{RESET}", "━".repeat(term_width()))
}

// ─── Live command completions ─────────────────────────────────────────────────

const COMPLETIONS: &[(&str, &str)] = &[
    ("/run", "execute workflow interactively"),
    ("/auto", "execute workflow (auto-approve)"),
    ("/claude", "run Claude directly"),
    ("/codex", "run Codex directly"),
    ("/mode", "show or set dry-run / execute mode"),
    ("/paste", "enter multi-line paste mode"),
    ("/help", "show commands"),
    ("/exit", "exit wisp"),
    ("/quit", "exit wisp"),
];

pub fn completions_for(input: &str) -> Vec<(&'static str, &'static str)> {
    if input == "/" || input.is_empty() {
        return COMPLETIONS.to_vec();
    }
    COMPLETIONS
        .iter()
        .filter(|(cmd, _)| cmd.starts_with(input))
        .copied()
        .collect()
}

// ─── Agent display names ──────────────────────────────────────────────────────

pub fn agent_display(name: &str) -> String {
    match name {
        "claude" => "Claude".to_string(),
        "codex" => "Codex".to_string(),
        _ => {
            let mut s = name.to_string();
            if let Some(first) = s.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            s
        }
    }
}

// ─── Interactive UI ───────────────────────────────────────────────────────────

pub fn no_config_hint() {
    let rule = heavy_rule();
    println!("{rule}");
    println!("  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp{RESET}  {GRAY}—{RESET}  not initialized");
    println!("{rule}");
    println!();
    println!("  Run {BOLD}wisp init{RESET} to get started.");
    println!();
}

// ─── Workflow header ──────────────────────────────────────────────────────────

pub fn header(task: &str, branch: &str, mode: &str, instruction_files: usize) {
    let rule = heavy_rule();

    println!("{rule}");
    println!(
        "  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp{RESET}  {GRAY}—{RESET}  implement · patch · review · ship"
    );
    println!("{rule}");
    println!();

    let task_preview = if task.chars().count() > 62 {
        let t: String = task.chars().take(61).collect();
        format!("{t}…")
    } else {
        task.to_string()
    };

    println!("  {GRAY}task    {RESET}{task_preview}");
    println!("  {GRAY}branch  {RESET}{branch}");
    println!("  {GRAY}mode    {RESET}{mode}");
    if instruction_files > 0 {
        let plural = if instruction_files == 1 { "" } else { "s" };
        println!("  {GRAY}files   {RESET}{instruction_files} instruction file{plural} loaded");
    }
    println!();
}

// ─── Agent step UI ────────────────────────────────────────────────────────────

pub fn agent_start(agent: &str, role: &str, step: usize, total: usize) {
    let name = agent_display(agent);
    println!(
        "  {GRAY}┌─{RESET} {BOLD}[{step}/{total}]{RESET}  {ACCENT}{name}{RESET}  {GRAY}—  {role}{RESET}"
    );
    println!();
}

pub fn agent_end(agent: &str, ok: bool) {
    let name = agent_display(agent);
    println!();
    if ok {
        println!("  {GRAY}└─{RESET}  {name}  {GREEN}done ✓{RESET}");
    } else {
        println!("  {GRAY}└─{RESET}  {name}  {RED}failed ✗{RESET}");
    }
    println!();
}

pub fn agent_line(line: &str) {
    println!("    {line}");
}

pub fn agent_blank() {
    println!();
}

pub fn wisp_note(msg: &str) {
    println!("  {GRAY}·  {msg}{RESET}");
    println!();
}

// ─── Finish banner ────────────────────────────────────────────────────────────

pub fn finish(session_path: &str, is_dry_run: bool) {
    let rule = heavy_rule();

    println!("{rule}");
    if is_dry_run {
        println!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  dry-run complete · no changes were made{RESET}"
        );
    } else {
        println!("  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  workflow complete{RESET}");
    }
    println!("{rule}");
    println!();
    println!("  {GRAY}session  {RESET}{DIM}{session_path}{RESET}");
    if is_dry_run {
        println!("  {GRAY}         use {WHITE}/run <task>{GRAY} to execute for real{RESET}");
    }
    println!();
}

// ─── Init output ──────────────────────────────────────────────────────────────

pub fn init_header() {
    let rule = heavy_rule();
    println!("{rule}");
    println!("  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp Init{RESET}");
    println!("{rule}");
    println!();
}

pub fn init_created(path: &str) {
    println!("  {GREEN}+{RESET}  {path}");
}

pub fn init_exists(path: &str) {
    println!("  {GRAY}·  {path}  (already exists){RESET}");
}

pub fn init_overwrite_hint() {
    println!(
        "  {YELLOW}!{RESET}  {BOLD}wisp.toml{RESET} already exists  {GRAY}—  use --force to overwrite{RESET}"
    );
}

pub fn init_done() {
    println!();
    println!("  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  wisp initialized{RESET}");
    println!();
    println!("  {GRAY}Edit {WHITE}wisp.toml{GRAY} to configure agents and workflow.{RESET}");
    println!("  {GRAY}Run {WHITE}wisp doctor{GRAY} to verify your setup.{RESET}");
    println!();
}

pub fn init_error(path: &str, err: &dyn std::fmt::Display) {
    println!("  {RED}✗{RESET}  failed to create {path}  {GRAY}—  {err}{RESET}");
}

// ─── Doctor output ────────────────────────────────────────────────────────────

pub fn doctor_header() {
    let rule = heavy_rule();
    println!("{rule}");
    println!("  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp Doctor{RESET}");
    println!("{rule}");
    println!();
}

pub fn doctor_check(label: &str, ok: bool, hint: Option<&str>) {
    if ok {
        println!("  {GREEN}✓{RESET}  {label}");
    } else {
        println!("  {RED}✗{RESET}  {label}");
        if let Some(h) = hint {
            println!("     {GRAY}{h}{RESET}");
        }
    }
}

pub fn doctor_summary(env_ok: bool, agents_ok: bool) {
    println!();
    if env_ok && agents_ok {
        println!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}all checks passed{RESET}  {GRAY}—  wisp is fully ready{RESET}"
        );
    } else if env_ok {
        println!("  {ACCENT}✦{RESET}  {GREEN}{BOLD}ready{RESET}  {GRAY}—  dry-run mode{RESET}");
        println!();
        println!(
            "  {GRAY}Install Claude CLI and Codex CLI to enable {WHITE}--execute-agents{GRAY}.{RESET}"
        );
    } else {
        println!("  {RED}✗  some checks failed{RESET}");
        println!();
        println!("  {GRAY}Run {WHITE}wisp init{GRAY} and install missing tools.{RESET}");
    }
    println!();
}

// ─── Thinking spinner ─────────────────────────────────────────────────────────

pub struct ThinkingSpinner {
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ThinkingSpinner {
    pub fn start() -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            while !stop_clone.load(Ordering::Relaxed) {
                print!(
                    "\r  {ACCENT}✦{RESET}  {GRAY}{} thinking…{RESET}   ",
                    frames[i % frames.len()]
                );
                let _ = std::io::stdout().flush();
                thread::sleep(Duration::from_millis(80));
                i = i.wrapping_add(1);
            }
            print!("\r{}\r", " ".repeat(40));
            let _ = std::io::stdout().flush();
        });

        ThinkingSpinner {
            stop_flag,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ThinkingSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}

// ─── Mode display ─────────────────────────────────────────────────────────────

pub fn mode_status(execute_agents: bool) {
    if execute_agents {
        println!(
            "  {ACCENT}✦{RESET}  mode: {GREEN}{BOLD}execute{RESET}  \
             {GRAY}— bare tasks invoke agents{RESET}"
        );
        println!("  {GRAY}Use {WHITE}/mode dry-run{GRAY} to switch to preview-only.{RESET}");
    } else {
        println!(
            "  {ACCENT}✦{RESET}  mode: {WHITE}{BOLD}dry-run{RESET}  \
             {GRAY}— bare tasks show a preview only (default){RESET}"
        );
        println!("  {GRAY}Use {WHITE}/mode execute{GRAY} to invoke agents for bare tasks.{RESET}");
    }
    println!();
}

pub fn mode_set(execute_agents: bool) {
    if execute_agents {
        println!(
            "  {GREEN}✓{RESET}  mode set to {GREEN}{BOLD}execute{RESET}  \
             {GRAY}— bare tasks will invoke agents{RESET}"
        );
    } else {
        println!(
            "  {GREEN}✓{RESET}  mode set to {WHITE}{BOLD}dry-run{RESET}  \
             {GRAY}— bare tasks will show a preview only{RESET}"
        );
    }
    println!();
}
