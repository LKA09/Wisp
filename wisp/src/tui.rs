use std::sync::mpsc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use std::path::PathBuf;

const LAVENDER: Color = Color::Rgb(180, 150, 255);
const DIM: Color = Color::Rgb(100, 100, 120);
const GREEN: Color = Color::Rgb(80, 200, 120);
const RED: Color = Color::Rgb(220, 80, 80);
const WHITE: Color = Color::Rgb(220, 220, 230);
const YELLOW: Color = Color::Rgb(220, 180, 80);

const CMDS: &[(&str, &str)] = &[
    ("/run", "execute workflow interactively"),
    ("/auto", "execute workflow (auto-approve)"),
    ("/claude", "run Claude directly"),
    ("/codex", "run Codex directly"),
    ("/mode", "show or set dry-run / execute mode"),
    ("/doctor", "check environment"),
    ("/init", "initialize wisp"),
    ("/help", "show commands"),
    ("/exit", "exit wisp"),
];

#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Input,
    Sessions,
}

struct App {
    input: String,
    lines: Vec<(String, Color)>,
    scroll: usize,
    auto_scroll: bool,
    sessions: Vec<PathBuf>,
    sessions_state: ListState,
    focus: Focus,
    execute_agents: bool,
    branch: String,
    config_ok: bool,
    quit: bool,
    /// Receiving end of workflow output channel. Some = workflow running.
    workflow_rx: Option<mpsc::Receiver<String>>,
}

impl App {
    fn new() -> Self {
        let s = crate::settings::Settings::load();
        let branch = crate::git::current_branch()
            .ok()
            .flatten()
            .unwrap_or_else(|| "unknown".to_string());
        let config_ok = crate::config::Config::exists();
        let sessions = load_sessions(8);

        let mut app = App {
            input: String::new(),
            lines: Vec::new(),
            scroll: 0,
            auto_scroll: true,
            sessions,
            sessions_state: ListState::default(),
            focus: Focus::Input,
            execute_agents: s.execute_agents,
            branch,
            config_ok,
            quit: false,
            workflow_rx: None,
        };
        app.push("  Wisp  —  local coding agent orchestrator", LAVENDER);
        app.push("", WHITE);
        app.push("  Type a task and press Enter. Use /help for commands.", DIM);
        app.push("", WHITE);
        app
    }

    fn push(&mut self, text: impl Into<String>, color: Color) {
        self.lines.push((text.into(), color));
        if self.auto_scroll {
            self.scroll = self.lines.len().saturating_sub(1);
        }
    }

    fn push_sep(&mut self) {
        self.push(
            "  ──────────────────────────────────────────────────",
            DIM,
        );
    }

    fn refresh_status(&mut self) {
        self.execute_agents = crate::settings::Settings::load().execute_agents;
        self.branch = crate::git::current_branch()
            .ok()
            .flatten()
            .unwrap_or_else(|| "unknown".to_string());
        self.config_ok = crate::config::Config::exists();
        self.sessions = load_sessions(8);
    }

    fn is_running(&self) -> bool {
        self.workflow_rx.is_some()
    }
}

fn load_sessions(n: usize) -> Vec<PathBuf> {
    let dir = std::path::Path::new(".wisp/sessions");
    if !dir.exists() {
        return Vec::new();
    }
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v.truncate(n);
    v
}

fn completions_for(query: &str) -> Vec<(&'static str, &'static str)> {
    if query.is_empty() {
        return CMDS.to_vec();
    }
    let prefix = format!("/{query}");
    CMDS.iter()
        .filter(|(c, _)| c.starts_with(&prefix))
        .copied()
        .collect()
}

fn show_completions(app: &App) -> bool {
    !app.is_running() && app.input.starts_with('/') && !app.input.contains(' ')
}

/// Colorize a plain-text line received from the workflow channel.
fn line_color(text: &str) -> Color {
    let t = text.trim();
    if t.contains("done ✓") || t.starts_with("✓") || t.contains("all checks passed") {
        GREEN
    } else if t.contains("failed ✗") || t.starts_with("✗") || t.contains("Error") || t.contains("error:") {
        RED
    } else if t.starts_with('+') && !t.starts_with("+++") {
        GREEN
    } else if t.starts_with('-') && !t.starts_with("---") {
        RED
    } else if t.starts_with("  ✦") || t.starts_with("  ┌") || t.starts_with("  └") {
        LAVENDER
    } else if t.is_empty() || t.starts_with("━") || t.starts_with("──") {
        DIM
    } else {
        WHITE
    }
}

pub fn run() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        // Drain any pending workflow output lines.
        // Use a local buffer to avoid holding the borrow on app.workflow_rx
        // while calling app.push (which needs &mut app).
        let mut new_lines: Vec<String> = Vec::new();
        let mut workflow_done = false;
        if let Some(rx) = app.workflow_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(line) => new_lines.push(line),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        workflow_done = true;
                        break;
                    }
                }
            }
        }
        for line in new_lines {
            let color = line_color(&line);
            app.push(line, color);
        }
        if workflow_done {
            app.workflow_rx = None;
            app.push_sep();
            app.push("  Workflow finished.", DIM);
            app.push("", WHITE);
            app.refresh_status();
        }

        terminal.draw(|f| draw(f, app))?;

        // While workflow runs: short poll so we keep redrawing for new lines.
        // While idle: blocking read so keystrokes have zero latency.
        if app.is_running() {
            if event::poll(Duration::from_millis(20))? {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        on_key(k.code, k.modifiers, app)?;
                    }
                    _ => {}
                }
            }
        } else {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    on_key(k.code, k.modifiers, app)?;
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.quit {
            break;
        }
    }
    Ok(())
}

fn on_key(
    code: KeyCode,
    mods: KeyModifiers,
    app: &mut App,
) -> anyhow::Result<()> {
    if app.focus == Focus::Sessions {
        match code {
            KeyCode::Esc | KeyCode::Tab => {
                app.focus = Focus::Input;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let max = app.sessions.len().saturating_sub(1);
                let i = app
                    .sessions_state
                    .selected()
                    .map(|i| (i + 1).min(max))
                    .unwrap_or(0);
                app.sessions_state.select(Some(i));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = app
                    .sessions_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                app.sessions_state.select(Some(i));
            }
            KeyCode::Char('q') => {
                app.quit = true;
            }
            _ => {}
        }
        return Ok(());
    }

    // Focus::Input
    match code {
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => {
            app.quit = true;
        }
        KeyCode::Tab => {
            app.focus = Focus::Sessions;
            if !app.sessions.is_empty() && app.sessions_state.selected().is_none() {
                app.sessions_state.select(Some(0));
            }
        }
        KeyCode::PageUp => {
            app.auto_scroll = false;
            app.scroll = app.scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            let max = app.lines.len().saturating_sub(1);
            app.scroll = (app.scroll + 10).min(max);
            app.auto_scroll = app.scroll >= max;
        }
        KeyCode::Up => {
            app.auto_scroll = false;
            app.scroll = app.scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            let max = app.lines.len().saturating_sub(1);
            app.scroll = (app.scroll + 1).min(max);
            app.auto_scroll = app.scroll >= max;
        }
        KeyCode::Backspace if !app.is_running() => {
            app.input.pop();
        }
        KeyCode::Esc => {
            if app.is_running() {
                // Esc during workflow: scroll to bottom / re-enable auto-scroll
                app.auto_scroll = true;
                app.scroll = app.lines.len().saturating_sub(1);
            } else {
                app.input.clear();
            }
        }
        KeyCode::Char(c) if !app.is_running() => {
            app.input.push(c);
        }
        KeyCode::Enter if !app.is_running() => {
            let raw = std::mem::take(&mut app.input);
            let trimmed = raw.trim().to_string();
            if !trimmed.is_empty() {
                on_submit(trimmed, app)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn on_submit(input: String, app: &mut App) -> anyhow::Result<()> {
    use crate::cli::InteractiveAction;
    use crate::agent::PermissionMode;

    // Colon aliases
    if input == ":q" || input == ":quit" {
        app.quit = true;
        return Ok(());
    }

    match crate::cli::parse_interactive_action(&input) {
        InteractiveAction::Exit => {
            app.quit = true;
        }
        InteractiveAction::Help => {
            app.push(format!("  > {input}"), LAVENDER);
            app.push_sep();
            for (cmd, desc) in CMDS {
                app.push(format!("    {cmd:<16}  {desc}"), WHITE);
            }
            app.push_sep();
        }
        InteractiveAction::PreviewCommands { query } => {
            match query.as_str() {
                "doctor" => {
                    app.push(format!("  > {input}"), LAVENDER);
                    spawn_workflow(app, move |tx| {
                        crate::display::set_tui_sink(tx);
                        crate::cli::doctor();
                    });
                }
                "init" => {
                    app.push(format!("  > {input}"), LAVENDER);
                    spawn_workflow(app, move |tx| {
                        crate::display::set_tui_sink(tx);
                        crate::cli::init(false);
                    });
                }
                q => {
                    app.push(format!("  > {input}"), LAVENDER);
                    let ms = completions_for(q);
                    if ms.is_empty() {
                        app.push(format!("  no matching command for /{q}"), DIM);
                    } else {
                        for (cmd, desc) in ms {
                            app.push(format!("    {cmd:<16}  {desc}"), DIM);
                        }
                    }
                }
            }
        }
        InteractiveAction::EnterPasteMode => {
            app.push(format!("  > {input}"), LAVENDER);
            app.push(
                "  Type your task directly in the input bar and press Enter.",
                DIM,
            );
        }
        InteractiveAction::ModeAction { arg } => {
            app.push(format!("  > {input}"), LAVENDER);
            spawn_workflow(app, move |tx| {
                crate::display::set_tui_sink(tx);
                crate::cli::mode(arg.as_deref());
            });
        }
        InteractiveAction::DryRunWorkflow { task } => {
            app.push(format!("  > {input}"), LAVENDER);
            spawn_workflow(app, move |tx| {
                crate::display::set_tui_sink(tx);
                crate::cli::summon(&task, false, false, PermissionMode::Interactive);
            });
        }
        InteractiveAction::BareTask {
            task,
            permission_mode,
        } => {
            app.push(format!("  > {input}"), LAVENDER);
            let execute = crate::settings::Settings::load().execute_agents;
            spawn_workflow(app, move |tx| {
                crate::display::set_tui_sink(tx);
                crate::cli::summon(&task, execute, false, permission_mode);
            });
        }
        InteractiveAction::ExecuteWorkflow {
            task,
            permission_mode,
        } => {
            app.push(format!("  > {input}"), LAVENDER);
            spawn_workflow(app, move |tx| {
                crate::display::set_tui_sink(tx);
                crate::cli::summon(&task, true, false, permission_mode);
            });
        }
        InteractiveAction::ExecuteSingleAgent {
            agent,
            task,
            permission_mode,
        } => {
            app.push(format!("  > {input}"), LAVENDER);
            spawn_workflow(app, move |tx| {
                crate::display::set_tui_sink(tx);
                crate::cli::ask(&agent, &task, true, false, permission_mode);
            });
        }
    }
    Ok(())
}

/// Spawn a workflow thread. The closure receives a `Sender<String>` and should
/// call `display::set_tui_sink(tx)` before doing any display output.
/// When the closure returns the sender drops, signalling "done" to the event loop.
fn spawn_workflow<F>(app: &mut App, f: F)
where
    F: FnOnce(mpsc::Sender<String>) + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<String>();
    app.workflow_rx = Some(rx);
    app.auto_scroll = true;
    std::thread::spawn(move || f(tx));
}

// ─── Drawing ──────────────────────────────────────────────────────────────────

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let sess_h = if app.sessions.is_empty() { 0u16 } else { 5u16 };
    let show_comp = show_completions(app);
    let query = if show_comp {
        app.input.trim_start_matches('/').to_string()
    } else {
        String::new()
    };
    let comp_h = if show_comp {
        (completions_for(&query).len() as u16 + 2).min(12)
    } else {
        0u16
    };

    let header_h = 3u16;
    let input_h = 3u16;
    let footer_h = 1u16;
    let fixed = header_h + sess_h + comp_h + input_h + footer_h;
    let out_h = area.height.saturating_sub(fixed);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Length(out_h),
            Constraint::Length(sess_h),
            Constraint::Length(comp_h),
            Constraint::Length(input_h),
            Constraint::Length(footer_h),
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_output(f, app, chunks[1]);
    if !app.sessions.is_empty() {
        draw_sessions(f, app, chunks[2]);
    }
    if show_comp {
        draw_completions(f, &query, chunks[3]);
    }
    draw_input(f, app, chunks[4]);
    draw_footer(f, app, chunks[5]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = if app.execute_agents { "execute" } else { "dry-run" };
    let mode_col = if app.execute_agents { GREEN } else { WHITE };
    let cfg_str = if app.config_ok { "ok" } else { "missing" };
    let cfg_col = if app.config_ok { GREEN } else { RED };

    let status = if app.is_running() {
        vec![
            Span::styled("  Wisp", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled("   branch: ", Style::default().fg(DIM)),
            Span::styled(app.branch.as_str(), Style::default().fg(WHITE)),
            Span::styled("   ", Style::default()),
            Span::styled("running...", Style::default().fg(YELLOW).add_modifier(Modifier::BOLD)),
        ]
    } else {
        vec![
            Span::styled("  Wisp", Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled("   branch: ", Style::default().fg(DIM)),
            Span::styled(app.branch.as_str(), Style::default().fg(WHITE)),
            Span::styled("   mode: ", Style::default().fg(DIM)),
            Span::styled(mode_str, Style::default().fg(mode_col).add_modifier(Modifier::BOLD)),
            Span::styled("   config: ", Style::default().fg(DIM)),
            Span::styled(cfg_str, Style::default().fg(cfg_col)),
        ]
    };

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(DIM));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(status)),
        Rect { y: inner.y, height: 1, ..inner },
    );
}

fn draw_output(f: &mut Frame, app: &App, area: Rect) {
    let h = area.height as usize;
    if h == 0 {
        return;
    }
    let total = app.lines.len();
    let start = if total > h {
        app.scroll.min(total - h)
    } else {
        0
    };
    let end = (start + h).min(total);

    let lines: Vec<Line> = app.lines[start..end]
        .iter()
        .map(|(text, color)| Line::from(Span::styled(text.as_str(), Style::default().fg(*color))))
        .collect();

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_sessions(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Sessions;
    let border_col = if focused { LAVENDER } else { DIM };

    let names: Vec<String> = app
        .sessions
        .iter()
        .map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string()
        })
        .collect();

    let items: Vec<ListItem> = names
        .iter()
        .map(|name| ListItem::new(format!("  {name}")).style(Style::default().fg(DIM)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Sessions ")
                .title_style(Style::default().fg(border_col))
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(border_col)),
        )
        .highlight_style(Style::default().fg(WHITE).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.sessions_state);
}

fn draw_completions(f: &mut Frame, query: &str, area: Rect) {
    let matches = completions_for(query);
    let items: Vec<ListItem> = matches
        .iter()
        .map(|(cmd, desc)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {cmd:<16} "), Style::default().fg(LAVENDER)),
                Span::styled(*desc, Style::default().fg(DIM)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(DIM)),
    );

    f.render_widget(list, area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let (border_col, prompt_text, cursor) = if app.is_running() {
        (DIM, "  waiting for workflow...", "")
    } else if app.focus == Focus::Input {
        (LAVENDER, "  > ", "_")
    } else {
        (DIM, "  > ", "_")
    };

    let line = if app.is_running() {
        Line::from(Span::styled(prompt_text, Style::default().fg(DIM)))
    } else {
        Line::from(vec![
            Span::styled(prompt_text, Style::default().fg(LAVENDER)),
            Span::styled(app.input.as_str(), Style::default().fg(WHITE)),
            Span::styled(cursor, Style::default().fg(LAVENDER).add_modifier(Modifier::SLOW_BLINK)),
        ])
    };

    let para = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(border_col)),
    );

    f.render_widget(para, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let line = if app.is_running() {
        Line::from(vec![
            Span::styled("  Esc", Style::default().fg(LAVENDER)),
            Span::styled(": scroll to bottom  ", Style::default().fg(DIM)),
            Span::styled("PgUp/↑↓", Style::default().fg(LAVENDER)),
            Span::styled(": scroll  ", Style::default().fg(DIM)),
            Span::styled("Ctrl+C", Style::default().fg(LAVENDER)),
            Span::styled(": quit", Style::default().fg(DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  Tab", Style::default().fg(LAVENDER)),
            Span::styled(": sessions  ", Style::default().fg(DIM)),
            Span::styled("PgUp/PgDn", Style::default().fg(LAVENDER)),
            Span::styled(": scroll  ", Style::default().fg(DIM)),
            Span::styled("Esc", Style::default().fg(LAVENDER)),
            Span::styled(": clear  ", Style::default().fg(DIM)),
            Span::styled("Ctrl+C", Style::default().fg(LAVENDER)),
            Span::styled(": quit", Style::default().fg(DIM)),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}
