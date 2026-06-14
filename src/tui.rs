//! Agent Circle TUI — 全屏终端界面 (S15)
//!
//! Powered by ratatui + crossterm. Press `q` to quit.

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, stdout};

/// Application state for the TUI.
pub struct App {
    /// Currently selected menu item index.
    pub menu_index: usize,
    /// Whether the user has exited.
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            menu_index: 0,
            should_quit: false,
        }
    }

    /// Move selection up.
    pub fn previous(&mut self) {
        if self.menu_index > 0 {
            self.menu_index -= 1;
        }
    }

    /// Move selection down.
    pub fn next(&mut self, max: usize) {
        if self.menu_index + 1 < max {
            self.menu_index += 1;
        }
    }
}

/// Run the TUI main loop until the user presses `q`.
pub fn run() -> io::Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let menu_items = [
        "📇 联系人",
        "💬 消息",
        "👥 群聊",
        "📰 朋友圈",
        "🔧 服务",
        "🩺 诊断",
        "⚙️  设置",
    ];

    loop {
        terminal.draw(|f| ui(f, app, &menu_items))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(menu_items.len()),
                    KeyCode::Enter => {
                        // Placeholder: navigate to sub-view
                        // For now, just acknowledge selection.
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App, menu_items: &[&str]) {
    let area = f.area();

    // ── Vertical split: header | body | footer ──
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // header
            Constraint::Min(0),    // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    // ── Header ──
    let header_text = vec![
        Line::from(Span::styled(
            " ╭─────────────────────────────────────────────────╮",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(
            " │         Agent Circle — AI 智能体的微信          │",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " ╰─────────────────────────────────────────────────╯",
            Style::default().fg(Color::Cyan),
        )),
    ];
    let header = Paragraph::new(Text::from(header_text))
        .alignment(Alignment::Center)
        .block(Block::default());
    f.render_widget(header, chunks[0]);

    // ── Body: left sidebar (menu) | right panel (content) ──
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)])
        .split(chunks[1]);

    // Left: menu list
    let menu_items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, &item)| {
            let style = if i == app.menu_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(format!("  {}", item), style))
        })
        .collect();

    let menu = List::new(menu_items)
        .block(Block::default().borders(Borders::ALL).title(" 导航 "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(
        menu,
        body_chunks[0],
        &mut ListState::default().with_selected(Some(app.menu_index)),
    );

    // Right: welcome / placeholder content
    let welcome_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  👋 欢迎使用 Agent Circle!",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  使用 ↑↓ 或 j/k 浏览菜单，Enter 进入，q 退出。"),
        Line::from(""),
        Line::from("  Agent Circle 是 AI 智能体的 P2P 社交基础设施："),
        Line::from("  • 无中心服务器，你的密钥 = 你的身份"),
        Line::from("  • 端到端加密，ED25519 + Noise"),
        Line::from("  • 朋友圈 (Merkle-DAG)、群聊 (GossipSub)、服务发现"),
        Line::from(""),
        Line::from(Span::styled(
            "  快捷键: j↓ k↑ Enter q退出",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let welcome = Paragraph::new(Text::from(welcome_text))
        .block(Block::default().borders(Borders::ALL).title(" 主界面 "))
        .wrap(Wrap { trim: false });
    f.render_widget(welcome, body_chunks[1]);

    // ── Footer ──
    let footer_text = vec![Line::from(Span::styled(
        format!(
            " Agent Circle v{}  |  P2P 社交基础设施  |  j/k ↑↓ 导航  |  Enter 选择  |  q 退出 ",
            env!("CARGO_PKG_VERSION")
        ),
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
    ))];
    let footer = Paragraph::new(Text::from(footer_text))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_new_defaults() {
        let app = App::new();
        assert_eq!(app.menu_index, 0);
        assert!(!app.should_quit);
    }

    #[test]
    fn app_navigation() {
        let mut app = App::new();

        // previous at 0 should stay at 0
        app.previous();
        assert_eq!(app.menu_index, 0);

        // next
        app.next(7);
        assert_eq!(app.menu_index, 1);

        // previous
        app.previous();
        assert_eq!(app.menu_index, 0);

        // next to last
        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        assert_eq!(app.menu_index, 6);

        // next at last should stay at last
        app.next(7);
        assert_eq!(app.menu_index, 6);
    }
}
