//! Network protocol for peer-to-peer compute resource sharing.
//!
//! Lightweight protocol built on top of the existing libp2p infrastructure
//! for advertising compute capabilities, negotiating resource deals, and
//! managing lending contracts between peers.
//!
//! ## Protocol Messages
//!
//! ```text
//! ┌─────────────┐     ResourceOffer      ┌─────────────┐
//! │  Lender     │ ─────────────────────►  │  Borrower   │
//! │  (idle)     │ ◄─────────────────────  │  (needs)    │
//! └─────────────┘     ResourceRequest     └─────────────┘
//!        │                                        │
//!        ▼                                        ▼
//! ┌─────────────┐     AllocationAck       ┌─────────────┐
//! │  Lender     │ ─────────────────────►  │  Borrower   │
//! │  (accept)   │                         │  (use)      │
//! └─────────────┘                         └─────────────┘
//!        │                                        │
//!        ▼                                        ▼
//! ┌─────────────┐     AllocationComplete ┌─────────────┐
//! │  Lender     │ ◄───────────────────── │  Borrower   │
//! │  (release)  │ ─────────────────────► │  (payment)  │
//! └─────────────┘     PaymentProof       └─────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Protocol version for compute sharing.
pub const COMPUTE_PROTOCOL_VERSION: &str = "/polygone/compute/1.0.0";

/// Gossip topic for compute capability announcements.
pub const COMPUTE_ANNOUNCE_TOPIC: &str = "polygone-compute-announce";

/// Request-response protocol for direct compute negotiations.
pub const COMPUTE_REQUEST_PROTOCOL: &str = "/polygone/compute/rr/1.0.0";

/// Maximum message size (1MB).
pub const MAX_MESSAGE_SIZE: usize = 1_048_576;

// ── Message Types ────────────────────────────────────────────────────────────

/// All messages in the compute sharing protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeMessage {
    /// A node announces its available resources (broadcast via gossip)
    AnnounceCapabilities(CapabilityAnnounce),

    /// A node requests resources from a specific peer
    ResourceRequest(ResourceRequestMessage),

    /// A node accepts a resource request
    ResourceAccept(ResourceAcceptMessage),

    /// A node rejects a resource request
    ResourceReject(ResourceRejectMessage),

    /// Heartbeat to keep allocation alive
    Heartbeat(HeartbeatMessage),

    /// Request is being cancelled
    CancelAllocation(CancelMessage),

    /// Allocation completed, requesting payment
    RequestPayment(PaymentRequestMessage),

    /// Payment proof / receipt
    PaymentProof(PaymentProofMessage),

    /// Resource usage update (periodic)
    UsageUpdate(UsageUpdateMessage),

    /// Peer is shutting down gracefully
    PeerShutdown(ShutdownMessage),
}

// ── Detailed Message Structures ──────────────────────────────────────────────

/// Broadcast by a node to announce its compute capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityAnnounce {
    /// Node identifier
    pub node_id: String,
    /// Human-readable node name
    pub node_name: String,
    /// Available RAM (MB)
    pub available_ram_mb: u64,
    /// Available CPU cores
    pub available_cpu_cores: u32,
    /// Available storage (GB)
    pub available_storage_gb: u64,
    /// GPU units available
    pub available_gpu_units: u32,
    /// Price per RAM MB-hour (POLY)
    pub price_ram_per_mb_hour: f64,
    /// Price per CPU core-hour (POLY)
    pub price_cpu_per_core_hour: f64,
    /// Price per storage GB-hour (POLY)
    pub price_storage_per_gb_hour: f64,
    /// Node uptime in seconds
    pub uptime_secs: u64,
    /// Current reputation score (0–100)
    pub reputation: u32,
    /// Announcement timestamp (epoch ms)
    pub timestamp_ms: u64,
    /// TTL for this announcement (seconds)
    pub ttl_secs: u32,
}

/// A request for compute resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequestMessage {
    /// Unique request ID
    pub request_id: String,
    /// Requesting node ID
    pub requester_id: String,
    /// Requester's reputation score
    pub requester_reputation: u32,
    /// Type of resource requested
    pub resource_type: ComputeResourceType,
    /// Amount requested
    pub amount: u64,
    /// Maximum price per unit-hour willing to pay (POLY)
    pub max_price_per_unit_hour: f64,
    /// Requested duration in seconds
    pub duration_secs: u64,
    /// Priority (0=background, 1=normal, 2=high, 3=critical)
    pub priority: u32,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
    /// Signature over the request (ed25519, hex-encoded)
    pub signature: String,
}

/// Acceptance of a resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAcceptMessage {
    /// Original request ID
    pub request_id: String,
    /// Allocation ID assigned by the lender
    pub allocation_id: String,
    /// Lender node ID
    pub lender_id: String,
    /// Agreed price per unit-hour (POLY)
    pub agreed_price_per_unit_hour: f64,
    /// Allocation start time (epoch ms)
    pub start_ms: u64,
    /// Allocation end time (epoch ms)
    pub end_ms: u64,
    /// Endpoint address for resource access (e.g., "/ip4/1.2.3.4/tcp/5001")
    pub endpoint: String,
    /// Authentication token for this allocation
    pub auth_token: String,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Rejection of a resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRejectMessage {
    /// Original request ID
    pub request_id: String,
    /// Reason for rejection
    pub reason: RejectReason,
    /// Human-readable explanation
    pub explanation: String,
    /// Suggested alternative lender node ID (if any)
    pub suggest_alternative: Option<String>,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Reasons for rejecting a resource request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RejectReason {
    /// No resources available
    InsufficientResources,
    /// Price too low
    PriceTooLow,
    /// Requester's reputation too low
    ReputationTooLow,
    /// Lender is at capacity
    AtCapacity,
    /// Resource is reserved for another borrower
    Reserved,
    /// Lender is about to become active (user returning)
    UserReturning,
    /// General/unknown reason
    Other(String),
}

impl RejectReason {
    pub fn label(&self) -> &str {
        match self {
            RejectReason::InsufficientResources => "Insufficient resources",
            RejectReason::PriceTooLow => "Price too low",
            RejectReason::ReputationTooLow => "Reputation too low",
            RejectReason::AtCapacity => "At capacity",
            RejectReason::Reserved => "Reserved",
            RejectReason::UserReturning => "User returning",
            RejectReason::Other(_) => "Other",
        }
    }
}

/// Periodic heartbeat to keep an allocation alive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    /// Allocation ID
    pub allocation_id: String,
    /// Sender node ID
    pub node_id: String,
    /// Current resource usage (RAM MB used)
    pub ram_usage_mb: u64,
    /// CPU usage (percentage)
    pub cpu_usage_pct: f32,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Cancellation of an allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelMessage {
    /// Allocation ID to cancel
    pub allocation_id: String,
    /// Node requesting cancellation
    pub node_id: String,
    /// Reason for cancellation
    pub reason: String,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Payment request after allocation completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequestMessage {
    /// Allocation ID
    pub allocation_id: String,
    /// Lender node ID
    pub lender_id: String,
    /// Borrower node ID
    pub borrower_id: String,
    /// Total amount owed (POLY)
    pub total_owed: f64,
    /// Resource type and amount used
    pub resource_type: ComputeResourceType,
    /// Amount used
    pub amount_used: u64,
    /// Duration used (seconds)
    pub duration_used_secs: u64,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Payment proof / receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentProofMessage {
    /// Allocation ID
    pub allocation_id: String,
    /// Payer node ID
    pub payer_id: String,
    /// Payee node ID
    pub payee_id: String,
    /// Amount paid (POLY)
    pub amount_paid: f64,
    /// Transaction hash or identifier
    pub tx_hash: String,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Resource usage update during an active allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageUpdateMessage {
    /// Allocation ID
    pub allocation_id: String,
    /// Node ID reporting usage
    pub node_id: String,
    /// RAM usage (MB)
    pub ram_mb: u64,
    /// CPU usage (cores)
    pub cpu_cores: f32,
    /// Storage usage (GB)
    pub storage_gb: u64,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Peer shutdown notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownMessage {
    /// Node ID shutting down
    pub node_id: String,
    /// Reason for shutdown
    pub reason: String,
    /// Timestamp (epoch ms)
    pub timestamp_ms: u64,
}

/// Resource types in the compute protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComputeResourceType {
    RamMB,
    CpuCores,
    StorageGB,
    GpuUnits,
}

impl ComputeResourceType {
    pub fn label(&self) -> &str {
        match self {
            ComputeResourceType::RamMB => "RAM (MB)",
            ComputeResourceType::CpuCores => "CPU Cores",
            ComputeResourceType::StorageGB => "Storage (GB)",
            ComputeResourceType::GpuUnits => "GPU Units",
        }
    }
}

// ── Message Validation ───────────────────────────────────────────────────────

impl ComputeMessage {
    /// Validate a message's basic invariants.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::AnnounceCapabilities(ann) => {
                if ann.node_id.is_empty() { return Err("empty node_id"); }
                if ann.timestamp_ms == 0 { return Err("invalid timestamp"); }
                if ann.ttl_secs == 0 { return Err("zero TTL"); }
                Ok(())
            }
            Self::ResourceRequest(req) => {
                if req.request_id.is_empty() { return Err("empty request_id"); }
                if req.requester_id.is_empty() { return Err("empty requester_id"); }
                if req.amount == 0 { return Err("zero amount"); }
                if req.duration_secs == 0 { return Err("zero duration"); }
                if req.max_price_per_unit_hour < 0.0 { return Err("negative price"); }
                Ok(())
            }
            Self::ResourceAccept(acc) => {
                if acc.request_id.is_empty() { return Err("empty request_id"); }
                if acc.allocation_id.is_empty() { return Err("empty allocation_id"); }
                if acc.agreed_price_per_unit_hour < 0.0 { return Err("negative price"); }
                Ok(())
            }
            Self::ResourceReject(rej) => {
                if rej.request_id.is_empty() { return Err("empty request_id"); }
                Ok(())
            }
            Self::Heartbeat(hb) => {
                if hb.allocation_id.is_empty() { return Err("empty allocation_id"); }
                Ok(())
            }
            Self::CancelAllocation(c) => {
                if c.allocation_id.is_empty() { return Err("empty allocation_id"); }
                Ok(())
            }
            Self::RequestPayment(p) => {
                if p.total_owed < 0.0 { return Err("negative payment"); }
                Ok(())
            }
            Self::PaymentProof(p) => {
                if p.amount_paid < 0.0 { return Err("negative payment"); }
                if p.tx_hash.is_empty() { return Err("empty tx_hash"); }
                Ok(())
            }
            Self::UsageUpdate(u) => {
                if u.allocation_id.is_empty() { return Err("empty allocation_id"); }
                Ok(())
            }
            Self::PeerShutdown(s) => {
                if s.node_id.is_empty() { return Err("empty node_id"); }
                Ok(())
            }
        }
    }

    /// Serialize the message to bytes (using bincode for efficiency).
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|e| format!("serialize error: {}", e))
    }

    /// Deserialize a message from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        bincode::deserialize(data).map_err(|e| format!("deserialize error: {}", e))
    }

    /// Get the message type name for logging.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::AnnounceCapabilities(_) => "AnnounceCapabilities",
            Self::ResourceRequest(_) => "ResourceRequest",
            Self::ResourceAccept(_) => "ResourceAccept",
            Self::ResourceReject(_) => "ResourceReject",
            Self::Heartbeat(_) => "Heartbeat",
            Self::CancelAllocation(_) => "CancelAllocation",
            Self::RequestPayment(_) => "RequestPayment",
            Self::PaymentProof(_) => "PaymentProof",
            Self::UsageUpdate(_) => "UsageUpdate",
            Self::PeerShutdown(_) => "PeerShutdown",
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Current epoch in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Generate a random allocation/request ID.
pub fn random_id() -> String {
    format!("id:{:016x}", rand::random::<u64>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announce_roundtrip() {
        let msg = ComputeMessage::AnnounceCapabilities(CapabilityAnnounce {
            node_id: "node-1".into(),
            node_name: "Test Node".into(),
            available_ram_mb: 4096,
            available_cpu_cores: 4,
            available_storage_gb: 100,
            available_gpu_units: 0,
            price_ram_per_mb_hour: 0.001,
            price_cpu_per_core_hour: 0.01,
            price_storage_per_gb_hour: 0.0005,
            uptime_secs: 3600,
            reputation: 85,
            timestamp_ms: now_ms(),
            ttl_secs: 300,
        });
        let bytes = msg.to_bytes().unwrap();
        let decoded = ComputeMessage::from_bytes(&bytes).unwrap();
        assert!(decoded.validate().is_ok());
    }

    #[test]
    fn resource_request_validation() {
        let msg = ComputeMessage::ResourceRequest(ResourceRequestMessage {
            request_id: "".into(),
            requester_id: "node-2".into(),
            requester_reputation: 70,
            resource_type: ComputeResourceType::RamMB,
            amount: 512,
            max_price_per_unit_hour: 0.005,
            duration_secs: 3600,
            priority: 1,
            timestamp_ms: now_ms(),
            signature: "abc123".into(),
        });
        assert!(msg.validate().is_err()); // empty request_id
    }

    #[test]
    fn reject_reason_labels() {
        assert_eq!(RejectReason::InsufficientResources.label(), "Insufficient resources");
        assert_eq!(RejectReason::PriceTooLow.label(), "Price too low");
    }
}
