//! Core types for time synchronization

use serde::{Deserialize, Serialize};
use crate::identity::NodeId;

/// Peer identifier (wraps NodeId for time sync context)
pub type PeerId = NodeId;

/// Millisecond-precision timestamp (Unix epoch)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Current time as Timestamp
    pub fn now() -> Self {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self(ms)
    }

    /// Create from milliseconds since epoch
    pub fn from_millis(ms: i64) -> Self {
        Self(ms)
    }

    /// Get milliseconds since epoch
    pub fn as_millis(&self) -> i64 {
        self.0
    }

    /// Saturating subtraction (returns difference in ms)
    pub fn saturating_sub(&self, other: Timestamp) -> i64 {
        self.0.saturating_sub(other.0)
    }

    /// Saturating addition
    pub fn saturating_add(&self, ms: i64) -> Self {
        Self(self.0.saturating_add(ms))
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Clock offset in milliseconds (positive = local clock is ahead)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeOffset(pub i64);

impl TimeOffset {
    pub fn zero() -> Self { Self(0) }
    pub fn from_millis(ms: i64) -> Self { Self(ms) }
    pub fn as_millis(&self) -> i64 { self.0 }
    pub fn abs(&self) -> u64 { self.0.abs() as u64 }
    pub fn is_within(&self, tolerance_ms: u64) -> bool { self.abs() <= tolerance_ms }
}

impl std::ops::Add for TimeOffset {
    type Output = Self;
    fn add(self, other: Self) -> Self { Self(self.0.saturating_add(other.0)) }
}

impl std::ops::Sub for TimeOffset {
    type Output = Self;
    fn sub(self, other: Self) -> Self { Self(self.0.saturating_sub(other.0)) }
}

/// Configuration for synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Gossip interval in milliseconds
    pub gossip_interval_ms: u64,
    /// Minimum peers for consensus
    pub min_peers: usize,
    /// Maximum RTT to consider peer (ms)
    pub max_rtt_ms: u32,
    /// Minimum confidence for peer to be reliable
    pub min_confidence: f64,
    /// Confidence threshold for network sync
    pub confidence_threshold: f64,
    /// Maximum correction per tick (ms)
    pub max_correction_per_tick_ms: u64,
    /// Enable NTP fallback
    pub ntp_fallback: bool,
    /// NTP servers to try
    pub ntp_servers: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            gossip_interval_ms: 30_000,      // 30s
            min_peers: 3,
            max_rtt_ms: 500,
            min_confidence: 0.5,
            confidence_threshold: 0.7,
            max_correction_per_tick_ms: 5,   // 5ms max step
            ntp_fallback: true,
            ntp_servers: vec![
                "pool.ntp.org".into(),
                "time.google.com".into(),
                "time.cloudflare.com".into(),
            ],
        }
    }
}

/// Synchronization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub network_offset_ms: i64,
    pub network_confidence: f64,
    pub peer_count: usize,
    pub median_rtt_ms: u64,
    pub is_synced: bool,
    pub time_since_sync_ms: u64,
    pub clock_source: ClockSource,
}

/// Clock synchronization source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockSource {
    /// No synchronization
    Unsynchronized,
    /// NTP fallback
    NtpFallback,
    /// Peer consensus (normal operation)
    PeerConsensus,
}

/// Per-peer time state
#[derive(Debug, Clone)]
pub struct PeerTimeState {
    pub peer_id: PeerId,
    pub offset_ms: i64,
    pub rtt_ms: u64,
    pub confidence: f64,
    pub offset_variance: f64,
    pub last_sync: Timestamp,
    pub sample_count: u64,
}

impl PeerTimeState {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            offset_ms: 0,
            rtt_ms: 0,
            confidence: 0.0,
            offset_variance: 0.0,
            last_sync: Timestamp::now(),
            sample_count: 0,
        }
    }

    pub fn is_reliable(&self, config: &SyncConfig) -> bool {
        self.confidence >= config.min_confidence
            && self.rtt_ms <= config.max_rtt_ms as u64
            && self.sample_count >= 3
    }

    pub fn is_fresh(&self, max_age_ms: u64) -> bool {
        Timestamp::now().saturating_sub(self.last_sync) <= max_age_ms as i64
    }
}

/// Gossip-serializable peer state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerTimeStateGossip {
    pub peer_id: PeerId,
    pub offset_ms: i64,
    pub confidence: f64,
    pub rtt_ms: u32,
    pub last_sync_age_ms: u64,
}

impl From<&PeerTimeState> for PeerTimeStateGossip {
    fn from(state: &PeerTimeState) -> Self {
        Self {
            peer_id: state.peer_id,
            offset_ms: state.offset_ms,
            confidence: state.confidence,
            rtt_ms: state.rtt_ms as u32,
            last_sync_age_ms: Timestamp::now().saturating_sub(state.last_sync) as u64,
        }
    }
}

/// Re-export MedianFilterConfig from filter to avoid duplication
pub use super::filter::MedianFilterConfig;