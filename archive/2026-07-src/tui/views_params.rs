//! Parameters view — configure network, ports, paths.
//! Read-only display of current configuration.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use super::app::App;
use super::widgets::block_plain;

struct Param {
    key: &'static str,
    value: String,
    description: &'static str,
}

impl Param {
    fn new(key: &'static str, value: &str, description: &'static str) -> Self {
        Self { key, value: value.to_string(), description }
    }
}

fn get_params() -> Vec<Param> {
    let key_dir = dirs::data_dir()
        .map(|d| d.join("polygone").join("keys").display().to_string())
        .unwrap_or_else(|| "~/.local/share/polygone/keys".to_string());

    let config_dir = dirs::data_dir()
        .map(|d| d.join("polygone").display().to_string())
        .unwrap_or_else(|| "~/.local/share/polygone".to_string());

    vec![
        Param::new("version", env!("CARGO_PKG_VERSION"), "Polygone version"),
        Param::new("network.listen", "0.0.0.0:4001", "P2P listen address"),
        Param::new("network.dht", "Kademlia (memory)", "DHT mode"),
        Param::new("node.threshold", "4-of-7", "Shamir reconstruction threshold"),
        Param::new("node.ttl", "3600s", "Ephemeral node TTL"),
        Param::new("crypto.kem", "ML-KEM-1024", "Key encapsulation (FIPS 203)"),
        Param::new("crypto.sign", "Ed25519", "Digital signatures (ML-DSA ready)"),
        Param::new("crypto.cipher", "AES-256-GCM", "Symmetric encryption"),
        Param::new("crypto.kdf", "BLAKE3", "Key derivation (domain-sep)"),
        Param::new("paths.keys", &key_dir, "Key storage"),
        Param::new("paths.config", &config_dir, "Config directory"),
    ]
}

pub fn render_params(frame: &mut Frame, area: Rect, _app: &App) {
    let params = get_params();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);
    let mid = params.len() / 2;
    render_param_table(frame, chunks[0], &params[..mid]);
    render_param_table(frame, chunks[1], &params[mid..]);
}

fn render_param_table(frame: &mut Frame, area: Rect, params: &[Param]) {
    let lines: Vec<Line> = params.iter().map(|p| {
        Line::from(vec![
            Span::styled(format!("  {}", p.key), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(&p.value, Style::default().fg(Color::Rgb(0xcb, 0xd5, 0xe1))),
            Span::styled(format!("  ({})", p.description), Color::Rgb(0x47, 0x55, 0x69)),
        ])
    }).collect();

    let p = Paragraph::new(lines)
        .block(block_plain("Parameters"))
        .style(ratatui::style::Style::default().bg(Color::Rgb(0x0f, 0x17, 0x2a)));
    frame.render_widget(p, area);
}