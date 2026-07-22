//! Polygone Time Sync Protocol (PTSP) — Wire Format
//!
//! Simple, versioned, extensible message format for time sync gossip.
//! Uses CBOR for compact binary encoding (via serde_cbor).
//!
//! Message flow:
//! 1. Periodic: Node broadcasts TimeSyncAnnounce (their clock)
//! 2. On receive: Peer replies with TimeSyncResponse (4 timestamps)
//! 3. Both update their filters

use super::types::{Timestamp, PeerId, PeerTimeStateGossip};
use crate::identity::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Protocol version
pub const PTSP_VERSION: u8 = 1;

/// Time sync message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum TimeSyncMessage {
    /// Periodic announcement of local time
    Announce(TimeSyncAnnounce),
    /// Request-response for precise offset calculation
    Request(TimeSyncRequest),
    Response(TimeSyncResponse),
    /// Gossip: share peer time states for faster convergence
    Gossip(TimeSyncGossip),
}

/// Periodic time announcement (broadcast via GossipSub)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSyncAnnounce {
    pub version: u8,
    pub peer_id: PeerId,
    pub timestamp: Timestamp,      // Sender's local time
    pub network_offset_ms: i64,    // Sender's estimated network offset
    pub confidence: f64,           // Sender's confidence in offset
    pub rtt_estimate_ms: u32,      // Typical RTT to peers
}

/// Request for precise synchronization (direct P2P)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSyncRequest {
    pub version: u8,
    pub request_id: u64,           // For matching response
    pub peer_id: PeerId,           // Requester
    pub t1: Timestamp,             // Requester send time
}

/// Response with 4 timestamps for NTP-style calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSyncResponse {
    pub version: u8,
    pub request_id: u64,
    pub responder_id: PeerId,
    pub t1: Timestamp,             // Original request send (from request)
    pub t2: Timestamp,             // Responder receive time
    pub t3: Timestamp,             // Responder send time
    pub t4: Timestamp,             // Filled by requester on receive
}

/// Gossip message: share known peer time states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSyncGossip {
    pub version: u8,
    pub originator: PeerId,
    pub peer_states: Vec<PeerTimeStateGossip>,
}

/// Encode message to CBOR bytes
pub fn encode(msg: &TimeSyncMessage) -> Vec<u8> {
    serde_cbor::to_vec(msg).unwrap_or_default()
}

/// Decode CBOR bytes to message
pub fn decode(bytes: &[u8]) -> Result<TimeSyncMessage, serde_cbor::Error> {
    serde_cbor::from_slice(bytes)
}

/// Calculate offset and RTT from 4 timestamps
/// 
/// Returns (offset_ms, rtt_ms) where:
/// - offset = ((t2 - t1) + (t3 - t4)) / 2
/// - rtt = (t4 - t1) - (t3 - t2)
pub fn calculate_offset_rtt(t1: Timestamp, t2: Timestamp, t3: Timestamp, t4: Timestamp) -> (i64, u64) {
    let offset = ((t2.0 - t1.0) + (t3.0 - t4.0)) / 2;
    let rtt = (t4.0 - t1.0).saturating_sub(t3.0 - t2.0) as u64;
    (offset, rtt)
}

/// Apply clock correction with smoothing
/// 
/// Returns the actual correction applied (may be capped)
pub fn apply_correction(current_offset: i64, target_offset: i64, max_step_ms: u64) -> i64 {
    let diff = target_offset - current_offset;
    let capped = diff.clamp(-(max_step_ms as i64), max_step_ms as i64);
    current_offset + capped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_offset_rtt() {
        // Symmetric: offset=0, rtt=10
        let (off, rtt) = calculate_offset_rtt(
            Timestamp(1000), Timestamp(1005), Timestamp(1010), Timestamp(1015)
        );
        assert_eq!(off, 0);
        assert_eq!(rtt, 10);

        // Server 10ms ahead
        let (off, rtt) = calculate_offset_rtt(
            Timestamp(1000), Timestamp(1015), Timestamp(1020), Timestamp(1025)
        );
        assert_eq!(off, 5); // (15 + (-5)) / 2 = 5
        assert_eq!(rtt, 20);
    }

    #[test]
    fn test_apply_correction_smoothing() {
        // Large jump capped
        let corr = apply_correction(0, 100, 10);
        assert_eq!(corr, 10);

        // Small jump applied fully
        let corr = apply_correction(10, 15, 10);
        assert_eq!(corr, 15);

        // Negative correction
        let corr = apply_correction(100, 0, 10);
        assert_eq!(corr, 90);
    }

    #[test]
        fn test_cbor_roundtrip() {
        let msg = TimeSyncMessage::Announce(TimeSyncAnnounce {
            version: PTSP_VERSION,
            peer_id: NodeId([42u8; 16]),
            timestamp: Timestamp(1234567890000),
            network_offset_ms: 5,
            confidence: 0.9,
            rtt_estimate_ms: 50,
        });

        let encoded = encode(&msg);
        let decoded = decode(&encoded).unwrap();
        
        match decoded {
            TimeSyncMessage::Announce(a) => {
                assert_eq!(a.peer_id.0[0], 42);
                assert_eq!(a.timestamp.0, 1234567890000);
                assert_eq!(a.network_offset_ms, 5);
            }
            _ => panic!("wrong type"),
        }
    }
}