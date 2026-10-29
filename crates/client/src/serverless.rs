//! Polygone Serverless — run functions on the network.
//!
//! Request → Shamir 4-of-7 → ≥4 nodes execute → result reassembled.

// `NodeId` n'est utilisé QUE par `ServerlessResult`, ci-dessous, qui est une
// ébauche non câblée (chantier 2b-13, Phase 0) : l'import suit la même feature.
#[cfg(feature = "experimental")]
use polygone_core::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerlessRequest {
    pub id: String,
    pub payload: Vec<u8>,
    pub threshold: u8,
    pub total: u8,
}

/// Ébauche NON CÂBLÉE : personne ne construit ni ne lit un `ServerlessResult`.
///
/// Le reste du module, lui, EST câblé (`polygone serverless` appelle
/// `ServerlessRequest::new` et `shards()` dans `main.rs`). On ne peut donc pas
/// mettre tout le fichier derrière la feature sans retirer une commande qui
/// existe : seule la partie morte y va, et `experimental` la compile pour qui
/// veut la finir.
#[cfg(feature = "experimental")]
#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[cfg(feature = "experimental")]
impl ServerlessResult {
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
