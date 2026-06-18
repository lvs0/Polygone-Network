//! All TUI views for POLYGONE — 4 tabs: Accueil, Favoris, Services, Paramètres.
//! Renders interactive dashboard with live node stats, modules, activity feed.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Sparkline, Wrap},
    Frame,
};

use super::app::{App, ModuleCard, ModuleStatus};
use super::widgets::*;

// ── Color palette ──────────────────────────────────────────────────────────────
const CYBER: Color = Color::Rgb(0x22, 0xd3, 0xee);
const CYBER_DIM: Color = Color::Rgb(0x08, 0x91, 0xb2);
const GREEN: Color = Color::Rgb(0x22, 0xc5, 0x5e);
const EMERALD: Color = Color::Rgb(0x34, 0xd3, 0x99);
const VIOLET: Color = Color::Rgb(0xa7, 0x8b, 0xfa);
const AMBER: Color = Color::Rgb(0xfb, 0xbd, 0x24);
const ROSE: Color = Color::Rgb(0xfb, 0x71, 0x85);
const SLATE_50: Color = Color::Rgb(0xf8, 0xfa, 0xfc);
const SLATE_300: Color = Color::Rgb(0xcb, 0xd5, 0xe1);
const SLATE_400: Color = Color::Rgb(0x94, 0xa3, 0xb8);
const SLATE_500: Color = Color::Rgb(0x64, 0x74, 0x8b);
const SLATE_600: Color = Color::Rgb(0x47, 0x55, 0x69);
const SLATE_700: Color = Color::Rgb(0x33, 0x41, 0x55);
const SLATE_800: Color = Color::Rgb(0x1e, 0x29, 0x3b);
const SLATE_900: Color = Color::Rgb(0x0f, 0x17, 0x2a);

// ── View enum ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard = 0,
    Favorites = 1,
    Services = 2,
    Settings = 3,
}

impl View {
    pub const COUNT: usize = 4;

    pub fn from_idx(idx: usize) -> Self {
        match idx % 4 {
            0 => Self::Dashboard,
            1 => Self::Favorites,
            2 => Self::Services,
            _ => Self::Settings,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "🏠 Accueil",
            Self::Favorites => "⭐ Favoris",
            Self::Services => "🧩 Services",
            Self::Settings => "⚙️ Paramètres",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

// ── Root render dispatcher ────────────────────────────────────────────────────

pub fn render_view(frame: &mut Frame, app: &App) {
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
        View::Settings => render_settings_view(frame, chunks[1], app),
    }

    render_activity_log(frame, chunks[2], app);
    render_status_bar(frame, chunks[3], app);
}

// ── Top bar: header + tabs ────────────────────────────────────────────────────

fn render_top_bar(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(3)])
        .split(area);

    // Header line
    let header = vec![Line::from(vec![
        Span::styled(" ⬡ ", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
        Span::styled("POLYGONE", Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)),
        Span::styled("  v1.0.0", Style::default().fg(SLATE_600)),
        Span::styled("  —  ", Style::default().fg(SLATE_700)),
        Span::styled("ML-KEM-1024", Style::default().fg(CYBER)),
        Span::styled(" · ", Style::default().fg(SLATE_700)),
        Span::styled("Shamir 4-of-7", Style::default().fg(CYBER)),
        Span::styled(" · ", Style::default().fg(SLATE_700)),
        Span::styled("AES-256-GCM", Style::default().fg(CYBER)),
        Span::styled(" · ", Style::default().fg(SLATE_700)),
        Span::styled("BLAKE3", Style::default().fg(CYBER)),
    ])];

    let p = Paragraph::new(header).style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[0]);

    // Tab bar
    let tab_labels = ["🏠 Accueil", "⭐ Favoris", "🧩 Services", "⚙️ Paramètres"];
    let tab_width = (area.width as usize).saturating_sub(2) / 4;

    // Background bar
    frame.render_widget(
        Paragraph::new(Line::from(Span::raw("")))
            .style(Style::default().bg(SLATE_900)),
        chunks[1],
    );

    let active_idx = app.current_view.index();
    for (i, label) in tab_labels.iter().enumerate() {
        let x = i * tab_width + 1;
        let is_active = i == active_idx;

        // Tab background
        if is_active {
            // Active tab: highlighted
            let tab_area = Rect::new(x as u16, chunks[1].y, tab_width as u16, 3);
            frame.render_widget(
                Paragraph::new(Line::from(Span::raw("")))
                    .style(Style::default().bg(SLATE_800)),
                tab_area,
            );
        }

        let style = if is_active {
            Style::default().fg(CYBER).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(SLATE_500)
        };

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(*label, style)))
                .style(Style::default().bg(if is_active { SLATE_800 } else { SLATE_900 })),
            Rect::new((x + 1) as u16, chunks[1].y + 1, tab_width as u16, 1),
        );

        // Active tab underline
        if is_active {
            frame.render_widget(
                Paragraph::new(Line::from(Span::raw("─".repeat(tab_width.saturating_sub(2)))))
                    .style(Style::default().fg(CYBER)),
                Rect::new((x + 1) as u16, chunks[1].y + 2, tab_width.saturating_sub(2) as u16, 1),
            );
        }
    }

    // Status dot on the right
    let dot = Span::styled(" ●", Style::default().fg(GREEN).add_modifier(Modifier::BOLD));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ", Style::default().fg(SLATE_500)),
            dot,
            Span::styled(" Actif", Style::default().fg(SLATE_500)),
        ])),
        Rect::new(area.width.saturating_sub(16), chunks[1].y + 1, 14, 1),
    );
}

// ── View: Dashboard (Accueil) ─────────────────────────────────────────────────

fn render_dashboard_view(frame: &mut Frame, area: Rect, app: &App) {
    // Three-panel layout: left (status + fragments), center (sparklines), right (modules)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(2, 5), Constraint::Ratio(1, 5), Constraint::Ratio(2, 5)])
        .split(area);

    // ── Left panel: Node status + bar ──
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Length(5)])
        .split(chunks[0]);

    // Node status card
    let s = &app.stats;
    let uptime_m = s.uptime_secs / 60;
    let snap = app.economy.snapshot();
    let refresh_secs = app.last_refresh.elapsed().as_secs();

    // Spec §4 (Accueil): pseudo + node_id_short, balance, refresh
    // indicator, pause state, shortcuts.
    let dot_color = if app.paused { AMBER } else { GREEN };
    let status_label = if app.paused { "EN PAUSE" } else { "ACTIF" };
    let node_lines = vec![
        Line::from(vec![
            Span::styled(" ● ", Style::default().fg(dot_color).add_modifier(Modifier::BOLD)),
            Span::styled(status_label, Style::default().fg(dot_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  depuis {uptime_m} min"), SLATE_500),
        ]),
        Line::from(vec![
            Span::styled("  Pseudo    ", SLATE_500),
            Span::styled(format!("@{}", app.identity.pseudo), Style::default().fg(SLATE_300).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ({})", &app.identity.node_id_short[..12]), SLATE_600),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Solde POLY", SLATE_500),
            Span::styled(format!("  {:>7.2}", snap.balance), Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            Span::styled("  POLY", SLATE_500),
        ]),
        Line::from(vec![
            Span::styled("  Drain     ", SLATE_500),
            Span::styled(format!("  {:>5.2} POLY/min", snap.rate_per_min), SLATE_400),
            Span::styled(format!("  ({} actif{})", snap.active, if snap.active > 1 { "s" } else { "" }), SLATE_600),
        ]),
        Line::from(vec![
            Span::styled("  Dernière MAJ  ", SLATE_500),
            Span::styled(format!("il y a {refresh_secs}s"), SLATE_400),
            Span::styled("  [R] pour rafraîchir", SLATE_600),
        ]),
        Line::from(vec![
            Span::styled("  Raccourcis   ", SLATE_500),
            Span::styled("[P]", AMBER), Span::styled("ause  ", SLATE_500),
            Span::styled("[R]", AMBER), Span::styled("afraîchir  ", SLATE_500),
            Span::styled("[U]", AMBER), Span::styled("pdate  ", SLATE_500),
            Span::styled("[Q]", AMBER), Span::styled("uitter", SLATE_500),
        ]),
    ];
    let p = Paragraph::new(node_lines)
        .block(block_card("Statut du nœud", CYBER))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, left_chunks[0]);

    // Fragments threshold — Gauge widget
    let frag_ratio = s.fragments_ready as f64 / s.fragments_needed as f64;
    let frag_color = if s.fragments_ready >= s.fragments_needed { GREEN } else { AMBER };
    let frag_gauge = Gauge::default()
        .block(Block::default()
            .title(Span::styled(
                format!(" Seuil Shamir {}/{}", s.fragments_ready, s.fragments_needed),
                Style::default().fg(frag_color).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(frag_color)))
        .gauge_style(Style::default().fg(frag_color).bg(SLATE_800))
        .ratio(frag_ratio)
        .label(Span::styled(
            if s.fragments_ready >= s.fragments_needed { "✔ PRÊT" } else { "⚠ INCOMPLET" },
            Style::default().fg(SLATE_900).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(frag_gauge, left_chunks[1]);

    // ── Right panel: Module cards grid ──
    render_module_grid(frame, chunks[2], &app.modules);

    // ── Center panel: Traffic sparklines ──
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Length(8), Constraint::Min(4)])
        .split(chunks[1]);

    // Traffic IN sparkline
    let in_data: Vec<u64> = app.traffic_history_in.iter().copied().collect();
    let spark_in = Sparkline::default()
        .block(Block::default()
            .title(Span::styled(" ↓ Trafic entrant ", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(CYBER_DIM)))
        .data(&in_data)
        .style(Style::default().fg(CYBER));
    frame.render_widget(spark_in, center_chunks[0]);

    // Traffic OUT sparkline
    let out_data: Vec<u64> = app.traffic_history_out.iter().copied().collect();
    let spark_out = Sparkline::default()
        .block(Block::default()
            .title(Span::styled(" ↑ Trafic sortant ", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(EMERALD)))
        .data(&out_data)
        .style(Style::default().fg(EMERALD));
    frame.render_widget(spark_out, center_chunks[1]);

    // Network health mini-display
    let health_lines = vec![
        Line::from(vec![
            Span::styled(" ● ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Réseau", Style::default().fg(SLATE_300).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Latence ", SLATE_500),
            Span::styled("12ms", Style::default().fg(GREEN)),
        ]),
        Line::from(vec![
            Span::styled("  Paquets ", SLATE_500),
            Span::styled("99.7%", Style::default().fg(GREEN)),
        ]),
        Line::from(vec![
            Span::styled("  Uptime  ", SLATE_500),
            Span::styled("99.99%", Style::default().fg(CYBER)),
        ]),
    ];
    let health_block = Block::default()
        .title(Span::styled(" 🌐 Santé ", Style::default().fg(EMERALD).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(EMERALD));
    let p = Paragraph::new(health_lines)
        .block(health_block)
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, center_chunks[2]);
}

// ── Module grid (re-used in Dashboard and Services) ───────────────────────────

fn render_module_grid(frame: &mut Frame, area: Rect, modules: &[ModuleCard]) {
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
                    render_module_card(frame, *cell, module);
                }
            }
        }
    }
}

fn render_module_card(frame: &mut Frame, area: Rect, module: &ModuleCard) {
    let status_color = module.status.color();
    let status_label = module.status.label();

    let _icon_style = Style::default();
    let (_border_accent, border_style) = match module.status {
        ModuleStatus::Running => (GREEN, Style::default().fg(GREEN)),
        ModuleStatus::Off => (SLATE_600, Style::default().fg(SLATE_700)),
        ModuleStatus::Error => (ROSE, Style::default().fg(ROSE)),
        ModuleStatus::ComingSoon => (AMBER, Style::default().fg(AMBER).add_modifier(Modifier::DIM)),
    };

    let lines = vec![
        Line::from(vec![
            Span::raw(format!(" {}  ", module.icon)),
            Span::styled(module.name, Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" [{}]", status_label), Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled(format!("     {}", module.desc), SLATE_500),
        ]),
        Line::from(vec![
            Span::styled("     v", SLATE_600),
            Span::styled(module.version, CYBER_DIM),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let p = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, area);
}

// ── View: Favorites ───────────────────────────────────────────────────────────

fn render_favorites_view(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(4)])
        .split(area);

    let header_lines = vec![
        Line::from(vec![
            Span::styled(" ⭐ Vos favoris", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("   Appuyez sur ", SLATE_500),
            Span::styled("M", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled(" Msg · ", SLATE_500),
            Span::styled("H", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled(" Hide · ", SLATE_500),
            Span::styled("D", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled(" Drive · ", SLATE_500),
            Span::styled("N", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled(" Mesh", SLATE_500),
        ]),
    ];
    let p = Paragraph::new(header_lines)
        .block(block_card("Favoris", AMBER))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[0]);

    // Show only running modules as favorites
    let favs: Vec<&ModuleCard> = app.modules.iter().filter(|m| m.status == ModuleStatus::Running).collect();
    if favs.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("   Aucun module actif pour le moment.", SLATE_500)]),
            Line::from(vec![Span::styled("   Activez Msg (M) ou Hide (H) depuis l'Accueil.", SLATE_600)]),
        ];
        let p = Paragraph::new(lines)
            .style(Style::default().bg(SLATE_900));
        frame.render_widget(p, chunks[1]);
    } else {
        render_module_grid(frame, chunks[1], &favs.iter().map(|m| (*m).clone()).collect::<Vec<_>>());
    }
}

// ── View: Services ────────────────────────────────────────────────────────────

fn render_services_view(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    // ── Header (legend) ──
    let header_lines = vec![
        Line::from(vec![
            Span::styled(" 🧩 Services Polygone ", Style::default().fg(CYBER).add_modifier(Modifier::BOLD)),
            Span::styled("— modules et extensions réseau", SLATE_500),
        ]),
        Line::from(vec![
            Span::styled("   M/H/D/N", Style::default().fg(CYBER)),
            Span::styled(" toggles · ", SLATE_500),
            Span::styled("[P]", AMBER), Span::styled("ause · ", SLATE_500),
            Span::styled("[R]", AMBER), Span::styled("afraîchir", SLATE_500),
        ]),
    ];
    let p = Paragraph::new(header_lines)
        .block(block_card("Services", VIOLET))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[0]);

    // ── Spec §4 (Services) : "persistance visuelle POLY" — a
    //    always-visible POLY balance / rate bar above the list of
    //    services so the user can see the cost of running them.
    let snap = app.economy.snapshot();
    let poly_lines = vec![
        Line::from(vec![
            Span::styled(" POLY ", Style::default().fg(SLATE_900).bg(AMBER).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {:.2}", snap.balance), Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            Span::styled("  ·  ", SLATE_500),
            Span::styled(format!("{:.2} POLY/min", snap.rate_per_min), SLATE_300),
            Span::styled("  ·  ", SLATE_500),
            Span::styled(format!("{} service(s) actif(s)", snap.active), SLATE_500),
        ]),
    ];
    let p = Paragraph::new(poly_lines)
        .block(block_card("Économie (statique)", AMBER))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[1]);

    // Full module grid with descriptions
    let _all_modules = ModuleCard::all();
    let grid_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ])
        .split(chunks[2]);

    let descs = [
        ("💬  Msg", "Messagerie E2E éphémère — auto-destruction, zéro persistance", "M", GREEN),
        ("👻  Hide", "Proxy SOCKS5 — navigation anonyme en un clic", "H", VIOLET),
        ("📁  Drive", "Stockage distribué chiffré — fichiers fragmentés", "D", AMBER),
        ("🔗  Mesh", "Réseau local P2P — mutualisation de machines", "N", EMERALD),
        ("🧠  Brain", "IA locale via Notch SLM — inférence privée (🔜)", "", AMBER),
    ];

    for (i, (name, desc, key, color)) in descs.iter().enumerate() {
        if let Some(chunk) = grid_chunks.get(i) {
            let s = format!(" {}  {}", name, desc);
            let line = if !key.is_empty() {
                Line::from(vec![
                    Span::styled(format!(" [{key}]"), Style::default().fg(*color).add_modifier(Modifier::BOLD)),
                    Span::styled(s, SLATE_300),
                ])
            } else {
                Line::from(vec![Span::styled(s, SLATE_400)])
            };
            let p = Paragraph::new(vec![Line::from(""), line])
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SLATE_700)))
                .style(Style::default().bg(SLATE_900));
            frame.render_widget(p, *chunk);
        }
    }
}

// ── View: Settings ────────────────────────────────────────────────────────────

fn render_settings_view(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);

    // Left: system controls
    let control_lines = vec![
        Line::from(vec![Span::styled("Gestion du système", Style::default().fg(CYBER).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled("  [M] Msg     toggle", SLATE_300)]),
        Line::from(vec![Span::styled("  [H] Hide    toggle", SLATE_300)]),
        Line::from(vec![Span::styled("  [D] Drive   toggle", SLATE_300)]),
        Line::from(vec![Span::styled("  [N] Mesh    toggle", SLATE_300)]),
        Line::from(""),
        Line::from(vec![Span::styled("Raccourcis", Style::default().fg(CYBER).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled("  ← → onglets", SLATE_400)]),
        Line::from(vec![Span::styled("  1-4  onglets directs", SLATE_400)]),
        Line::from(vec![Span::styled("  q    quitter", SLATE_400)]),
    ];
    let p = Paragraph::new(control_lines)
        .block(block_card("Paramètres", CYBER))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[0]);

    // Right: system status
    let running_count = app.modules.iter().filter(|m| m.status == ModuleStatus::Running).count();
    // Spec §4 (Paramètres) — "Statut du drive : 10GB/∞ (Quota
    // configurable)" and a shortcut to the web admin. The Drive
    // module is the 3rd entry in `ModuleCard::all()`.
    let drive_running = app.modules.iter()
        .find(|m| m.name == "Drive")
        .map(|m| m.status == ModuleStatus::Running)
        .unwrap_or(false);
    let drive_quota = if drive_running { "10.0 GB" } else { "OFF" };
    let drive_quota_color = if drive_running { GREEN } else { SLATE_500 };
    let web_admin_url = if drive_running { "http://127.0.0.1:8080/admin" } else { "—" };
    let status_lines = vec![
        Line::from(vec![Span::styled("État du système", Style::default().fg(CYBER).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Nœud:       ", SLATE_500),
            Span::styled(if app.paused { "● En pause" } else { "● Actif" },
                Style::default().fg(if app.paused { AMBER } else { GREEN })),
        ]),
        Line::from(vec![
            Span::styled("  Modules:    ", SLATE_500),
            Span::styled(format!("{}/5 actifs", running_count), SLATE_300),
        ]),
        Line::from(vec![
            Span::styled("  Fragments:  ", SLATE_500),
            Span::styled(format!("{}/{}", app.stats.fragments_ready, app.stats.fragments_needed), SLATE_300),
        ]),
        Line::from(vec![
            Span::styled("  Pairings:   ", SLATE_500),
            Span::styled(format!("{} nœuds", app.stats.peers), SLATE_300),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("Drive (stockage)", Style::default().fg(CYBER).add_modifier(Modifier::BOLD))]),
        Line::from(vec![
            Span::styled("  Statut:     ", SLATE_500),
            Span::styled(drive_quota, Style::default().fg(drive_quota_color).add_modifier(Modifier::BOLD)),
            Span::styled("  /  ∞ ", SLATE_500),
        ]),
        Line::from(vec![
            Span::styled("  Web admin:  ", SLATE_500),
            Span::styled(web_admin_url, Style::default().fg(if drive_running { CYBER } else { SLATE_500 })),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⬡ ", CYBER),
            Span::styled(format!("{:.2}", app.economy.snapshot().balance), Style::default().fg(SLATE_50).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  •  {:.2}/min", app.economy.snapshot().rate_per_min), SLATE_500),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("Polygone TUI v1.0.0", SLATE_600)]),
        Line::from(vec![Span::styled("MIT · github.com/lvs0/Polygone-Network", SLATE_700)]),
    ];
    let p = Paragraph::new(status_lines)
        .block(block_card("Status", GREEN))
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, chunks[1]);
}

// ── Activity log ─────────────────────────────────────────────────────────────

fn render_activity_log(frame: &mut Frame, area: Rect, app: &App) {
    let height = area.height.saturating_sub(2) as usize;
    let msgs = &app.messages;
    let start = msgs.len().saturating_sub(height);

    let lines: Vec<Line> = msgs[start..].iter().map(|(kind, msg, ts)| {
        let ts_s = if *ts >= 3600 {
            format!("{}h{}m", ts / 3600, (ts % 3600) / 60)
        } else {
            format!("{}m", ts / 60)
        };
        Line::from(vec![
            Span::styled(format!(" {} ", kind.symbol()), Style::default().fg(kind.color())),
            Span::styled(msg.clone(), SLATE_300),
            Span::styled(format!("  {} ago", ts_s), SLATE_600),
        ])
    }).collect();

    let block = Block::default()
        .title(Span::styled(" 📋 Activité ", Style::default().fg(SLATE_500)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SLATE_700));

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, area);
}

// ── Status bar ───────────────────────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let s = &app.stats;
    let bar = Line::from(vec![
        Span::styled(" ↑↓ naviguer ", Style::default().fg(SLATE_500)),
        Span::styled("·", SLATE_700),
        Span::styled(" ↵ sél.", Style::default().fg(SLATE_500)),
        Span::styled("·", SLATE_700),
        Span::styled(" ← → onglets", Style::default().fg(SLATE_500)),
        Span::styled("·", SLATE_700),
        Span::styled(" q quitter", Style::default().fg(SLATE_500)),
        Span::styled("  ", SLATE_700),
        Span::styled(format!("⬡ {} · {:.1}/m", s.balance, s.consumption), Style::default().fg(CYBER_DIM)),
        Span::styled("  ", SLATE_700),
        Span::styled("v1.0.0", SLATE_600),
    ]);

    let p = Paragraph::new(bar)
        .style(Style::default().bg(SLATE_900));
    frame.render_widget(p, area);
}