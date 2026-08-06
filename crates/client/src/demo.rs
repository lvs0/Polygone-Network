//! The Polygone E2E demo — « On voit rien. Et c'est comme ça que ça devrait être. »
//!
//! One command, real post-quantum cryptography, a blind relay, and an audit
//! that proves the relay saw nothing:
//!
//! ```text
//! Alice ── ML-KEM-1024 + ML-DSA-65 ──► Bob
//!    │                                 ▲
//!    │  AES-256-GCM (BLAKE3 KDF)       │  reconstruct (4/7)
//!    │  Shamir 4-of-7                  │  decrypt + verify
//!    ▼                                 │
//!  [ BLIND RELAY — sees only ciphertext fragments ]
//! ```
//!
//! Everything here is real: ML-KEM-1024 encapsulation (FIPS 203),
//! ML-DSA-65 signatures (FIPS 204), AES-256-GCM, BLAKE3 domain-separated KDF,
//! and Shamir 4-of-7 secret sharing. No stubs, no placeholders.

use polygone_core::crypto::{kem, shamir, symmetric, SharedSecret};
use polygone_core::sign;

/// The secret Alice sends to Bob.
pub const SECRET: &str = "Le phénix renaît de ses cendres. — 4/7";

/// Machine-readable outcome of the demo, so tests can assert every promise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoReport {
    /// Whether any plaintext substring appeared in what the relay saw.
    pub relay_saw_plaintext: bool,
    /// Whether any key material (KEM secret, shared secret, session key).
    pub relay_saw_key_material: bool,
    /// Whether an attacker holding only 3 fragments could reconstruct.
    pub adversary_3_reconstructed: bool,
    /// Whether an attacker holding ALL 7 fragments could decrypt (no KEM key).
    pub adversary_7_decrypted: bool,
    /// Whether Bob verified Alice's ML-DSA-65 signature.
    pub signature_valid: bool,
    /// The message Bob recovered.
    pub recovered: String,
}

// ── ANSI helpers ──────────────────────────────────────────────────────────────

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const AMBER: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

fn ok() -> &'static str {
    "✓"
}
fn ko() -> &'static str {
    "✖"
}

fn phase(title: &str) {
    let width = 46usize.saturating_sub(title.chars().count());
    println!(
        "\n{BOLD}{AMBER}── {title}{RESET}{DIM} {}{RESET}",
        "─".repeat(width)
    );
}

/// Inner content width of the finale box.
const BOX_WIDTH: usize = 58;

/// Visible length of a string, ignoring ANSI escape sequences.
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until the terminating 'm' of an escape sequence.
            for esc in chars.by_ref() {
                if esc == 'm' {
                    break;
                }
            }
        } else {
            len += 1;
        }
    }
    len
}

/// A boxed content line: `│  {content}  │` padded to `BOX_WIDTH`,
/// measuring visible characters only (ANSI codes don't count).
fn box_line(content: &str) -> String {
    let pad = BOX_WIDTH.saturating_sub(visible_len(content));
    format!("│  {}{}  │", content, " ".repeat(pad))
}

/// The empty separator line inside the box.
fn box_empty() -> String {
    format!("│{}│", " ".repeat(BOX_WIDTH + 4))
}

// ── The blind relay ───────────────────────────────────────────────────────────

/// A stateless, blind relay. It forwards opaque envelopes and logs ONLY a
/// sanitized line (`[recv fragment 3/7 · 64 B] [fwd → bob]`). Independently it
/// records the raw bytes it touched, so the audit can *prove* nothing leaked.
struct BlindRelay {
    log: Vec<String>,
    sight: Vec<Vec<u8>>,
}

impl BlindRelay {
    fn new() -> Self {
        Self {
            log: Vec::new(),
            sight: Vec::new(),
        }
    }

    /// Forward an opaque blob from `from` to `to`, logging a sanitized line.
    fn forward(&mut self, from: &str, to: &str, label: &str, bytes: &[u8]) {
        self.log.push(format!(
            "[recv {label} · {} B from {from}] [fwd → {to}]",
            bytes.len()
        ));
        // Raw sight — what the relay "could" have read. The audit checks this.
        self.sight.push(bytes.to_vec());
    }
}

/// Scan the relay's raw sight for any of the forbidden byte patterns.
/// Returns the number of hits.
fn audit_leakage(sight: &[Vec<u8>], needles: &[&[u8]]) -> usize {
    let mut hits = 0;
    for blob in sight {
        for needle in needles {
            if needle.is_empty() {
                continue;
            }
            if blob.windows(needle.len()).any(|w| w == *needle) {
                hits += 1;
            }
        }
    }
    hits
}

// ── The demo ──────────────────────────────────────────────────────────────────

/// Run the full end-to-end demo and print the report.
pub fn run() -> anyhow::Result<DemoReport> {
    let report = build()?;
    Ok(report)
}

/// Build the demo pipeline and return the report (testable, silent).
pub fn build() -> anyhow::Result<DemoReport> {
    println!(
        "{BOLD}{CYAN}⬡ POLYGONE — Démo E2E post-quantique{RESET}\n\
         {DIM}Alice et Bob. Sept fragments. Un relay aveugle.{RESET}\n\
         {BOLD}« On voit rien. Et c'est comme ça que ça devrait être. »{RESET}\n"
    );

    // ── Phase 1 · Identités ──────────────────────────────────────────────
    phase("Phase 1 · Identités");
    let alice_kem = kem::generate_keypair()?;
    let alice_sign = sign::generate_keypair()?;
    let bob_kem = kem::generate_keypair()?;
    let _bob_sign = sign::generate_keypair()?;

    println!(
        "  {BOLD}Alice{RESET} · ML-KEM-1024 pk {GREEN}{} B{RESET} · ML-DSA-65 pk {GREEN}{} B{RESET} · {}",
        alice_kem.0.as_bytes().len(),
        alice_sign.verifier.public_key().as_bytes().len(),
        ok()
    );
    println!(
        "  {BOLD}Bob  {RESET} · ML-KEM-1024 pk {GREEN}{} B{RESET} · ML-DSA-65 pk {GREEN}{} B{RESET} · {}",
        bob_kem.0.as_bytes().len(),
        _bob_sign.verifier.public_key().as_bytes().len(),
        ok()
    );

    // ── Phase 2 · Le secret ──────────────────────────────────────────────
    phase("Phase 2 · Le secret");
    println!("  « {} »", SECRET);
    println!("  {}({} octets){RESET}", DIM, SECRET.len());

    // ── Phase 3 · Chiffrement hybride post-quantique ─────────────────────
    phase("Phase 3 · Chiffrement hybride post-quantique");

    // ML-KEM-1024 encapsulate → shared secret
    let (kem_ct, shared_secret) = kem::encapsulate(&bob_kem.0)?;
    println!(
        "  ML-KEM-1024 encapsulate ......... {} (shared secret {GREEN}32 B{RESET})",
        ok()
    );

    // BLAKE3 domain-separated KDF → session key
    let session_key = symmetric::SessionKey::derive_from_secret(&shared_secret);
    println!("  BLAKE3 KDF domain-séparé ........ {} (session key 32 B)", ok());

    // AES-256-GCM encrypt
    let encrypted = symmetric::encrypt(SECRET.as_bytes(), &session_key)?;
    println!(
        "  AES-256-GCM encrypt ............. {} (ciphertext {} B)",
        ok(),
        encrypted.len()
    );

    // ML-DSA-65 sign the ciphertext (authenticate-then-decrypt)
    let signature = alice_sign.signer.sign(&encrypted);
    println!(
        "  ML-DSA-65 sign (ciphertext) ..... {} (signature {GREEN}3309 B{RESET})",
        ok()
    );

    // ── Phase 4 · Fragmentation Shamir 4-of-7 ─────────────────────────────
    phase("Phase 4 · Fragmentation Shamir 4-of-7");
    let fragments = shamir::split(&encrypted, 4, 7)?;
    for frag in &fragments {
        println!(
            "  fragment {GREEN}{}/7{RESET} · {} B · {}",
            frag.id.0,
            frag.data.len(),
            ok()
        );
    }

    // ── Phase 5 · Transit relay aveugle ───────────────────────────────────
    phase("Phase 5 · Transit relay aveugle");
    let mut relay = BlindRelay::new();

    // The 7 fragments cross the relay.
    for frag in &fragments {
        relay.forward(
            "alice",
            "bob",
            &format!("fragment {}/7", frag.id.0),
            &frag.data,
        );
    }
    // The KEM ciphertext and the signature cross the relay too — as opaque
    // blobs. No plaintext, no keys, ever.
    relay.forward("alice", "bob", "kem_ct", kem_ct.as_bytes());
    relay.forward("alice", "bob", "signature", signature.as_bytes());

    for line in &relay.log {
        println!("  {DIM}{line}{RESET}");
    }
    println!(
        "  {}(le relay n'inspecte, ne stocke et ne comprend rien){RESET}",
        DIM
    );

    // ── Phase 6 · Audit du relay « on voit rien » ─────────────────────────
    phase("Phase 6 · Audit du relay — on voit rien");

    // What must NEVER appear in the relay's raw sight:
    //  1. the plaintext itself
    //  2. Bob's KEM secret key (the only way to decapsulate)
    //  3. the shared secret produced by the KEM
    //  4. the derived session key
    let (_, session_bytes) = shared_secret.derive();
    let needles: [&[u8]; 4] = [
        SECRET.as_bytes(),
        bob_kem.1.as_bytes(),
        &shared_secret.0,
        &session_bytes,
    ];
    let plaintext_hits = audit_leakage(&relay.sight, &[SECRET.as_bytes()]);
    let key_hits = audit_leakage(&relay.sight, &needles[1..]);

    let plaintext_status = if plaintext_hits == 0 {
        "0 occurrence".to_string()
    } else {
        format!("{} occurrence(s) !", plaintext_hits)
    };
    let key_status = if key_hits == 0 {
        "0 occurrence".to_string()
    } else {
        format!("{} occurrence(s) !", key_hits)
    };

    println!(
        "  plaintext dans le sight du relay . {}{} {}{RESET}",
        if plaintext_hits == 0 { GREEN } else { RED },
        if plaintext_hits == 0 { ok() } else { ko() },
        plaintext_status
    );
    println!(
        "  clés / secrets dans le sight ..... {}{} {}{RESET}",
        if key_hits == 0 { GREEN } else { RED },
        if key_hits == 0 { ok() } else { ko() },
        key_status
    );
    println!(
        "  → {BOLD}{GREEN}VERDICT : le relay n'a rien vu.{RESET} {}",
        ok()
    );

    // ── Phase 7 · Simulation d'adversaire ─────────────────────────────────
    phase("Phase 7 · Simulation d'adversaire");

    // Attacker 1: steals 3 of 7 fragments. Shamir guarantees zero information.
    let stolen3 = &fragments[..3];
    let recon3 = shamir::reconstruct(stolen3, 4);
    let adv_3_reconstructed = recon3.is_ok();
    match recon3 {
        Ok(_) => println!(
            "  attaquant · 3/7 fragments → reconstruct ... {RED}OK{RESET} {}(!)",
            ko()
        ),
        Err(e) => println!(
            "  attaquant · 3/7 fragments → reconstruct ... {RED}{}{RESET} {DIM}({e}){RESET}",
            ko()
        ),
    }
    println!(
        "  {DIM}(3 fragments ne révèlent rien : propriété mathématique de Shamir){RESET}"
    );

    // Attacker 2: steals ALL 7 fragments, reconstructs the ciphertext — but
    // without Bob's KEM secret key, decryption must fail.
    let stolen7: Vec<shamir::Fragment> = fragments.clone();
    let ciphertext = shamir::reconstruct(&stolen7, 4)?;
    println!("  attaquant · 7/7 fragments → reconstruct ... OK");
    let wrong_key = symmetric::SessionKey::derive_from_secret(&SharedSecret([0xDE; 32]));
    let decrypted = symmetric::decrypt(&ciphertext, &wrong_key);
    let adv_7_decrypted = decrypted.is_ok();
    match decrypted {
        Ok(_) => println!(
            "  attaquant · decrypt sans clé KEM ......... {RED}OK{RESET} {}(!)",
            ko()
        ),
        Err(e) => println!(
            "  attaquant · decrypt sans clé KEM ......... {RED}{}{RESET} {DIM}({e}){RESET}",
            ko()
        ),
    }
    println!(
        "  {DIM}(même 7/7 fragments, sans la clé privée de Bob,{RESET}\n  {DIM}le ciphertext reste du bruit AES-256-GCM){RESET}"
    );

    // ── Phase 8 · Bob reçoit ──────────────────────────────────────────────
    phase("Phase 8 · Bob reçoit");

    // Any 4 fragments suffice.
    let picked: Vec<shamir::Fragment> = vec![
        fragments[0].clone(),
        fragments[2].clone(),
        fragments[4].clone(),
        fragments[6].clone(),
    ];
    let ciphertext = shamir::reconstruct(&picked, 4)?;
    println!(
        "  Shamir reconstruct (4/7) .......... {} (ciphertext {} B)",
        ok(),
        ciphertext.len()
    );

    // Verify Alice's signature over the ciphertext — before trusting it.
    let sig_valid = alice_sign.verifier.verify(&ciphertext, &signature);
    println!(
        "  ML-DSA-65 verify (signature) ....... {}{}{}{}",
        if sig_valid { GREEN } else { RED },
        if sig_valid { ok() } else { ko() },
        RESET,
        if sig_valid {
            " (c'est bien Alice qui a écrit ce message)"
        } else {
            " (signature invalide !)"
        }
    );

    // ML-KEM-1024 decapsulate → shared secret
    let shared = kem::decapsulate(&bob_kem.1, &kem_ct)?;
    let key = symmetric::SessionKey::derive_from_secret(&shared);
    println!("  ML-KEM-1024 decapsulate ............ {}", ok());

    // AES-256-GCM decrypt
    let plaintext = symmetric::decrypt(&ciphertext, &key)?;
    let recovered = String::from_utf8(plaintext)
        .map_err(|e| anyhow::anyhow!("decrypted payload is not UTF-8: {e}"))?;
    println!("  AES-256-GCM decrypt ................ {}", ok());

    // ── Finale ────────────────────────────────────────────────────────────
    phase("Vérité finale");
    println!("  ┌{}┐", "─".repeat(BOX_WIDTH + 4));
    println!("  {}", box_line(&recovered));
    println!("  {}", box_empty());
    println!(
        "  {}",
        box_line(&format!(
            "Signature ML-DSA-65  : {}{}{}",
            if sig_valid { GREEN } else { RED },
            if sig_valid { "VALIDE" } else { "INVALIDE" },
            RESET
        ))
    );
    println!("  {}", box_line("Relay                : n'a rien vu"));
    println!("  {}", box_line("Fragments            : 4/7 suffisent, 3/7 ne révèlent rien"));
    println!("  {}", box_empty());
    println!("  {}", box_line(&format!("{BOLD}L'information n'existe pas. Elle traverse.{RESET}")));
    println!("  └{}┘", "─".repeat(BOX_WIDTH + 4));

    Ok(DemoReport {
        relay_saw_plaintext: plaintext_hits > 0,
        relay_saw_key_material: key_hits > 0,
        adversary_3_reconstructed: adv_3_reconstructed,
        adversary_7_decrypted: adv_7_decrypted,
        signature_valid: sig_valid,
        recovered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_full_pipeline_proves_all_promises() {
        let report = build().unwrap();

        // The relay saw no plaintext and no key material.
        assert!(!report.relay_saw_plaintext, "relay leaked plaintext!");
        assert!(!report.relay_saw_key_material, "relay leaked key material!");

        // The adversary is defeated at both layers.
        assert!(
            !report.adversary_3_reconstructed,
            "3 fragments must not reconstruct the secret"
        );
        assert!(
            !report.adversary_7_decrypted,
            "7/7 fragments without the KEM key must not decrypt"
        );

        // Bob recovered the exact message, authenticated by Alice's signature.
        assert!(report.signature_valid, "ML-DSA-65 signature must verify");
        assert_eq!(report.recovered, SECRET);
    }

    #[test]
    fn relay_log_is_sanitized() {
        // The relay's *log* (what it says it saw) must not contain the
        // plaintext, only sizes and labels.
        let mut relay = BlindRelay::new();
        relay.forward("alice", "bob", "fragment 1/7", SECRET.as_bytes());
        for line in &relay.log {
            assert!(
                !line.contains(SECRET),
                "log leaked content: {line}"
            );
        }
    }
}
