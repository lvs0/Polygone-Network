//! Polygone-Compute: power lending daemon
//!
//! Detects idle CPU/RAM resources and lends them to the network.
//! Smart detection pauses lending when the user is active.
//!
//! Integration with Ollama for local inference sharing.
//! Stealth mode for invisible operation.
//! POLY token tracking for lending income/expenses.
//! Network protocol for peer-to-peer resource negotiation.

mod idle;
mod daemon;
pub mod lending;
pub mod stealth;
pub mod protocol;

pub use idle::{IdleDetector, SystemMetrics};
pub use daemon::{ComputeDaemon, ComputeConfig, ComputeStatus, daemon_is_running, write_pid, remove_pid, daemon_pid_path};
pub use lending::{ResourceScheduler, ResourceLimits, ResourceType, TaskPriority, ResourceRequest, ResourceAllocation, LendingStats};
pub use stealth::{StealthMode, StealthConfig};
pub use protocol::{ComputeMessage, CapabilityAnnounce, ResourceRequestMessage, ResourceAcceptMessage, ResourceRejectMessage};
