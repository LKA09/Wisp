//! Full-screen TUI shell for Wisp interactive mode.
//!
//! The TUI sits on top of the existing CLI layer:
//!   - Header bar, scrollable output panel, completion popup, input bar.
//!   - When a workflow or agent task runs, the TUI briefly suspends
//!     (leaves alternate screen) so the CLI output is visible on the
//!     normal terminal, then resumes when the user presses Enter.
//!
//! Layout:
//!   ┌──────────────────────────────────────────────────────┐
//!   │  ✦  Wisp                       branch  [mode]        │  header
//!   ├──────────────────────────────────────────────────────┤
//!   │                                                       │
//!   │  [scrollable output]                                  │  output
//!   │                                                       │
//!   ├──────────────────────────────────────────────────────┤
//!   │  /run    execute workflow interactively               │  completions
//!   │  /auto   execute workflow (auto-approve)              │  (only when
//!   │  …                                                    │   typing /)
//!   ├──────────────────────────────────────────────────────┤
//!   │  ›  [input]▌                                         │  input
//!   └──────────────────────────────────────────────────────┘

use std::io::{self, BufRead, Write};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

// ─── Colour palette (matches display.rs) ─────────────────────────────────────

const LAVENDER: Color = Color::Rgb(180, 150, 255);
const GRAY: Color = Color::DarkGray;

// ─── Output line ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct OutputLine {
    text: String,
    style: Style,
}

impl OutputLine {
    fn blank() -> Self {
        Self::raw("")
    }
    fn raw(s: impl Into<String>) -> Self {
        OutputLine { text: s.into(), style: Style::default() }
    }
    fn dim(s: impl Into<String>) -> Self {
        OutputLine { text: s.into(), style: Style::default().fg(GRAY) }
    }
    fn accent(s: impl Into<String>) -> Self {
        OutputLine { text: s.into(), style: Style::default().fg(LAVENDER) }
    }
    fn green(s: impl Into<String>) -> Self {
        OutputLine { text: s.into(), style: Style::default().fg(Color::Green) }
    }
    fn bold(s: impl Into<String>) -> Self {
        OutputLine {
            text: s.into(),
            style: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        }
    }
}

// ─── App state ────────────────────────────────────────────────────────────────

struct App {
    input: String,
    output: Vec<OutputLine>,
    /// Lines scrolled up from the bottom (0 = pinned to bottom).
    scroll: usize,
    /// Cached from settings so we don't re-read the file every frame.
    execute_agents: bool,
    /// Cached current git branch (empty string if not in a repo).
    branch: String,
}

impl App {
    fn new() -> Self {
        let execute_agents = crate::settings::Settings::load().execute_agents;
        let branch = crate::git::current_branch()
            .ok()
            .flatten()
            .unwrap_or_default();

        let mut a = App {
            input: String::new(),
            output: Vec::new(),
            scroll: 0,
            execute_agents,
            branch,
        };

        // Welcome banner
        a.push(OutputLine::blank());
        a.push(OutputLine::dim(
            "  ✦  Wisp  ·  Claude implements · Codex ships · you stay in control",
        ));
        a.push(OutputLine::blank());
        a.push(OutputLine::dim(
            "  /help for commands  ·  /run to execute  ·  exit to quit",
        ));
        a.push(OutputLine::blank());
        a
    }

    fn push(&mut self, line: OutputLine) {
        self.output.push(line);
        self.scroll = 0; // auto-scroll to bottom on new content
    }

    /// Command completions for the current input (only when starting with /).
    fn completions(&self) -> Vec<(&'static str, &'static str)> {
        if !self.input.starts_with('/') || self.input.contains(' ') {
            return vec![];
        }
        crate::display::completions_for(&self.input)
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Run the full-screen TUI interactive shell. Returns when the user exits.
pub fn run() -> anyhow::Result<()> {
    if !crate::config::Config::exists() {
        crate::display::no_config_hint();
        return Ok(());
    }

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut app = App::new();
    let result = event_loop(&mut terminal, &mut app);

    // Always restore terminal, even on error.
    let _ = terminal.show_cursor();
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    result
}

// ─── Event loop ───────────────────────────────────────────────────────────────

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    terminal.draw(|f| draw(f, app))?;

    loop {
        // Block until an event arrives — zero polling lag.
        match event::read()? {
            Event::Key(key) if key.kind == event::KeyEventKind::Press => match key.code {
                // Ctrl+C: clear input, or quit if input is already empty
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if app.input.is_empty() {
                        break;
                    }
                    app.input.clear();
                }

                // Submit
                KeyCode::Enter => {
                    let raw = std::mem::take(&mut app.input);
                    let input = raw.trim().to_string();
                    if input.is_empty() {
                        continue;
                    }

                    // Echo the submitted line into the output panel
                    app.push(OutputLine::accent(format!("  › {input}")));

                    let quit = handle_input(&input, terminal, app)?;
                    if quit {
                        break;
                    }
                }

                // Typing
                KeyCode::Char(c) => { app.input.push(c); }
                KeyCode::Backspace => { app.input.pop(); }
                KeyCode::Esc => { app.input.clear(); }

                // Scrolling
                KeyCode::PageUp => {
                    app.scroll = app.scroll.saturating_add(10);
                }
                KeyCode::PageDown => {
                    app.scroll = app.scroll.saturating_sub(10);
                }
                KeyCode::Up if app.input.is_empty() => {
                    app.scroll = app.scroll.saturating_add(1);
                }
                KeyCode::Down if app.input.is_empty() => {
                    app.scroll = app.scroll.saturating_sub(1);
                }
                KeyCode::End => { app.scroll = 0; }

                _ => {}
            },

            Event::Resize(_, _) => {} // ratatui redraws on next iteration

            _ => {}
        }

        terminal.draw(|f| draw(f, app))?;
    }

    // Brief goodbye frame
    app.push(OutputLine::blank());
    app.push(OutputLine::dim("  ✦  bye"));
    app.push(OutputLine::blank());
    let _ = terminal.draw(|f| draw(f, app));
    std::thread::sleep(std::time::Duration::from_millis(300));

    Ok(())
}

// ─── Action handling ──────────────────────────────────────────────────────────

/// Dispatch a submitted input. Returns `true` if the user requested exit.
fn handle_input(
    input: &str,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<bool> {
    use crate::cli::{InteractiveAction, parse_interactive_action};

    match parse_interactive_action(input) {
        InteractiveAction::Exit => return Ok(true),

        InteractiveAction::Help => push_help(app),

        // Completions are already rendered in the popup — nothing to add
        InteractiveAction::PreviewCommands { .. } => {}

        InteractiveAction::ModeAction { arg } => {
            crate::cli::mode(arg.as_deref());
            // Refresh cached value
            app.execute_agents = crate::settings::Settings::load().execute_agents;
            if app.execute_agents {
                app.push(OutputLine::green(
                    "  ✓  mode  execute  ·  bare tasks will invoke agents",
                ));
            } else {
                app.push(OutputLine::raw(
                    "  ✓  mode  dry-run  ·  bare tasks will show a preview only",
                ));
            }
            app.push(OutputLine::blank());
        }

        // Everything else requires leaving the TUI temporarily
        action => {
            leave_and_run(terminal, app, action)?;
        }
    }

    Ok(false)
}

/// Suspend the TUI, run a CLI action with normal terminal output, then return.
fn leave_and_run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    action: crate::cli::InteractiveAction,
) -> anyhow::Result<()> {
    use crate::agent::PermissionMode;
    use crate::cli::InteractiveAction;

    // ── Suspend TUI ────────────────────────────────────────────────────────
    let _ = terminal.show_cursor();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    // ── Execute the action (normal CLI output) ─────────────────────────────
    match action {
        InteractiveAction::DryRunWorkflow { task } => {
            crate::cli::summon(&task, false, false, PermissionMode::Interactive);
        }
        InteractiveAction::BareTask { task, permission_mode } => {
            crate::cli::summon(&task, app.execute_agents, false, permission_mode);
        }
        InteractiveAction::ExecuteWorkflow { task, permission_mode } => {
            crate::cli::summon(&task, true, false, permission_mode);
        }
        InteractiveAction::ExecuteSingleAgent { agent, task, permission_mode } => {
            crate::cli::ask(&agent, &task, true, false, permission_mode);
        }
        InteractiveAction::EnterPasteMode => {
            run_paste_outside_tui(app.execute_agents);
        }
        _ => {}
    }

    // ── Return prompt ──────────────────────────────────────────────────────
    println!();
    println!(
        "  \x1b[90m─── Press Enter to return to Wisp ───────────────────────────\x1b[0m"
    );
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();

    // ── Resume TUI ─────────────────────────────────────────────────────────
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.hide_cursor()?;
    terminal.clear()?;

    Ok(())
}

/// Paste mode when the TUI is suspended (no raw mode, plain stdin).
fn run_paste_outside_tui(execute_agents: bool) {
    use crate::agent::PermissionMode;
    use crate::cli::{InteractiveAction, parse_interactive_action};

    println!();
    println!("  \x1b[90m[paste mode — enter content, then /end on its own line]\x1b[0m");
    println!();

    let stdin = io::stdin();
    let mut lines: Vec<String> = Vec::new();
    for line in stdin.lock().lines() {
        let l = line.unwrap_or_default();
        if l.trim() == "/end" {
            break;
        }
        lines.push(l);
    }

    if lines.is_empty() {
        return;
    }

    print!("  \x1b[90mcommand (/run  /auto  /claude  /codex  or Enter for dry-run)\x1b[0m\n  \x1b[38;2;180;150;255m›\x1b[0m  ");
    io::stdout().flush().ok();
    let mut cmd = String::new();
    io::stdin().read_line(&mut cmd).ok();

    let task = lines.join("\n");
    let combined = format!("{}\n{}", task.trim(), cmd.trim());

    match parse_interactive_action(&combined) {
        InteractiveAction::ExecuteWorkflow { task, permission_mode } => {
            crate::cli::summon(&task, true, false, permission_mode);
        }
        InteractiveAction::BareTask { task, permission_mode } => {
            crate::cli::summon(&task, execute_agents, false, permission_mode);
        }
        InteractiveAction::ExecuteSingleAgent { agent, task, permission_mode } => {
            crate::cli::ask(&agent, &task, true, false, permission_mode);
        }
        InteractiveAction::DryRunWorkflow { task } => {
            crate::cli::summon(&task, false, false, PermissionMode::Interactive);
        }
        _ => {}
    }
}

// ─── Rendering ────────────────────────────────────────────────────────────────

fn draw(f: &mut Frame, app: &App) {
    let completions = app.completions();
    let comp_h = completions.len().min(9) as u16;

    // Build vertical layout dynamically based on whether completions are shown
    let constraints: Vec<Constraint> = if comp_h > 0 {
        vec![
            Constraint::Length(1),           // header
            Constraint::Min(3),              // output
            Constraint::Length(comp_h + 1),  // completions (includes top border)
            Constraint::Length(2),           // input (includes top border)
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    render_header(f, chunks[0], app);
    render_output(f, chunks[1], app);

    if comp_h > 0 {
        render_completions(f, chunks[2], &completions);
        render_input(f, chunks[3], app);
    } else {
        render_input(f, chunks[2], app);
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let mode_label = if app.execute_agents { "execute" } else { "dry-run" };
    let branch = if app.branch.is_empty() { "─" } else { app.branch.as_str() };
    let right = format!("  {}  [{}]  ", branch, mode_label);

    // Visible character widths (no ANSI codes here — ratatui handles styling)
    // "  ✦  Wisp" = 9 visible chars
    let left_vis = 9usize;
    let pad = (area.width as usize).saturating_sub(left_vis + right.len()).max(1);

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("✦  ", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
        Span::styled("Wisp", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, Style::default().fg(GRAY)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

fn render_output(f: &mut Frame, area: Rect, app: &App) {
    let height = area.height as usize;
    let total = app.output.len();

    let scroll = app.scroll.min(total.saturating_sub(height));
    let start = total.saturating_sub(height + scroll);
    let end = (start + height).min(total);

    let items: Vec<ListItem> = app.output[start..end]
        .iter()
        .map(|ol| ListItem::new(Line::from(Span::styled(ol.text.as_str(), ol.style))))
        .collect();

    f.render_widget(List::new(items), area);
}

fn render_completions(f: &mut Frame, area: Rect, completions: &[(&'static str, &'static str)]) {
    let items: Vec<ListItem> = completions
        .iter()
        .map(|(cmd, desc)| {
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{cmd:<22}"), Style::default().fg(Color::White)),
                Span::styled(*desc, Style::default().fg(GRAY)),
            ]))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(GRAY));

    f.render_widget(List::new(items).block(block), area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(GRAY));

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("›  ", Style::default().fg(LAVENDER)),
        Span::styled(app.input.as_str(), Style::default().fg(Color::White)),
        Span::styled("▌", Style::default().fg(LAVENDER)),
    ]);

    f.render_widget(Paragraph::new(line).block(block), area);
}

// ─── Help content ─────────────────────────────────────────────────────────────

fn push_help(app: &mut App) {
    app.push(OutputLine::blank());
    app.push(OutputLine::bold("  Commands"));
    app.push(OutputLine::blank());

    let cmds: &[(&str, &str)] = &[
        ("<task>", "run task (respects /mode setting)"),
        ("/run <task>", "execute workflow interactively"),
        ("/auto <task>", "execute workflow (auto-approve)"),
        ("/claude <task>", "run Claude directly"),
        ("/codex <task>", "run Codex directly"),
        ("/mode [dry-run|execute]", "show or set default mode"),
        ("/paste", "multi-line paste mode"),
        ("/help", "show this help"),
        ("exit / quit", "exit wisp"),
    ];

    for (cmd, desc) in cmds {
        app.push(OutputLine {
            text: format!("  {cmd:<28}{desc}"),
            style: Style::default().fg(GRAY),
        });
    }

    app.push(OutputLine::blank());
    app.push(OutputLine::dim(
        "  Scroll: PageUp / PageDown / ↑↓ arrows   End: jump to bottom",
    ));
    app.push(OutputLine::blank());
}
