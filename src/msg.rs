//! msg — Polygone messaging: end-to-end encrypted send/receive.
//!
//! Full pipeline:
//!   Send: plaintext → AES-256-GCM encrypt → Shamir 4-of-7 split → hex output
//!   Receive: hex fragments → Shamir reconstruct → AES-256-GCM decrypt → plaintext
//!
//! All key exchange goes through ML-KEM-1024.

use crate::crypto::kem::{self, KemCiphertext, KemPublicKey, KemSecretKey};
use crate::crypto::shamir;
use crate::crypto::symmetric::{self, SessionKey};
use crate::{PolygoneError, Result};

/// A user-facing message fragment in hex format.
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Fragment index (1-based, 1..=7)
    pub index: u8,
    /// Shamir share data
    pub share: Vec<u8>,
}

impl Fragment {
    pub fn to_hex(&self) -> String {
        format!("{:02x}:{}", self.index, hex::encode(&self.share))
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.trim().splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(PolygoneError::ParseFragment);
        }
        let index: u8 = parts[0]
            .parse()
            .map_err(|_| PolygoneError::ParseFragment)?;
        let share = hex::decode(parts[1])
            .map_err(|_| PolygoneError::ParseFragment)?;
        Ok(Fragment { index, share })
    }
}

/// The full output of a send operation.
#[derive(Debug, Clone)]
pub struct SendOutput {
    /// ML-KEM-1024 ciphertext for the recipient
    pub kem_ct: KemCiphertext,
    /// Sender's public key
    pub sender_pk: KemPublicKey,
    /// The 7 Shamir fragments
    pub fragments: Vec<Fragment>,
}

impl SendOutput {
    /// Display in a parseable text format.
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("KEM_CT:{}\n", self.kem_ct.to_hex()));
        out.push_str(&format!("SENDER_PK:{}\n", self.sender_pk.to_hex()));
        for f in &self.fragments {
            out.push_str(&format!("FRAG:{}\n", f.to_hex()));
        }
        out
    }

    /// Parse from the display format.
    pub fn parse(input: &str) -> Result<Self> {
        let mut kem_ct: Option<KemCiphertext> = None;
        let mut sender_pk: Option<KemPublicKey> = None;
        let mut fragments: Vec<Fragment> = Vec::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("KEM_CT:") {
                kem_ct = Some(KemCiphertext::from_hex(rest)?);
            } else if let Some(rest) = line.strip_prefix("SENDER_PK:") {
                sender_pk = Some(KemPublicKey::from_hex(rest)?);
            } else if let Some(rest) = line.strip_prefix("FRAG:") {
                fragments.push(Fragment::from_hex(rest)?);
            }
        }

        Ok(SendOutput {
            kem_ct: kem_ct.ok_or(PolygoneError::MissingField("KEM_CT"))?,
            sender_pk: sender_pk.ok_or(PolygoneError::MissingField("SENDER_PK"))?,
            fragments,
        })
    }
}

/// Encrypt a plaintext message and produce 7 Shamir fragments.
///
/// 1. Generate ephemeral KEM keypair
/// 2. Encapsulate against recipient's public key → shared secret
/// 3. Derive session key from shared secret (BLAKE3 domain-separated)
/// 4. AES-256-GCM encrypt the plaintext
/// 5. Shamir 4-of-7 split the ciphertext
/// 6. Return KEM ciphertext + 7 fragments + sender's public key
pub fn send(
    plaintext: &str,
    recipient_pk: &KemPublicKey,
) -> Result<SendOutput> {
    // 1. Ephemeral sender keypair
    let (sender_pk, _sender_sk) = kem::generate_keypair()?;

    // 2. KEM encapsulate
    let (kem_ct, shared_secret) = kem::encapsulate(recipient_pk)?;

    // 3. Derive session key
    let session_key = SessionKey::derive_from_secret(&shared_secret);

    // 4. AES-256-GCM encrypt
    let encrypted = symmetric::encrypt(plaintext.as_bytes(), &session_key)?;

    // 5. Shamir 4-of-7 split
    let shares = shamir::split(&encrypted, 4, 7)?;

    let fragments: Vec<Fragment> = shares
        .into_iter()
        .map(|s| Fragment {
            index: s.id.0,
            share: s.data,
        })
        .collect();

    Ok(SendOutput {
        kem_ct,
        sender_pk,
        fragments,
    })
}

/// Decrypt a message from fragments using the recipient's secret key.
///
/// 1. KEM decapsulate → shared secret
/// 2. Derive session key
/// 3. Shamir reconstruct ciphertext (need ≥ 4 fragments)
/// 4. AES-256-GCM decrypt → plaintext
pub fn receive(
    output: &SendOutput,
    recipient_sk: &KemSecretKey,
) -> Result<String> {
    // 1. KEM decapsulate
    let shared_secret = kem::decapsulate(recipient_sk, &output.kem_ct)?;

    // 2. Derive session key
    let session_key = SessionKey::derive_from_secret(&shared_secret);

    // 3. Shamir reconstruct
    let frags: Vec<shamir::Fragment> = output
        .fragments
        .iter()
        .map(|f| shamir::Fragment {
            id: shamir::FragmentId(f.index),
            data: f.share.clone(),
        })
        .collect();
    let ciphertext = shamir::reconstruct(&frags, 4)?;

    // 4. AES-256-GCM decrypt
    let plaintext = symmetric::decrypt(&ciphertext, &session_key)?;

    String::from_utf8(plaintext)
        .map_err(|e| PolygoneError::Message(format!("UTF-8 decode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_receive_round_trip() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "Coucou Lévy — Polygone fonctionne.";

        let output = send(msg, &recipient_pk).unwrap();
        assert_eq!(output.fragments.len(), 7);

        let decrypted = receive(&output, &recipient_sk).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn send_receive_long_message() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "⬡ The network dissolves. Keys are zeroed. The exchange did not happen. "
            .repeat(20);  // ~1400 bytes

        let output = send(&msg, &recipient_pk).unwrap();
        let decrypted = receive(&output, &recipient_sk).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn insufficient_fragments_rejected() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "Ce message sera fragmenté.";

        let mut output = send(msg, &recipient_pk).unwrap();
        output.fragments.truncate(3);  // keep only 3 of 7

        let result = receive(&output, &recipient_sk);
        assert!(result.is_err());
    }

    #[test]
    fn fragment_hex_roundtrip() {
        let f = Fragment {
            index: 3,
            share: vec![0x42; 64],
        };
        let hex = f.to_hex();
        let f2 = Fragment::from_hex(&hex).unwrap();
        assert_eq!(f2.index, 3);
        assert_eq!(f2.share, vec![0x42; 64]);
    }

    #[test]
    fn send_output_display_parse() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let output = send("Test display/parse", &recipient_pk).unwrap();
        let text = output.display();
        let parsed = SendOutput::parse(&text).unwrap();
        assert_eq!(parsed.fragments.len(), 7);
        assert_eq!(parsed.kem_ct.to_hex(), output.kem_ct.to_hex());
    }

    #[test]
    fn alice_bob_full_flow() {
        // Alice generates her keypair
        let (alice_pk, alice_sk) = kem::generate_keypair().unwrap();

        // Bob sends a message to Alice
        let msg = "Alice, on se retrouve à 20h. — Bob";
        let output = send(msg, &alice_pk).unwrap();

        // Alice receives and decrypts
        let decrypted = receive(&output, &alice_sk).unwrap();
        assert_eq!(decrypted, msg);
    }
}
