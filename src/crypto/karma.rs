use serde::{Deserialize, Serialize};
use crate::crypto::sign::{SignPublicKey, Signature, SignSecretKey};
use crate::Result;
use std::time::{SystemTime, UNIX_EPOCH};

/// A signed proof of work/contribution in the Polygone network.
/// Requesters sign these for workers to acknowledge service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkVoucher {
    /// PeerId of the node that performed the work (The Worker)
    pub worker_id: String,
    /// PeerId of the node that requested the service (The Requester)
    pub requester_id: String,
    /// Number of service units (shards relayed, inference steps, etc.)
    pub units: u64,
    /// Unix timestamp of issuance
    pub issued_at: u64,
    /// The ML-DSA-65 signature of the above data by the Requester
    pub signature: Signature,
}

impl WorkVoucher {
    /// Create and sign a new voucher.
    pub fn new(
        worker_id: String,
        requester_id: String,
        units: u64,
        secret_key: &SignSecretKey,
    ) -> Result<Self> {
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Data to sign: worker + requester + units + timestamp
        let mut data = Vec::new();
        data.extend_from_slice(worker_id.as_bytes());
        data.extend_from_slice(requester_id.as_bytes());
        data.extend_from_slice(&units.to_le_bytes());
        data.extend_from_slice(&issued_at.to_le_bytes());

        let signature = crate::crypto::sign::sign(secret_key, &data);

        Ok(Self {
            worker_id,
            requester_id,
            units,
            issued_at,
            signature,
        })
    }

    /// Verify the voucher's authenticity using the Requester's public key.
    pub fn verify(&self, public_key: &SignPublicKey) -> Result<bool> {
        let mut data = Vec::new();
        data.extend_from_slice(self.worker_id.as_bytes());
        data.extend_from_slice(self.requester_id.as_bytes());
        data.extend_from_slice(&self.units.to_le_bytes());
        data.extend_from_slice(&self.issued_at.to_le_bytes());

        crate::crypto::sign::verify(public_key, &data, &self.signature)?;
        Ok(true)
    }
}

/// A simple local database for WorkVouchers.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct KarmaStore {
    pub vouchers: Vec<WorkVoucher>,
}

impl KarmaStore {
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read(path)?;
        let store = bincode::deserialize(&data).map_err(|e| crate::crypto::error::PolygoneError::Serialization(e.to_string()))?;
        Ok(store)
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = bincode::serialize(self).map_err(|e| crate::crypto::error::PolygoneError::Serialization(e.to_string()))?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn total_units(&self) -> u64 {
        self.vouchers.iter().map(|v| v.units).sum()
    }
}

/// Detects if the system is idle based on CPU load.
pub struct IdleMonitor;

impl IdleMonitor {
    /// Returns true if the system load is below the threshold.
    /// On Linux, checks /proc/loadavg.
    pub fn is_idle(threshold: f32) -> bool {
        if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(first_val) = loadavg.split_whitespace().next() {
                if let Ok(val) = first_val.parse::<f32>() {
                    return val < threshold;
                }
            }
        }
        false // Default to not idle if we can't check
    }
}
