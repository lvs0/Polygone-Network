//! Polygone Time Synchronization Protocol
//!
//! Decentralized clock synchronization for P2P nodes.
//! Based on NTP principles but adapted for mesh topology with
//! median filtering and Byzantine fault tolerance.
//!
//! ## Protocol Overview
//!
//! Each node maintains a local clock offset estimate relative to
//! the network consensus time. Nodes exchange timestamped messages
//! and compute pairwise offsets using the standard NTP formula:
//!
//! ```text
//!     T1 = t1 (client send)
//!     T2 = t2 (server receive)
//!     T3 = t3 (server send)
//!     T4 = t4 (client receive)
//!
//!     offset = ((T2 - T1) + (T3 - T4)) / 2
//!     delay  = (T4 - T1) - (T3 - T2)
//! ```
//!
//! Network consensus uses median of peer offsets with confidence weighting.

pub mod filter;
pub mod protocol;
pub mod types;

pub use filter::{MedianFilter, MedianFilterConfig, WeightedMedianFilter};
pub use protocol::TimeSyncMessage;
pub use types::{Timestamp, TimeOffset, SyncConfig, SyncStats, PeerId, PeerTimeState, ClockSource};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_calculation() {
        // T1=1000, T2=1005, T3=1010, T4=1015
        // offset = ((1005-1000) + (1010-1015)) / 2 = (5 - 5) / 2 = 0
        // delay = (1015-1000) - (1010-1005) = 15 - 5 = 10
        let offset = ((1005i64 - 1000) + (1010 - 1015)) / 2;
        let delay = (1015 - 1000) - (1010 - 1005);
        assert_eq!(offset, 0);
        assert_eq!(delay, 10);
    }

    #[test]
    fn test_positive_offset() {
        // Server clock is 10ms ahead
        // T1=1000, T2=1015, T3=1020, T4=1025
        // offset = ((1015-1000) + (1020-1025)) / 2 = (15 - 5) / 2 = 5
        let offset = ((1015i64 - 1000) + (1020 - 1025)) / 2;
        assert_eq!(offset, 5);
    }
}