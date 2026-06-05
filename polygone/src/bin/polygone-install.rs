//! POLYGONE Installer — Full-screen TUI
//!
//! Flow:
//!   Welcome → (if installed: Update/Uninstall/Reinstall/Launch) → Install → Configure
//!   → Dashboard
//!
//! Navigation: ↑↓ navigate · ENTER select · ESC back · q quit

use std::path::PathBuf;
use std::process::Command;

use crossterm::event::{self, Event, KeyEventKind, KeyCode};
use ratatui::{
    layout::{Alignment, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use serde_json::{json, Value};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ─── Services ────────────────────────────────────────────────────────────────
const WELCOME_ACTION_COUNT: usize = 6;
const WELCOME_ACTION_ICONS: [&str; WELCOME_ACTION_COUNT] = [
    "\u{21A7}", // ↧ install
    "\u{25B6}", // ▶ dashboard
    "\u{2699}", // ⚙ services
    "\u{26A1}", // ⚡ node
    "\u{2713}", // ✓ self-test
    "\u{2717}", // ✗ quit
];
const WELCOME_ACTION_NAMES_EN: [&str; WELCOME_ACTION_COUNT] = [
    "Install Polygone",
    "Open Dashboard",
    "Manage Services",
    "Node Control",
    "Run Self-Test",
    "Quit",
];
const WELCOME_ACTION_NAMES_FR: [&str; WELCOME_ACTION_COUNT] = [
    "Installer Polygone",
    "Ouvrir le Dashboard",
    "Gerer les Services",
    "Controle des Noeuds",
    "Lancer le Self-Test",
    "Quitter",
];
const WELCOME_ACTION_DESCS_EN: [&str; WELCOME_ACTION_COUNT] = [
    "Download or build + configure Polygone",
    "Launch the main TUI dashboard",
    "Enable/disable ecosystem modules",
    "Pause, resume or disable your node",
    "Test the cryptographic stack",
    "Exit the installer",
];
const WELCOME_ACTION_DESCS_FR: [&str; WELCOME_ACTION_COUNT] = [
    "Telecharger ou compiler + configurer Polygone",
    "Lancer le dashboard principal",
    "Activer/desactiver les modules de l'ecosysteme",
    "Mettre en pause, reprendre ou desactiver votre noeud",
    "Tester la pile cryptographique",
    "Quitter l'installeur",
];

const SERVICE_COUNT: usize = 8;
const SERVICE_NAMES: [&str; SERVICE_COUNT] = [
    "polygone",
    "polygone-drive",
    "polygone-hide",
    "polygone-petals",
    "polygone-brain",
    "polygone-shell",
    "polygone-server",
    "polygone-cli",
];
const SERVICE_DESCS: [&str; SERVICE_COUNT] = [
    "Core network",
    "Encrypted distributed storage",
    "Privacy tunnel (SOCKS5)",
    "Distributed LLM inference",
    "AI diagnostics & monitoring",
    "Secure shell interface",
    "P2P relay server",
    "Command-line tools",
];

// ─── Colors ─────────────────────────────────────────────────────────────────
const C_SURFACE: Color = Color::Rgb(17, 17, 24);
const C_BORDER:  Color = Color::Rgb(30, 30, 46);
const C_COBALT:  Color = Color::Rgb(26, 107, 255);
const C_GREEN:   Color = Color::Rgb(40, 200, 64);
const C_RED:     Color = Color::Rgb(255, 59, 48);
const C_YELLOW:  Color = Color::Rgb(255, 204, 0);
const C_TEXT:    Color = Color::Rgb(200, 200, 232);
const C_DIM:     Color = Color::Rgb(74, 74, 106);

// ─── Install state ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq)]
enum InstallState {
    Welcome,
    AlreadyInstalled,
    Installing,
    Configure,
    UsernameInput,
    NodeModeSelect,
    ConfigureServices,
    Dashboard,
    DashboardOutput,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum NodeChoice { None, Passive, Active }

#[derive(Debug, Clone, Copy, PartialEq)]
enum Lang { EN, FR }

#[derive(Debug, Clone, Copy, PartialEq)]
enum MenuAction { None, Update, Reinstall, Uninstall, Launch }

// ─── Config ─────────────────────────────────────────────────────────────────
struct Config {
    lang: Lang,
    username: String,
    node: NodeChoice,
    install_dir: PathBuf,
    config_dir: PathBuf,
}

impl Config {
    fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polygone");
        let install_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/bin");
        Self {
            lang: Lang::EN,
            username: String::new(),
            node: NodeChoice::None,
            install_dir,
            config_dir,
        }
    }

    fn load(&mut self) {
        let cfg_file = self.config_dir.join("config.json");
        if let Ok(data) = std::fs::read_to_string(&cfg_file) {
            if let Ok(v) = data.parse::<Value>() {
                if let Some(u) = v.get("username").and_then(|x| x.as_str()) {
                    self.username = u.to_string();
                }
                if let Some(n) = v.get("node_mode").and_then(|x| x.as_str()) {
                    self.node = match n {
                        "passive" => NodeChoice::Passive,
                        "active" => NodeChoice::Active,
                        _ => NodeChoice::None,
                    };
                }
                if let Some(l) = v.get("language").and_then(|x| x.as_str()) {
                    self.lang = match l { "fr" => Lang::FR, _ => Lang::EN };
                }
            }
        }
    }

    #[allow(dead_code)]
    fn save(&self) {
        std::fs::create_dir_all(&self.config_dir).ok();
        let node_str = match self.node {
            NodeChoice::None => "none",
            NodeChoice::Passive => "passive",
            NodeChoice::Active => "active",
        };
        let lang_str = match self.lang { Lang::FR => "fr", Lang::EN => "en" };
        let obj = json!({
            "version": VERSION,
            "username": &self.username,
            "node_mode": node_str,
            "language": lang_str,
        });
        std::fs::write(self.config_dir.join("config.json"), serde_json::to_string_pretty(&obj).unwrap_or_default()).ok();
    }

    fn tr(&self, en: &str, fr: &str) -> String {
        match self.lang { Lang::FR => fr.to_string(), Lang::EN => en.to_string() }
    }

    fn label(&self) -> String {
        match self.lang { Lang::FR => "Français".to_string(), Lang::EN => "English".to_string() }
    }
}

fn step_msg(lang: Lang, en: &str, fr: &str) -> String {
    match lang { Lang::FR => fr.to_string(), Lang::EN => en.to_string() }
}

// ─── App ──────────────────────────────────────────────────────────────────────
struct App {
    state: InstallState,
    config: Config,
    menu_idx: usize,
    menu_actions: Vec<MenuAction>,
    installing: bool,
    install_pct: f32,
    install_status: String,
    install_log: Vec<String>,
    install_error: Option<String>,
    dash_tab: usize,
    dash_item: usize,
    services_enabled: Vec<bool>,
    services_step: usize,
    config_step: usize,       // 0=lang, 1=username, 2=node, 3=done
    username_input: String,  // buffer for username typing
    node_idx: usize,         // 0=none, 1=passive, 2=active
    welcome_idx: usize,      // index in welcome quick-launch menu
    dash_output: String,
    dash_output_title: String,
    dash_running: bool,
}

impl App {
    fn new() -> Self {
        let mut config = Config::new();
        config.load();
        Self {
            state: InstallState::Welcome,
            config,
            menu_idx: 0,
            menu_actions: Vec::new(),
            installing: false,
            install_pct: 0.0,
            install_status: String::new(),
            install_log: Vec::new(),
            install_error: None,
            dash_tab: 0,
            dash_item: 0,
            services_enabled: vec![true, false, false, false, false, false, false, false],
            services_step: 0,
            config_step: 0,
            username_input: String::new(),
            node_idx: 0,
            welcome_idx: 0,
            dash_output: String::new(),
            dash_output_title: String::new(),
            dash_running: false,
        }
    }

    fn binary_path(&self) -> PathBuf {
        self.config.install_dir.join("polygone")
    }

    fn is_installed(&self) -> bool {
        self.binary_path().exists()
    }

    fn push_log(&mut self, msg: String) {
        self.install_log.push(msg);
        if self.install_log.len() > 8 { self.install_log.remove(0); }
    }

    fn run_install(&mut self) {
        let install_dir = self.config.install_dir.clone();
        let install_dest = self.binary_path();
        let lang = self.config.lang;
        let node_mode = self.config.node;
        let username = self.config.username.clone();
        let config_dir = self.config.config_dir.clone();

        let url = format!(
            "https://github.com/lvs0/Polygone/releases/download/v{}/polygone",
            VERSION
        );

        // Step 1: Downloading
        self.install_status = format!("[1/4] {}", step_msg(lang, "Downloading Polygone...", "Téléchargement de Polygone..."));
        self.install_pct = 0.15;
        self.push_log(self.install_status.clone());

        std::fs::create_dir_all(&install_dir).ok();

        // Try download first
        let dl = Command::new("curl")
            .args(["-fsSL", "-o", "/tmp/polygone"])
            .arg(&url)
            .output();

        match dl {
            Ok(out) if out.status.success() => {
                self.install_status = format!("[2/4] {}", step_msg(lang, "Installing...", "Installation..."));
                self.install_pct = 0.60;
                self.push_log(self.install_status.clone());
                if std::fs::copy("/tmp/polygone", &install_dest).is_ok() {
                    #[cfg(unix)] {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&install_dest, std::fs::Permissions::from_mode(0o755)).ok();
                    }
                    self.install_status = format!("[3/4] {}", step_msg(lang, "Configuring...", "Configuration..."));
                    self.install_pct = 0.85;
                    self.push_log(self.install_status.clone());

                    // Save config
                    std::fs::create_dir_all(&config_dir).ok();
                    let node_str = match node_mode {
                        NodeChoice::None => "none",
                        NodeChoice::Passive => "passive",
                        NodeChoice::Active => "active",
                    };
                    let lang_str = match lang { Lang::FR => "fr", Lang::EN => "en" };
                    let obj = json!({
                        "version": VERSION,
                        "username": username,
                        "node_mode": node_str,
                        "language": lang_str,
                    });
                    std::fs::write(config_dir.join("config.json"), serde_json::to_string_pretty(&obj).unwrap_or_default()).ok();

                    self.install_status = format!("[4/4] {}", step_msg(lang, "Done!", "Terminé!"));
                    self.install_pct = 1.0;
                    self.push_log("Done!".to_string());
                    self.state = InstallState::ConfigureServices;
                } else {
                    self.install_error = Some(step_msg(lang, "Copy failed", "Échec de la copie"));
                }
            }
            _ => {
                self.install_status = format!("[1/4] {}", step_msg(lang, "Building from source...", "Compilation depuis les sources..."));
                self.install_pct = 0.20;
                self.push_log(self.install_status.clone());

                let build_dir = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("polygone-src");

                if !build_dir.exists() {
                    self.push_log("Cloning Polygone repository...".to_string());
                    let _ = Command::new("git")
                        .args(["clone", "https://github.com/lvs0/Polygone.git"])
                        .arg(&build_dir)
                        .output();
                }

                self.install_status = format!("[2/4] {}", step_msg(lang, "Compiling (may take a while)...", "Compilation (peut prendre du temps)..."));
                self.install_pct = 0.50;
                self.push_log("cargo build --release".to_string());
                let build = Command::new("cargo")
                    .current_dir(&build_dir)
                    .args(["build", "--release", "--bin", "polygone"])
                    .output();

                match build {
                    Ok(out) if out.status.success() => {
                        self.install_status = format!("[3/4] {}", step_msg(lang, "Installing compiled binary...", "Installation du binaire compilé..."));
                        self.install_pct = 0.85;
                        self.push_log(self.install_status.clone());
                        let src = build_dir.join("target/release/polygone");
                        if std::fs::copy(&src, &install_dest).is_ok() {
                            #[cfg(unix)] {
                                use std::os::unix::fs::PermissionsExt;
                                std::fs::set_permissions(&install_dest, std::fs::Permissions::from_mode(0o755)).ok();
                            }
                            // Save config
                            std::fs::create_dir_all(&config_dir).ok();
                            let node_str = match node_mode {
                                NodeChoice::None => "none",
                                NodeChoice::Passive => "passive",
                                NodeChoice::Active => "active",
                            };
                            let lang_str = match lang { Lang::FR => "fr", Lang::EN => "en" };
                            let obj = json!({
                                "version": VERSION,
                                "username": username,
                                "node_mode": node_str,
                                "language": lang_str,
                            });
                            std::fs::write(config_dir.join("config.json"), serde_json::to_string_pretty(&obj).unwrap_or_default()).ok();

                            self.install_status = format!("[4/4] {}", step_msg(lang, "Done!", "Terminé!"));
                            self.install_pct = 1.0;
                            self.push_log("Done!".to_string());
                            self.state = InstallState::Configure;
                        } else {
                            self.install_error = Some(step_msg(lang, "Copy failed", "Échec de la copie"));
                        }
                    }
                    _ => {
                        self.install_error = Some(step_msg(lang,
                            "Build failed. Install Rust: curl https://sh.rustup.rs | sh",
                            "Compilation échouée. Installe Rust: curl https://sh.rustup.rs | sh"));
                    }
                }
            }
        }
    }




    fn save_services(&self) {
        let path = self.config.config_dir.join("services.json");
        std::fs::create_dir_all(&self.config.config_dir).ok();
        let obj = serde_json::json!({
            "services": self.services_enabled,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&obj).unwrap_or_default()).ok();
    }

    fn do_uninstall(&mut self) {
        if let Ok(metadata) = std::fs::metadata(&self.binary_path()) {
            if metadata.permissions().readonly() {
                #[cfg(unix)] { let _ = Command::new("chmod").arg("u+w").arg(&self.binary_path()).output(); }
            }
        }
        std::fs::remove_file(&self.binary_path()).ok();
        std::fs::remove_file(self.config.config_dir.join("config.json")).ok();
        self.push_log("Uninstalled Polygone".to_string());
        self.state = InstallState::Done;
    }

    // ─── Corner logo (persistent on all screens) ─────────────────────────────
    fn draw_corner_logo(&self, f: &mut Frame, _size: Rect) {
        let logo = format!("{} POLYGONE v{}", "⬡", env!("CARGO_PKG_VERSION"));
        let logo_len = logo.len() as u16;
        let rect = Rect::new(2, 0, logo_len + 2, 2);
        let p = Paragraph::new(vec![
            Line::from(vec![Span::raw("")]),
            Line::from(vec![Span::styled(&logo, Style::new().fg(C_COBALT).bold())]),
        ]).style(Style::new().bg(C_SURFACE));
        f.render_widget(p, rect);
    }

    // ─── Main draw ────────────────────────────────────────────────────────────
    fn draw(&self, f: &mut Frame) {
        let size = f.area();
        f.render_widget(Clear, size);
        match self.state {
            InstallState::Welcome => self.draw_welcome(f, size),
            InstallState::AlreadyInstalled => self.draw_already_installed(f, size),
            InstallState::Installing => self.draw_installing(f, size),
            InstallState::Configure => self.draw_configure(f, size),
            InstallState::UsernameInput => self.draw_username_input(f, size),
            InstallState::NodeModeSelect => self.draw_node_mode_select(f, size),
            InstallState::ConfigureServices => self.draw_configure_services(f, size),
            InstallState::Dashboard => self.draw_dashboard(f, size),
            InstallState::DashboardOutput => self.draw_dashboard_output(f, size),
            InstallState::Done => self.draw_done(f, size),
        }
        // Always show corner logo
        self.draw_corner_logo(f, size);
    }

    fn centered(&self, w: u16, h: u16, size: Rect) -> Rect {
        let x = (size.width.saturating_sub(w)) / 2;
        let y = (size.height.saturating_sub(h)) / 2;
        Rect::new(x, y, w.min(size.width), h.min(size.height))
    }

    fn block(&self, title: &str) -> Block<'_> {
        Block::new()
            .title(format!("  {}  ", title))
            .title_style(Style::new().fg(C_COBALT).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(C_BORDER))
            .style(Style::new().bg(C_SURFACE))
    }

    // ── Logo ────────────────────────────────────────────────────────────────
    fn draw_logo_at(&self, f: &mut Frame, area: Rect) {
        // Full hex logo — centered, impressive
        let lines = vec![
            Line::from(vec![Span::styled("                     ⬡  P O L Y G O N E", Style::new().fg(C_COBALT).bold())]),
            Line::from(vec![Span::raw("")]),
            Line::from(vec![Span::styled("           ⬡                         ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("         ⬡   ⬡                   ⬡   ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("       ⬡       ⬡               ⬡       ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("     ⬡           ⬡           ⬡           ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("   ⬡               ⬡       ⬡               ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("     ⬡           ⬡           ⬡           ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("       ⬡       ⬡               ⬡       ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("         ⬡   ⬡                   ⬡   ⬡", Style::new().fg(C_COBALT))]),
            Line::from(vec![Span::styled("           ⬡                         ⬡", Style::new().fg(C_COBALT))]),
        ];
        let p = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::new().fg(C_COBALT));
        f.render_widget(p, area);
    }

    // ── Welcome ─────────────────────────────────────────────────────────────
    fn draw_welcome(&self, f: &mut Frame, size: Rect) {
        let box_w = 68u16.min(size.width.saturating_sub(4));
        let box_h = 22u16.min(size.height.saturating_sub(3));
        let rect = self.centered(box_w, box_h, size);
        f.render_widget(self.block(""), rect);

        // Logo — full hex art centered
        let logo_rect = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, 9);
        self.draw_logo_at(f, logo_rect);

        // Action menu below logo
        let tr = |en, fr| self.config.tr(en, fr);
        let menu_rect = Rect::new(rect.x + 2, rect.y + 9, rect.width - 4, rect.height - 11);

        let lines: Vec<Line> = vec![
            Line::from(vec![Span::raw("  "), Span::raw(tr("Privacy that leaves no trace.", "La confidentialite sans trace."))]),
            Line::from(Span::raw("")),
        ]
        .into_iter()
        .chain((0..WELCOME_ACTION_COUNT).map(|i| {
            let icon = WELCOME_ACTION_ICONS[i];
            let name_en = WELCOME_ACTION_NAMES_EN[i];
            let desc_en = WELCOME_ACTION_DESCS_EN[i];
            let name_fr = WELCOME_ACTION_NAMES_FR[i];
            let desc_fr = WELCOME_ACTION_DESCS_FR[i];
            let name = tr(name_en, name_fr);
            let desc = tr(desc_en, desc_fr);
            let sel = self.welcome_idx == i;
            let style = if sel { Style::new().fg(C_GREEN).bold() } else { Style::new().fg(C_TEXT) };
            let icon_col = if sel { C_COBALT } else { C_DIM };
            let prefix = if sel { "▶" } else { " " };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(prefix, style),
                Span::raw(" "),
                Span::styled(icon, Style::new().fg(icon_col)),
                Span::raw(" "),
                Span::styled(name, style),
                Span::raw("  —  "),
                Span::styled(desc, Style::new().fg(C_DIM)),
            ])
        }))
        .chain(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::raw("  "), Span::styled("↑↓", Style::new().fg(C_COBALT)), Span::raw(format!("  {}", tr("navigate", "naviguer"))), Span::raw("    "), Span::styled("ENTER", Style::new().fg(C_GREEN)), Span::raw(format!("  {}", tr("select", "selectionner")))]),
        ])
        .collect();

        let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT));
        f.render_widget(p, menu_rect);
    }
    // ── Already installed ───────────────────────────────────────────────────
    fn draw_already_installed(&self, f: &mut Frame, size: Rect) {
        let box_w = 52u16.min(size.width.saturating_sub(6));
        let box_h = (4 + self.menu_actions.len() as u16 * 3).min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Polygone is already installed", "Polygone est déjà installé");
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let items: Vec<ListItem> = self.menu_actions.iter().enumerate().map(|(i, action)| {
            let sel = self.menu_idx == i;
            let icon = if sel { "▶" } else { " " };
            let style = if sel { Style::new().fg(C_GREEN).bold() } else { Style::new().fg(C_TEXT) };
            let label: String = match action {
                MenuAction::Update => self.config.tr("Update to latest version", "Mettre à jour"),
                MenuAction::Reinstall => self.config.tr("Reinstall Polygone", "Réinstaller Polygone"),
                MenuAction::Uninstall => self.config.tr("Uninstall Polygone", "Désinstaller Polygone"),
                MenuAction::Launch => self.config.tr("Launch Polygone", "Lancer Polygone"),
                MenuAction::None => String::new(),
            };
            ListItem::new(vec![
                Line::from(vec![Span::styled(icon, style), Span::raw("  "), Span::styled(label, style)]),
                Line::from(Span::raw("")),
            ])
        }).collect();

        let list = List::new(items).style(Style::new().fg(C_TEXT));
        f.render_widget(list, inner);

        let nav = Paragraph::new(vec![Line::from(vec![
            Span::raw("  "),
            Span::styled("↑↓", Style::new().fg(C_COBALT)),
            Span::raw(" navigate  "),
            Span::styled("ENTER", Style::new().fg(C_GREEN)),
            Span::raw(" select  "),
            Span::styled("ESC", Style::new().fg(C_DIM)),
            Span::raw(" back"),
        ])]).alignment(Alignment::Center).style(Style::new().fg(C_DIM));
        f.render_widget(nav, inner);
    }

    // ── Installing ──────────────────────────────────────────────────────────
    fn draw_installing(&self, f: &mut Frame, size: Rect) {
        let box_w = 60u16.min(size.width.saturating_sub(6));
        let box_h = 22u16.min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Installing Polygone", "Installation de Polygone"); f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let pct = (self.install_pct * 100.0) as u16;
        let status_line = Line::from(vec![
            Span::raw("  "),
            Span::raw(&self.install_status),
            Span::raw(format!("  {}%", pct)),
        ]);

        let log_lines: Vec<Line> = self.install_log.iter()
            .map(|l| Line::from(Span::styled(format!("  {}", l), Style::new().fg(C_DIM))))
            .collect();

        let error_line = self.install_error.as_ref().map(|e| {
            Line::from(vec![Span::styled("✗ ERROR: ", Style::new().fg(C_RED)), Span::raw(e)])
        });

        let all: Vec<Line> = [
            vec![Line::from("")],
            log_lines,
            vec![Line::from("")],
            vec![status_line],
            if let Some(ref e) = error_line { vec![e.clone()] } else { vec![] },
        ].concat();

        let p = Paragraph::new(all).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Configure ───────────────────────────────────────────────────────────
    fn draw_configure(&self, f: &mut Frame, size: Rect) {
        let box_w = 56u16.min(size.width.saturating_sub(6));
        let box_h = 22u16.min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Configure Polygone", "Configurer Polygone");
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let tr = |en, fr| self.config.tr(en, fr);
        let lang_label = self.config.label();
        let user_label = if self.config.username.is_empty() {
            tr("anonymous", "anonyme").to_string()
        } else {
            self.config.username.clone()
        };
        let node_label: String = match self.config.node {
            NodeChoice::None => tr("Disabled", "Desactive").to_string(),
            NodeChoice::Passive => tr("Passive", "Passif").to_string(),
            NodeChoice::Active => tr("Active", "Actif").to_string(),
        };

        let rows: Vec<(&str, String, bool)> = vec![
            ("Language", lang_label, self.config_step == 0),
            ("Username", user_label, self.config_step == 1),
            ("Node", node_label, self.config_step == 2),
        ];

        let lines: Vec<Line> = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  \u{2713} Polygone installed!", Style::new().fg(C_GREEN).bold())]),
            Line::from(Span::raw("")),
        ]
        .into_iter()
        .chain(rows.iter().map(|(key, val, sel)| {
            let style = if *sel { Style::new().fg(C_GREEN).bold() } else { Style::new().fg(C_TEXT) };
            let icon = if *sel { "▶" } else { " " };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(icon, style),
                Span::raw("  "),
                Span::raw(*key),
                Span::raw(": "),
                Span::styled(val, Style::new().fg(C_COBALT)),
            ])
        }))
        .chain(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::raw("  "), Span::styled("↑↓", Style::new().fg(C_COBALT)), Span::raw(format!("  {}", tr("select", "selectionner")))]),
            Line::from(vec![Span::raw("  "), Span::styled("ENTER", Style::new().fg(C_GREEN)), Span::raw(format!("  {}", tr("edit / continue", "editer / continuer")))]),
            Line::from(vec![Span::raw("  "), Span::styled("→", Style::new().fg(C_YELLOW)), Span::raw(format!("  {}", tr("services setup next", "config services apres")))]),
            Line::from(Span::raw("")),
        ])
        .collect();

        let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Username Input ────────────────────────────────────────────────────────
    fn draw_username_input(&self, f: &mut Frame, size: Rect) {
        let box_w = 52u16.min(size.width.saturating_sub(6));
        let box_h = 14u16.min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Enter your name", "Entrez votre nom");
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let prompt = self.config.tr("Your name (optional):", "Votre nom (optionnel) :");
        let display_name = if self.username_input.is_empty() {
            "_".to_string()
        } else {
            self.username_input.clone()
        };

        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::raw("  "), Span::raw(prompt)]),
            Line::from(""),
            Line::from(vec![Span::raw("  "), Span::styled(&display_name, Style::new().fg(C_GREEN))]),
            Line::from(""),
            Line::from(vec![Span::raw("  "), Span::styled("ENTER", Style::new().fg(C_GREEN).bold()), Span::raw(format!("  {}", self.config.tr("Confirm", "Confirmer")))]),
            Line::from(vec![Span::raw("  "), Span::styled("ESC", Style::new().fg(C_DIM)), Span::raw(format!("  {}", self.config.tr("Skip", "Passer")))]),
            Line::from(""),
        ];
        let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Node Mode Select ──────────────────────────────────────────────────────
    fn draw_node_mode_select(&self, f: &mut Frame, size: Rect) {
        let box_w = 64u16.min(size.width.saturating_sub(6));
        let box_h = 22u16.min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Node system", "Systeme de noeuds");
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let tr = |en, fr| self.config.tr(en, fr);
        let modes = [
            ("Off", tr("Disabled — no sharing", "Desactive — pas de partage")),
            ("Passive", tr("Passive — share bandwidth only, invisible, pausable", "Passif — partage bande passante uniquement, invisible, mettable en pause")),
            ("Active", tr("Active — share bandwidth + compute power", "Actif — partage bande passante + puissance de calcul")),
        ];

        let header: Vec<Line> = vec![
            Line::from(""),
            Line::from(vec![Span::raw("  "), Span::styled(tr("How does it work?", "Comment ca marche?"), Style::new().fg(C_COBALT).bold())]),
            Line::from(vec![Span::raw("  "), Span::raw(tr("The node runs in the background.", "Le noeud fonctionne en arriere-plan."))]),
            Line::from(vec![Span::raw("  "), Span::raw(tr("It is intelligent, invisible, and can be disabled anytime.", "Il est intelligent, invisible, et peut etre desactive a tout moment."))]),
            Line::from(vec![Span::raw("  "), Span::raw(tr("You can pause it for 1h, 4h, or disable completely.", "Vous pouvez le mettre en pause 1h, 4h, ou desactiver completement."))]),
            Line::from(vec![Span::raw("")]),
        ];

        let mode_lines: Vec<Line> = (0..3).map(|i| {
            let (name, desc) = (modes[i].0.to_string(), modes[i].1.to_string());
            let sel = self.node_idx == i;
            let icon = if sel { "▶" } else { " " };
            let style = if sel { Style::new().fg(C_GREEN).bold() } else { Style::new().fg(C_TEXT) };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(icon, style),
                Span::raw("  "),
                Span::styled(name.clone(), style),
                Span::raw("  —  "),
                Span::raw(desc.clone()),
            ])
        }).collect();

        let footer: Vec<Line> = vec![
            Line::from(vec![Span::raw("  "), Span::styled("←→ ", Style::new().fg(C_COBALT)), Span::raw(tr("to choose", "pour choisir"))]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("ENTER", Style::new().fg(C_GREEN).bold()),
                Span::raw(format!("  {}", tr("Confirm", "Confirmer"))),
                Span::raw("    "),
                Span::styled("ESC", Style::new().fg(C_DIM)),
                Span::raw(format!("  {}", tr("Back", "Retour"))),
            ]),
        ];

        let all: Vec<Line> = header.into_iter().chain(mode_lines.into_iter()).chain(footer.into_iter()).collect();
        let p = Paragraph::new(all).style(Style::new().fg(C_TEXT)).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    // ── Configure Services ────────────────────────────────────────────────────
    fn draw_configure_services(&self, f: &mut Frame, size: Rect) {
        let box_w = 64u16.min(size.width.saturating_sub(4));
        let box_h = (4 + SERVICE_COUNT as u16 * 2 + 6).min(size.height.saturating_sub(4));
        let rect = self.centered(box_w, box_h, size);
        let title = self.config.tr("Choose your services", "Choisissez vos services");
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        let tr = |en, fr| self.config.tr(en, fr);

        // Show all services with toggle state
        let lines: Vec<Line> = (0..SERVICE_COUNT).flat_map(|i| {
            let name = SERVICE_NAMES[i];
            let desc = SERVICE_DESCS[i];
            let on = self.services_enabled[i];
            let icon = if on { "[●]" } else { "[○]" };
            let col = if on { C_GREEN } else { C_DIM };
            let sel = self.services_step == i;
            vec![
                Line::from(vec![
                    if sel { Span::styled("  ▶ ", Style::new().fg(C_COBALT)) } else { Span::raw("    ") },
                    Span::styled(icon, Style::new().fg(col)),
                    Span::raw("  "),
                    Span::raw(name),
                    Span::raw("  —  "),
                    Span::raw(desc),
                ]),
            ]
        }).collect();

        // Navigation hint
        let hint = vec![
            Line::from(Span::raw("")),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("←→", Style::new().fg(C_COBALT)),
                Span::raw(" "),
                Span::raw(tr("toggle on/off", "activer/desactiver")),
                Span::raw("    "),
                Span::styled("↑↓", Style::new().fg(C_COBALT)),
                Span::raw(" "),
                Span::raw(tr("navigate", "naviguer")),
            ]),
            Line::from(Span::raw("")),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("ENTER", Style::new().fg(C_GREEN).bold()),
                Span::raw(format!("  {}", tr("Continue to Dashboard", "Continuer vers le Dashboard"))),
            ]),
        ];

        let all: Vec<Line> = lines.into_iter().chain(hint.into_iter()).collect();
        let p = Paragraph::new(all).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Dashboard ───────────────────────────────────────────────────────────
    fn draw_dashboard(&self, f: &mut Frame, size: Rect) {
        let header_h = 3u16;
        let tab_h = 3u16;
        let footer_h = 2u16;

        // Header
        let header_rect = Rect::new(0, 0, size.width, header_h);
        let hblock = Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(C_BORDER))
            .style(Style::new().bg(C_SURFACE));
        f.render_widget(hblock, header_rect);
        let user_label = if self.config.username.is_empty() { "anonymous".to_string() } else { self.config.username.clone() };
        let header_lines = vec![Line::from(vec![
            Span::styled("⬡ Polygone", Style::new().fg(C_COBALT).bold()),
            Span::raw(format!("  v{}  ·  {}", VERSION, user_label)),
        ])];
        f.render_widget(Paragraph::new(header_lines).style(Style::new().fg(C_TEXT)), header_rect);

        // Tab bar
        let tab_rect = Rect::new(0, header_h, size.width, tab_h);
        let tabs = ["Home", "Services", "Nodes", "Settings"];
        let tab_lines: Vec<Line> = tabs.iter().enumerate().map(|(i, tab)| {
            let sel = self.dash_tab == i;
            let style = if sel { Style::new().fg(C_GREEN).bold() } else { Style::new().fg(C_DIM) };
            let prefix = if sel { "▶ " } else { "  " };
            Line::from(vec![
                Span::raw("   "),
                Span::styled(prefix, style),
                Span::styled(*tab, style),
            ])
        }).collect();
        f.render_widget(Paragraph::new(tab_lines).style(Style::new().fg(C_TEXT)), tab_rect);

        // Content
        let content_y = header_h + tab_h;
        let content_h = size.height.saturating_sub(content_y + footer_h);
        let content_rect = Rect::new(0, content_y, size.width, content_h);

        match self.dash_tab {
            0 => self.draw_tab_home(f, content_rect),
            1 => self.draw_tab_services(f, content_rect),
            2 => self.draw_tab_nodes(f, content_rect),
            3 => self.draw_tab_settings(f, content_rect),
            _ => {}
        }

        // Footer
        let footer_rect = Rect::new(0, size.height - footer_h, size.width, footer_h);
        let fblock = Block::new()
            .borders(Borders::TOP)
            .border_style(Style::new().fg(C_BORDER))
            .style(Style::new().bg(C_SURFACE));
        f.render_widget(fblock, footer_rect);
        let node_status: &'static str = match self.config.node {
            NodeChoice::None => "Node: off",
            NodeChoice::Passive => "Node: passive",
            NodeChoice::Active => "Node: active",
        };
        let footer_lines = vec![Line::from(vec![
            Span::styled(node_status, Style::new().fg(C_COBALT)),
            Span::raw("  ·  Polygone v"),
            Span::raw(VERSION),
        ])];
        f.render_widget(Paragraph::new(footer_lines).style(Style::new().fg(C_DIM)), footer_rect);
    }

    fn draw_tab_home(&self, f: &mut Frame, rect: Rect) {
        let inner = rect.inner(Margin::new(1, 1));
        let tr = |en, fr| self.config.tr(en, fr);
        let node_status = match self.config.node {
            NodeChoice::None => tr("Disabled", "Désactivé"),
            NodeChoice::Passive => tr("Passive (invisible)", "Passif (invisible)"),
            NodeChoice::Active => tr("Active (sharing power)", "Actif (puissance partagée)"),
        };
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  ⬡ POLYGONE", Style::new().fg(C_COBALT).bold()), Span::raw("  —  Privacy that leaves no trace.")]),
            Line::from(""),
            Line::from(vec![Span::raw("  Status: "), Span::styled("Running", Style::new().fg(C_GREEN))]),
            Line::from(vec![Span::raw("  User:   "), Span::raw(if self.config.username.is_empty() { "anonymous".to_string() } else { self.config.username.clone() })]),
            Line::from(vec![Span::raw("  Node:   "), Span::raw(&node_status)]),
            Line::from(Span::raw("")),
            Line::from(Span::raw("  Quick actions:")),
            Line::from(""),
            Line::from(vec![Span::styled(if self.dash_item == 0 { "  ▶ " } else { "    " }, Style::new().fg(C_GREEN)), Span::raw(tr("Run self-test", "Test crypto"))]),
            Line::from(vec![Span::styled(if self.dash_item == 1 { "  ▶ " } else { "    " }, Style::new().fg(C_GREEN)), Span::raw(tr("Generate keys", "Générer clés"))]),
            Line::from(vec![Span::styled(if self.dash_item == 2 { "  ▶ " } else { "    " }, Style::new().fg(C_GREEN)), Span::raw(tr("Send a message", "Envoyer un message"))]),
            Line::from(Span::raw("")),
        ];
        let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT)).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn draw_tab_services(&self, f: &mut Frame, rect: Rect) {
        let inner = rect.inner(Margin::new(1, 1));
        let lines: Vec<Line> = vec![Line::from(Span::raw("  Services "))]
            .into_iter()
            .chain((0..SERVICE_COUNT).flat_map(|i| {
                let name = SERVICE_NAMES[i];
                let desc = SERVICE_DESCS[i];
                let on = self.services_enabled[i];
                let icon = if on { "●" } else { "○" };
                let col = if on { C_GREEN } else { C_DIM };
                vec![
                    Line::from(vec![
                        if self.dash_item == i { Span::styled("  ▶ ", Style::new().fg(C_GREEN)) } else { Span::raw("    ") },
                        Span::styled(icon, Style::new().fg(col)),
                        Span::raw("  "),
                        Span::raw(name),
                        Span::raw("  —  "),
                        Span::raw(desc),
                    ]),
                ]
            }))
            .collect();
        let footer_lines = vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::raw("  "), Span::styled("←→", Style::new().fg(C_COBALT)), Span::raw(" toggle on/off")]),
        ];
        let all: Vec<Line> = lines.into_iter().chain(footer_lines.into_iter()).collect();
        let p = Paragraph::new(all).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    fn draw_tab_nodes(&self, f: &mut Frame, rect: Rect) {
        let inner = rect.inner(Margin::new(1, 1));
        let tr = |en, fr| self.config.tr(en, fr);
        let node_lines: Vec<Line> = match self.config.node {
            NodeChoice::None => vec![
                Line::from(Span::raw("  Node is disabled.")),
                Line::from(Span::raw("")),
                Line::from(vec![Span::raw("  The node system is "), Span::styled("intelligent and invisible.", Style::new().fg(C_COBALT))]),
                Line::from(Span::raw("  Share bandwidth without slowing your computer.")),
                Line::from(Span::raw("  Can be paused or disabled at any time.")),
                Line::from(Span::raw("")),
                Line::from(vec![Span::raw("  "), Span::styled("Enable passive node?", Style::new().fg(C_GREEN))]),
                Line::from(Span::raw("  Passive = share bandwidth only, invisible, always pausable.")),
                Line::from(Span::raw("")),
            ],
            NodeChoice::Passive | NodeChoice::Active => vec![
                Line::from(vec![Span::raw("  Node: "), Span::styled(match self.config.node {
                    NodeChoice::Passive => "Passive",
                    NodeChoice::Active => "Active",
                    _ => "",
                }, Style::new().fg(C_GREEN))]),
                Line::from(Span::raw("")),
                Line::from(vec![Span::raw("  The node is "), Span::styled("intelligent and invisible.", Style::new().fg(C_COBALT))]),
                Line::from(Span::raw("  It shares bandwidth in the background.")),
                Line::from(Span::raw("  You can pause it anytime.")),
                Line::from(Span::raw("")),
                Line::from(vec![Span::styled(if self.dash_item == 0 { "  ▶ " } else { "    " }, Style::new().fg(C_YELLOW)), Span::raw(tr("Pause for 1 hour", "Pause 1h"))]),
                Line::from(vec![Span::styled(if self.dash_item == 1 { "  ▶ " } else { "    " }, Style::new().fg(C_YELLOW)), Span::raw(tr("Pause for 4 hours", "Pause 4h"))]),
                Line::from(vec![Span::styled(if self.dash_item == 2 { "  ▶ " } else { "    " }, Style::new().fg(C_RED)), Span::raw(tr("Disable node", "Désactiver le noeud"))]),
                Line::from(Span::raw("")),
            ],
        };

        let all: Vec<Line> = [vec![Line::from(Span::raw("  Nodes  "))], node_lines].concat();
        let p = Paragraph::new(all).style(Style::new().fg(C_TEXT)).wrap(Wrap { trim: true });
        f.render_widget(p, inner);
    }

    fn draw_tab_settings(&self, f: &mut Frame, rect: Rect) {
        let inner = rect.inner(Margin::new(1, 1));
        let tr = |en, fr| self.config.tr(en, fr);
        let settings: Vec<(String, String)> = vec![
            (tr("Username", "Nom d'utilisateur"), if self.config.username.is_empty() { "anonymous".to_string() } else { self.config.username.clone() }),
            (tr("Language", "Langue"), self.config.label()),
            (tr("Node mode", "Mode noeud"), match self.config.node {
                NodeChoice::None => tr("Disabled", "Désactivé"),
                NodeChoice::Passive => tr("Passive", "Passif"),
                NodeChoice::Active => tr("Active", "Actif"),
            }),
            ("Version".to_string(), VERSION.to_string()),
        ];

        let lines: Vec<Line> = vec![Line::from(Span::raw("  Settings  "))]
            .into_iter()
            .chain(settings.iter().enumerate().flat_map(|(i, (key, val))| {
                vec![
                    Line::from(vec![
                        if self.dash_item == i { Span::styled("  ▶ ", Style::new().fg(C_GREEN)) } else { Span::raw("    ") },
                        Span::raw(key),
                        Span::raw(": "),
                        Span::styled(val, Style::new().fg(C_COBALT)),
                    ]),
                ]
            }))
            .collect();

        let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Dashboard Output ──────────────────────────────────────────────────
    fn draw_dashboard_output(&self, f: &mut Frame, size: Rect) {
        let box_w = 72u16.min(size.width.saturating_sub(4));
        let box_h = 18u16.min(size.height.saturating_sub(4));
        let rect = self.centered(box_w, box_h, size);
        let title = if self.dash_running {
            format!("  {}  ", self.dash_output_title)
        } else {
            format!("  {}  ", self.dash_output_title)
        };
        f.render_widget(self.block(&title), rect);
        let inner = rect.inner(Margin::new(2, 1));

        if self.dash_running {
            let lines = vec![
                Line::from(""),
                Line::from(vec![Span::styled("  Running...", Style::new().fg(C_COBALT))]),
                Line::from(Span::raw("")),
                Line::from(vec![Span::raw("  "), Span::raw(self.dash_output.clone())]),
            ];
            let p = Paragraph::new(lines).style(Style::new().fg(C_TEXT));
            f.render_widget(p, inner);
        } else {
            let output_lines: Vec<Line> = self.dash_output.lines()
                .map(|l| {
                    if l.contains("✔") || l.contains("passed") {
                        Line::from(vec![Span::raw("  "), Span::styled(l, Style::new().fg(C_GREEN))])
                    } else if l.contains("✖") || l.contains("FAILED") {
                        Line::from(vec![Span::raw("  "), Span::styled(l, Style::new().fg(Color::Red))])
                    } else if l.contains("[") {
                        Line::from(vec![Span::raw("  "), Span::styled(l, Style::new().fg(C_COBALT))])
                    } else {
                        Line::from(vec![Span::raw("  "), Span::raw(l)])


                    }
                })
                .collect();
            let footer = vec![
                Line::from(Span::raw("")),
                Line::from(vec![Span::styled("  ENTER", Style::new().fg(C_GREEN)), Span::raw(" / "), Span::styled("ESC", Style::new().fg(C_DIM)), Span::raw(format!("  {}", self.config.tr("Close", "Fermer")))]),
            ];
            let all: Vec<Line> = output_lines.into_iter().chain(footer.into_iter()).collect();
            let p = Paragraph::new(all).style(Style::new().fg(C_TEXT)).wrap(Wrap { trim: true });
            f.render_widget(p, inner);
        }
    }


    // ── Done ───────────────────────────────────────────────────────────────
    fn draw_done(&self, f: &mut Frame, size: Rect) {
        let box_w = 46u16.min(size.width.saturating_sub(6));
        let box_h = 12u16.min(size.height.saturating_sub(6));
        let rect = self.centered(box_w, box_h, size);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(C_BORDER))
            .style(Style::new().bg(C_SURFACE));
        f.render_widget(block, rect);
        let inner = rect.inner(Margin::new(2, 2));
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  ⬡  Goodbye.", Style::new().fg(C_COBALT).bold())]),
            Line::from(Span::raw("")),
            Line::from(Span::raw("  Thank you for choosing privacy.")),
            Line::from(Span::raw("")),
        ];
        let p = Paragraph::new(lines).alignment(Alignment::Center).style(Style::new().fg(C_TEXT));
        f.render_widget(p, inner);
    }

    // ── Event handling ─────────────────────────────────────────────────────
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat { return; }

        match self.state {
            InstallState::Welcome => {
                if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    self.welcome_idx = (self.welcome_idx + 1).min(5);
                } else if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    self.welcome_idx = self.welcome_idx.saturating_sub(1);
                } else if key.code == KeyCode::Enter || key.code == KeyCode::Char(' ') {
                    match self.welcome_idx {
                        0 => {
                            if self.is_installed() {
                                self.menu_actions = vec![MenuAction::Update, MenuAction::Reinstall, MenuAction::Uninstall, MenuAction::Launch];
                                self.menu_idx = 0;
                                self.state = InstallState::AlreadyInstalled;
                            } else {
                                self.state = InstallState::Installing;
                            }
                        }
                        1 => { self.state = InstallState::Dashboard; }
                        2 => { self.state = InstallState::ConfigureServices; self.dash_tab = 1; }
                        3 => { self.dash_tab = 2; self.state = InstallState::Dashboard; }
                        4 => { self.dash_output_title = self.config.tr("Self-Test", "Auto-Test"); self.dash_output = "".to_string(); self.dash_running = true; self.state = InstallState::DashboardOutput; }
                        _ => { self.state = InstallState::Done; }
                    }
                } else if key.code == KeyCode::Char('q') {
                    self.state = InstallState::Done;
                }
            }

            InstallState::AlreadyInstalled => {
                if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    self.menu_idx = self.menu_idx.saturating_sub(1);
                } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    self.menu_idx = (self.menu_idx + 1).min(self.menu_actions.len().saturating_sub(1));
                } else if key.code == KeyCode::Enter {
                    match self.menu_actions.get(self.menu_idx) {
                        Some(MenuAction::Update) | Some(MenuAction::Reinstall) => {
                            self.state = InstallState::Installing;
                        }
                        Some(MenuAction::Uninstall) => {
                            self.do_uninstall();
                        }
                        Some(MenuAction::Launch) => {
                            self.state = InstallState::Dashboard;
                        }
                        _ => {}
                    }
                } else if key.code == KeyCode::Esc {
                    self.state = InstallState::Welcome;
                }
            }

            InstallState::Installing => {
                if self.install_error.is_some() && key.code == KeyCode::Enter {
                    self.state = InstallState::Done;
                }
            }

            InstallState::Configure => {
                if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    self.config_step = self.config_step.saturating_sub(1);
                } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    self.config_step = (self.config_step + 1).min(3);
                } else if key.code == KeyCode::Enter {
                    match self.config_step {
                        0 => { self.config.lang = match self.config.lang { Lang::EN => Lang::FR, Lang::FR => Lang::EN }; }
                        1 => { self.username_input = self.config.username.clone(); self.state = InstallState::UsernameInput; }
                        2 => { self.state = InstallState::NodeModeSelect; }
                        _ => { self.config.save(); self.state = InstallState::ConfigureServices; }
                    }
                }
            }

            InstallState::UsernameInput => {
                if key.code == KeyCode::Enter {
                    self.config.username = self.username_input.clone();
                    self.state = InstallState::Configure;
                } else if key.code == KeyCode::Esc {
                    self.state = InstallState::Configure;
                } else if let KeyCode::Char(c) = key.code {
                    self.username_input.push(c);
                } else if key.code == KeyCode::Backspace {
                    self.username_input.pop();
                }
            }

            InstallState::NodeModeSelect => {
                if key.code == KeyCode::Left || key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    self.node_idx = self.node_idx.saturating_sub(1);
                } else if key.code == KeyCode::Right || key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    self.node_idx = (self.node_idx + 1).min(2);
                } else if key.code == KeyCode::Enter {
                    self.config.node = match self.node_idx {
                        1 => NodeChoice::Passive,
                        2 => NodeChoice::Active,
                        _ => NodeChoice::None,
                    };
                    self.state = InstallState::Configure;
                } else if key.code == KeyCode::Esc {
                    self.state = InstallState::Configure;
                }
            }

            InstallState::ConfigureServices => {
                if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    if self.services_step > 0 {
                        self.services_step -= 1;
                    }
                } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    if self.services_step < SERVICE_COUNT.saturating_sub(1) {
                        self.services_step += 1;
                    }
                } else if key.code == KeyCode::Left || key.code == KeyCode::Right || key.code == KeyCode::Char(' ') {
                    // Toggle current service (but polygone is always on)
                    if self.services_step > 0 {
                        self.services_enabled[self.services_step] = !self.services_enabled[self.services_step];
                    }
                } else if key.code == KeyCode::Enter {
                    self.state = InstallState::Dashboard;
                }
            }

            InstallState::Dashboard => {
                let tab_count = 4usize;
                let item_counts = [3usize, SERVICE_COUNT, 3, 4];

                if key.code == KeyCode::Left || key.code == KeyCode::Char('h') {
                    self.dash_tab = (self.dash_tab + tab_count - 1) % tab_count;
                    self.dash_item = 0;
                } else if key.code == KeyCode::Right || key.code == KeyCode::Char('l') {
                    // In Services tab: toggle current service
                    if self.dash_tab == 1 {
                        if self.dash_item > 0 {  // polygone is always on
                            self.services_enabled[self.dash_item] = !self.services_enabled[self.dash_item];
                            self.save_services();
                        }
                    } else {
                        self.dash_tab = (self.dash_tab + 1) % tab_count;
                        self.dash_item = 0;
                    }
                } else if key.code == KeyCode::Up || key.code == KeyCode::Char('k') {
                    self.dash_item = self.dash_item.saturating_sub(1);
                } else if key.code == KeyCode::Down || key.code == KeyCode::Char('j') {
                    let max = *item_counts.get(self.dash_tab).unwrap_or(&1);
                    self.dash_item = (self.dash_item + 1).min(max.saturating_sub(1));
                } else if key.code == KeyCode::Char(' ') {
                    // Space also toggles in services tab
                    if self.dash_tab == 1 && self.dash_item > 0 {
                        self.services_enabled[self.dash_item] = !self.services_enabled[self.dash_item];
                        self.save_services();
                    }
                } else if key.code == KeyCode::Enter {
                    match self.dash_tab {
                        0 => { // Home tab actions
                            let tr = |en, fr| self.config.tr(en, fr);
                            match self.dash_item {
                                0 => { // Self-test
                                    self.dash_output_title = tr("Self-Test", "Auto-Test");
                                    self.dash_output = "".to_string();
                                    self.dash_running = true;
                                    self.state = InstallState::DashboardOutput;
                                }
                                1 => { // Generate keys
                                    self.dash_output_title = tr("Key Generation", "Génération de clés");
                                    self.dash_output = "".to_string();
                                    self.dash_running = true;
                                    self.state = InstallState::DashboardOutput;
                                }
                                _ => {} // message / other — skip for now
                            }
                        }
                        2 => { // Node tab actions
                            let tr = |en, fr| self.config.tr(en, fr);
                            match self.dash_item {
                                0 => { // Pause 1h
                                    self.dash_output_title = tr("Node Paused", "Noeud en pause");
                                    self.dash_output = tr("Node paused for 1 hour. Resume anytime from Dashboard > Nodes.", "Noeud mis en pause pour 1 heure. Reprenez à tout moment depuis Dashboard > Noeuds.");
                                    self.dash_running = false;
                                    self.state = InstallState::DashboardOutput;
                                }
                                1 => { // Pause 4h
                                    self.dash_output_title = tr("Node Paused", "Noeud en pause");
                                    self.dash_output = tr("Node paused for 4 hours. Resume anytime from Dashboard > Nodes.", "Noeud mis en pause pour 4 heures. Reprenez à tout moment depuis Dashboard > Noeuds.");
                                    self.dash_running = false;
                                    self.state = InstallState::DashboardOutput;
                                }
                                2 => { // Disable node
                                    let (title, msg) = (tr("Node Disabled", "Noeud désactivé"), tr("Node has been disabled. You can re-enable it from Settings > Node.", "Le noeud a été désactivé. Vous pouvez le réactiver depuis Paramètres > Noeud."));
                                    self.config.node = NodeChoice::None;
                                    self.config.save();
                                    self.dash_output_title = title;
                                    self.dash_output = msg;
                                    self.dash_running = false;
                                    self.state = InstallState::DashboardOutput;
                                }
                                _ => {}
                            }
                        }
                        3 => { // Settings tab
                            match self.dash_item {
                                0 => { self.config.lang = match self.config.lang { Lang::FR => Lang::EN, Lang::EN => Lang::FR }; self.config.save(); }
                                1 => { self.username_input = self.config.username.clone(); self.state = InstallState::UsernameInput; }
                                2 => { self.node_idx = match self.config.node { NodeChoice::None => 0, NodeChoice::Passive => 1, NodeChoice::Active => 2 }; self.state = InstallState::NodeModeSelect; }
                                _ => { self.state = InstallState::Welcome; }
                            }
                        }
                        _ => {}
                    }
                } else if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                    self.state = InstallState::Done;
                }
            }

            InstallState::DashboardOutput => {
                if key.code == KeyCode::Enter || key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                    self.dash_running = false;
                    self.state = InstallState::Dashboard;
                }
            }

            InstallState::Done => {}
        }
    }

    // ── Run loop ────────────────────────────────────────────────────────────
    fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if self.state == InstallState::Installing && !self.installing {
                self.installing = true;
                self.run_install();
            }

            // Run dashboard commands (self-test, keygen) when dash_running is set
            if self.dash_running && self.state == InstallState::DashboardOutput {
                let bin_path = self.binary_path();
                let cmd_name = if self.dash_output_title.contains("Self") || self.dash_output_title.contains("Test") {
                    "self-test"
                } else {
                    "keygen"
                };
                let output = Command::new(&bin_path)
                    .arg(cmd_name)
                    .output();
                self.dash_output = match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        if stdout.is_empty() && stderr.is_empty() {
                            format!("Command ran with no output.
Path: {:?}", bin_path)
                        } else {
                            format!("{}{}", stdout, stderr)
                        }
                    }
                    Err(e) => format!("Failed to run command: {}
Path: {:?}", e, bin_path),
                };
                self.dash_running = false;
            }

            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            }

            if self.state == InstallState::Done { break; }
        }
        Ok(())
    }
}

// ─── Main ─────────────────────────────────────────────────────────────────────
fn main() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    crossterm::terminal::enable_raw_mode()?;
    let result = App::new().run(&mut terminal);
    crossterm::terminal::disable_raw_mode()?;
    ratatui::restore();
    result
}
