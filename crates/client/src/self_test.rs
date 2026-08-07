//! self_test — the `polygone test` command: real crypto, real assertions.
//!
//! Runs the five post-quantum primitives end to end and reports honestly.
//! Exit code 0 = all green. Any failure = non-zero and explicit.

use anyhow::Result;
use polygone_core::crypto::{kem, shamir, symmetric, SharedSecret};
use polygone_core::sign;

pub fn run() -> Result<()> {
    println!("⬡ POLYGONE — self-test\n");

    let mut passed = 0;
    let total = 7;

    // 1. ML-KEM-1024 round-trip
    let (pk, sk) = kem::generate_keypair()?;
    let (ct, ss1) = kem::encapsulate(&pk)?;
    let ss2 = kem::decapsulate(&sk, &ct)?;
    if ss1 == ss2 {
        println!("  [1/{total}] ML-KEM-1024 round-trip ........... ✓");
        passed += 1;
    } else {
        println!("  [1/{total}] ML-KEM-1024 round-trip ........... ✖");
    }

    // 2. AES-256-GCM encrypt/decrypt + tag
    let key = symmetric::SessionKey::from_bytes([0x42; 32]);
    let cipher = symmetric::encrypt(b"polygone est ephemeral", &key)?;
    match symmetric::decrypt(&cipher, &key) {
        Ok(plain) if plain == b"polygone est ephemeral" => {
            println!("  [2/{total}] AES-256-GCM round-trip ............. ✓");
            passed += 1;
        }
        _ => println!("  [2/{total}] AES-256-GCM round-trip ............. ✖"),
    }

    // 3. BLAKE3 domain-separated KDF
    let secret = SharedSecret([0xAB; 32]);
    let (topo, session) = secret.derive();
    if topo != session {
        println!("  [3/{total}] BLAKE3 KDF domain-séparé ............ ✓");
        passed += 1;
    } else {
        println!("  [3/{total}] BLAKE3 KDF domain-séparé ............ ✖");
    }

    // 4. Shamir 4-of-7 reconstruction
    let shares = shamir::split(b"post-quantum threshold secret", 4, 7)?;
    match shamir::reconstruct(&shares[..4], 4) {
        Ok(recovered) if recovered == b"post-quantum threshold secret" => {
            println!("  [4/{total}] Shamir 4-of-7 ........................ ✓");
            passed += 1;
        }
        _ => println!("  [4/{total}] Shamir 4-of-7 ........................ ✖"),
    }

    // 5. Shamir 3/7 -> nothing (the threshold is real)
    if shamir::reconstruct(&shares[..3], 4).is_err() {
        println!("  [5/{total}] Shamir 3/7 → rien ..................... ✓");
        passed += 1;
    } else {
        println!("  [5/{total}] Shamir 3/7 → rien ..................... ✖");
    }

    // 6. ML-DSA-65 sign + verify
    let kp = sign::generate_keypair()?;
    let msg = b"polygone handshake";
    let sig = kp.signer.sign(msg);
    if kp.verifier.verify(msg, &sig) && sig.as_bytes().len() == 3309 {
        println!("  [6/{total}] ML-DSA-65 sign + verify .............. ✓");
        passed += 1;
    } else {
        println!("  [5/{total}] ML-DSA-65 sign + verify .............. ✖");
    }

    // 7. ML-DSA tamper detection
    if !kp.verifier.verify(b"tampered", &sig) {
        println!("  [7/{total}] ML-DSA-65 tamper détecté ............. ✓");
        passed += 1;
    } else {
        println!("  [7/{total}] ML-DSA-65 tamper détecté ............. ✖");
    }

    println!();
    if passed == 7 {
        println!("  {passed}/7 tests — tout est vert. ✓");
        println!("  L'information n'existe pas. Elle traverse.");
        Ok(())
    } else {
        anyhow::bail!("{passed}/7 tests — échec. Ne pas utiliser en production.");
    }
}
