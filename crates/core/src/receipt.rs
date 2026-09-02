//! Phase 8+ — Verifiable receipts (ML-DSA-65).
//!
//! Minimal, non-invasive addition:
//! - `Receipt` = sender || receiver || ts || payload_hash
//! - sign/verify via existing `crate::sign::{Signer,Verifier,Signature}`

use crate::{NodeId, Signer, Verifier};
use blake3::Hasher;
use pqcrypto_traits::sign::SignedMessage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub sender: NodeId,
    pub receiver: NodeId,
    pub ts: u64,
    pub payload_hash: [u8; 32],
    pub signature: Vec<u8>,
}

impl Receipt {
    /// Build a receipt payload bytes (unsigned).
    fn payload_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + 16 + 8 + 32);
        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(self.receiver.as_bytes());
        buf.extend_from_slice(&self.ts.to_be_bytes());
        buf.extend_from_slice(&self.payload_hash);
        buf
    }

    /// Sign this receipt.
    pub fn sign(&mut self, signer: &Signer) {
        let msg = self.payload_bytes();
        let signed = pqcrypto_mldsa::mldsa65::sign(&msg, signer.secret_key().inner());
        let sig_len = signed.as_bytes().len() - msg.len();
        self.signature = signed.as_bytes()[..sig_len].to_vec();
    }

    /// Verify ML-DSA-65 signature.
    pub fn verify(&self, verifier: &Verifier) -> bool {
        let msg = self.payload_bytes();
        let mut combined = self.signature.clone();
        combined.extend_from_slice(&msg);
        match pqcrypto_mldsa::mldsa65::SignedMessage::from_bytes(&combined) {
            Ok(signed_msg) => {
                pqcrypto_mldsa::mldsa65::open(&signed_msg, verifier.public_key().inner()).is_ok()
            }
            Err(_) => false,
        }
    }

    /// Compute payload hash from raw bytes.
    pub fn hash_payload(payload: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(payload);
        *hasher.finalize().as_bytes()
    }
}
