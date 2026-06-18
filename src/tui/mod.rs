//! Terminal User Interface for POLYGONE.
//! Ratatui + Crossterm dashboard with 4 tabs, live modules, activity log.

pub mod app;
pub mod views;
pub mod widgets;

pub use app::{App, ModuleCard, ModuleStatus, run_tui};
pub use views::{View, render_view};