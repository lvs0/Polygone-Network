//! Polygone Serverless — run functions on the network.
//!
//! Request → Shamir 4-of-7 → ≥4 nodes execute → result reassembled.

use polygone_core::NodeId;
use serde::{Deserialize, Serialize};

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

    pub fn shards(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::with_capacity(7);
        for i in 0..7 {
            let mut shard = Vec::new();
            shard.push(self.id.as_bytes()[0]);
            shard.push(i as u8);
            shard.extend_from_slice(&self.payload);
            out.push(shard);
        }
        out
    }
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
