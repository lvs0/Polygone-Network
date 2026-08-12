//! Time Synchronization Engine — Core Logic
//!
//! Embeddable time sync engine. No async runtime dependency.
//! Can be driven by any event loop (tokio, async-std, smol, bare metal).

use super::{PeerTimeState, SyncConfig, SyncStats, ClockSource, Timestamp, PeerId};
use super::filter::{MedianFilter, MedianFilterConfig, WeightedMedianFilter};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Main time synchronization engine.
/// 
/// Usage:
/// ```no_run
/// let mut engine = TimeSyncEngine::new(config);
/// engine.bootstrap_ntp().await?;
/// loop {
///     engine.process_gossip(peer_id, their_timestamp, our_receive_time);
///     if let Some(correction) = engine.tick() {
///         apply_clock_correction(correction);
///     }
/// }
/// ```
pub struct TimeSyncEngine {
    config: SyncConfig,
    peer_states: HashMap<PeerId, PeerTimeState>,
    /// Per-peer median filters for offset estimation
    peer_filters: HashMap<PeerId, MedianFilter>,
    /// Global weighted median for network consensus
    consensus_filter: WeightedMedianFilter,
    /// Recent corrections applied (for smoothing)
    recent_corrections: VecDeque<i64>,
    /// Current network offset estimate
    network_offset_ms: i64,
    /// Current confidence
    network_confidence: f64,
    /// Clock source
    clock_source: ClockSource,
    /// Last sync timestamp
    last_sync_time: Timestamp,
    /// NTP bootstrap done
    ntp_bootstrapped: bool,
}

impl TimeSyncEngine {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            peer_states: HashMap::new(),
            peer_filters: HashMap::new(),
            consensus_filter: WeightedMedianFilter::new(20),
            recent_corrections: VecDeque::with_capacity(100),
            network_offset_ms: 0,
            network_confidence: 0.0,
            clock_source: ClockSource::Unsynchronized,
            last_sync_time: Timestamp(0),
            ntp_bootstrapped: false,
        }
    }

    /// Bootstrap time from NTP servers (blocking, uses std::net)
    pub fn bootstrap_ntp(&mut self) -> Result<(), NtpError> {
        if self.ntp_bootstrapped {
            return Ok(());
        }
        for server in &self.config.ntp_servers {
            match ntp_sync(server) {
                Ok(offset) => {
                    self.network_offset_ms = offset;
                    self.network_confidence = 0.9;
                    self.clock_source = ClockSource::NtpBootstrap;
                    self.ntp_bootstrapped = true;
                    self.last_sync_time = Timestamp::now();
                    log::info!("NTP bootstrap: offset={}ms from {}", offset, server);
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("NTP sync failed with {}: {}", server, e);
                }
            }
        }
        Err(NtpError::AllServersFailed)
    }

    /// Process a time sync gossip message from a peer.
    /// 
    /// Protocol (PTSP - Polygone Time Sync Protocol):
    /// 1. Peer sends: { t1: their_send_time, peer_id }
    /// 2. We receive at t2 (our clock)
    /// 3. We reply immediately with { t1, t2, t3: our_send_time }
    /// 4. Peer receives at t4, computes offset = ((t2-t1) + (t3-t4))/2
    ///    RTT = (t4-t1) - (t3-t2)
    pub fn process_gossip(
        &mut self,
        peer_id: PeerId,
        their_send_time: Timestamp,
        our_receive_time: Timestamp,
    ) -> Option<Timestamp> {
        let our_send_time = Timestamp::now();
        
        // We'll reply with our_send_time so peer can compute
        // For now, we compute our side estimate
        let rtt = our_send_time.saturating_sub(their_send_time);
        if rtt > self.config.max_rtt_ms as i64 {
            return None; // Peer too far
        }

        // Simple offset estimate: (their_time - our_time) at midpoint
        // More accurate would use the 4-timestamp protocol
        let offset = their_send_time.0 - our_receive_time.0;

        // Update peer state
        let state = self.peer_states.entry(peer_id).or_insert_with(|| PeerTimeState::new(peer_id));
        state.rtt_ms = rtt as u64;
        state.last_sync = our_receive_time;
        state.sample_count += 1;

        // Update per-peer median filter
        let filter = self.peer_filters.entry(peer_id).or_insert_with(|| {
            MedianFilter::new(MedianFilterConfig {
                window_size: 7,
                min_samples: 3,
            })
        });
        filter.add(offset);

        // If peer has enough samples, update consensus
        if let Some((median_offset, confidence)) = filter.median_with_confidence() {
            state.offset_ms = median_offset;
            state.confidence = confidence;
            
            // Compute variance
            let samples: Vec<i64> = filter.samples.iter().copied().collect();
            if samples.len() >= 3 {
                let mean = samples.iter().sum::<i64>() as f64 / samples.len() as f64;
                let var = samples.iter()
                    .map(|&x| (x as f64 - mean).powi(2))
                    .sum::<f64>() / samples.len() as f64;
                state.offset_variance = var;
            }

            // Add to global consensus filter (weighted by inverse RTT)
            let weight = 1.0 / (state.rtt_ms as f64 + 1.0);
            self.consensus_filter.add(median_offset, weight * confidence);

            // Recompute network consensus
            self.recompute_consensus();
        }

        Some(our_send_time)
    }

    fn recompute_consensus(&mut self) {
        if let Some((median, confidence)) = self.consensus_filter.weighted_median() {
            // Count reliable peers
            let reliable_peers = self.peer_states.values()
                .filter(|s| s.is_reliable(&self.config))
                .count();

            if reliable_peers >= self.config.min_peers && confidence >= self.config.confidence_threshold {
                self.network_offset_ms = median;
                self.network_confidence = confidence;
                self.clock_source = ClockSource::PeerConsensus;
                self.last_sync_time = Timestamp::now();
            }
        }
    }

    /// Called periodically (e.g., every 5s). Returns clock correction to apply (ms).
    /// Correction is capped at max_correction_per_tick_ms for stability.
    pub fn tick(&mut self) -> Option<i64> {
        // Age out stale peers
        let now = Timestamp::now();
        let max_age = self.config.gossip_interval_ms * 3;
        self.peer_states.retain(|_, s| s.is_fresh(max_age));
        
        // Recompute if peers changed
        self.recompute_consensus();

        // Compute correction toward network offset
        let current_offset = self.network_offset_ms;
        let correction = current_offset.saturating_sub(self.recent_corrections.back().copied().unwrap_or(0));
        
        let capped = correction.clamp(
            -(self.config.max_correction_per_tick_ms as i64),
            self.config.max_correction_per_tick_ms as i64
        );

        if capped != 0 {
            self.recent_corrections.push_back(capped);
            if self.recent_corrections.len() > 100 {
                self.recent_corrections.pop_front();
            }
            Some(capped)
        } else {
            None
        }
    }

    /// Get current synchronized time (system time + network offset)
    pub fn now(&self) -> Timestamp {
        Timestamp::now().saturating_add(self.network_offset_ms)
    }

    /// Get sync statistics
    pub fn stats(&self) -> SyncStats {
        let reliable_peers: Vec<_> = self.peer_states.values()
            .filter(|s| s.is_reliable(&self.config))
            .collect();
        
        let median_rtt = if reliable_peers.is_empty() {
            0
        } else {
            let mut rtts: Vec<u64> = reliable_peers.iter().map(|s| s.rtt_ms).collect();
            rtts.sort_unstable();
            rtts[rtts.len() / 2]
        };

        SyncStats {
            network_offset_ms: self.network_offset_ms,
            network_confidence: self.network_confidence,
            peer_count: reliable_peers.len(),
            median_rtt_ms: median_rtt,
            is_synced: self.network_confidence >= self.config.confidence_threshold 
                && self.peer_count() >= self.config.min_peers,
            time_since_sync_ms: Timestamp::now().saturating_sub(self.last_sync_time) as u64,
            clock_source: self.clock_source,
        }
    }

    pub fn peer_count(&self) -> usize {
        self.peer_states.values().filter(|s| s.is_reliable(&self.config)).count()
    }

    /// Get best peers for low-latency routing
    pub fn best_peers(&self, count: usize) -> Vec<PeerId> {
        let mut peers: Vec<_> = self.peer_states.values()
            .filter(|s| s.is_reliable(&self.config))
            .collect();
        peers.sort_by_key(|s| s.rtt_ms);
        peers.into_iter().take(count).map(|s| s.peer_id).collect()
    }

    /// Manual NTP fallback
    pub fn ntp_fallback(&mut self) -> Result<(), NtpError> {
        if !self.config.ntp_fallback {
            return Err(NtpError::FallbackDisabled);
        }
        self.bootstrap_ntp()?;
        self.clock_source = ClockSource::NtpFallback;
        Ok(())
    }
}

/// Simple NTP client (blocking, no deps)
fn ntp_sync(server: &str) -> Result<i64, NtpError> {
    use std::net::UdpSocket;
    use std::time::Duration;

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    
    let addr = format!("{}:123", server).parse().map_err(|_| NtpError::InvalidAddress)?;
    
    // NTP packet: LI=0, VN=4, Mode=3 (client)
    let mut packet = [0u8; 48];
    packet[0] = 0x1B; // 00 011 011
    
    let t1 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    socket.send_to(&packet, addr)?;
    socket.recv_from(&mut packet)?;
    let t4 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

    // Parse response
    // Transmit timestamp at offset 40 (10 * 4 bytes)
    let tx_secs = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]) as u64;
    let tx_frac = u32::from_be_bytes([packet[44], packet[45], packet[46], packet[47]]) as u64;
    let t3 = tx_secs * 1000 + (tx_frac * 1000) / 0x100000000;

    // Originate timestamp at offset 24
    let orig_secs = u32::from_be_bytes([packet[24], packet[25], packet[26], packet[27]]) as u64;
    let orig_frac = u32::from_be_bytes([packet[28], packet[29], packet[30], packet[31]]) as u64;
    let t2 = orig_secs * 1000 + (orig_frac * 1000) / 0x100000000;

    // Offset = ((t2 - t1) + (t3 - t4)) / 2
    let offset = ((t2 as i64 - t1 as i64) + (t3 as i64 - t4 as i64)) / 2;
    
    Ok(offset)
}

#[derive(Debug, thiserror::Error)]
pub enum NtpError {
    #[error("All NTP servers failed")]
    AllServersFailed,
    #[error("Invalid server address")]
    InvalidAddress,
    #[error("NTP fallback disabled")]
    FallbackDisabled,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_creation() {
        let config = SyncConfig::default();
        let engine = TimeSyncEngine::new(config);
        assert_eq!(engine.clock_source, ClockSource::Unsynchronized);
    }

    #[test]
    fn process_gossip_updates_peer() {
        let config = SyncConfig { max_rtt_ms: 1000, ..Default::default() };
        let mut engine = TimeSyncEngine::new(config);
        let peer = PeerId([1u8; 32]);
        
        let their_time = Timestamp(1000000);
        let our_time = Timestamp(1000050); // 50ms later
        
        engine.process_gossip(peer, their_time, our_time);
        
        let state = engine.peer_states.get(&peer).unwrap();
        assert_eq!(state.sample_count, 1);
        assert!(state.rtt_ms > 0);
    }
}