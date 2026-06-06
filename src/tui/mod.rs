//! Terminal User Interface for POLYGONE.
//! Ratatui + Crossterm dashboard with 5 tabs, live modules, activity log.
//! Features: splash screen, sparklines, gauge, topology, message composer, themes.

pub mod app;
pub mod views;
pub mod widgets;

pub use app::{App, ModuleCard, ModuleStatus, Theme, run_tui};
pub use views::{View, render_view, render_splash};
