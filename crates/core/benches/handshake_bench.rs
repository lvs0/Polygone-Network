//! ML-DSA-65 handshake benchmark: keygen / sign / verify breakdown, using the
//! same `polygone_core::sign` API the handshake path uses.
//! Target: sign+verify <= 200 µs per handshake (D2 decision gate).
//!
//! Honest reporting: the D2 gate was set at <= 200 µs total. This bench
//! prints the per-step numbers so the gate can be evaluated with data.

use polygone_core::sign;
use std::hint::black_box;

const ITER: usize = 1000;

/// Time `f` over `ITER` iterations, returning the average in microseconds.
fn avg_us(mut f: impl FnMut()) -> f64 {
    let mut total_ns = 0u128;
    for _ in 0..ITER {
        let start = std::time::Instant::now();
        f();
        total_ns += start.elapsed().as_nanos();
    }
    total_ns as f64 / ITER as f64 / 1_000.0
}

fn main() {
    // Warmup (avx2 init, page faults)
    for _ in 0..10 {
        let kp = sign::generate_keypair().unwrap();
        let sig = kp.signer.sign(b"polygone handshake benchmark");
        black_box(kp.verifier.verify(b"polygone handshake benchmark", &sig));
    }

    let msg = b"polygone handshake benchmark";

    let keygen_us = avg_us(|| {
        black_box(sign::generate_keypair().unwrap());
    });

    let kp = sign::generate_keypair().unwrap();
    let sign_us = avg_us(|| {
        black_box(kp.signer.sign(msg));
    });

    let sig = kp.signer.sign(msg);
    let verify_us = avg_us(|| {
        black_box(kp.verifier.verify(msg, &sig));
    });

    let total_us = sign_us + verify_us;
    let hps = 1_000_000.0 / total_us;

    println!("ITER={ITER}");
    println!("  keygen  : {keygen_us:8.3} µs");
    println!("  sign    : {sign_us:8.3} µs");
    println!("  verify  : {verify_us:8.3} µs");
    println!("  total   : {total_us:8.3} µs  (sign+verify, per handshake)");
    println!("  capacity: {hps:8.0} handshakes/sec/core");

    if total_us > 200.0 {
        eprintln!("D2 GATE FAILED: {total_us:.3} µs > 200 µs");
        println!("=> D2 status: KO on this hardware (269 µs-class, see DECISIONS.md)");
        std::process::exit(1);
    } else {
        println!("D2 GATE PASSED: {total_us:.3} µs <= 200 µs");
    }
}
