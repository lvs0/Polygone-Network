//! Polygone Serverless — run functions on the network.
//!
//! Request → Shamir 4-of-7 → ≥4 nodes execute → result reassembled.

use polygone_core::NodeId;
use polygone_core::crypto::shamir::{split, reconstruct, Fragment, FragmentId};
use serde::{Deserialize, Serialize};

use anyhow::{Result, anyhow};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerlessRequest {
    pub id: String,
    pub payload: Vec<u8>,
    pub threshold: u8,
    pub total: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ServerlessResult {
    pub request_id: String,
    pub result: Vec<u8>,
    pub nodes: Vec<NodeId>,
}

impl ServerlessRequest {
    pub fn new(payload: Vec<u8>) -> Self {
        let id = uuid_short();
        Self {
            id,
            payload,
            threshold: 4,
            total: 7,
        }
    }

    /// Split payload into Shamir 4-of-7 fragments using polygone_core (sharks crate).
    /// Returns 7 fragments, each as Vec<u8> with [index][data] wire format.
    pub fn shards(&self) -> Result<Vec<Vec<u8>>> {
        let fragments = split(&self.payload, self.threshold, self.total)
            .map_err(|e| anyhow!(e))?;
        Ok(fragments
            .into_iter()
            .map(|f| {
                let mut v = Vec::with_capacity(1 + f.data.len());
                v.push(f.id.0);
                v.extend_from_slice(&f.data);
                v
            })
            .collect())
    }
}

/// Reconstruct secret from ≥4 Shamir fragments (wire format: [index][data]).
#[allow(dead_code)]
pub fn reconstruct_from_shards(shards: &[Vec<u8>]) -> Result<Vec<u8>> {
    let fragments: Vec<Fragment> = shards
        .iter()
        .filter_map(|s| {
            if s.is_empty() {
                return None;
            }
            let id = FragmentId(s[0]);
            let data = s[1..].to_vec();
            Some(Fragment { id, data })
        })
        .collect();
    if fragments.len() < 4 {
        anyhow::bail!("need ≥4 shares");
    }
    reconstruct(&fragments, 4).map_err(|e| anyhow!(e))
}

impl ServerlessResult {
    #[allow(dead_code)]
    pub fn new(request_id: String, result: Vec<u8>, nodes: Vec<NodeId>) -> Self {
        Self {
            request_id,
            result,
            nodes,
        }
    }
}

fn uuid_short() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut s = String::new();
    for _ in 0..8 {
        s.push(char::from(b'A' + rng.gen_range(0..26)));
    }
    s
}
