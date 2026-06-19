/// Terminal conversation UI for Wisp.
/// Uses ANSI escape codes and adapts to the current terminal width.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DEFAULT_WIDTH: usize = 64;
const MIN_WIDTH: usize = 24;
const INDENT: &str = "  ";

// ANSI codes
const RST: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[96m"; // Claude
const YLW: &str = "\x1b[93m"; // Codex
const GRN: &str = "\x1b[92m"; // success
const RED: &str = "\x1b[91m"; // error
const MAG: &str = "\x1b[95m"; // Wisp brand
const GRAY: &str = "\x1b[90m"; // dim chrome

pub fn header(task: &str, branch: &str, mode: &str, n_instructions: usize) {
    println!();
    thick_rule();
    println!();
    println!("{INDENT}{BOLD}{MAG}✦ Wisp{RST}");
    print_wrapped(task, BOLD, RST);

    let meta = if n_instructions > 0 {
        format!("{branch}  ·  {mode}  ·  {n_instructions} instruction file(s)")
    } else {
        format!("{branch}  ·  {mode}")
    };
    print_wrapped(&meta, GRAY, RST);
    println!();
    thick_rule();
    println!();
}

pub fn agent_start(agent: &str, role: &str, step: usize, total: usize) {
    let color = agent_color(agent);
    println!();
    println!(
        "{INDENT}{BOLD}{color}{}{RST}{DIM}{GRAY} → {RST}{DIM}{GRAY}[{step}/{total}]{RST}",
        agent_display(agent)
    );
    print_wrapped(&format!("role: {role}"), GRAY, RST);
    thin_rule();
}

pub fn agent_line(line: &str) {
    println!("{INDENT}│  {line}");
}

pub fn agent_blank() {
    println!("{INDENT}│");
}

pub fn agent_end(agent: &str, ok: bool) {
    thin_rule();
    if ok {
        println!("{INDENT}{GRN}✓{RST}  {} done\n", agent_display(agent));
    } else {
        println!("{INDENT}{RED}✗{RST}  {} error\n", agent_display(agent));
    }
}

pub fn wisp_note(msg: &str) {
    print_wrapped(&format!("wisp →  {msg}"), GRAY, RST);
}

pub fn finish(session_path: &str, dry_run: bool) {
    println!();
    thick_rule();
    println!();
    println!("{INDENT}{GRN}✓{RST}  Session saved");
    print_wrapped(&format!("→  {session_path}"), GRAY, RST);
    if dry_run {
        println!();
        print_wrapped("Pass --execute-agents to invoke real agents.", GRAY, RST);
    }
    println!();
    thick_rule();
    println!();
}

pub fn interactive_header() {
    println!();
    thick_rule();
    println!();

    if content_width() >= 40 {
        println!("{INDENT}{BOLD}{MAG}✦ Wisp{RST}  —  local coding agent");
        print_wrapped(
            "Claude implements  ·  Codex ships  ·  you stay in control",
            GRAY,
            RST,
        );
    } else {
        println!("{INDENT}{BOLD}{MAG}✦ Wisp{RST}");
        print_wrapped("local coding agent", GRAY, RST);
        print_wrapped("Claude implements", GRAY, RST);
        print_wrapped("Codex ships", GRAY, RST);
        print_wrapped("you stay in control", GRAY, RST);
    }

    println!();
    thick_rule();
    println!();
    print_wrapped("Type a task and press Enter — default is dry-run preview.", GRAY, RST);

    if content_width() >= 52 {
        println!(
            "{INDENT}{GRAY}Default is dry-run. Use {RST}{BOLD}!{RST}{GRAY} to execute.  {RST}{BOLD}exit{RST}{GRAY} to quit.{RST}"
        );
    } else {
        print_wrapped("Default is dry-run preview.", GRAY, RST);
        print_wrapped("Prefix with ! to execute agents.", GRAY, RST);
        print_wrapped("Type exit to quit.", GRAY, RST);
    }
}

pub fn interactive_prompt() {
    print!("{INDENT}{MAG}✦{RST} ");
}

pub fn interactive_help() {
    println!();
    println!("{INDENT}{BOLD}Commands:{RST}");
    print_wrapped("<task>      dry-run preview only", GRAY, RST);
    print_wrapped("!<task>     execute agents for real", GRAY, RST);
    print_wrapped("~<task>     explicit dry-run preview", GRAY, RST);
    print_wrapped("exit        quit Wisp", GRAY, RST);
    print_wrapped("help        show this", GRAY, RST);
    println!();
}

pub fn goodbye() {
    println!();
    println!("{INDENT}{GRAY}Bye.{RST}");
    println!();
}

pub fn no_config_hint() {
    println!();
    thick_rule();
    println!();
    println!("{INDENT}{BOLD}{MAG}✦ Wisp{RST}");
    println!();
    print_wrapped("No wisp.toml found in this directory.", GRAY, RST);
    println!();
    print_wrapped("Run wisp init to set up Wisp here.", BOLD, RST);
    println!();
    thick_rule();
    println!();
}

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
                print!("\r{INDENT}│  \x1b[90m{} thinking...\x1b[0m", frames[i % frames.len()]);
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_millis(80));
                i += 1;
            }
        });
        ThinkingSpinner {
            running,
            thread: Some(thread),
        }
    }

    pub fn stop(&mut self) {
        if self.thread.is_none() {
            return;
        }
        self.running.store(false, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        use std::io::Write;
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    }
}

pub fn agent_display(agent: &str) -> &'static str {
    match agent {
        "claude" => "Claude",
        "codex" => "Codex",
        _ => "Agent",
    }
}

fn agent_color(agent: &str) -> &'static str {
    match agent {
        "claude" => CYAN,
        "codex" => YLW,
        _ => RST,
    }
}

fn thick_rule() {
    println!("{GRAY}{}{RST}", "━".repeat(content_width() + INDENT.len()));
}

fn thin_rule() {
    println!("{INDENT}{GRAY}{}{RST}", "─".repeat(content_width()));
}

fn print_wrapped(text: &str, prefix: &str, suffix: &str) {
    for line in wrap_text(text, content_width()) {
        println!("{INDENT}{prefix}{line}{suffix}");
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();

    for paragraph in text.lines() {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let current_len = current.chars().count();
            let word_len = word.chars().count();
            let next_len = if current.is_empty() {
                word_len
            } else {
                current_len + 1 + word_len
            };

            if next_len <= width {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
                continue;
            }

            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }

            if word_len <= width {
                current.push_str(word);
            } else {
                split_long_word(word, width, &mut lines, &mut current);
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn split_long_word(word: &str, width: usize, lines: &mut Vec<String>, current: &mut String) {
    let mut chunk = String::new();
    for ch in word.chars() {
        if chunk.chars().count() >= width {
            lines.push(chunk);
            chunk = String::new();
        }
        chunk.push(ch);
    }

    if chunk.chars().count() == width {
        lines.push(chunk);
    } else {
        *current = chunk;
    }
}

fn content_width() -> usize {
    terminal_width()
        .saturating_sub(INDENT.len())
        .max(MIN_WIDTH)
}

fn terminal_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > INDENT.len())
        .or_else(detect_terminal_width)
        .unwrap_or(DEFAULT_WIDTH)
}

#[cfg(windows)]
fn detect_terminal_width() -> Option<usize> {
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
        size: Coord,
        cursor_position: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(n_std_handle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleScreenBufferInfo(
            h_console_output: *mut std::ffi::c_void,
            lp_console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
        ) -> i32;
    }

    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() {
            return None;
        }

        let mut info = ConsoleScreenBufferInfo {
            size: Coord { x: 0, y: 0 },
            cursor_position: Coord { x: 0, y: 0 },
            attributes: 0,
            window: SmallRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            maximum_window_size: Coord { x: 0, y: 0 },
        };

        if GetConsoleScreenBufferInfo(handle, &mut info) == 0 {
            return None;
        }

        let width = i32::from(info.window.right) - i32::from(info.window.left) + 1;
        usize::try_from(width).ok()
    }
}

#[cfg(not(windows))]
fn detect_terminal_width() -> Option<usize> {
    None
}
