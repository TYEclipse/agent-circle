//! Agent Circle TUI — 全屏终端界面 (S15)
//!
//! Powered by ratatui + crossterm. Press `q` to quit, Esc to go back.

use crate::storage;
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

/// Which view is currently displayed in the right panel.
#[derive(Clone, PartialEq, Debug)]
enum View {
    Home,
    Contacts,
}

/// Application state for the TUI.
pub struct App {
    /// Currently selected menu item index.
    menu_index: usize,
    /// Current active view.
    view: View,
    /// Selected contact index in the contacts view.
    contact_index: usize,
    /// Loaded contacts.
    contacts: Vec<storage::Contact>,
    /// Whether the user has exited.
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let contacts = load_contacts();
        Self {
            menu_index: 0,
            view: View::Home,
            contact_index: 0,
            contacts,
            should_quit: false,
        }
    }

    /// Move selection up (menu or contact list depending on view).
    fn previous(&mut self) {
        match self.view {
            View::Home => {
                if self.menu_index > 0 {
                    self.menu_index -= 1;
                }
            }
            View::Contacts => {
                if self.contact_index > 0 {
                    self.contact_index -= 1;
                }
            }
        }
    }

    /// Move selection down.
    fn next(&mut self, max: usize) {
        match self.view {
            View::Home => {
                if self.menu_index + 1 < max {
                    self.menu_index += 1;
                }
            }
            View::Contacts => {
                if self.contacts.is_empty() {
                    return;
                }
                if self.contact_index + 1 < self.contacts.len() {
                    self.contact_index += 1;
                }
            }
        }
    }
}

/// Load contacts from storage. Returns empty vec on any error.
fn load_contacts() -> Vec<storage::Contact> {
    match storage::resolve_data_dir(None) {
        Ok(dir) => storage::load_contacts(Some(&dir)).unwrap_or_default(),
        Err(_) => Vec::new(),
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
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match app.view {
                View::Home => match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(menu_items.len()),
                    KeyCode::Enter => {
                        // Navigate to selected view
                        match app.menu_index {
                            0 => {
                                app.view = View::Contacts;
                                app.contact_index = 0;
                                // Refresh contacts on every entry
                                app.contacts = load_contacts();
                            }
                            _ => {} // Other views not yet implemented
                        }
                    }
                    _ => {}
                },
                View::Contacts => match key.code {
                    KeyCode::Esc => {
                        app.view = View::Home;
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.next(app.contacts.len());
                    }
                    KeyCode::Enter => {
                        // Placeholder: open chat with selected contact
                        // (will be implemented in R153)
                    }
                    _ => {}
                },
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
    let menu_list_items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, &item)| {
            let style = if i == app.menu_index && app.view == View::Home {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else if i == app.menu_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(format!("  {}", item), style))
        })
        .collect();

    let menu = List::new(menu_list_items)
        .block(Block::default().borders(Borders::ALL).title(" 导航 "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let menu_selection = if app.view == View::Home {
        Some(app.menu_index)
    } else {
        Some(app.menu_index)
    };
    f.render_stateful_widget(
        menu,
        body_chunks[0],
        &mut ListState::default().with_selected(menu_selection),
    );

    // Right panel — depends on current view
    match app.view {
        View::Home => render_home(f, body_chunks[1]),
        View::Contacts => render_contacts(f, body_chunks[1], app),
    }

    // ── Footer ──
    let footer_hint = match app.view {
        View::Home => "j/k ↑↓ 导航  |  Enter 进入  |  q 退出",
        View::Contacts => "j/k ↑↓ 选人  |  Enter 进入聊天  |  Esc 返回",
    };
    let footer_text = vec![Line::from(Span::styled(
        format!(
            " Agent Circle v{}  |  P2P 社交基础设施  |  {} ",
            env!("CARGO_PKG_VERSION"),
            footer_hint,
        ),
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
    ))];
    let footer = Paragraph::new(Text::from(footer_text))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_home(f: &mut Frame, area: ratatui::layout::Rect) {
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
    f.render_widget(welcome, area);
}

fn render_contacts(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    if app.contacts.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  📭 暂无联系人",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from("  使用 CLI 添加联系人："),
            Line::from(Span::styled(
                "    agent-circle contact add <name> <peer-id> <did>",
                Style::default().fg(Color::Cyan),
            )),
        ];
        let empty = Paragraph::new(Text::from(empty_text))
            .block(Block::default().borders(Borders::ALL).title(" 联系人 "))
            .wrap(Wrap { trim: false });
        f.render_widget(empty, area);
        return;
    }

    // Split contacts area: list on left, detail on right
    let contact_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // Contact list
    let contact_items: Vec<ListItem> = app
        .contacts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == app.contact_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(format!("  {}", c.name), style))
        })
        .collect();

    let contact_list = List::new(contact_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" 联系人 ({} 位) ", app.contacts.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(
        contact_list,
        contact_chunks[0],
        &mut ListState::default().with_selected(Some(app.contact_index)),
    );

    // Contact detail
    if let Some(contact) = app.contacts.get(app.contact_index) {
        let detail_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  👤 {}", contact.name),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  Peer ID: {}", contact.peer_id),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!("  DID:     {}", contact.did),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!("  添加时间: {}", contact.added_at),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  按 Enter 开始聊天",
                Style::default().fg(Color::Green),
            )),
        ];
        let detail = Paragraph::new(Text::from(detail_text))
            .block(Block::default().borders(Borders::ALL).title(" 详情 "))
            .wrap(Wrap { trim: false });
        f.render_widget(detail, contact_chunks[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_new_defaults() {
        let app = App::new();
        assert_eq!(app.menu_index, 0);
        assert_eq!(app.view, View::Home);
        assert!(!app.should_quit);
    }

    #[test]
    fn app_navigation_home() {
        let mut app = App::new();

        app.previous();
        assert_eq!(app.menu_index, 0);

        app.next(7);
        assert_eq!(app.menu_index, 1);

        app.previous();
        assert_eq!(app.menu_index, 0);

        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        app.next(7);
        assert_eq!(app.menu_index, 6);

        app.next(7);
        assert_eq!(app.menu_index, 6);
    }

    #[test]
    fn app_view_switching() {
        let mut app = App::new();
        assert_eq!(app.view, View::Home);

        // Simulate Enter on contacts (menu_index=0)
        app.menu_index = 0;
        app.view = View::Contacts;
        assert_eq!(app.view, View::Contacts);
        assert_eq!(app.contact_index, 0);

        // Go back
        app.view = View::Home;
        assert_eq!(app.view, View::Home);
    }
}
