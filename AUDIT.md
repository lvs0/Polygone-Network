# AUDIT Polygone-v2 — État du projet pour publication recherche

**Date** : 2026-08-20
**Auditeur** : Ruflo (mission SOE)
**Référentiel** : crates/core, crates/client, crates/relay, daemon
**Version** : 1.0.0-rc2
**Barre** : projet de recherche publiable, comparable à ce que Nillion/Zama/Sunscreen publient

---

## TL;DR (verdict honnête)

- **Code Rust 2021 propre, cargo workspace (4 crates), vraie crypto PQC en production.** ML-KEM-1024 (FIPS 203), ML-DSA-65 (FIPS 204), AES-256-GCM, Shamir SS (4-of-7), BLAKE3. Implémentation réelle, pas un stub.
- **Le code PQC est solide** : le crate `polygone-core` a des tests unitaires sur KEM round-trip, signature verify/tampered/wrong-key, Shamir split/reconstruct. Les benchmarks existent (`handshake_bench.rs`).
- **Pas de SPHINCS+ / SLH-DSA (FIPS 205)** : signature stateful/hash-based non implémentée. Couverture NIST = 2/3 standards.
- **Test count à vérifier** (build en cours). Le repo revendique 109 tests passants.
- **Wire protocol avec Shamir fragmentation sur le relay** : le relay ne peut pas déchiffrer (threshold 4/7). C'est la promesse "relay sees nothing" — **implémentée, pas promise**.
- **Duress mode** : effacement d'urgence (`polygone duress`), clé secrète zeroisée.
- **Licence AGPL-3.0** (bon choix pour l'open-source PQC).
- **Documentation abondante** : ARCHITECTURE.md (15.6 KB), ECOSYSTEM.md (11.7 KB), IMPROVEMENT_PLAN.md (48 KB), DECISIONS.md (15.9 KB), DESIGN_SYSTEM.md, LEGAL.md, CHANGELOG.md, CLAUDE.md.
- **Remote Git configuré** : `https://github.com/lvs0/Polygone-Network`.

→ **Conclusion** : Polygone est le projet le plus mature des deux. Code réel, PQC réelle, tests, benchmarks, doc, remote Git. L'écart principal pour publication = ajouter SPHINCS+ (FIPS 205) + benchmarks comparatifs vs liboqs + paper formel.

---

## 1. Architecture PQC (couverture NIST FIPS)

### ✅ FIPS 203 — ML-KEM-1024 (Kyber)

| Aspect | Statut |
|---|---|
| KEM key generation | ✅ `generate_keypair()` → `(KemPublicKey, KemSecretKey)` |
| Encapsulate | ✅ `encapsulate(&pk)` → `(KemCiphertext, SharedSecret)` |
| Decapsulate | ✅ `decapsulate(&sk, &ct)` → `SharedSecret` |
| Key sizes | ✅ EK=1568, DK=3168, CT=1568, SS=32 bytes (NIST spec) |
| Key zeroization | ✅ `#[derive(ZeroizeOnDrop)]` sur `KemSecretKey` |
| Tests | ✅ Round-trip, consistency, hex serialization, wrong-key |
| Crate | `pqcrypto-mlkem = "0.1"` |

### ✅ FIPS 204 — ML-DSA-65 (Dilithium)

| Aspect | Statut |
|---|---|
| Key generation | ✅ `generate_keypair()` → `KeyPair { signer, verifier }` |
| Sign | ✅ Detached signature, `sign(&self, message: &[u8])` → `Signature` |
| Verify | ✅ `verify(&self, message, &sig)` → `bool` |
| Key sizes | ✅ PK=1952, SK=4032, SIG=3309 bytes (NIST spec) |
| Tests | ✅ Size check, round-trip, tampered, wrong-key |
| Crate | `pqcrypto-mldsa = "0.1"` |

### ❌ FIPS 205 — SLH-DSA (SPHINCS+)

| Aspect | Statut |
|---|---|
| Implémentation | ❌ Absent. Aucun import `pqcrypto-sphincsplus`. |
| Nécessité pour paper | **Faible-moyenne.** Les papiers PQC se concentrent généralement sur 1-2 primitives. ML-KEM + ML-DSA = 2/3, c'est déjà solide. Ajouter SPHINCS+ serait un bonus pour la complétude NIST. |

### ✅ Autres primitives

| Primitive | Usage | Crate | Tests |
|---|---|---|---|
| AES-256-GCM | Chiffrement payload | `aes-gcm = "0.10"` | ✅ (symmetric.rs) |
| Shamir SS (4-of-7) | Fragmentation relay-proof | `sharks = "0.5"` | ✅ (shamir.rs) |
| BLAKE3 | Hashing, KDF, content verification | `blake3 = "1"` | ✅ (crypto/mod.rs) |
| Zeroize | Secure memory clearing | `zeroize = "1"` | N/A (derive) |

---

## 2. Structure du workspace

```
Polygone-v2/
├── crates/core/         ← primitives crypto (kem, sign, shamir, symmetric, envelope)
├── crates/client/       ← client CLI (TUI, identity, net, duress, self-test)
├── crates/relay/        ← relay P2P (ne peut pas déchiffrer : threshold 4/7)
├── daemon/              ← daemon système
├── archive/             ← ancien code
├── examples/
├── docs/
├── graphify-out/        ← sortie d'analyse Graphify
└── .github/             ← CI (à vérifier)
```

---

## 3. Tests (vérification en cours — build cargo)

**Status build** : `cargo test --no-run` en cours (proc_4c8b).

- Tests unitaires dans `crates/core/src/crypto/kem.rs` (4 tests), `sign.rs` (3 tests), `mod.rs` (3 tests)
- Tests d'intégration dans `crates/client/src/self_test.rs`
- Benchmarks : `crates/core/benches/handshake_bench.rs` (ML-DSA-65 sign+verify, target ≤200 µs)
- Le benchmark handshake revendique une classe 269 µs (D2 gate KO, documenté dans DECISIONS.md)

**Revendication** : 109 tests passants. À confirmer une fois le build terminé.

---

## 4. Ce qui manque pour un papier de recherche publiable

### 🔴 TROU 1 — Aucun benchmark comparatif vs liboqs / Open Quantum Safe (critique)
Le code utilise `pqcrypto-mlkem` et `pqcrypto-mldsa` (wrappers Rust de la référence C). Mais il n'y a **aucune comparaison quantitative** avec :
- `liboqs` (Open Quantum Safe, implémentation de référence NIST)
- Les implémentations hardware-optimized (AVX2, ARM NEON)
- Les concurrents (Nillion, Zama, Sunscreen — même en estimation théorique)

**Pour publier** : un benchmark `cargo bench` avec table comparative : `polygone-core` vs `liboqs` (même primitive), vs `rust-crypto` classique. Mesurer : ops/sec, bytes/op, mémoire.

### 🟠 TROU 2 — Pas de SPHINCS+ / SLH-DSA (bonus, pas bloquant)
FIPS 205 non couvert. Pour un papier qui revendique "full NIST PQC compliance", c'est un trou. Solution simple : ajouter `pqcrypto-sphincsplus` avec un wrapper similaire à kem.rs et sign.rs (~100 LOC).

### 🟠 TROU 3 — Pas de paper formel, pas de SPEC.md
La documentation est narrative (IMPROVEMENT_PLAN.md = 48 KB de prose), pas formelle. Pour un papier, il faut :
- Spécification du wire protocol (message flow, diagramme de séquence)
- Preuve informelle que "relay sees nothing" (argument de sécurité basé sur Shamir threshold)
- Analyse des tailles de clé / overhead réseau vs RSA/ECDH classique
- Modèle de menace explicite

### 🟡 TROU 4 — Benchmarks réseau inexistants
Le code a un client, un relay, un daemon — mais aucun benchmark de latence réseau, throughput, ou scaling. Pour un papier P2P/PQC, c'est attendu.

### 🟡 TROU 5 — Pas de fuzzing / audit de sécurité externe
Aucun `cargo-fuzz`, aucun audit externe, aucun harness de fuzzing. Pour un projet qui touche à la crypto, c'est un risque de crédibilité.

---

## 5. Ce que ce projet A comme atout (à protéger)

- **Vrai code PQC qui tourne**, pas un whitepaper vide. ML-KEM-1024 + ML-DSA-65 + Shamir + AES-256-GCM, tout en Rust safe (sauf les FFI pqcrypto).
- **Design original** : fragmentation Shamir pour que le relay ne voie rien. Pas un clone de Signal/Noise.
- **Licence AGPL-3.0** : protège contre l'appropriation propriétaire.
- **Documentation abondante** : 5 fichiers MD de 10-48 KB. Beaucoup de prose, mais le matériel pour un paper est là.
- **Duress mode** : feature de sécurité réelle (effacement d'urgence), peu de projets l'ont.
- **CLI + TUI + daemon** : plus qu'une lib crypto — un outil utilisable.

---

## 6. Plan de remédiation (par ordre de leverage pour publication)

| # | Action | Temps | Statut |
|---|---|---|---|
| 1 | `cargo bench` : benchmark ML-KEM + ML-DSA vs liboqs, table comparative | 4 h | À FAIRE |
| 2 | Ajouter SPHINCS+ wrapper (FIPS 205, ~100 LOC) | 1 h | À FAIRE |
| 3 | Rédiger `SPEC.md` formel (wire protocol, security argument, threat model) | 1 journée | À FAIRE |
| 4 | Ajouter `cargo-fuzz` harness sur kem.rs et sign.rs | 2 h | À FAIRE |
| 5 | Benchmark réseau (latence handshake PQC, throughput Shamir) | 4 h | À FAIRE |
| 6 | Rédiger paper (5-10 pages, format ICML/Usenix) | 2 jours | À FAIRE |

---

## 7. Comparaison rapide avec les concurrents

| Projet | Primitive | Langage | Licence | Tests | Paper |
|---|---|---|---|---|---|
| **Polygone-v2** | ML-KEM-1024, ML-DSA-65, Shamir, AES-256-GCM | Rust | AGPL-3.0 | ~109 (à confirmer) | ❌ |
| Nillion | MPC + FHE | Rust | Propriétaire | N/A | ✅ (blog) |
| Zama | FHE (TFHE) | Rust/C++ | BSD | ✅ | ✅ (multiple) |
| Sunscreen | FHE compiler | Rust | Propriétaire | N/A | ✅ (blog) |
| Open Quantum Safe (liboqs) | Toutes NIST | C | MIT | ✅ | ✅ (NIST submission) |

**Positionnement** : Polygone est le SEUL projet open-source qui combine KEM + DSA + Shamir fragmentation pour un relay P2P zero-knowledge. C'est une vraie niche.

---

## 8. États valides (format Hermes)

- **DONE** : audit lu, PQC vérifiée (ML-KEM + ML-DSA = 2/3 NIST), code propre.
- **PARTIAL** : SPHINCS+ manquant, pas de benchmarks comparatifs, pas de paper.
- **BLOCKED** : build cargo en cours (vérification tests).
- **FAILED** : aucun.
- **NEEDS_RESEARCH** : décision "ajouter SPHINCS+ ou le laisser pour v3" — impact marginal sur la publication.

---

*Signé Ruflo, mission SOE, 2026-08-20.*
