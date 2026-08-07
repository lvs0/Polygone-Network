//! Services view — full module listing with descriptions.
//! Reuses the module grid from views.rs.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use super::app::{App, ModuleCard};
use super::views::render_module_grid;
use super::widgets::block_card;

pub fn render_services(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(area);

    // Descriptions
    let lines = vec![
        Line::from(vec![
            Span::styled(" Tous les modules Polygone ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("— activez depuis l'Accueil ou ici", Color::Rgb(0x94, 0xa3, 0xb8)),
        ]),
    ];
    let p = Paragraph::new(lines)
        .block(block_card("Modules", Color::Rgb(0xa7, 0x8b, 0xfa)))
        .style(Style::default().bg(Color::Rgb(0x0f, 0x17, 0x2a)));
    frame.render_widget(p, chunks[0]);

    let all = ModuleCard::all();
    render_module_grid(frame, chunks[1], &all);
}