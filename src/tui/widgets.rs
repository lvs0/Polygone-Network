//! Reusable TUI widgets for POLYGONE — block helpers, gauges, sparklines, topology.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Sparkline},
    Frame,
};

use super::app::{Theme, ThemeColors, App};

// ── Theme-aware color helper ─────────────────────────────────────────────────

/// Convert a (u8, u8, u8) tuple to a ratatui Color.
pub fn rgb(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

// ── Block helpers ────────────────────────────────────────────────────────────

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

/// Create a theme-aware block card.
pub fn block_card_themed(title: &str, accent: (u8, u8, u8), bg: (u8, u8, u8)) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(rgb(accent)).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(accent)).add_modifier(Modifier::DIM))
        .style(Style::default().bg(rgb(bg)))
}

// ── Indicator helpers ────────────────────────────────────────────────────────

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

/// Renders a horizontal progress bar (text-based).
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

// ── Gauge widget (theme-aware) ──────────────────────────────────────────────

/// Render a styled gauge for fragments or progress.
pub fn render_gauge(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    ratio: f64,
    theme: Theme,
) {
    let c = theme.colors();
    let ratio_clamped = ratio.clamp(0.0, 1.0);
    let pct = (ratio_clamped * 100.0) as u16;

    let color = if ratio_clamped >= 1.0 {
        rgb(c.green)
    } else if ratio_clamped >= 0.7 {
        rgb(c.accent)
    } else {
        rgb(c.amber)
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                format!(" {label} "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(color).add_modifier(Modifier::DIM))
            .style(Style::default().bg(rgb(c.bg)))
        )
        .gauge_style(Style::default().fg(color).bg(rgb(c.bg_3)))
        .ratio(ratio_clamped)
        .label(Span::styled(
            format!("{pct}%"),
            Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(gauge, area);
}

// ── Sparkline widget (theme-aware) ──────────────────────────────────────────

/// Render a sparkline chart for traffic data.
pub fn render_sparkline(
    frame: &mut Frame,
    area: Rect,
    data: &[u64],
    label: &str,
    color: (u8, u8, u8),
    bg: (u8, u8, u8),
) {
    let sparkline = Sparkline::default()
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(
                format!(" {label} "),
                Style::default().fg(rgb(color)).add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(rgb(color)).add_modifier(Modifier::DIM))
            .style(Style::default().bg(rgb(bg)))
        )
        .data(data)
        .max(500)
        .style(Style::default().fg(rgb(color)).bg(rgb(bg)));

    frame.render_widget(sparkline, area);
}

// ── Network topology visualization ───────────────────────────────────────────

/// Render a mini network topology in the given area.
pub fn render_topology(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    use ratatui::widgets::Canvas;
    use ratatui::widgets::canvas::Line as CanvasLine;
    use ratatui::widgets::canvas::Circle;

    let c = app.theme.colors();

    let canvas = Canvas::default()
        .block(block_card_themed("Topologie", c.accent, c.bg))
        .x_bounds([0.0, 100.0])
        .y_bounds([0.0, 100.0])
        .paint(|ctx| {
            // Draw edges
            for edge in &app.topo_edges {
                if let (Some(from), Some(to)) = (
                    app.topo_nodes.get(edge.from),
                    app.topo_nodes.get(edge.to),
                ) {
                    let opacity = if from.online && to.online { 0.4 } else { 0.1 };
                    let edge_color = if from.online && to.online {
                        rgb(c.accent)
                    } else {
                        rgb(c.text_faint)
                    };
                    ctx.draw(&CanvasLine {
                        x1: from.x * 100.0,
                        y1: from.y * 100.0,
                        x2: to.x * 100.0,
                        y2: to.y * 100.0,
                        color: edge_color,
                    });
                }
            }

            // Draw nodes
            for node in &app.topo_nodes {
                let (r, g, b) = if node.is_self {
                    c.accent
                } else if node.online {
                    c.green
                } else {
                    c.text_faint
                };
                let radius = if node.is_self { 4.0 } else { 2.5 };
                ctx.draw(&Circle {
                    x: node.x * 100.0,
                    y: node.y * 100.0,
                    radius,
                    color: rgb((r, g, b)),
                });
            }
        });

    frame.render_widget(canvas, area);
}

// ── Encryption pipeline visualization ────────────────────────────────────────

/// Render the encryption pipeline for the message composer view.
pub fn render_encrypt_pipeline(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let c = app.theme.colors();
    let steps = &app.composer_steps;
    let h = area.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("  📝 ", Style::default().fg(rgb(c.accent))),
        Span::styled("Composez votre message éphémère", Style::default().fg(rgb(c.text_hi)).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    for (i, step) in steps.iter().enumerate() {
        let (icon, color) = if step.done {
            ("✔", c.green)
        } else if i > 0 && !steps[i - 1].done {
            ("○", c.text_faint)
        } else {
            (spinner(app.tick), c.amber)
        };

        let line = Line::from(vec![
            Span::styled(format!("  {icon} "), Style::default().fg(rgb(color)).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:<20}", step.label),
                Style::default().fg(if step.done { rgb(c.text) } else { rgb(c.text_faint) }),
            ),
            Span::styled(step.detail, Style::default().fg(rgb(c.text_dim))),
        ]);
        lines.push(line);

        // Draw connector line between steps
        if i < steps.len() - 1 {
            let connector_color = if step.done { c.accent } else { c.text_faint };
            lines.push(Line::from(vec![
                Span::styled("    │", Style::default().fg(rgb(connector_color))),
            ]));
        }
    }

    let p = Paragraph::new(lines)
        .block(block_card_themed("Pipeline de chiffrement", c.violet, c.bg))
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, area);
}

// ── Status bar with keyboard shortcuts ──────────────────────────────────────

/// Render the bottom status bar with context-aware keyboard shortcuts.
pub fn render_status_bar_themed(
    frame: &mut Frame,
    area: Rect,
    app: &App,
) {
    let c = app.theme.colors();
    let s = &app.stats;

    let shortcuts = match app.current_view {
        super::app::View::Dashboard => "← → onglets · 1-5 direct · T thème · M/H/D/N toggle · q quitter",
        super::app::View::Favorites => "← → onglets · M/H/D/N toggle · q quitter",
        super::app::View::Services => "← → onglets · M/H/D/N toggle · q quitter",
        super::app::View::Settings => "← → onglets · T thème · q quitter",
        super::app::View::Composer => "← → onglets · q quitter",
    };

    let bar = Line::from(vec![
        Span::styled(
            format!(" {shortcuts}"),
            Style::default().fg(rgb(c.text_faint)),
        ),
        Span::styled("  ", Style::default().fg(rgb(c.border))),
        Span::styled(
            format!("⬡ {} · {:.1}/m", s.balance, s.consumption),
            Style::default().fg(rgb(c.accent_dim)),
        ),
        Span::styled("  ", Style::default().fg(rgb(c.border))),
        Span::styled(
            format!("🎨 {}", app.theme.label()),
            Style::default().fg(rgb(c.text_dim)),
        ),
        Span::styled("  ", Style::default().fg(rgb(c.border))),
        Span::styled("v1.0.0", Style::default().fg(rgb(c.text_faint))),
    ]);

    let p = Paragraph::new(bar)
        .style(Style::default().bg(rgb(c.bg)));
    frame.render_widget(p, area);
}
