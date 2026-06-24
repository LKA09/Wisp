use std::cell::RefCell;
use std::io::Write;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
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

// ─── TUI output sink ──────────────────────────────────────────────────────────

thread_local! {
    static SINK: RefCell<Option<mpsc::Sender<String>>> = const { RefCell::new(None) };
}

/// Route all display output to this channel instead of stdout (TUI mode).
/// Call from the workflow thread before running the workflow.
pub fn set_tui_sink(tx: mpsc::Sender<String>) {
    SINK.with(|s| *s.borrow_mut() = Some(tx));
}

/// Drop the sink and return to normal stdout printing.
#[allow(dead_code)]
pub fn clear_tui_sink() {
    SINK.with(|s| *s.borrow_mut() = None);
}

/// True when a TUI sink is active on this thread.
pub fn is_tui_active() -> bool {
    SINK.with(|s| s.borrow().is_some())
}

/// Print one line: send to TUI channel (ANSI stripped) or println! to stdout.
fn emit(line: &str) {
    SINK.with(|s| {
        let b = s.borrow();
        if let Some(tx) = b.as_ref() {
            let _ = tx.send(strip_ansi(line));
        } else {
            println!("{line}");
        }
    });
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ─── Terminal width ───────────────────────────────────────────────────────────

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

// ─── Workflow header ──────────────────────────────────────────────────────────

pub fn header(task: &str, branch: &str, mode: &str, instruction_files: usize) {
    let rule = heavy_rule();

    emit(&rule);
    emit(&format!(
        "  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp{RESET}  {GRAY}—{RESET}  implement · patch · review · ship"
    ));
    emit(&rule);
    emit("");

    let task_preview = if task.chars().count() > 62 {
        let t: String = task.chars().take(61).collect();
        format!("{t}…")
    } else {
        task.to_string()
    };

    emit(&format!("  {GRAY}task    {RESET}{task_preview}"));
    emit(&format!("  {GRAY}branch  {RESET}{branch}"));
    emit(&format!("  {GRAY}mode    {RESET}{mode}"));
    if instruction_files > 0 {
        let plural = if instruction_files == 1 { "" } else { "s" };
        emit(&format!(
            "  {GRAY}files   {RESET}{instruction_files} instruction file{plural} loaded"
        ));
    }
    emit("");
}

// ─── Agent step UI ────────────────────────────────────────────────────────────

pub fn agent_start(agent: &str, role: &str, step: usize, total: usize) {
    let name = agent_display(agent);
    emit(&format!(
        "  {GRAY}┌─{RESET} {BOLD}[{step}/{total}]{RESET}  {ACCENT}{name}{RESET}  {GRAY}—  {role}{RESET}"
    ));
    emit("");
}

pub fn agent_end(agent: &str, ok: bool) {
    let name = agent_display(agent);
    emit("");
    if ok {
        emit(&format!("  {GRAY}└─{RESET}  {name}  {GREEN}done ✓{RESET}"));
    } else {
        emit(&format!("  {GRAY}└─{RESET}  {name}  {RED}failed ✗{RESET}"));
    }
    emit("");
}

pub fn agent_line(line: &str) {
    emit(&format!("    {line}"));
}

pub fn agent_blank() {
    emit("");
}

pub fn wisp_note(msg: &str) {
    emit(&format!("  {GRAY}·  {msg}{RESET}"));
    emit("");
}

// ─── Finish banner ────────────────────────────────────────────────────────────

pub fn finish(session_path: &str, is_dry_run: bool) {
    let rule = heavy_rule();

    emit(&rule);
    if is_dry_run {
        emit(&format!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  dry-run complete · no changes were made{RESET}"
        ));
    } else {
        emit(&format!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  workflow complete{RESET}"
        ));
    }
    emit(&rule);
    emit("");
    emit(&format!(
        "  {GRAY}session  {RESET}{DIM}{session_path}{RESET}"
    ));
    if is_dry_run {
        emit(&format!(
            "  {GRAY}         use {WHITE}/run <task>{GRAY} to execute for real{RESET}"
        ));
    }
    emit("");
}

// ─── Init output ──────────────────────────────────────────────────────────────

pub fn init_header() {
    let rule = heavy_rule();
    emit(&rule);
    emit(&format!(
        "  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp Init{RESET}"
    ));
    emit(&rule);
    emit("");
}

pub fn init_created(path: &str) {
    emit(&format!("  {GREEN}+{RESET}  {path}"));
}

pub fn init_exists(path: &str) {
    emit(&format!("  {GRAY}·  {path}  (already exists){RESET}"));
}

pub fn init_overwrite_hint() {
    emit(&format!(
        "  {YELLOW}!{RESET}  {BOLD}wisp.toml{RESET} already exists  {GRAY}—  use --force to overwrite{RESET}"
    ));
}

pub fn init_done() {
    emit("");
    emit(&format!(
        "  {ACCENT}✦{RESET}  {GREEN}{BOLD}done{RESET}  {GRAY}—  wisp initialized{RESET}"
    ));
    emit("");
    emit(&format!(
        "  {GRAY}Edit {WHITE}wisp.toml{GRAY} to configure agents and workflow.{RESET}"
    ));
    emit(&format!(
        "  {GRAY}Run {WHITE}wisp doctor{GRAY} to verify your setup.{RESET}"
    ));
    emit("");
}

pub fn init_error(path: &str, err: &dyn std::fmt::Display) {
    emit(&format!(
        "  {RED}✗{RESET}  failed to create {path}  {GRAY}—  {err}{RESET}"
    ));
}

// ─── Doctor output ────────────────────────────────────────────────────────────

pub fn doctor_header() {
    let rule = heavy_rule();
    emit(&rule);
    emit(&format!(
        "  {ACCENT}✦{RESET}  {BOLD}{WHITE}Wisp Doctor{RESET}"
    ));
    emit(&rule);
    emit("");
}

pub fn doctor_check(label: &str, ok: bool, hint: Option<&str>) {
    if ok {
        emit(&format!("  {GREEN}✓{RESET}  {label}"));
    } else {
        emit(&format!("  {RED}✗{RESET}  {label}"));
        if let Some(h) = hint {
            emit(&format!("     {GRAY}{h}{RESET}"));
        }
    }
}

pub fn doctor_summary(env_ok: bool, agents_ok: bool) {
    emit("");
    if env_ok && agents_ok {
        emit(&format!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}all checks passed{RESET}  {GRAY}—  wisp is fully ready{RESET}"
        ));
    } else if env_ok {
        emit(&format!(
            "  {ACCENT}✦{RESET}  {GREEN}{BOLD}ready{RESET}  {GRAY}—  dry-run mode{RESET}"
        ));
        emit("");
        emit(&format!(
            "  {GRAY}Install Claude CLI and Codex CLI to enable {WHITE}--execute-agents{GRAY}.{RESET}"
        ));
    } else {
        emit(&format!("  {RED}✗  some checks failed{RESET}"));
        emit("");
        emit(&format!(
            "  {GRAY}Run {WHITE}wisp init{GRAY} and install missing tools.{RESET}"
        ));
    }
    emit("");
}

// ─── Thinking spinner ─────────────────────────────────────────────────────────

pub struct ThinkingSpinner {
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ThinkingSpinner {
    pub fn start() -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));

        // In TUI mode the spinner's print! calls would corrupt the channel output.
        // Skip the spinner thread; the TUI shows streaming lines directly.
        if is_tui_active() {
            return ThinkingSpinner {
                stop_flag,
                handle: None,
            };
        }

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
        emit(&format!(
            "  {ACCENT}✦{RESET}  mode: {GREEN}{BOLD}execute{RESET}  \
             {GRAY}— bare tasks invoke agents{RESET}"
        ));
        emit(&format!(
            "  {GRAY}Use {WHITE}/mode dry-run{GRAY} to switch to preview-only.{RESET}"
        ));
    } else {
        emit(&format!(
            "  {ACCENT}✦{RESET}  mode: {WHITE}{BOLD}dry-run{RESET}  \
             {GRAY}— bare tasks show a preview only (default){RESET}"
        ));
        emit(&format!(
            "  {GRAY}Use {WHITE}/mode execute{GRAY} to invoke agents for bare tasks.{RESET}"
        ));
    }
    emit("");
}

pub fn mode_set(execute_agents: bool) {
    if execute_agents {
        emit(&format!(
            "  {GREEN}✓{RESET}  mode set to {GREEN}{BOLD}execute{RESET}  \
             {GRAY}— bare tasks will invoke agents{RESET}"
        ));
    } else {
        emit(&format!(
            "  {GREEN}✓{RESET}  mode set to {WHITE}{BOLD}dry-run{RESET}  \
             {GRAY}— bare tasks will show a preview only{RESET}"
        ));
    }
    emit("");
}
