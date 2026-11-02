//! `polygone-bench` — reliability, speed and statistics for Polygone.
//!
//! Run it in release mode (the post-quantum primitives are much faster there):
//!
//! ```bash
//! cargo run --release -p polygone-bench            # full run
//! cargo run --release -p polygone-bench -- --quick # fast smoke run
//! ```
//!
//! It writes a `REPORT.md` next to the working directory and exits non-zero if
//! any reliability invariant is violated.

mod stats;

use polygone_core::{crypto, open, seal, shamir, Fragment, Identity, PolyError, SealOptions};
use rand::RngCore;
use stats::{chi_square_p_value, chi_square_uniform, mean, percentile, stddev};
use std::collections::HashSet;
use std::time::Instant;

#[derive(Default)]
struct Report {
    out: String,
    failures: usize,
}

impl Report {
    fn h1(&mut self, title: &str) {
        self.out.push_str(&format!("\n## {title}\n\n"));
    }
    fn h2(&mut self, title: &str) {
        self.out.push_str(&format!("\n### {title}\n\n"));
    }
    fn line(&mut self, text: impl AsRef<str>) {
        self.out.push_str(text.as_ref());
        self.out.push('\n');
    }
    /// Record a boolean invariant, returning it.
    fn check(&mut self, ok: bool, label: &str) -> bool {
        if ok {
            self.line(format!("- ✅ {label}"));
        } else {
            self.failures += 1;
            self.line(format!("- ❌ {label}"));
        }
        ok
    }
}

fn main() {
    let quick = std::env::args().any(|arg| arg == "--quick");
    let mut report = Report::default();
    let scope = if quick { "rapide" } else { "complet" };

    report.line("# Polygone — Rapport fiabilité / rapidité / statistiques");
    report.line("");
    report.line(format!(
        "- Crate `polygone-core` v{}",
        polygone_core::VERSION
    ));
    report.line(format!(
        "- Profil : {scope} ({})",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ));
    report.line(format!(
        "- Horodatage : {} (unix)",
        polygone_core::unix_now()
    ));
    report.line(format!("- {}", polygone_core::BANNER));
    if cfg!(debug_assertions) {
        report.line("");
        report.line(
            "> ⚠ Binaire en **debug** : les temps absolus sont pessimistes. Utiliser `--release`.",
        );
    }

    let primitives = check_primitives(&mut report);
    let speed = measure_speed(quick, &mut report);
    measure_reliability(quick, &mut report);
    measure_statistics(quick, &mut report);
    summarise(&mut report, primitives, &speed);

    print!("{}", report.out);
    if let Err(error) = std::fs::write("REPORT.md", &report.out) {
        eprintln!("(impossible d'écrire REPORT.md : {error})");
    } else {
        eprintln!("→ rapport écrit dans REPORT.md");
    }

    if report.failures > 0 {
        eprintln!("✗ {} invariant(s) de fiabilité violé(s)", report.failures);
        std::process::exit(1);
    }
}

struct PrimitiveSizes {
    kem_pk: usize,
    kem_ct: usize,
    sig_pk: usize,
    signature: usize,
}

fn check_primitives(report: &mut Report) -> PrimitiveSizes {
    report.h1("1. Invariants des primitives");
    let sizes = crypto::sizes();
    let roundtrip = (|| -> Result<bool, ()> {
        let (kem_pk, kem_sk, sig_pk, sig_sk) = crypto::keypair();
        let (ct, key_a) = crypto::encapsulate(&kem_pk).map_err(|_| ())?;
        let key_b = crypto::decapsulate(&kem_sk, &ct).map_err(|_| ())?;
        let sig = crypto::sign(b"x", &sig_sk).map_err(|_| ())?;
        crypto::verify(b"x", &sig, &sig_pk).map_err(|_| ())?;
        Ok(key_a == key_b)
    })()
    .unwrap_or(false);

    report.check(
        sizes.kem_pk == 1568 && sizes.kem_ct == 1568,
        "ML-KEM-1024 : clé publique et chiffré = 1568 octets",
    );
    report.check(
        sizes.kem_sk == 3168,
        "ML-KEM-1024 : clé secrète = 3168 octets",
    );
    report.check(
        sizes.sig_pk == 1952,
        "ML-DSA-65 : clé publique = 1952 octets",
    );
    report.check(
        sizes.signature == 3309,
        "ML-DSA-65 : signature = 3309 octets",
    );
    report.check(roundtrip, "encapsulation et signature font l'aller-retour");

    PrimitiveSizes {
        kem_pk: sizes.kem_pk,
        kem_ct: sizes.kem_ct,
        sig_pk: sizes.sig_pk,
        signature: sizes.signature,
    }
}

/// Run `f` `n` times, returning each duration in microseconds.
fn measure<F: FnMut()>(n: usize, mut f: F) -> Vec<f64> {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        f();
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    samples
}

fn human_us(micros: f64) -> String {
    if micros < 1_000.0 {
        format!("{micros:.1} µs")
    } else if micros < 1_000_000.0 {
        format!("{:.3} ms", micros / 1_000.0)
    } else {
        format!("{:.3} s", micros / 1_000_000.0)
    }
}

fn op_line(report: &mut Report, label: &str, n: usize, samples: &[f64]) {
    report.line(format!(
        "| {label} | {n} | {} | {} | {} | {} | {} |",
        human_us(mean(samples)),
        human_us(stddev(samples)),
        human_us(percentile(samples, 50.0)),
        human_us(percentile(samples, 95.0)),
        human_us(percentile(samples, 99.0)),
    ));
}

fn throughput_line(report: &mut Report, label: &str, bytes: usize, samples: &[f64]) {
    let seconds = mean(samples) / 1e6;
    let mib_per_s = (bytes as f64 / 1_048_576.0) / seconds;
    report.line(format!(
        "| {label} | {} | {} | {mib_per_s:.1} Mio/s |",
        human_bytes(bytes),
        human_us(mean(samples)),
    ));
}

fn human_bytes(bytes: usize) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} Mio", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} Kio", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} o")
    }
}

fn scaled(quick: bool, full: usize) -> usize {
    if quick {
        (full / 5).max(5)
    } else {
        full
    }
}

struct SpeedSummary {
    seal_us: f64,
    open_us: f64,
    e2e_us: f64,
}

fn measure_speed(quick: bool, report: &mut Report) -> SpeedSummary {
    report.h1("2. Rapidité");
    report.h2("Latence par opération");
    report.line("| Opération | n | moyenne | écart-type | p50 | p95 | p99 |");
    report.line("|---|---|---|---|---|---|---|");

    let keygen = measure(scaled(quick, 20), || {
        let _ = Identity::generate("bench");
    });
    op_line(
        report,
        "Génération d'identité (ML-KEM + ML-DSA)",
        keygen.len(),
        &keygen,
    );

    let identity = Identity::generate("bench-recipient");
    let (kem_pk, kem_sk, sig_pk, sig_sk) = crypto::keypair();

    let encaps = measure(scaled(quick, 200), || {
        let _ = crypto::encapsulate(&kem_pk);
    });
    op_line(report, "ML-KEM-1024 encapsulate", encaps.len(), &encaps);

    let (kem_ct, _) = crypto::encapsulate(&kem_pk).unwrap();
    let decaps = measure(scaled(quick, 200), || {
        let _ = crypto::decapsulate(&kem_sk, &kem_ct);
    });
    op_line(report, "ML-KEM-1024 decapsulate", decaps.len(), &decaps);

    let sign = measure(scaled(quick, 200), || {
        let _ = crypto::sign(b"benchmark message", &sig_sk);
    });
    op_line(report, "ML-DSA-65 sign", sign.len(), &sign);

    let signature = crypto::sign(b"benchmark message", &sig_sk).unwrap();
    let verify = measure(scaled(quick, 200), || {
        let _ = crypto::verify(b"benchmark message", &signature, &sig_pk);
    });
    op_line(report, "ML-DSA-65 verify", verify.len(), &verify);

    let small = [0u8; 256];
    let seal_samples = measure(scaled(quick, 200), || {
        let _ = seal(
            &small,
            &identity.kem_pk,
            &identity.recipient_tag(),
            &identity,
            SealOptions::default(),
        );
    });
    op_line(
        report,
        "seal (message 256 o → 7 fragments)",
        seal_samples.len(),
        &seal_samples,
    );

    let fragments = seal(
        &small,
        &identity.kem_pk,
        &identity.recipient_tag(),
        &identity,
        SealOptions::default(),
    )
    .unwrap();
    let open_samples = measure(scaled(quick, 200), || {
        let _ = open(&fragments[..4], &identity.kem_sk, polygone_core::unix_now());
    });
    op_line(
        report,
        "open (4 fragments → clair)",
        open_samples.len(),
        &open_samples,
    );

    report.h2("Débit symétrique et partage de secret");
    report.line("| Opération | taille | moyenne | débit |");
    report.line("|---|---|---|---|");

    let bulk = vec![0xABu8; 1_048_576];
    let key = [7u8; 32];
    let nonce = [3u8; 12];
    let encrypt = measure(scaled(quick, 20), || {
        let _ = crypto::aead_encrypt(&key, &nonce, &bulk);
    });
    throughput_line(report, "AES-256-GCM chiffrement", bulk.len(), &encrypt);

    let ciphertext = crypto::aead_encrypt(&key, &nonce, &bulk).unwrap();
    let decrypt = measure(scaled(quick, 20), || {
        let _ = crypto::aead_decrypt(&key, &nonce, &ciphertext);
    });
    throughput_line(report, "AES-256-GCM déchiffrement", bulk.len(), &decrypt);

    let secret = vec![0x5Au8; 262_144];
    let split = measure(scaled(quick, 20), || {
        let _ = shamir::split(&secret, 4, 7);
    });
    throughput_line(report, "Shamir split 4-of-7", secret.len(), &split);

    let shares = shamir::split(&secret, 4, 7).unwrap();
    let subset: Vec<(u8, Vec<u8>)> = (0..4).map(|i| ((i + 1) as u8, shares[i].clone())).collect();
    let reconstruct = measure(scaled(quick, 20), || {
        let _ = shamir::reconstruct(&subset);
    });
    throughput_line(
        report,
        "Shamir reconstruct 4-of-7",
        secret.len(),
        &reconstruct,
    );

    report.h2("Latence de bout en bout (seal → open)");
    let e2e = measure(scaled(quick, 500), || {
        let f = seal(
            &small,
            &identity.kem_pk,
            &identity.recipient_tag(),
            &identity,
            SealOptions::default(),
        )
        .unwrap();
        let _ = open(&f[..4], &identity.kem_sk, polygone_core::unix_now());
    });
    report.line(format!(
        "- moyenne {} · p50 {} · p95 {} · p99 {}",
        human_us(mean(&e2e)),
        human_us(percentile(&e2e, 50.0)),
        human_us(percentile(&e2e, 95.0)),
        human_us(percentile(&e2e, 99.0)),
    ));
    report.line(format!(
        "- débit applicatif ~{:.0} messages/s (1 message = 256 o)",
        1_000_000.0 / mean(&e2e)
    ));

    SpeedSummary {
        seal_us: mean(&seal_samples),
        open_us: mean(&open_samples),
        e2e_us: mean(&e2e),
    }
}

fn random_message(rng: &mut impl RngCore, max: usize) -> Vec<u8> {
    let mut length_byte = [0u8; 2];
    rng.fill_bytes(&mut length_byte);
    let length = (u16::from_le_bytes(length_byte) as usize % max).max(1);
    let mut message = vec![0u8; length];
    rng.fill_bytes(&mut message);
    message
}

fn measure_reliability(quick: bool, report: &mut Report) {
    report.h1("3. Fiabilité");
    let mut rng = rand::thread_rng();
    let alice = Identity::generate("alice");
    let bob = Identity::generate("bob");
    let eve = Identity::generate("eve");
    let now = polygone_core::unix_now();

    // --- Round-trips ---
    let n = scaled(quick, 500);
    let mut roundtrip_ok = 0usize;
    for _ in 0..n {
        let message = random_message(&mut rng, 512);
        let fragments = seal(
            &message,
            &bob.kem_pk,
            &bob.recipient_tag(),
            &alice,
            SealOptions::default(),
        )
        .unwrap();
        if open(&fragments[..4], &bob.kem_sk, now)
            .map(|out| out == message)
            .unwrap_or(false)
        {
            roundtrip_ok += 1;
        }
    }
    report.h2("Aller-retour");
    report.check(
        roundtrip_ok == n,
        &format!("{roundtrip_ok}/{n} messages reconstruits à l'identique (4 fragments sur 7)"),
    );

    // --- Fragment-loss tolerance: every 4-subset works, every 3-subset fails ---
    report.h2("Tolérance à la perte de fragments");
    let message = b"perte de fragments";
    let fragments = seal(
        message,
        &bob.kem_pk,
        &bob.recipient_tag(),
        &alice,
        SealOptions::default(),
    )
    .unwrap();

    let mut four_ok = 0;
    let mut four_total = 0;
    let mut three_refused = 0;
    let mut three_total = 0;
    for indices in subsets(7, 4) {
        four_total += 1;
        let group: Vec<Fragment> = indices.iter().map(|&i| fragments[i].clone()).collect();
        if open(&group, &bob.kem_sk, now)
            .map(|out| out == message)
            .unwrap_or(false)
        {
            four_ok += 1;
        }
    }
    for indices in subsets(7, 3) {
        three_total += 1;
        let group: Vec<Fragment> = indices.iter().map(|&i| fragments[i].clone()).collect();
        if matches!(
            open(&group, &bob.kem_sk, now),
            Err(PolyError::NotEnoughFragments { .. })
        ) {
            three_refused += 1;
        }
    }
    report.check(
        four_ok == four_total,
        &format!("{four_ok}/{four_total} sous-ensembles de 4 fragments reconstruisent"),
    );
    report.check(
        three_refused == three_total,
        &format!("{three_refused}/{three_total} sous-ensembles de 3 fragments sont refusés"),
    );

    // --- Tamper detection ---
    report.h2("Détection d'altération");
    let tamper_n = scaled(quick, 500);
    let mut tampers_caught = 0usize;
    for _ in 0..tamper_n {
        let message = random_message(&mut rng, 256);
        let mut fragments = seal(
            &message,
            &bob.kem_pk,
            &bob.recipient_tag(),
            &alice,
            SealOptions::default(),
        )
        .unwrap();
        let index = (rng_fill(&mut rng) as usize) % 4;
        let mut payload = polygone_core::from_b64(&fragments[index].payload).unwrap();
        let flip = rng_fill(&mut rng) as usize % payload.len();
        payload[flip] ^= 0xff;
        fragments[index].payload = polygone_core::to_b64(&payload);
        if open(&fragments[..4], &bob.kem_sk, now).is_err() {
            tampers_caught += 1;
        }
    }
    report.check(
        tampers_caught == tamper_n,
        &format!("{tampers_caught}/{tamper_n} fragments corrompus détectés"),
    );

    // --- Forgery / wrong recipient ---
    report.h2("Usurpation et mauvais destinataire");
    let forgery_n = scaled(quick, 200);
    let mut forgeries_rejected = 0usize;
    let mut wrong_recipient_rejected = 0usize;
    for _ in 0..forgery_n {
        let message = random_message(&mut rng, 128);
        let fragments = seal(
            &message,
            &bob.kem_pk,
            &bob.recipient_tag(),
            &alice,
            SealOptions::default(),
        )
        .unwrap();

        let mut forged = fragments.clone();
        forged[0].sender_sig_pk = eve.sig_pk_b64();
        if open(&forged[..4], &bob.kem_sk, now).is_err() {
            forgeries_rejected += 1;
        }

        if open(&fragments[..4], &eve.kem_sk, now).is_err() {
            wrong_recipient_rejected += 1;
        }
    }
    report.check(
        forgeries_rejected == forgery_n,
        &format!("{forgeries_rejected}/{forgery_n} signatures usurpées rejetées"),
    );
    report.check(
        wrong_recipient_rejected == forgery_n,
        &format!("{wrong_recipient_rejected}/{forgery_n} déchiffrements par un tiers refusés"),
    );

    // --- Expiry ---
    report.h2("Éphémérité (TTL)");
    let fragments = seal(
        message,
        &bob.kem_pk,
        &bob.recipient_tag(),
        &alice,
        SealOptions::default(),
    )
    .unwrap();
    let expired = open(
        &fragments[..4],
        &bob.kem_sk,
        fragments[0].created_at + fragments[0].ttl_secs + 1,
    );
    report.check(
        matches!(expired, Err(PolyError::Expired)),
        "un ensemble expiré est refusé",
    );
    let fresh = open(&fragments[..4], &bob.kem_sk, fragments[0].created_at + 1);
    report.check(fresh.is_ok(), "un ensemble frais est accepté");
}

fn rng_fill(rng: &mut impl RngCore) -> u8 {
    let mut byte = [0u8; 1];
    rng.fill_bytes(&mut byte);
    byte[0]
}

/// All index subsets of size `k` from `0..n`.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    fn recurse(
        start: usize,
        n: usize,
        k: usize,
        current: &mut Vec<usize>,
        result: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == k {
            result.push(current.clone());
            return;
        }
        for i in start..n {
            current.push(i);
            recurse(i + 1, n, k, current, result);
            current.pop();
        }
    }
    recurse(0, n, k, &mut current, &mut result);
    result
}

fn measure_statistics(quick: bool, report: &mut Report) {
    report.h1("4. Statistiques");
    let mut rng = rand::thread_rng();

    // --- Key uniqueness ---
    let keys = scaled(quick, 500);
    let mut seen = HashSet::new();
    let mut duplicates = 0;
    for _ in 0..keys {
        let identity = Identity::generate("stats");
        if !seen.insert(crypto::hash(&identity.kem_pk)) {
            duplicates += 1;
        }
    }
    report.h2("Unicité");
    report.line("| Test | n | doublons | verdict |");
    report.line("|---|---|---|---|");
    report.line(format!(
        "| Clés KEM générées distinctes | {keys} | {duplicates} | {} |",
        verdict(duplicates == 0)
    ));

    // --- Nonce / message id uniqueness and keystream freshness ---
    let seals = scaled(quick, 2000);
    let alice = Identity::generate("alice");
    let bob = Identity::generate("bob");
    let mut ids = HashSet::new();
    let mut nonces = HashSet::new();
    let mut kem_cts = HashSet::new();
    let mut duplicates = 0;
    for _ in 0..seals {
        let fragments = seal(
            b"same ciphertext each time?",
            &bob.kem_pk,
            &bob.recipient_tag(),
            &alice,
            SealOptions::default(),
        )
        .unwrap();
        if !ids.insert(fragments[0].message_id.clone()) {
            duplicates += 1;
        }
        if !nonces.insert(fragments[0].nonce.clone()) {
            duplicates += 1;
        }
        if !kem_cts.insert(fragments[0].kem_ct.clone()) {
            duplicates += 1;
        }
    }
    report.line(format!(
        "| message_id, nonces et kem_ct distincts (même message) | {seals} | {duplicates} | {} |",
        verdict(duplicates == 0)
    ));

    // --- Shamir shares look uniform ---
    report.h2("Uniformité du partage de secret");
    let secret = vec![0u8; 4096];
    let shares = shamir::split(&secret, 4, 7).unwrap();
    let mut counts = [0u64; 256];
    let mut total = 0u64;
    for share in &shares {
        for &byte in share {
            counts[byte as usize] += 1;
            total += 1;
        }
    }
    let chi = chi_square_uniform(&counts);
    let p = chi_square_p_value(chi, 255.0);
    report.line(format!(
        "- Octets observés : {total} (7 parts × 4096 octets)"
    ));
    report.line(format!("- Chi-deux (ddl 255) : {chi:.1}"));
    report.line(format!("- p approximatif : {p:.3}"));
    report.line(format!(
        "- Verdict : {}",
        if p >= 0.01 {
            "compatible avec une distribution uniforme (aucun biais détectable)"
        } else {
            "biais détectable — à investiguer"
        }
    ));
    if p < 0.001 {
        report.failures += 1;
    }

    // --- Overhead ---
    report.h2("Surcoût sur le fil");
    let payload = random_message(&mut rng, 200).len().max(64);
    let message = vec![0xA5u8; payload];
    let fragments = seal(
        &message,
        &bob.kem_pk,
        &bob.recipient_tag(),
        &alice,
        SealOptions::default(),
    )
    .unwrap();
    let ciphertext_bytes: usize = fragments
        .iter()
        .map(|f| polygone_core::from_b64(&f.payload).unwrap().len())
        .sum();
    let wire_bytes: usize = fragments.iter().map(serde_len).sum();
    report.line("| Grandeur | Octets |");
    report.line("|---|---|");
    report.line(format!("| Message en clair | {payload} |"));
    report.line(format!(
        "| Charge utile chiffrée (Σ fragments) | {ciphertext_bytes} |"
    ));
    report.line(format!(
        "| Sur le fil (Σ fragments sérialisés, métadonnées répliquées) | {wire_bytes} |"
    ));
    report.line(format!(
        "| Facteur d'expansion | {:.1}× |",
        wire_bytes as f64 / payload as f64
    ));
    report.line("");
    report.line("Note : la clé KEM (1568 o), la clé publique de signature (1952 o) et la");
    report.line("signature (3309 o) sont répliquées sur chaque fragment. C'est le prix de");
    report.line("l'absence de tout état partagé côté relais.");
}

fn serde_len(fragment: &Fragment) -> usize {
    serde_json_len(fragment)
}

/// JSON length without pulling serde_json into this crate's dependency list.
fn serde_json_len(fragment: &Fragment) -> usize {
    // Fields are base64/hex text; approximate precisely enough for a report by
    // summing the actual string lengths plus structural characters.
    let text_fields = [
        fragment.message_id.len(),
        fragment.kem_ct.len(),
        fragment.nonce.len(),
        fragment.payload.len(),
        fragment.signature.len(),
        fragment.sender_sig_pk.len(),
        fragment.sender_pseudo.len(),
        fragment.recipient_tag.len(),
    ];
    let strings: usize = text_fields.iter().sum();
    strings + text_fields.len() * 3 + 96
}

fn verdict(ok: bool) -> &'static str {
    if ok {
        "✅"
    } else {
        "❌"
    }
}

fn summarise(report: &mut Report, primitives: PrimitiveSizes, speed: &SpeedSummary) {
    report.h1("5. Synthèse");
    report.line(format!(
        "- Tailles : KEM pk {} o · KEM ct {} o · sig pk {} o · signature {} o",
        primitives.kem_pk, primitives.kem_ct, primitives.sig_pk, primitives.signature
    ));
    report.line(format!("- seal() ~{}", human_us(speed.seal_us)));
    report.line(format!("- open() ~{}", human_us(speed.open_us)));
    report.line(format!("- bout en bout ~{}", human_us(speed.e2e_us)));
    report.line(format!(
        "- Invariants de fiabilité violés : {}",
        report.failures
    ));
    report.line("");
    if report.failures == 0 {
        report.line("**Verdict global : ✅ tous les invariants tiennent.**");
    } else {
        report.line("**Verdict global : ❌ des invariants ont échoué.**");
    }
}
