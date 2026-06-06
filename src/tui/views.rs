//! All TUI views for POLYGONE — 5 tabs: Accueil, Favoris, Services, Composer, Paramètres.
//! Renders interactive dashboard with live node stats, modules, activity feed.
//! Features: sparklines, gauge, topology, message composer, theme support.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use super::app::{App, MessageKind, ModuleCard, ModuleStatus, Theme};
use super::widgets::*;

// ── Color palette (legacy constants for backward compatibility) ───────────────
// These are used by the inline render functions. For theme-aware rendering,
// use theme.colors() instead.

// ── View enum ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard = 0,
    Favorites = 1,
    Services = 2,
    Composer = 3,
    Settings = 4,
}

impl View {
    pub const COUNT: usize = 5;

    pub fn from_idx(idx: usize) -> Self {
        match idx % 5 {
            0 => Self::Dashboard,
            1 => Self::Favorites,
            2 => Self::Services,
            3 => Self::Composer,
            _ => Self::Settings,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "🏠 Accueil",
            Self::Favorites => "⭐ Favoris",
            Self::Services => "🧩 Services",
            Self::Composer => "📝 Composer",
            Self::Settings => "⚙️ Params",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

// ── Splash screen ────────────────────────────────────────────────────────────

/// Render the animated splash / boot screen.
pub fn render_splash(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let c = app.theme.colors();

    // Full-screen background
    let bg = Paragraph::new("")
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(bg, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

    // ASCII art logo
    let logo_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "         ╔══════════════════════════════╗",
            Style::default().fg(rgb(c.accent)),
        )),
        Line::from(Span::styled(
            "         ║   ⬡  P O L Y G O N E  ⬡     ║",
            Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "         ║  Post-quantum privacy mesh   ║",
            Style::default().fg(rgb(c.text_dim)),
        )),
        Line::from(Span::styled(
            "         ╚══════════════════════════════╝",
            Style::default().fg(rgb(c.accent)),
        )),
        Line::from(""),
    ];

    let logo = Paragraph::new(logo_lines)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(logo, chunks[0]);

    // Loading bar
    let progress = app.splash_progress;
    let bar_width = 40;
    let filled = (progress * bar_width as f32) as usize;
    let bar: String = (0..bar_width)
        .map(|i| if i < filled { '█' } else { '░' })
        .collect();

    let loading_lines = vec![
        Line::from(vec![
            Span::styled("    ", Style::default().bg(rgb(c.bg))),
            Span::styled(bar, Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("    ", Style::default().bg(rgb(c.bg))),
            Span::styled(
                format!("  {}%", (progress * 100.0) as u32),
                Style::default().fg(rgb(c.text_dim)),
            ),
        ]),
    ];

    let loading = Paragraph::new(loading_lines)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(loading, chunks[1]);

    // Status messages
    let status_lines = vec![
        Line::from(vec![
            Span::styled("  → ", Style::default().fg(rgb(c.green))),
            Span::styled("Initialisation ML-KEM-1024...", Style::default().fg(rgb(c.text_dim))),
        ]),
    ];
    let status = Paragraph::new(status_lines)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(status, chunks[2]);

    // Crypto suite info
    let crypto_lines = vec![
        Line::from(vec![
            Span::styled("    ML-KEM-1024", Style::default().fg(rgb(c.accent))),
            Span::styled(" · ", Style::default().fg(rgb(c.border))),
            Span::styled("Shamir 4-of-7", Style::default().fg(rgb(c.accent))),
            Span::styled(" · ", Style::default().fg(rgb(c.border))),
            Span::styled("AES-256-GCM", Style::default().fg(rgb(c.accent))),
            Span::styled(" · ", Style::default().fg(rgb(c.border))),
            Span::styled("BLAKE3", Style::default().fg(rgb(c.accent))),
        ]),
        Line::from(vec![
            Span::styled(
                "    Appuyez sur une touche pour continuer...",
                Style::default().fg(rgb(c.text_faint)).add_modifier(Modifier::ITALIC),
            ),
        ]),
    ];
    let crypto = Paragraph::new(crypto_lines)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(crypto, chunks[3]);
}

// ── Root render dispatcher ───────────────────────────────────────────────────

pub fn render_view(frame: &mut Frame, app: &App) {
    let c = app.theme.colors();

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),    // header + tab bar
            Constraint::Min(12),      // main content
            Constraint::Length(7),    // activity log
            Constraint::Length(1),    // status bar
        ])
        .split(area);

    render_top_bar(frame, chunks[0], app);

    match app.current_view {
        View::Dashboard => render_dashboard_view(frame, chunks[1], app),
        View::Favorites => render_favorites_view(frame, chunks[1], app),
        View::Services => render_services_view(frame, chunks[1], app),
        View::Composer => render_composer_view(frame, chunks[1], app),
        View::Settings => render_settings_view(frame, chunks[1], app),
    }

    render_activity_log(frame, chunks[2], app);
    render_status_bar_themed(frame, chunks[3], app);
}

// ── Top bar: header + tabs ───────────────────────────────────────────────────

fn render_top_bar(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(3)])
        .split(area);

    // Header line
    let header = vec![Line::from(vec![
        Span::styled(" ⬡ ", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        Span::styled("POLYGONE", Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD)),
        Span::styled("  v1.0.0", Style::default().fg(rgb(c.text_faint))),
        Span::styled("  —  ", Style::default().fg(rgb(c.border))),
        Span::styled("ML-KEM-1024", Style::default().fg(rgb(c.accent))),
        Span::styled(" · ", Style::default().fg(rgb(c.border))),
        Span::styled("Shamir 4-of-7", Style::default().fg(rgb(c.accent))),
        Span::styled(" · ", Style::default().fg(rgb(c.border))),
        Span::styled("AES-256-GCM", Style::default().fg(rgb(c.accent))),
        Span::styled(" · ", Style::default().fg(rgb(c.border))),
        Span::styled("BLAKE3", Style::default().fg(rgb(c.accent))),
    ])];

    let p = Paragraph::new(header).style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, chunks[0]);

    // Tab bar
    let tab_width = (area.width as usize).saturating_sub(2) / View::COUNT;

    // Background bar
    frame.render_widget(
        Paragraph::new(Line::from(Span::raw("")))
            .style(Style::default().bg(rgb(c.bg))),
        chunks[1],
    );

    let active_idx = app.current_view.index();
    for i in 0..View::COUNT {
        let view = View::from_idx(i);
        let label = view.label();
        let x = i * tab_width + 1;
        let is_active = i == active_idx;

        // Tab background
        if is_active {
            let tab_area = Rect::new(x as u16, chunks[1].y, tab_width as u16, 3);
            frame.render_widget(
                Paragraph::new(Line::from(Span::raw("")))
                    .style(Style::default().bg(rgb(c.bg_2))),
                tab_area,
            );
        }

        let style = if is_active {
            Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(rgb(c.text_faint))
        };

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, style)))
                .style(Style::default().bg(if is_active { rgb(c.bg_2) } else { rgb(c.bg) })),
            Rect::new((x + 1) as u16, chunks[1].y + 1, tab_width as u16, 1),
        );

        // Active tab underline
        if is_active {
            frame.render_widget(
                Paragraph::new(Line::from(Span::raw("─".repeat(tab_width.saturating_sub(2)))))
                    .style(Style::default().fg(rgb(c.accent))),
                Rect::new((x + 1) as u16, chunks[1].y + 2, tab_width.saturating_sub(2) as u16, 1),
            );
        }
    }

    // Status dot on the right
    let dot = Span::styled(" ●", Style::default().fg(rgb(c.green)).add_modifier(Modifier::BOLD));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default().fg(rgb(c.text_faint))),
            dot,
            Span::styled(" Actif", Style::default().fg(rgb(c.text_faint))),
        ])),
        Rect::new(area.width.saturating_sub(16), chunks[1].y + 1, 14, 1),
    );
}

// ── View: Dashboard (Accueil) ────────────────────────────────────────────────

fn render_dashboard_view(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    // Three-panel layout: left (node + gauge), center (topology), right (modules)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    // ── Left panel: Node status + sparklines ──
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // node status
            Constraint::Length(5),   // traffic in sparkline
            Constraint::Length(5),   // traffic out sparkline
        ])
        .split(chunks[0]);

    // Node status card
    let s = &app.stats;
    let uptime_m = s.uptime_secs / 60;
    let uptime_h = uptime_m / 60;
    let uptime_min = uptime_m % 60;

    let node_lines = vec![
        Line::from(vec![
            Span::styled(" ● ", Style::default().fg(rgb(c.green)).add_modifier(Modifier::BOLD)),
            Span::styled("ACTIF", Style::default().fg(rgb(c.green)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  depuis {uptime_h}h{uptime_min:02}m"), Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Pairings  ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(format!("{} nœuds", s.peers), Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("Spinner   ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(spinner(app.tick), Style::default().fg(rgb(c.accent))),
        ]),
    ];
    let p = Paragraph::new(node_lines)
        .block(block_card_themed("Statut du nœud", c.accent, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, left_chunks[0]);

    // Traffic sparklines
    render_sparkline(
        frame,
        left_chunks[1],
        &app.traffic_in_history,
        "Traffic ↓",
        c.green,
        c.bg,
    );

    render_sparkline(
        frame,
        left_chunks[2],
        &app.traffic_out_history,
        "Traffic ↑",
        c.accent,
        c.bg,
    );

    // ── Center panel: Fragments gauge + topology ──
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),   // fragments gauge
            Constraint::Min(8),     // topology
        ])
        .split(chunks[1]);

    // Fragments gauge (replaces text bar)
    let frag_ratio = s.fragments_ready as f64 / s.fragments_needed as f64;
    render_gauge(
        frame,
        center_chunks[0],
        &format!("Shamir {}/{}", s.fragments_ready, s.fragments_needed),
        frag_ratio,
        app.theme,
    );

    // Network topology mini-map
    render_topology(frame, center_chunks[1], app);

    // ── Right panel: Module cards grid ──
    render_module_grid(frame, chunks[2], &app.modules, app.theme);
}

// ── Module grid (re-used in Dashboard, Services, Favorites) ──────────────────

fn render_module_grid(frame: &mut Frame, area: Rect, modules: &[ModuleCard], theme: Theme) {
    let c = theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ])
        .split(area);

    for (row_idx, row) in modules.chunks(2).enumerate() {
        if row_idx >= 3 { break; }
        if let Some(chunk) = chunks.get(row_idx) {
            let row_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                .split(*chunk);

            for (col_idx, module) in row.iter().enumerate() {
                if let Some(cell) = row_chunks.get(col_idx) {
                    render_module_card(frame, *cell, module, theme);
                }
            }
        }
    }
}

fn render_module_card(frame: &mut Frame, area: Rect, module: &ModuleCard, theme: Theme) {
    let c = theme.colors();
    let status_color = module.status.color(theme);
    let status_label = module.status.label();

    let (border_accent, border_style) = match module.status {
        ModuleStatus::Running => (c.green, Style::default().fg(rgb(c.green))),
        ModuleStatus::Off => (c.text_faint, Style::default().fg(rgb(c.border))),
        ModuleStatus::Error => (c.rose, Style::default().fg(rgb(c.rose))),
        ModuleStatus::ComingSoon => (c.amber, Style::default().fg(rgb(c.amber)).add_modifier(Modifier::DIM)),
    };

    let mut lines = vec![
        Line::from(vec![
            Span::raw(format!(" {}  ", module.icon)),
            Span::styled(module.name, Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" [{}]", status_label), Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled(format!("     {}", module.desc), Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(vec![
            Span::styled("     v", Style::default().fg(rgb(c.text_faint))),
            Span::styled(module.version, Style::default().fg(rgb(c.accent_dim))),
        ]),
    ];

    // Add progress bar for Coming Soon modules
    if let Some(progress) = module.status.progress() {
        let bar_width = 16;
        let filled = (progress as usize * bar_width) / 100;
        let bar: String = (0..bar_width)
            .map(|i| if i < filled { '█' } else { '░' })
            .collect();
        lines.push(Line::from(vec![
            Span::styled("     ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(bar, Style::default().fg(rgb(c.amber)).add_modifier(Modifier::DIM)),
            Span::styled(format!(" {progress}%"), Style::default().fg(rgb(c.amber))),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let p = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, area);
}

// ── View: Favorites ──────────────────────────────────────────────────────────

fn render_favorites_view(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(4)])
        .split(area);

    let header_lines = vec![
        Line::from(vec![
            Span::styled(" ⭐ Vos favoris", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("   Appuyez sur ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("M", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled(" Msg · ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("H", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled(" Hide · ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("D", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled(" Drive · ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("N", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled(" Mesh", Style::default().fg(rgb(c.text_faint))),
        ]),
    ];
    let p = Paragraph::new(header_lines)
        .block(block_card_themed("Favoris", c.amber, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, chunks[0]);

    // Show only running modules as favorites
    let favs: Vec<&ModuleCard> = app.modules.iter().filter(|m| m.status == ModuleStatus::Running).collect();
    if favs.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("   Aucun module actif pour le moment.", Style::default().fg(rgb(c.text_faint))),
            ]),
            Line::from(vec![
                Span::styled("   Activez Msg (M) ou Hide (H) depuis l'Accueil.", Style::default().fg(rgb(c.text_faint))),
            ]),
        ];
        let p = Paragraph::new(lines)
            .style(Style::default().bg(rgb(c.bg)));
        frame.render_widget(p, chunks[1]);
    } else {
        render_module_grid(frame, chunks[1], &favs.iter().map(|m| (*m).clone()).collect::<Vec<_>>(), app.theme);
    }
}

// ── View: Services ───────────────────────────────────────────────────────────

fn render_services_view(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    let header_lines = vec![
        Line::from(vec![
            Span::styled(" 🧩 Services Polygone ", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled("— modules et extensions réseau", Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(vec![
            Span::styled("   ↑↓ ", Style::default().fg(rgb(c.accent))),
            Span::styled("naviguer  ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("M/H/D/N", Style::default().fg(rgb(c.accent))),
            Span::styled(" toggles", Style::default().fg(rgb(c.text_faint))),
        ]),
    ];
    let p = Paragraph::new(header_lines)
        .block(block_card_themed("Services", c.violet, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, chunks[0]);

    // Full module grid with descriptions
    let grid_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ])
        .split(chunks[1]);

    let descs = [
        ("💬  Msg", "Messagerie E2E éphémère — auto-destruction, zéro persistance", "M", c.green),
        ("👻  Hide", "Proxy SOCKS5 — navigation anonyme en un clic", "H", c.violet),
        ("📁  Drive", "Stockage distribué chiffré — fichiers fragmentés", "D", c.amber),
        ("🔗  Mesh", "Réseau local P2P — mutualisation de machines", "N", c.emerald),
        ("🧠  Brain", "IA locale via Notch SLM — inférence privée (🔜)", "", c.amber),
    ];

    for (i, (name, desc, key, color)) in descs.iter().enumerate() {
        if let Some(chunk) = grid_chunks.get(i) {
            let s = format!(" {}  {}", name, desc);
            let line = if !key.is_empty() {
                Line::from(vec![
                    Span::styled(format!(" [{key}]"), Style::default().fg(rgb(*color)).add_modifier(Modifier::BOLD)),
                    Span::styled(s, Style::default().fg(rgb(c.text))),
                ])
            } else {
                Line::from(vec![Span::styled(s, Style::default().fg(rgb(c.text_dim)))])
            };

            // Add progress bar for Coming Soon modules
            let mut lines = vec![Line::from(""), line];
            if key.is_empty() {
                let bar_width = 20;
                let progress = 35usize;
                let filled = progress * bar_width / 100;
                let bar: String = (0..bar_width)
                    .map(|j| if j < filled { '█' } else { '░' })
                    .collect();
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default().fg(rgb(c.text_faint))),
                    Span::styled(bar, Style::default().fg(rgb(c.amber)).add_modifier(Modifier::DIM)),
                    Span::styled(" 35% en développement", Style::default().fg(rgb(c.amber))),
                ]));
            }

            let p = Paragraph::new(lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(rgb(c.border)))
                    .style(Style::default().bg(rgb(c.bg))))
                .style(Style::default().bg(rgb(c.bg)));
            frame.render_widget(p, *chunk);
        }
    }
}

// ── View: Composer (message encryption visualizer) ───────────────────────────

fn render_composer_view(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // Left: Message input area
    let input_lines = vec![
        Line::from(vec![
            Span::styled(" 📝 ", Style::default().fg(rgb(c.accent))),
            Span::styled("Nouveau message éphémère", Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Destinataire : ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("alice@polygone.local", Style::default().fg(rgb(c.accent))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ┌──────────────────────────────────────┐", Style::default().fg(rgb(c.border))),
        ]),
        Line::from(vec![
            Span::styled("  │ ", Style::default().fg(rgb(c.border))),
            Span::styled("Tapez votre message ici...              ", Style::default().fg(rgb(c.text_faint)).add_modifier(Modifier::ITALIC)),
            Span::styled("│", Style::default().fg(rgb(c.border))),
        ]),
        Line::from(vec![
            Span::styled("  │                                      │", Style::default().fg(rgb(c.border))),
        ]),
        Line::from(vec![
            Span::styled("  │  Message auto-destructeur             │", Style::default().fg(rgb(c.text_faint))),
            Span::styled("│", Style::default().fg(rgb(c.border))),
        ]),
        Line::from(vec![
            Span::styled("  └──────────────────────────────────────┘", Style::default().fg(rgb(c.border))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  TTL: ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("5 min", Style::default().fg(rgb(c.amber))),
            Span::styled("  |  ", Style::default().fg(rgb(c.border))),
            Span::styled("Chiffrement: ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("AES-256-GCM", Style::default().fg(rgb(c.accent))),
            Span::styled("  |  ", Style::default().fg(rgb(c.border))),
            Span::styled("KEM: ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("ML-KEM-1024", Style::default().fg(rgb(c.accent))),
        ]),
    ];

    let input = Paragraph::new(input_lines)
        .block(block_card_themed("Composer", c.accent, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(input, chunks[0]);

    // Right: Encryption pipeline
    render_encrypt_pipeline(frame, chunks[1], app);
}

// ── View: Settings ───────────────────────────────────────────────────────────

fn render_settings_view(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // Left: system controls + keyboard shortcuts
    let control_lines = vec![
        Line::from(vec![
            Span::styled("Gestion du système", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [M] Msg     toggle", Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("  [H] Hide    toggle", Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("  [D] Drive   toggle", Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("  [N] Mesh    toggle", Style::default().fg(rgb(c.text))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Raccourcis", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ← → onglets", Style::default().fg(rgb(c.text_dim))),
        ]),
        Line::from(vec![
            Span::styled("  1-5 onglets directs", Style::default().fg(rgb(c.text_dim))),
        ]),
        Line::from(vec![
            Span::styled("  T  thème suivant", Style::default().fg(rgb(c.text_dim))),
        ]),
        Line::from(vec![
            Span::styled("  q  quitter", Style::default().fg(rgb(c.text_dim))),
        ]),
    ];
    let p = Paragraph::new(control_lines)
        .block(block_card_themed("Paramètres", c.accent, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, chunks[0]);

    // Right: system status + theme selector
    let running_count = app.modules.iter().filter(|m| m.status == ModuleStatus::Running).count();
    let status_lines = vec![
        Line::from(vec![
            Span::styled("État du système", Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Nœud:       ", Style::default().fg(rgb(c.text_faint))),
            Span::styled("● Actif", Style::default().fg(rgb(c.green))),
        ]),
        Line::from(vec![
            Span::styled("  Modules:    ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(format!("{}/5 actifs", running_count), Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("  Fragments:  ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(format!("{}/{}", app.stats.fragments_ready, app.stats.fragments_needed), Style::default().fg(rgb(c.text))),
        ]),
        Line::from(vec![
            Span::styled("  Pairings:   ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(format!("{} nœuds", app.stats.peers), Style::default().fg(rgb(c.text))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⬡ ", Style::default().fg(rgb(c.accent))),
            Span::styled(format!("{}", app.stats.balance), Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  •  {:.1}/min", app.stats.consumption), Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Thème actuel: ", Style::default().fg(rgb(c.text_faint))),
            Span::styled(app.theme.label(), Style::default().fg(rgb(c.accent)).add_modifier(Modifier::BOLD)),
            Span::styled("  [T] pour changer", Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Polygone TUI v1.0.0", Style::default().fg(rgb(c.text_faint))),
        ]),
        Line::from(vec![
            Span::styled("MIT · github.com/lvs0/Polygone-Network", Style::default().fg(rgb(c.text_faint))),
        ]),
    ];
    let p = Paragraph::new(status_lines)
        .block(block_card_themed("Status", c.green, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, chunks[1]);
}

// ── Activity log ─────────────────────────────────────────────────────────────

fn render_activity_log(frame: &mut Frame, area: Rect, app: &App) {
    let c = app.theme.colors();
    let height = area.height.saturating_sub(2) as usize;
    let msgs = &app.messages;
    let start = msgs.len().saturating_sub(height).saturating_sub(app.log_offset);

    let lines: Vec<Line> = msgs[start..].iter().map(|(kind, msg, ts)| {
        // Improved timestamp formatting
        let ts_s = if *ts >= 86400 {
            format!("{}d{}h", ts / 86400, (ts % 86400) / 3600)
        } else if *ts >= 3600 {
            format!("{}h{:02}m", ts / 3600, (ts % 3600) / 60)
        } else if *ts >= 60 {
            format!("{:02}m{:02}s", ts / 60, ts % 60)
        } else {
            format!("{}s", ts)
        };

        // Color-code by kind
        let kind_color = kind.color(app.theme);

        Line::from(vec![
            Span::styled(format!(" {} ", kind.symbol()), Style::default().fg(kind_color).add_modifier(Modifier::BOLD)),
            Span::styled(msg.clone(), Style::default().fg(rgb(c.text))),
            Span::styled(format!("  {} ago", ts_s), Style::default().fg(rgb(c.text_faint))),
        ])
    }).collect();

    let block = Block::default()
        .title(Span::styled(
            format!(" 📋 Activité {} ", if app.log_offset > 0 { "↑↓" } else { "" }),
            Style::default().fg(rgb(c.text_dim)),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(c.border)));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, area);
}

// ── Status bar ───────────────────────────────────────────────────────────────
// Note: render_status_bar_themed is in widgets.rs and called from render_view.
