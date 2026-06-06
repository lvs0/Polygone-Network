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

// ── Color themes ─────────────────────────────────────────────────────────────

/// Supported color themes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// Default cyber dark theme (cyan accent)
    Cyber,
    /// Solarized Dark
    Solarized,
    /// Dracula
    Dracula,
}

impl Theme {
    pub const ALL: &'static [Theme] = &[Theme::Cyber, Theme::Solarized, Theme::Dracula];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cyber => "Cyber Dark",
            Self::Solarized => "Solarized Dark",
            Self::Dracula => "Dracula",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Cyber => Self::Solarized,
            Self::Solarized => Self::Dracula,
            Self::Dracula => Self::Cyber,
        }
    }

    /// Returns the full color palette for this theme.
    pub fn colors(self) -> ThemeColors {
        match self {
            Self::Cyber => ThemeColors {
                accent:      (0x22, 0xd3, 0xee),
                accent_dim:  (0x08, 0x91, 0xb2),
                green:       (0x22, 0xc5, 0x5e),
                emerald:     (0x34, 0xd3, 0x99),
                violet:      (0xa7, 0x8b, 0xfa),
                amber:       (0xfb, 0xbd, 0x24),
                rose:        (0xfb, 0x71, 0x85),
                text:        (0xcb, 0xd5, 0xe1),
                text_hi:     (0xf8, 0xfa, 0xfc),
                text_dim:    (0x94, 0xa3, 0xb8),
                text_faint:  (0x64, 0x74, 0x8b),
                border:      (0x33, 0x41, 0x55),
                border_dim:  (0x47, 0x55, 0x69),
                bg:          (0x0f, 0x17, 0x2a),
                bg_2:        (0x1e, 0x29, 0x3b),
                bg_3:        (0x0a, 0x0e, 0x17),
                surface:     (0x33, 0x41, 0x55),
            },
            Self::Solarized => ThemeColors {
                accent:      (0x2a, 0xaa, 0xb2),
                accent_dim:  (0x07, 0x6e, 0x73),
                green:       (0x85, 0x99, 0x00),
                emerald:     (0x6c, 0xc0, 0x80),
                violet:      (0x6c, 0x71, 0xc4),
                amber:       (0xb5, 0x89, 0x00),
                rose:        (0xdc, 0x32, 0x2f),
                text:        (0x93, 0xa1, 0xa1),
                text_hi:     (0xfd, 0xf6, 0xec),
                text_dim:    (0x83, 0x94, 0x96),
                text_faint:  (0x65, 0x7b, 0x83),
                border:      (0x07, 0x36, 0x42),
                border_dim:  (0x58, 0x6e, 0x75),
                bg:          (0x00, 0x2b, 0x36),
                bg_2:        (0x07, 0x36, 0x42),
                bg_3:        (0x00, 0x20, 0x29),
                surface:     (0x07, 0x36, 0x42),
            },
            Self::Dracula => ThemeColors {
                accent:      (0xff, 0x79, 0xc6),
                accent_dim:  (0xbd, 0x93, 0xf9),
                green:       (0x50, 0xfb, 0x75),
                emerald:     (0x01, 0xe2, 0x93),
                violet:      (0xbd, 0x93, 0xf9),
                amber:       (0xff, 0xb8, 0x6c),
                rose:        (0xff, 0x55, 0x55),
                text:        (0xf8, 0xf8, 0xf2),
                text_hi:     (0xf8, 0xf8, 0xf2),
                text_dim:    (0x62, 0x72, 0xa4),
                text_faint:  (0x44, 0x47, 0x5a),
                border:      (0x44, 0x47, 0x5a),
                border_dim:  (0x62, 0x72, 0xa4),
                bg:          (0x28, 0x2a, 0x36),
                bg_2:        (0x34, 0x37, 0x46),
                bg_3:        (0x21, 0x22, 0x2c),
                surface:     (0x44, 0x47, 0x5a),
            },
        }
    }
}

/// Full color palette for a theme.
#[derive(Clone, Copy)]
pub struct ThemeColors {
    pub accent:     (u8, u8, u8),
    pub accent_dim: (u8, u8, u8),
    pub green:      (u8, u8, u8),
    pub emerald:    (u8, u8, u8),
    pub violet:     (u8, u8, u8),
    pub amber:      (u8, u8, u8),
    pub rose:       (u8, u8, u8),
    pub text:       (u8, u8, u8),
    pub text_hi:    (u8, u8, u8),
    pub text_dim:   (u8, u8, u8),
    pub text_faint: (u8, u8, u8),
    pub border:     (u8, u8, u8),
    pub border_dim: (u8, u8, u8),
    pub bg:         (u8, u8, u8),
    pub bg_2:       (u8, u8, u8),
    pub bg_3:       (u8, u8, u8),
    pub surface:    (u8, u8, u8),
}

impl ThemeColors {
    /// Helper to convert a tuple to ratatui Color.
    pub fn color(self, c: (u8, u8, u8)) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(c.0, c.1, c.2)
    }
}

// ── Severity level for log messages ──────────────────────────────────────────

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
    pub fn color(self, theme: Theme) -> ratatui::style::Color {
        let c = theme.colors();
        use ratatui::style::Color;
        match self {
            Self::Info => Color::Rgb(c.accent.0, c.accent.1, c.accent.2),
            Self::Success => Color::Rgb(c.green.0, c.green.1, c.green.2),
            Self::Error => Color::Rgb(c.rose.0, c.rose.1, c.rose.2),
            Self::Warn => Color::Rgb(c.amber.0, c.amber.1, c.amber.2),
        }
    }
}

// ── Module status ────────────────────────────────────────────────────────────

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
    pub fn color(&self, theme: Theme) -> ratatui::style::Color {
        let c = theme.colors();
        use ratatui::style::Color;
        match self {
            Self::Off => Color::Rgb(c.text_faint.0, c.text_faint.1, c.text_faint.2),
            Self::Running => Color::Rgb(c.green.0, c.green.1, c.green.2),
            Self::Error => Color::Rgb(c.rose.0, c.rose.1, c.rose.2),
            Self::ComingSoon => Color::Rgb(c.amber.0, c.amber.1, c.amber.2),
        }
    }

    /// Progress percentage for "Coming Soon" modules (0-100), or None.
    pub fn progress(&self) -> Option<u16> {
        match self {
            Self::ComingSoon => Some(35), // simulated progress
            _ => None,
        }
    }
}

// ── Module card ──────────────────────────────────────────────────────────────

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

// ── Node stats ───────────────────────────────────────────────────────────────

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

// ── Message composer state ───────────────────────────────────────────────────

/// Encryption step displayed in the message composer view.
#[derive(Clone)]
pub struct EncryptStep {
    pub label: &'static str,
    pub detail: &'static str,
    pub done: bool,
}

// ── Network topology node ────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TopoNode {
    pub label: String,
    pub x: f64, // 0.0 - 1.0 relative position
    pub y: f64,
    pub is_self: bool,
    pub online: bool,
}

#[derive(Clone)]
pub struct TopoEdge {
    pub from: usize,
    pub to: usize,
}

// ── Global application state ─────────────────────────────────────────────────

pub struct App {
    pub current_view: View,
    pub should_quit: bool,
    pub messages: Vec<(MessageKind, String, u64)>, // (kind, text, timestamp_secs)
    pub tick: u64,
    pub stats: NodeStats,
    pub modules: Vec<ModuleCard>,
    pub theme: Theme,
    /// Show splash screen on startup
    pub show_splash: bool,
    /// Splash screen progress (0.0 - 1.0)
    pub splash_progress: f32,
    /// Traffic history for sparklines (last 40 samples)
    pub traffic_in_history: Vec<u64>,
    pub traffic_out_history: Vec<u64>,
    /// Activity log scroll offset
    pub log_scroll: usize,
    /// Message composer state
    pub composer_text: String,
    pub composer_steps: Vec<EncryptStep>,
    /// Network topology
    pub topo_nodes: Vec<TopoNode>,
    pub topo_edges: Vec<TopoEdge>,
    /// Activity log scroll position
    pub log_offset: usize,
}

impl App {
    pub fn new() -> Self {
        let topo_nodes = vec![
            TopoNode { label: "Toi".into(), x: 0.5, y: 0.45, is_self: true, online: true },
            TopoNode { label: "Alice".into(), x: 0.18, y: 0.2, is_self: false, online: true },
            TopoNode { label: "Bob".into(), x: 0.82, y: 0.2, is_self: false, online: true },
            TopoNode { label: "Carol".into(), x: 0.15, y: 0.75, is_self: false, online: true },
            TopoNode { label: "Dave".into(), x: 0.85, y: 0.75, is_self: false, online: true },
            TopoNode { label: "Eve".into(), x: 0.35, y: 0.9, is_self: false, online: false },
            TopoNode { label: "Frank".into(), x: 0.65, y: 0.9, is_self: false, online: true },
        ];

        let topo_edges = vec![
            TopoEdge { from: 0, to: 1 },
            TopoEdge { from: 0, to: 2 },
            TopoEdge { from: 0, to: 3 },
            TopoEdge { from: 0, to: 4 },
            TopoEdge { from: 1, to: 5 },
            TopoEdge { from: 2, to: 6 },
            TopoEdge { from: 3, to: 6 },
            TopoEdge { from: 4, to: 5 },
        ];

        let composer_steps = vec![
            EncryptStep { label: "Message saisi", detail: "Texte en clair", done: true },
            EncryptStep { label: "KDF BLAKE3", detail: "Dérivation de clé de session", done: true },
            EncryptStep { label: "AES-256-GCM", detail: "Chiffrement symétrique", done: false },
            EncryptStep { label: "ML-KEM-1024", detail: "Encapsulation de clé publique", done: false },
            EncryptStep { label: "Shamir 4-of-7", detail: "Fragmentation du ciphertext", done: false },
            EncryptStep { label: "Transmission", detail: "Envoi via libp2p", done: false },
        ];

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
            theme: Theme::Cyber,
            show_splash: true,
            splash_progress: 0.0,
            traffic_in_history: (0..40).map(|i| (i as u64 * 50) % 300).collect(),
            traffic_out_history: (0..40).map(|i| ((i * 37) as u64 * 30) % 200).collect(),
            log_scroll: 0,
            composer_text: String::new(),
            composer_steps,
            topo_nodes,
            topo_edges,
            log_offset: 0,
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
        // If splash is showing, any key dismisses it
        if self.show_splash {
            self.show_splash = false;
            return;
        }

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
            KeyCode::Char('5') => self.current_view = View::Composer,
            // Scroll activity log
            KeyCode::Up if self.current_view == View::Dashboard => {
                self.log_offset = self.log_offset.saturating_add(1);
            }
            KeyCode::Down if self.current_view == View::Dashboard => {
                self.log_offset = self.log_offset.saturating_sub(1);
            }
            // Theme toggle with 't' key
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.theme = self.theme.next();
                let name = self.theme.label();
                self.push_msg(MessageKind::Info, format!("Thème → {name}"));
            }
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

    /// Simulate a tick: update stats, traffic history, etc.
    pub fn simulate_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        // Simulate uptime
        if self.tick % 10 == 0 {
            self.stats.uptime_secs += 1;
        }

        // Simulate traffic with random walk
        let step_in = ((self.tick as f64 * 0.7).sin() * 80.0 + 150.0).max(0.0);
        let step_out = ((self.tick as f64 * 0.5).cos() * 50.0 + 100.0).max(0.0);
        self.stats.traffic_in = step_in;
        self.stats.traffic_out = step_out;

        // Update sparkline history every 3 ticks
        if self.tick % 3 == 0 {
            self.traffic_in_history.push(step_in as u64);
            self.traffic_out_history.push(step_out as u64);
            if self.traffic_in_history.len() > 40 {
                self.traffic_in_history.remove(0);
            }
            if self.traffic_out_history.len() > 40 {
                self.traffic_out_history.remove(0);
            }
        }

        // Animate composer steps
        if self.current_view == View::Composer && self.tick % 15 == 0 {
            for step in &mut self.composer_steps {
                if !step.done {
                    step.done = true;
                    break;
                }
            }
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
        // Handle splash screen
        if app.show_splash {
            terminal.draw(|frame| {
                super::views::render_splash(frame, &app);
            })?;

            if event::poll(Duration::from_millis(30))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        app.show_splash = false;
                    }
                }
            }

            // Animate splash progress
            app.splash_progress = (app.splash_progress + 0.05).min(1.0);
            if app.splash_progress >= 1.0 {
                // Wait a beat then dismiss
                if event::poll(Duration::from_millis(500))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            app.show_splash = false;
                        }
                    }
                }
                app.show_splash = false;
            }
            continue;
        }

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

        app.simulate_tick();

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
