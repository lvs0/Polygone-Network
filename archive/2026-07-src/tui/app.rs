//! Main TUI application loop — Polygone dashboard.
//! Arrow-key navigation between tabs, live state, module toggles.

use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use super::views::{render_view, View};
use super::views_menu;
use crate::economy::Ticker;
use crate::identity::{load_or_create as load_identity, Identity};

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

    // ── Spec §4 / §5 : ecosystem identity, POLY economy, refresh,
    //    pause, dirty-flag (event-driven rendering per spec).
    pub identity: Identity,
    pub economy: Ticker,
    /// `true` after any state change that requires a re-draw. Reset
    /// by the render loop after drawing. Spec §4 forbids continuous
    /// polling; we redraw only on event or on explicit `[R]` press.
    pub dirty: bool,
    pub last_refresh: Instant,
    pub paused: bool,
    /// Traffic history for sparklines (last 30 samples, each = 1 second)
    pub traffic_history_in: VecDeque<u64>,
    pub traffic_history_out: VecDeque<u64>,

    // ── Phase 3 (ETAPE 3) : landing menu + persistent state.
    pub menu: views_menu::MenuState,
    pub persistent: views_menu::PersistentState,
}

impl App {
    pub fn new() -> Self {
        // Phase 3 : respect any persisted pause from the previous
        // session (user set "Pause 60 min" → quit → relaunch). The
        // `paused` field must mirror `persistent.pause_active()` at
        // startup, otherwise the heartbeat from `run_tui` bypasses
        // the pause.
        let persistent = views_menu::PersistentState::load();
        let initial_paused = persistent.pause_active();

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

            identity: load_identity(),
            economy: Ticker::load(),
            dirty: true,    // Force first draw.
            last_refresh: Instant::now(),
            paused: initial_paused,
            traffic_history_in: VecDeque::with_capacity(30),
            traffic_history_out: VecDeque::with_capacity(30),

            menu: views_menu::MenuState::default(),
            persistent,
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
        // Landing menu (Phase 3) — absorbs all keys except q / Ctrl+C,
        // which still quit globally. Esc on the main menu goes back to
        // the dashboard; inside sub-screens it pops one level.
        if self.current_view == View::Menu {
            // q / Ctrl+C always quit, even from the menu.
            if key == KeyCode::Char('q')
                || (key == KeyCode::Char('c')
                    && modifiers.contains(KeyModifiers::CONTROL))
            {
                let _ = self.persistent.save();
                self.should_quit = true;
                return;
            }
            let (outcome, effect) = views_menu::handle_menu_key(
                &mut self.menu,
                &mut self.persistent,
                key,
            );
            match (outcome, effect) {
                (
                    views_menu::MenuOutcome::Stay,
                    views_menu::MenuSideEffect::CheckedUpdate,
                ) => {
                    self.push_msg(MessageKind::Info, "MAJ : v1.0.0 (canal stable)");
                }
                (
                    views_menu::MenuOutcome::Stay,
                    views_menu::MenuSideEffect::ToggledAutoUpdate(v),
                ) => {
                    self.push_msg(
                        MessageKind::Info,
                        format!("MAJ auto : {}", if v { "ON" } else { "OFF" }),
                    );
                }
                (
                    views_menu::MenuOutcome::Stay,
                    views_menu::MenuSideEffect::PausedFor(until),
                ) => {
                    self.push_msg(
                        MessageKind::Warn,
                        format!("Nœud en pause jusqu'à {}", until),
                    );
                    self.paused = true;
                    let snap_active_modules = self
                        .modules
                        .iter()
                        .filter(|m| m.status == ModuleStatus::Running)
                        .count() as u32;
                    self.economy.set_active(if self.paused { 0 } else { snap_active_modules });
                }
                _ => {}
            }
            match outcome {
                views_menu::MenuOutcome::Stay => {
                    self.dirty = true;
                    return;
                }
                views_menu::MenuOutcome::OpenDashboard => {
                    self.current_view = View::Dashboard;
                    self.push_msg(MessageKind::Success, "Dashboard ouvert");
                    self.dirty = true;
                    return;
                }
                views_menu::MenuOutcome::Quit => {
                    let _ = self.persistent.save();
                    self.should_quit = true;
                    return;
                }
            }
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
            // Spec §4 : no continuous polling, explicit refresh
            // (Accueil and global). Reset the last-refresh clock and
            // mark the frame dirty so the renderer picks it up.
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.last_refresh = Instant::now();
                self.economy.tick();
                self.dirty = true;
                self.push_msg(MessageKind::Info, "Rafraîchi (POLY drainé)");
                return;
            }
            // Spec §4 : `[P]` pause node (suspends all services).
            // Phase 3 : manual un-pause via `[P]` also clears any
            // persisted scheduled pause so the two sources of truth
            // stay in sync (no UI deadlock if `pause_until` has
            // elapsed while the in-memory toggle stayed true).
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.paused = !self.paused;
                if !self.paused {
                    self.persistent.pause_until = None;
                    let _ = self.persistent.save();
                }
                let state = if self.paused { "en pause" } else { "repris" };
                self.economy.set_active(if self.paused { 0 } else {
                    self.modules.iter().filter(|m| m.status == ModuleStatus::Running).count() as u32
                });
                self.push_msg(MessageKind::Warn, format!("Nœud {state}"));
                self.dirty = true;
                return;
            }
            // Spec §4 : `[U]` update (stub — would call the updater
            // service in a real install).
            KeyCode::Char('u') | KeyCode::Char('U') => {
                self.push_msg(MessageKind::Info, "Mise à jour : v1.0.0 (canal stable)");
                self.dirty = true;
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
        // Spec §4: redraw only on event (no continuous polling). The
        // dirty flag is set by `handle_key` whenever the state
        // changes. We also drain POLY once a second while the user
        // isn't pausing — that's the only background work, and it
        // never blocks event delivery.
        if app.dirty {
            terminal.draw(|frame| {
                render_view(frame, &app);
            })?;
            app.dirty = false;
        }

        // 1s heartbeat: tick POLY and uptime, mark dirty if something
        // visibly changed.
        if event::poll(Duration::from_millis(1000))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers);
                }
            }
        } else {
            // Timeout — tick background work, no event.
            // `effective_paused` mirrors both the in-memory `[P]`
            // toggle (`app.paused`) AND the persisted scheduled pause
            // (`persistent.pause_active()`). Covering the OR makes
            // the ticker stop when the user paused-then-quit and
            // resume normally once `pause_until` elapses.
            let effective_paused = app.paused || app.persistent.pause_active();
            if !effective_paused {
                app.economy.tick();
                app.stats.uptime_secs = app.stats.uptime_secs.wrapping_add(1);
                // Simulate traffic variation for sparkline demo
                let base_in = 128.0 + (app.tick as f64 * 7.3).sin() * 64.0;
                let base_out = 96.0 + (app.tick as f64 * 5.1).cos() * 48.0;
                app.stats.traffic_in = base_in.max(0.0);
                app.stats.traffic_out = base_out.max(0.0);
                app.traffic_history_in.push_back(app.stats.traffic_in as u64);
                app.traffic_history_out.push_back(app.stats.traffic_out as u64);
                if app.traffic_history_in.len() > 30 {
                    app.traffic_history_in.pop_front();
                    app.traffic_history_out.pop_front();
                }
                app.tick = app.tick.wrapping_add(1);
                app.dirty = true;
            }
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