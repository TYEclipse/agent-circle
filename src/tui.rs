//! Agent Circle TUI — 全屏终端界面 (S15)
//!
//! Powered by ratatui + crossterm. Press `q` to quit, Esc to go back.

use crate::storage;
use crate::timeline::Timeline;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
    /// Chat with the contact at the given index.
    Chat(usize),
    /// Timeline / 朋友圈 view.
    Timeline,
    /// Group chat list.
    Groups,
}

/// Color theme.
#[derive(Clone, PartialEq, Debug)]
enum Theme {
    Dark,
    Light,
}

/// A chat message bubble.
#[derive(Clone)]
struct ChatMessage {
    /// Message content.
    text: String,
    /// true = sent by me, false = received.
    is_me: bool,
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
    /// Chat input buffer (user-typed text not yet sent).
    input: String,
    /// Chat messages for the current conversation.
    messages: Vec<ChatMessage>,
    /// Scroll offset for messages list (0 = most recent at bottom).
    scroll_offset: usize,
    /// Loaded timeline data.
    timeline: Timeline,
    /// Scroll offset for timeline view.
    timeline_scroll: usize,
    /// Selected group index in the groups view.
    group_index: usize,
    /// Notification queue (shown as banner).
    notifications: Vec<String>,
    /// Countdown timer for notification display (render ticks).
    notification_timer: usize,
    /// Current theme.
    theme: Theme,
    /// Accessibility mode — disables color, adds screen-reader hints.
    accessible: bool,
    /// Whether the user has exited.
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let contacts = load_contacts();
        let timeline = load_timeline();
        Self {
            menu_index: 0,
            view: View::Home,
            contact_index: 0,
            contacts,
            input: String::new(),
            messages: Vec::new(),
            scroll_offset: 0,
            timeline,
            timeline_scroll: 0,
            group_index: 0,
            notifications: Vec::new(),
            notification_timer: 0,
            theme: Theme::Dark,
            accessible: false,
            should_quit: false,
        }
    }

    /// Open a chat view for the given contact index.
    fn open_chat(&mut self, idx: usize) {
        self.view = View::Chat(idx);
        self.input.clear();
        self.messages = Vec::new();
        self.scroll_offset = 0;
    }

    /// Go back from chat to contacts.
    fn close_chat(&mut self) {
        self.view = View::Contacts;
        self.input.clear();
        self.messages.clear();
    }

    /// Send the current input as a message.
    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.messages.push(ChatMessage { text, is_me: true });
        self.input.clear();
    }

    /// Push a notification to the queue. Also rings terminal bell.
    fn notify(&mut self, msg: &str) {
        self.notifications.push(msg.to_string());
        self.notification_timer = 5; // show for ~5 ticks
                                     // Terminal bell
        print!("\x07");
    }

    /// Tick the notification timer. Returns true if still showing.
    fn tick_notification(&mut self) -> bool {
        if self.notification_timer > 0 {
            self.notification_timer -= 1;
            if self.notification_timer == 0 {
                self.notifications.clear();
            }
            true
        } else {
            false
        }
    }

    /// Toggle theme between dark and light.
    fn toggle_theme(&mut self) {
        self.theme = match self.theme {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };
    }

    /// Get accent color based on current theme.
    #[allow(dead_code)]
    fn accent_color(&self) -> Color {
        if self.accessible {
            return Color::White;
        }
        match self.theme {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    /// Get dim color based on current theme.
    fn dim_color(&self) -> Color {
        if self.accessible {
            return Color::White;
        }
        match self.theme {
            Theme::Dark => Color::DarkGray,
            Theme::Light => Color::Gray,
        }
    }

    /// Check if a11y mode is active (no color, just text).
    #[allow(dead_code)]
    fn a11y(&self) -> bool {
        self.accessible
    }

    /// Move selection up (menu, contact list, or chat scroll).
    fn previous(&mut self) {
        match &self.view {
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
            View::Chat(_) => {
                // Scroll up in messages
                if self.scroll_offset + 1 < self.messages.len() {
                    self.scroll_offset += 1;
                }
            }
            View::Timeline => {
                if self.timeline_scroll + 1 < self.timeline.nodes.len() {
                    self.timeline_scroll += 1;
                }
            }
            View::Groups => {
                if self.group_index > 0 {
                    self.group_index -= 1;
                }
            }
        }
    }

    /// Move selection down.
    fn next(&mut self, _max: usize) {
        match &self.view {
            View::Home => {
                if self.menu_index + 1 < 7 {
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
            View::Chat(_) => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            View::Timeline => {
                if self.timeline_scroll > 0 {
                    self.timeline_scroll -= 1;
                }
            }
            View::Groups => {
                if self.group_index + 1 < 5 {
                    self.group_index += 1;
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

/// Load timeline from storage. Returns empty timeline on any error.
fn load_timeline() -> Timeline {
    match storage::resolve_data_dir(None) {
        Ok(dir) => storage::load_timeline(Some(&dir)).unwrap_or_else(|_| Timeline::new()),
        Err(_) => Timeline::new(),
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
        app.tick_notification();

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // ── Global shortcuts (work from any view) ──
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Char('q') if ctrl => {
                    app.should_quit = true;
                    return Ok(());
                }
                KeyCode::Char('c') if ctrl => {
                    app.view = View::Contacts;
                    app.contact_index = 0;
                    app.contacts = load_contacts();
                    continue;
                }
                KeyCode::Char('t') if ctrl => {
                    app.view = View::Timeline;
                    app.timeline_scroll = 0;
                    app.timeline = load_timeline();
                    continue;
                }
                KeyCode::Char('g') if ctrl => {
                    app.view = View::Groups;
                    app.group_index = 0;
                    continue;
                }
                KeyCode::F(5) => {
                    app.toggle_theme();
                    continue;
                }
                KeyCode::F(1) => {
                    app.accessible = !app.accessible;
                    continue;
                }
                _ => {}
            }

            match &app.view {
                View::Home => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(menu_items.len()),
                    KeyCode::Enter => match app.menu_index {
                        0 => {
                            app.view = View::Contacts;
                            app.contact_index = 0;
                            app.contacts = load_contacts();
                        }
                        2 => {
                            app.view = View::Groups;
                            app.group_index = 0;
                        }
                        3 => {
                            app.view = View::Timeline;
                            app.timeline_scroll = 0;
                            app.timeline = load_timeline();
                        }
                        _ => {}
                    },
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
                        if !app.contacts.is_empty() {
                            let idx = app.contact_index;
                            let name = app.contacts[idx].name.clone();
                            app.notify(&format!("进入与 {} 的聊天", name));
                            app.open_chat(idx);
                        }
                    }
                    _ => {}
                },
                View::Chat(_) => match key.code {
                    KeyCode::Esc => {
                        app.close_chat();
                    }
                    KeyCode::Enter => {
                        app.send_message();
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(0),
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    _ => {}
                },
                View::Timeline => match key.code {
                    KeyCode::Esc => {
                        app.view = View::Home;
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(0),
                    _ => {}
                },
                View::Groups => match key.code {
                    KeyCode::Esc => {
                        app.view = View::Home;
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(0),
                    KeyCode::Enter => {
                        // Placeholder: enter group chat
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

    // ── Notification banner ──
    if app.notification_timer > 0 {
        let notif_text = app.notifications.last().map(|s| s.as_str()).unwrap_or("");
        let notif_banner = Paragraph::new(Line::from(Span::styled(
            format!(" 🔔 {}", notif_text),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);
        // Render just above the body (overlapping slightly into header area)
        let banner_area = Rect {
            x: area.x,
            y: area.y + 4,
            width: area.width,
            height: 1,
        };
        f.render_widget(notif_banner, banner_area);
    }

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
    f.render_stateful_widget(
        menu,
        body_chunks[0],
        &mut ListState::default().with_selected(Some(app.menu_index)),
    );

    // Right panel — depends on current view
    match &app.view {
        View::Home => render_home(f, body_chunks[1]),
        View::Contacts => render_contacts(f, body_chunks[1], app),
        View::Chat(idx) => render_chat(f, body_chunks[1], app, *idx),
        View::Timeline => render_timeline(f, body_chunks[1], app),
        View::Groups => render_groups(f, body_chunks[1], app),
    }

    // ── Footer ──
    let footer_hint = match &app.view {
        View::Home => "j/k ↑↓ 导航  |  Enter 进入  |  q 退出",
        View::Contacts => "j/k ↑↓ 选人  |  Enter 进入聊天  |  Esc 返回",
        View::Chat(_) => "输入消息  |  Enter 发送  |  Esc 返回",
        View::Timeline => "j/k ↑↓ 滚动  |  Esc 返回",
        View::Groups => "j/k ↑↓ 选群  |  Enter 进入群聊  |  Esc 返回",
    };
    let theme_hint = match app.theme {
        Theme::Dark => "🌙",
        Theme::Light => "☀️",
    };
    let a11y_hint = if app.accessible { " ♿" } else { "" };
    let footer_text = vec![Line::from(Span::styled(
        format!(
            " Agent Circle v{}  |  P2P 社交基础设施  |  {}  | F5{} F1♿  |  Ctrl+T/C/G/Q: 快速导航 ",
            env!("CARGO_PKG_VERSION"),
            footer_hint,
            theme_hint,
        ),
        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
    )), Line::from(Span::styled(
        format!(" {}{}", footer_hint, a11y_hint),
        Style::default().fg(app.dim_color()).add_modifier(Modifier::DIM),
    ))];
    let footer = Paragraph::new(Text::from(footer_text))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_home(f: &mut Frame, area: Rect) {
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

fn render_contacts(f: &mut Frame, area: Rect, app: &App) {
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

    let contact_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

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

fn render_chat(f: &mut Frame, area: Rect, app: &App, contact_idx: usize) {
    let contact = match app.contacts.get(contact_idx) {
        Some(c) => c,
        None => return,
    };

    let chat_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // chat header (contact name + peer id)
            Constraint::Min(1),    // messages area
            Constraint::Length(3), // input bar
        ])
        .split(area);

    // ── Chat header ──
    let header_text = vec![
        Line::from(Span::styled(
            format!(" 💬 与 {} 聊天", contact.name),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("    Peer ID: {}", contact.peer_id),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let chat_header =
        Paragraph::new(Text::from(header_text)).block(Block::default().borders(Borders::ALL));
    f.render_widget(chat_header, chat_chunks[0]);

    // ── Messages area ──
    if app.messages.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "  暂无消息 — 输入内容后按 Enter 发送",
            Style::default().fg(Color::DarkGray),
        )))
        .block(Block::default().borders(Borders::ALL).title(" 消息 "))
        .alignment(Alignment::Center);
        f.render_widget(empty_msg, chat_chunks[1]);
    } else {
        // Show messages with scroll_offset (newest at bottom)
        let msg_area = chat_chunks[1];
        let visible_count = (msg_area.height as usize).saturating_sub(4); // borders + padding

        let msgs: Vec<Line> = app
            .messages
            .iter()
            .rev()
            .skip(app.scroll_offset)
            .take(visible_count)
            .rev()
            .map(|m| {
                let prefix = if m.is_me {
                    "  🫵 我: "
                } else {
                    "  🤖 对方: "
                };
                let style = if m.is_me {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                Line::from(Span::styled(format!("{}{}", prefix, m.text), style))
            })
            .collect();

        let scroll_hint = if app.scroll_offset > 0 {
            format!(" (已上滚 {} 条)", app.scroll_offset)
        } else {
            String::new()
        };

        let messages_widget = Paragraph::new(Text::from(msgs))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" 消息{} ", scroll_hint)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(messages_widget, msg_area);
    }

    // ── Input bar ──
    let input_text = if app.input.is_empty() {
        Line::from(Span::styled(
            "  输入消息...",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::from(Span::styled(
            format!("  > {}", app.input),
            Style::default().fg(Color::White),
        ))
    };
    let input_widget = Paragraph::new(Text::from(vec![input_text]))
        .block(Block::default().borders(Borders::ALL).title(" 输入 "))
        .wrap(Wrap { trim: false });
    f.render_widget(input_widget, chat_chunks[2]);
}

fn render_timeline(f: &mut Frame, area: Rect, app: &App) {
    if app.timeline.nodes.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  📰 暂无朋友圈动态",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from("  使用 CLI 发布朋友圈："),
            Line::from(Span::styled(
                "    agent-circle timeline post <content>",
                Style::default().fg(Color::Cyan),
            )),
        ];
        let empty = Paragraph::new(Text::from(empty_text))
            .block(Block::default().borders(Borders::ALL).title(" 朋友圈 "))
            .wrap(Wrap { trim: false });
        f.render_widget(empty, area);
        return;
    }

    let visible = (area.height as usize).saturating_sub(2);
    let posts: Vec<Line> = app
        .timeline
        .nodes
        .iter()
        .rev()
        .skip(app.timeline_scroll)
        .take(visible)
        .map(|node| {
            let ts_str = chrono::DateTime::from_timestamp(node.ts, 0)
                .map(|dt| dt.format("%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let author_short = if node.author.len() > 12 {
                format!("{}...", &node.author[..12])
            } else {
                node.author.clone()
            };
            let content = if node.content.len() > 60 {
                format!("{}...", &node.content[..60])
            } else {
                node.content.clone()
            };
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", ts_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}", author_short),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(": "),
                Span::styled(content, Style::default()),
            ])
        })
        .collect();

    let title = if app.timeline_scroll > 0 {
        format!(
            " 朋友圈 ({} 条, 已上滚 {}) ",
            app.timeline.nodes.len(),
            app.timeline_scroll
        )
    } else {
        format!(" 朋友圈 ({} 条) ", app.timeline.nodes.len())
    };

    let timeline_widget = Paragraph::new(Text::from(posts))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(timeline_widget, area);
}

fn render_groups(f: &mut Frame, area: Rect, app: &App) {
    // Mock groups since group storage is GossipSub-based (ephemeral)
    let mock_groups: &[(&str, &str, &str)] = &[
        ("🎮 游戏频道", "7 人在线", "话题: StarCraft, Minecraft"),
        ("🤖 AI 讨论组", "12 人在线", "话题: LLM, Agent, P2P"),
        ("📚 技术分享", "5 人在线", "话题: Rust, Wasm, libp2p"),
        ("☕ 摸鱼群", "3 人在线", "话题: 随便聊聊"),
        ("🔧 开发者群", "9 人在线", "话题: 开源, CI/CD, DevOps"),
    ];

    let group_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    let group_items: Vec<ListItem> = mock_groups
        .iter()
        .enumerate()
        .map(|(i, &(name, count, _))| {
            let style = if i == app.group_index {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(format!("  {}  {}", name, count), style))
        })
        .collect();

    let group_list = List::new(group_items)
        .block(Block::default().borders(Borders::ALL).title(" 群聊列表 "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(
        group_list,
        group_chunks[0],
        &mut ListState::default().with_selected(Some(app.group_index)),
    );

    // Group detail
    if let Some(&(name, count, topic)) = mock_groups.get(app.group_index) {
        let detail_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}", name),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  在线: {}", count),
                Style::default().fg(Color::Green),
            )),
            Line::from(Span::styled(
                format!("  {}", topic),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  GossipSub 协议 — 去中心化群聊",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  按 Enter 加入群聊",
                Style::default().fg(Color::Green),
            )),
        ];
        let detail = Paragraph::new(Text::from(detail_text))
            .block(Block::default().borders(Borders::ALL).title(" 群详情 "))
            .wrap(Wrap { trim: false });
        f.render_widget(detail, group_chunks[1]);
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
        assert!(app.input.is_empty());
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

        for _ in 0..6 {
            app.next(7);
        }
        assert_eq!(app.menu_index, 6);

        app.next(7);
        assert_eq!(app.menu_index, 6);
    }

    #[test]
    fn app_view_switching() {
        let mut app = App::new();
        assert_eq!(app.view, View::Home);

        app.view = View::Contacts;
        assert_eq!(app.view, View::Contacts);
        assert_eq!(app.contact_index, 0);

        app.view = View::Home;
        assert_eq!(app.view, View::Home);
    }

    #[test]
    fn app_chat_input() {
        let mut app = App::new();
        app.input.push('H');
        app.input.push('i');
        assert_eq!(app.input, "Hi");

        // Backspace
        app.input.pop();
        assert_eq!(app.input, "H");

        // Send
        app.input.clear();
        app.input.push_str("Hello!");
        app.send_message();
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].text, "Hello!");
        assert!(app.messages[0].is_me);
        assert!(app.input.is_empty());
    }

    #[test]
    fn app_send_empty_message() {
        let mut app = App::new();
        app.send_message(); // empty input
        assert!(app.messages.is_empty());

        app.input.push_str("   ");
        app.send_message(); // whitespace only
        assert!(app.messages.is_empty());
    }

    #[test]
    fn app_open_close_chat() {
        let mut app = App::new();
        app.open_chat(0);
        assert_eq!(app.view, View::Chat(0));
        assert!(app.input.is_empty());
        assert!(app.messages.is_empty());

        app.close_chat();
        assert_eq!(app.view, View::Contacts);
    }

    #[test]
    fn app_chat_scroll() {
        let mut app = App::new();
        app.view = View::Chat(0);
        for i in 0..10 {
            app.messages.push(ChatMessage {
                text: format!("msg {}", i),
                is_me: true,
            });
        }

        // scroll_offset starts at 0
        assert_eq!(app.scroll_offset, 0);

        // scroll up
        app.previous();
        assert_eq!(app.scroll_offset, 1);

        app.previous();
        assert_eq!(app.scroll_offset, 2);

        // scroll down
        app.next(0);
        assert_eq!(app.scroll_offset, 1);

        // can't scroll past bounds
        for _ in 0..20 {
            app.previous();
        }
        assert_eq!(app.scroll_offset, 9); // max is len-1=9
    }
}
