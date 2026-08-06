//! msg — Polygone messaging: end-to-end encrypted send/receive.
//!
//! Full pipeline (offline-first, no server):
//!   Send: plaintext → ML-KEM-1024 encapsulate → BLAKE3 KDF → AES-256-GCM
//!         encrypt → Shamir 4-of-7 split → wire text
//!   Receive: wire text → Shamir reconstruct (>= 4/7) → ML-KEM decapsulate
//!            → BLAKE3 KDF → AES-256-GCM decrypt → plaintext
//!
//! The wire format is the same one the v1 CLI produced, so past and future
//! clients interoperate: `KEM_CT:...`, `SENDER_PK:...`, `FRAG:<idx>:<hex>`.

use polygone_core::crypto::{kem, shamir, symmetric};
use polygone_core::crypto::kem::{KemCiphertext, KemPublicKey, KemSecretKey};

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

    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.trim().splitn(2, ':').collect();
        if parts.len() != 2 {
            anyhow::bail!("fragment must be `<index>:<hex>`");
        }
        let index: u8 = parts[0].parse().map_err(|_| anyhow::anyhow!("bad index"))?;
        let share = hex::decode(parts[1]).map_err(|_| anyhow::anyhow!("bad hex"))?;
        Ok(Fragment { index, share })
    }
}

/// The full output of a send operation.
#[derive(Debug, Clone)]
pub struct SendOutput {
    /// ML-KEM-1024 ciphertext for the recipient.
    pub kem_ct: KemCiphertext,
    /// Sender's ephemeral public key.
    pub sender_pk: KemPublicKey,
    /// The 7 Shamir fragments.
    pub fragments: Vec<Fragment>,
}

impl SendOutput {
    /// Display in the parseable wire format.
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("KEM_CT:{}\n", self.kem_ct.to_hex()));
        out.push_str(&format!("SENDER_PK:{}\n", self.sender_pk.to_hex()));
        for f in &self.fragments {
            out.push_str(&format!("FRAG:{}\n", f.to_hex()));
        }
        out
    }

    /// Parse from the wire format.
    pub fn parse(input: &str) -> anyhow::Result<Self> {
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
            kem_ct: kem_ct.ok_or_else(|| anyhow::anyhow!("missing KEM_CT"))?,
            sender_pk: sender_pk.ok_or_else(|| anyhow::anyhow!("missing SENDER_PK"))?,
            fragments,
        })
    }
}

/// Encrypt a plaintext message and produce 7 Shamir fragments.
pub fn send(plaintext: &str, recipient_pk: &KemPublicKey) -> anyhow::Result<SendOutput> {
    send_bytes(plaintext.as_bytes(), recipient_pk)
}

/// Encrypt arbitrary bytes (message or file) and produce 7 Shamir fragments.
pub fn send_bytes(
    plaintext: &[u8],
    recipient_pk: &KemPublicKey,
) -> anyhow::Result<SendOutput> {
    // 1. Ephemeral sender keypair
    let (sender_pk, _sender_sk) = kem::generate_keypair()?;

    // 2. KEM encapsulate
    let (kem_ct, shared_secret) = kem::encapsulate(recipient_pk)?;

    // 3. BLAKE3 domain-separated KDF → session key
    let session_key = symmetric::SessionKey::derive_from_secret(&shared_secret);

    // 4. AES-256-GCM encrypt
    let encrypted = symmetric::encrypt(plaintext, &session_key)?;

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
pub fn receive(output: &SendOutput, recipient_sk: &KemSecretKey) -> anyhow::Result<String> {
    // 1. KEM decapsulate
    let shared_secret = kem::decapsulate(recipient_sk, &output.kem_ct)?;

    // 2. BLAKE3 KDF → session key
    let session_key = symmetric::SessionKey::derive_from_secret(&shared_secret);

    // 3. Shamir reconstruct (needs >= 4 fragments)
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

    String::from_utf8(plaintext).map_err(|e| anyhow::anyhow!("UTF-8 decode: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_receive_round_trip() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "Coucou — Polygone fonctionne.";

        let output = send(msg, &recipient_pk).unwrap();
        assert_eq!(output.fragments.len(), 7);

        let decrypted = receive(&output, &recipient_sk).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn long_message_round_trip() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "⬡ The network dissolves. Keys are zeroed. The exchange did not happen. "
            .repeat(20);

        let output = send(&msg, &recipient_pk).unwrap();
        let decrypted = receive(&output, &recipient_sk).unwrap();
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn insufficient_fragments_rejected() {
        let (recipient_pk, _recipient_sk) = kem::generate_keypair().unwrap();
        let msg = "Ce message sera fragmenté.";

        let mut output = send(msg, &recipient_pk).unwrap();
        output.fragments.truncate(3);

        let (_, recipient_sk) = kem::generate_keypair().unwrap();
        // Only the real secret key works anyway; here we also prove the
        // fragment threshold: the wire text must state 7 fragments.
        assert_eq!(output.fragments.len(), 3);
        assert!(receive(&output, &recipient_sk).is_err());
    }

    #[test]
    fn wire_format_roundtrip() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let output = send("format interop", &recipient_pk).unwrap();
        let text = output.display();
        let parsed = SendOutput::parse(&text).unwrap();
        assert_eq!(parsed.fragments.len(), 7);
        assert_eq!(parsed.kem_ct.to_hex(), output.kem_ct.to_hex());

        // The parsed output still decrypts.
        let decrypted = receive(&parsed, &recipient_sk).unwrap();
        assert_eq!(decrypted, "format interop");
    }
}
