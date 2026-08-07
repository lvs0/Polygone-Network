//! Polygone master menu — vertical arrow-key navigation.
//!
//! Landing screen on `polygone` with no subcommand. Sub-menu for
//! temporary pause (30 min / 1 h / 3 h / custom). Persists state in
//! `~/.config/polygone/state.json`. Honest: provides toggle, pause,
//! update-stamp; `MAJ` itself stays a stub until Phase 4 binaries.

use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use serde::{Deserialize, Serialize};

// ── Palette (mirrors views.rs) ──────────────────────────────────────────────
const CYBER: Color = Color::Rgb(0x22, 0xd3, 0xee);
const CYBER_DIM: Color = Color::Rgb(0x08, 0x91, 0xb2);
const AMBER: Color = Color::Rgb(0xfb, 0xbd, 0x24);
const SLATE_50: Color = Color::Rgb(0xf8, 0xfa, 0xfc);
const SLATE_400: Color = Color::Rgb(0x94, 0xa3, 0xb8);
const SLATE_500: Color = Color::Rgb(0x64, 0x74, 0x8b);
const SLATE_600: Color = Color::Rgb(0x47, 0x55, 0x69);
const SLATE_700: Color = Color::Rgb(0x33, 0x41, 0x55);
const SLATE_900: Color = Color::Rgb(0x0f, 0x17, 0x2a);

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Top-level menu items ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    OpenMain     = 0,
    Update       = 1,
    AutoUpdate   = 2,
    PauseSubmenu = 3,
    Quit         = 4,
}

impl MenuItem {
    pub const COUNT: usize = 5;

    pub fn from_idx(i: usize) -> Self {
        match i % Self::COUNT {
            0 => Self::OpenMain,
            1 => Self::Update,
            2 => Self::AutoUpdate,
            3 => Self::PauseSubmenu,
            _ => Self::Quit,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenMain     => "Ouvrir le menu principal",
            Self::Update       => "Vérifier les mises à jour",
            Self::AutoUpdate   => "Mises à jour automatiques",
            Self::PauseSubmenu => "Pause temporaire",
            Self::Quit         => "Éteindre Polygone",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Self::OpenMain     => {
                "Dashboard 4 onglets : Accueil · Favoris · Services · Paramètres."
            }
            Self::Update       => {
                "Recherche la dernière version stable (canal officiel)."
            }
            Self::AutoUpdate   => {
                "Lance la vérification en arrière-plan au démarrage."
            }
            Self::PauseSubmenu => {
                "Suspendre le nœud sans quitter Polygone."
            }
            Self::Quit         => {
                "Quitter Polygone proprement (zéro log local)."
            }
        }
    }
}

// ── Pause sub-menu ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseOption {
    Minutes30 = 0,
    Hours1    = 1,
    Hours3    = 2,
    Custom    = 3,
}

impl PauseOption {
    pub const COUNT: usize = 4;

    pub fn from_idx(i: usize) -> Self {
        match i % Self::COUNT {
            0 => Self::Minutes30,
            1 => Self::Hours1,
            2 => Self::Hours3,
            _ => Self::Custom,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minutes30 => "30 minutes",
            Self::Hours1    => "1 heure",
            Self::Hours3    => "3 heures",
            Self::Custom    => "Personnalisé (minutes)",
        }
    }

    pub fn minutes(self) -> i64 {
        match self {
            Self::Minutes30 => 30,
            Self::Hours1    => 60,
            Self::Hours3    => 180,
            Self::Custom    => 0,
        }
    }
}

// ── Sub-screen of the menu flow ─────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MenuScreen {
    #[default]
    Main,
    PauseSubmenu,
    PauseCustom,
}

// ── Live menu state held inside App ──────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct MenuState {
    pub screen: MenuScreen,
    pub selected: usize,
    pub pause_selected: usize,
    pub custom_buffer: String,
}

// ── Persisted state (saved to ~/.config/polygone/state.json) ────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PersistentState {
    pub version: String,
    pub last_update_check: u64,
    pub auto_update: bool,
    pub pause_until: Option<u64>, // unix timestamp (seconds)
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            version: VERSION.into(),
            last_update_check: 0,
            auto_update: false,
            pause_until: None,
        }
    }
}

impl PersistentState {
    pub fn path() -> Option<PathBuf> {
        let base = dirs::config_dir()?.join("polygone");
        Some(base.join("state.json"))
    }

    fn ensure_dir() -> io::Result<()> {
        if let Some(p) = Self::path() {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }

    pub fn load() -> Self {
        let mut s = Self::default();
        if let Some(p) = Self::path() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(parsed) = serde_json::from_str::<Self>(&text) {
                    s = parsed;
                    s.version = VERSION.into(); // always current
                }
            }
        }
        let _ = Self::ensure_dir();
        s
    }

    pub fn save(&self) -> io::Result<()> {
        Self::ensure_dir()?;
        let p = Self::path().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no config dir available")
        })?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(&p, json)
    }

    /// True iff a `pause_until` is set and still in the future.
    pub fn pause_active(&self) -> bool {
        matches!(self.pause_until, Some(until) if until > unix_now_secs())
    }
}

fn state_path_label() -> String {
    PersistentState::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<config dir unavailable>".into())
}

// ── Outcome the caller acts on ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    Stay,
    OpenDashboard,
    Quit,
}

/// Side-effects of "Vérifier MAJ" / toggle "MAJ auto".
/// Carried back to the caller so it can emit a feed message and decide
/// if the node should pause per `PersistentState::pause_active()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSideEffect {
    CheckedUpdate,
    ToggledAutoUpdate(bool), // new value
    PausedFor(u64),          // pause_until timestamp
    None,
}

/// Single entry point: applies the key, returns (what should the App
/// do, what notable side-effect happened — if any).
pub fn handle_menu_key(
    state: &mut MenuState,
    persistent: &mut PersistentState,
    key: KeyCode,
) -> (MenuOutcome, MenuSideEffect) {
    match state.screen {
        MenuScreen::Main => match key {
            KeyCode::Up => {
                state.selected = if state.selected == 0 {
                    MenuItem::COUNT - 1
                } else {
                    state.selected - 1
                };
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Down => {
                state.selected = (state.selected + 1) % MenuItem::COUNT;
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Enter => match MenuItem::from_idx(state.selected) {
                MenuItem::OpenMain => (MenuOutcome::OpenDashboard, MenuSideEffect::None),
                MenuItem::Quit => (MenuOutcome::Quit, MenuSideEffect::None),
                MenuItem::Update => {
                    persistent.last_update_check = unix_now_secs();
                    let _ = persistent.save();
                    (MenuOutcome::Stay, MenuSideEffect::CheckedUpdate)
                }
                MenuItem::AutoUpdate => {
                    persistent.auto_update = !persistent.auto_update;
                    let _ = persistent.save();
                    let v = persistent.auto_update;
                    (MenuOutcome::Stay, MenuSideEffect::ToggledAutoUpdate(v))
                }
                MenuItem::PauseSubmenu => {
                    state.screen = MenuScreen::PauseSubmenu;
                    state.pause_selected = 0;
                    (MenuOutcome::Stay, MenuSideEffect::None)
                }
            },
            KeyCode::Esc => (MenuOutcome::OpenDashboard, MenuSideEffect::None),
            _ => (MenuOutcome::Stay, MenuSideEffect::None),
        },

        MenuScreen::PauseSubmenu => match key {
            KeyCode::Up => {
                state.pause_selected = if state.pause_selected == 0 {
                    PauseOption::COUNT - 1
                } else {
                    state.pause_selected - 1
                };
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Down => {
                state.pause_selected =
                    (state.pause_selected + 1) % PauseOption::COUNT;
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Enter => {
                let opt = PauseOption::from_idx(state.pause_selected);
                if opt == PauseOption::Custom {
                    state.screen = MenuScreen::PauseCustom;
                    state.custom_buffer.clear();
                    (MenuOutcome::Stay, MenuSideEffect::None)
                } else {
                    let mins = opt.minutes();
                    let until = if mins > 0 {
                        let u = unix_now_secs() + (mins as u64) * 60;
                        persistent.pause_until = Some(u);
                        let _ = persistent.save();
                        Some(u)
                    } else {
                        None
                    };
                    state.screen = MenuScreen::Main;
                    (
                        MenuOutcome::Stay,
                        match until {
                            Some(u) => MenuSideEffect::PausedFor(u),
                            None => MenuSideEffect::None,
                        },
                    )
                }
            }
            KeyCode::Esc => {
                state.screen = MenuScreen::Main;
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            _ => (MenuOutcome::Stay, MenuSideEffect::None),
        },

        MenuScreen::PauseCustom => match key {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if state.custom_buffer.len() < 6 {
                    state.custom_buffer.push(c);
                }
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Backspace => {
                state.custom_buffer.pop();
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            KeyCode::Enter => {
                let mut effect = MenuSideEffect::None;
                if let Ok(min) = state.custom_buffer.parse::<i64>() {
                    if min > 0 && min <= 99999 {
                        let u = unix_now_secs() + (min as u64) * 60;
                        persistent.pause_until = Some(u);
                        let _ = persistent.save();
                        effect = MenuSideEffect::PausedFor(u);
                    }
                }
                state.custom_buffer.clear();
                state.screen = MenuScreen::Main;
                (MenuOutcome::Stay, effect)
            }
            KeyCode::Esc => {
                state.custom_buffer.clear();
                state.screen = MenuScreen::PauseSubmenu;
                (MenuOutcome::Stay, MenuSideEffect::None)
            }
            _ => (MenuOutcome::Stay, MenuSideEffect::None),
        },
    }
}

// ── Render ──────────────────────────────────────────────────────────────────

pub fn render_menu(frame: &mut Frame, menu: &MenuState, persistent: &PersistentState) {
    match menu.screen {
        MenuScreen::Main => render_main(frame, frame.area(), menu, persistent),
        MenuScreen::PauseSubmenu => render_pause_submenu(frame, frame.area(), menu),
        MenuScreen::PauseCustom => render_pause_custom(frame, frame.area(), menu),
    }
}

fn render_main(
    frame: &mut Frame,
    area: Rect,
    menu: &MenuState,
    persistent: &PersistentState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Min(8),    // items + selected description
            Constraint::Length(3), // help
            Constraint::Length(1), // state path
        ])
        .split(area);

    // ── Title ──
    let title = vec![
        Line::from(vec![
            Span::styled(" ⬡ ", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled("POLYGONE ", Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)),
            Span::styled(format!("v{VERSION}"), SLATE_600),
            Span::styled("  —  Menu", Style::default().fg(CYBER)),
        ]),
        Line::from(Span::styled(
            " L'information n'existe pas. Elle traverse.",
            SLATE_500,
        )),
    ];
    frame.render_widget(
        Paragraph::new(title)
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(SLATE_700)),
            ),
        chunks[0],
    );

    // ── Items + description ──
    let mut lines: Vec<Line> = (0..MenuItem::COUNT)
        .map(|i| {
            let item = MenuItem::from_idx(i);
            let selected = i == menu.selected;
            let indicator = if selected { "▶ " } else { "  " };
            // Ternary must return `Style` from both branches in
            // order to bind coherently. Wrap the else colors.
            let style_label = if selected {
                Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SLATE_400)
            };
            let style_ind = if selected {
                Style::default().fg(CYBER).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SLATE_700)
            };
            let mut spans = vec![
                Span::styled(indicator, style_ind),
                Span::styled(item.label(), style_label),
            ];
            let suffix = match item {
                MenuItem::AutoUpdate => format!(
                    "  [{}]",
                    if persistent.auto_update { "ON " } else { "OFF" }
                ),
                MenuItem::PauseSubmenu => {
                    if let Some(until) = persistent.pause_until {
                        let now = unix_now_secs();
                        if until > now {
                            format!("  [{} min restants]", (until - now) / 60)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                }
                _ => String::new(),
            };
            if !suffix.is_empty() {
                spans.push(Span::styled(
                    suffix,
                    Style::default().fg(if item == MenuItem::AutoUpdate {
                        CYBER
                    } else {
                        AMBER
                    }),
                ));
            }
            Line::from(spans)
        })
        .collect();

    let selected_item = MenuItem::from_idx(menu.selected);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("   {}", selected_item.desc()),
        SLATE_500,
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(CYBER_DIM)),
            ),
        chunks[1],
    );

    // ── Help ──
    let help = Line::from(vec![
        Span::styled(" ↑↓ ", CYBER), Span::styled("naviguer  ", SLATE_500),
        Span::styled("↵ ", CYBER), Span::styled("valider  ", SLATE_500),
        Span::styled("Échap ", CYBER), Span::styled("dashboard  ", SLATE_500),
        Span::styled("q ", CYBER), Span::styled("quitter", SLATE_500),
    ]);
    frame.render_widget(
        Paragraph::new(vec![help])
            .style(Style::default().bg(SLATE_900)),
        chunks[2],
    );

    // ── State path ──
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ⛁ ", SLATE_600),
            Span::styled(state_path_label(), SLATE_600),
        ]))
        .style(Style::default().bg(SLATE_900)),
        chunks[3],
    );
}

fn render_pause_submenu(frame: &mut Frame, area: Rect, menu: &MenuState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Line::from(vec![
        Span::styled(" ⬡  ", CYBER),
        Span::styled("Pause temporaire", Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)),
        Span::styled("  —  combien de temps ?", SLATE_500),
    ]);
    frame.render_widget(
        Paragraph::new(vec![title, Line::from("")])
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(SLATE_700)),
            ),
        chunks[0],
    );

    let lines: Vec<Line> = (0..PauseOption::COUNT)
        .map(|i| {
            let opt = PauseOption::from_idx(i);
            let selected = i == menu.pause_selected;
            let ind = if selected { "▶ " } else { "  " };
            // Both branches must be `Style` for assignment.
            let style_label = if selected {
                Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SLATE_400)
            };
            let style_ind = if selected {
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(SLATE_700)
            };
            Line::from(vec![
                Span::styled(ind, style_ind),
                Span::styled(opt.label(), style_label),
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(AMBER)),
            ),
        chunks[1],
    );

    let help = Line::from(vec![
        Span::styled(" ↑↓ ", AMBER), Span::styled("naviguer  ", SLATE_500),
        Span::styled("↵ ", AMBER), Span::styled("valider  ", SLATE_500),
        Span::styled("Échap ", AMBER), Span::styled("retour", SLATE_500),
    ]);
    frame.render_widget(
        Paragraph::new(vec![help])
            .style(Style::default().bg(SLATE_900)),
        chunks[3.min(chunks.len() - 1)],
    );
}

fn render_pause_custom(frame: &mut Frame, area: Rect, menu: &MenuState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Line::from(vec![
        Span::styled(" ⬡  ", CYBER),
        Span::styled(
            "Pause personnalisée",
            Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  —  en minutes", SLATE_500),
    ]);
    frame.render_widget(
        Paragraph::new(vec![title, Line::from("")])
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(SLATE_700)),
            ),
        chunks[0],
    );

    let body = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("   Minutes : ", SLATE_500),
            Span::styled(
                format!("{}▏", menu.custom_buffer),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "   (0 < N ≤ 99999 — ↵ valider, Échap retour)",
            SLATE_600,
        )),
    ];
    frame.render_widget(
        Paragraph::new(body)
            .style(Style::default().bg(SLATE_900))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(AMBER)),
            ),
        chunks[1],
    );

    let help = Line::from(vec![
        Span::styled(" chiffres ", AMBER), Span::styled("…  ", SLATE_500),
        Span::styled("⌫ ", AMBER), Span::styled("effacer  ", SLATE_500),
        Span::styled("↵ ", AMBER), Span::styled("valider  ", SLATE_500),
        Span::styled("Échap ", AMBER), Span::styled("retour", SLATE_500),
    ]);
    frame.render_widget(
        Paragraph::new(vec![help])
            .style(Style::default().bg(SLATE_900)),
        chunks[3.min(chunks.len() - 1)],
    );
}
