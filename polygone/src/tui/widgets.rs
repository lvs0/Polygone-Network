//! Reusable TUI widgets for POLYGONE — block helpers, header, footer.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};


/// Create a themed block card.
pub fn block_card(title: &str, accent: Color) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent).add_modifier(Modifier::DIM))
}

/// Create a plain bordered block.
pub fn block_plain(title: &str) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(Color::Rgb(0x64, 0x74, 0x8b)),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(0x33, 0x41, 0x55)))
}

/// Renders a small active indicator dot.
pub fn dot(active: bool) -> Span<'static> {
    if active {
        Span::styled(" ●", Style::default().fg(Color::Rgb(0x22, 0xc5, 0x5e)))
    } else {
        Span::styled(" ○", Style::default().fg(Color::Rgb(0x47, 0x55, 0x69)))
    }
}

/// Renders a spinner character based on tick.
pub fn spinner(tick: u64) -> &'static str {
    ["◐", "◓", "◑", "◒"][(tick / 3 % 4) as usize]
}

/// Renders a horizontal progress bar.
pub fn progress_bar(filled: usize, total: usize, width: usize) -> Line<'static> {
    let count = filled.min(total);
    let bar: String = (0..width).map(|i| if i < count * width / total.max(1) { '█' } else { '░' }).collect();
    let color = if count >= total {
        Color::Rgb(0x22, 0xc5, 0x5e)
    } else {
        Color::Rgb(0xfb, 0xbd, 0x24)
    };
    Line::from(Span::styled(bar, Style::default().fg(color)))
}