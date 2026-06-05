//! Main TUI application loop — Polygone dashboard.
//! Arrow-key navigation between tabs, live state, module toggles.

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use super::views::{render_view, View};

/// Severity level for log messages.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Success,
    Error,
    Warn,
}

impl MessageKind {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✔",
            Self::Error => "✖",
            Self::Warn => "⚠",
        }
    }
    pub fn color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Info => Color::Cyan,
            Self::Success => Color::Rgb(0x22, 0xc5, 0x5e),
            Self::Error => Color::Rgb(0xfb, 0x71, 0x85),
            Self::Warn => Color::Rgb(0xfb, 0xbd, 0x24),
        }
    }
}

/// Module runtime status.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModuleStatus {
    Off,
    Running,
    Error,
    ComingSoon,
}

impl ModuleStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Running => "Running",
            Self::Error => "Error",
            Self::ComingSoon => "🔜",
        }
    }
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Off => Color::Rgb(0x64, 0x74, 0x8b),
            Self::Running => Color::Rgb(0x22, 0xc5, 0x5e),
            Self::Error => Color::Rgb(0xfb, 0x71, 0x85),
            Self::ComingSoon => Color::Rgb(0xfb, 0xbd, 0x24),
        }
    }
}

/// Represents one module card in the dashboard.
#[derive(Clone)]
pub struct ModuleCard {
    pub name: &'static str,
    pub icon: &'static str,
    pub desc: &'static str,
    pub status: ModuleStatus,
    pub version: &'static str,
}

impl ModuleCard {
    pub fn all() -> Vec<Self> {
        vec![
            Self {
                name: "Msg",
                icon: "💬",
                desc: "Messagerie E2E éphémère",
                status: ModuleStatus::Running,
                version: "0.1",
            },
            Self {
                name: "Hide",
                icon: "👻",
                desc: "Proxy SOCKS5 · anonymisation",
                status: ModuleStatus::Running,
                version: "0.1",
            },
            Self {
                name: "Drive",
                icon: "📁",
                desc: "Stockage distribué chiffré",
                status: ModuleStatus::Off,
                version: "0.5",
            },
            Self {
                name: "Mesh",
                icon: "🔗",
                desc: "Réseau local P2P",
                status: ModuleStatus::Off,
                version: "0.3",
            },
            Self {
                name: "Brain",
                icon: "🧠",
                desc: "IA locale · Notch SLM",
                status: ModuleStatus::ComingSoon,
                version: "α",
            },
        ]
    }
}

/// Stats displayed on the dashboard.
#[derive(Clone)]
pub struct NodeStats {
    pub uptime_secs: u64,
    pub peers: u32,
    pub traffic_in: f64,   // bytes/sec
    pub traffic_out: f64,  // bytes/sec
    pub fragments_ready: u32,
    pub fragments_needed: u32,
    pub balance: u32,
    pub consumption: f64,  // per minute
}

impl Default for NodeStats {
    fn default() -> Self {
        Self {
            uptime_secs: 1080,
            peers: 3,
            traffic_in: 0.0,
            traffic_out: 0.0,
            fragments_ready: 3,
            fragments_needed: 4,
            balance: 10,
            consumption: 0.1,
        }
    }
}

/// Global application state.
pub struct App {
    pub current_view: View,
    pub should_quit: bool,
    pub messages: Vec<(MessageKind, String, u64)>, // (kind, text, timestamp_secs)
    pub tick: u64,
    pub stats: NodeStats,
    pub modules: Vec<ModuleCard>,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_view: View::Dashboard,
            should_quit: false,
            messages: vec![
                (MessageKind::Success, "⬡ Polygone v1.0.0 démarré".into(), 1080),
                (MessageKind::Info, "Clé ML-KEM-1024 générée".into(), 1020),
                (MessageKind::Success, "Pairing node 127.0.0.1:4001 ✓".into(), 900),
                (MessageKind::Warn, "Cache zéroé — 4.2 MB libérés".into(), 720),
                (MessageKind::Success, "Test Shamir 4-of-7 : 35/35 ✓".into(), 480),
                (MessageKind::Info, "Tunnel Hide SOCKS5 ready :9050".into(), 300),
            ],
            tick: 0,
            stats: NodeStats::default(),
            modules: ModuleCard::all(),
        }
    }

    pub fn push_msg(&mut self, kind: MessageKind, msg: impl Into<String>) {
        let s = msg.into();
        if self.messages.len() >= 50 {
            self.messages.remove(0);
        }
        self.messages.push((kind, s, self.stats.uptime_secs));
    }

    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            // Tab navigation (left/right arrows)
            KeyCode::Left => {
                let idx = self.current_view as isize;
                let len = View::COUNT as isize;
                self.current_view = View::from_idx(((idx - 1 + len) % len) as usize);
            }
            KeyCode::Right => {
                let idx = self.current_view as isize;
                let len = View::COUNT as isize;
                self.current_view = View::from_idx(((idx + 1) % len) as usize);
            }
            // Direct tab selection with number keys
            KeyCode::Char('1') => self.current_view = View::Dashboard,
            KeyCode::Char('2') => self.current_view = View::Favorites,
            KeyCode::Char('3') => self.current_view = View::Services,
            KeyCode::Char('4') => self.current_view = View::Settings,
            // Toggle Msg
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(ref mut m) = self.modules.iter_mut().find(|m| m.name == "Msg") {
                    m.status = match m.status {
                        ModuleStatus::Running => ModuleStatus::Off,
                        _ => ModuleStatus::Running,
                    };
                    let state = if m.status == ModuleStatus::Running { "activé" } else { "désactivé" };
                    self.push_msg(MessageKind::Info, format!("Msg {state}"));
                }
            }
            // Toggle Hide
            KeyCode::Char('h') | KeyCode::Char('H') => {
                if let Some(ref mut m) = self.modules.iter_mut().find(|m| m.name == "Hide") {
                    m.status = match m.status {
                        ModuleStatus::Running => ModuleStatus::Off,
                        _ => ModuleStatus::Running,
                    };
                    let state = if m.status == ModuleStatus::Running { "activé" } else { "désactivé" };
                    self.push_msg(MessageKind::Info, format!("Hide {state}"));
                }
            }
            // Toggle Drive
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(ref mut m) = self.modules.iter_mut().find(|m| m.name == "Drive") {
                    m.status = match m.status {
                        ModuleStatus::Running => ModuleStatus::Off,
                        ModuleStatus::Off => ModuleStatus::Running,
                        other => other,
                    };
                    let state = if m.status == ModuleStatus::Running { "activé" } else { "désactivé" };
                    self.push_msg(MessageKind::Info, format!("Drive {state}"));
                }
            }
            // Toggle Mesh
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Some(ref mut m) = self.modules.iter_mut().find(|m| m.name == "Mesh") {
                    m.status = match m.status {
                        ModuleStatus::Running => ModuleStatus::Off,
                        ModuleStatus::Off => ModuleStatus::Running,
                        other => other,
                    };
                    let state = if m.status == ModuleStatus::Running { "activé" } else { "désactivé" };
                    self.push_msg(MessageKind::Info, format!("Mesh {state}"));
                }
            }
            _ => {}
        }
    }
}

/// Initialize the terminal, run the TUI, and restore on exit.
pub fn run_tui(initial_view: View) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.current_view = initial_view;

    loop {
        terminal.draw(|frame| {
            render_view(frame, &app);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers);
                }
            }
        }

        app.tick = app.tick.wrapping_add(1);
        // Simulate uptime
        if app.tick % 10 == 0 {
            app.stats.uptime_secs += 1;
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}