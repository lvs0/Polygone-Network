//! Polygone-Compute: power lending daemon
//!
//! Detects idle CPU/RAM resources and lends them to the network.
//! Smart detection pauses lending when the user is active.
//!
//! Integration with Ollama for local inference sharing.

mod idle;
mod daemon;

pub use idle::{IdleDetector, SystemMetrics};
pub use daemon::{ComputeDaemon, ComputeConfig, ComputeStatus, daemon_is_running, write_pid, remove_pid, daemon_pid_path};
