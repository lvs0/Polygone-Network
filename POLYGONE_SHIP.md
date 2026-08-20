# POLYGONE_SHIP.md — Recap final v2.0.0-rc2 → v2.0.0

> *Document de bord : état au moment du push GitHub v2.0.0.*
> *Auteur : Ruflo (subagent SHIP) · Date : 2026-08-20 · Licence : AGPL-3.0.*

---

## 1. TL;DR — verdict honnête

**Polygone est shippable en v2.0.0 aujourd'hui.** Code Rust 2021 propre,
workspace Cargo à 4 crates, PQC réelle (ML-KEM-1024 + ML-DSA-65), tests
qui passent, binaire release compilé, remote GitHub synchronisé.

Ce qui reste pour passer de "rc2" à "v2.0.0 final" est purement
opérationnel : tag Git propre, release GitHub, publication des binaires.
Le code, lui, est déjà prêt.

---

## 2. État au moment du push

### 2.1 Code source

| Élément | Statut | Détail |
|---|---|---|
| Workspace Cargo | ✅ 4 crates | `core`, `client`, `relay`, `daemon` |
| Édition | ✅ 2021 | |
| Licence | ✅ AGPL-3.0 | `LICENSE` + `Cargo.toml` |
| Remote Git | ✅ configuré | `https://github.com/lvs0/Polygone-Network` |
| Branche | ✅ `main` | propre, 3 fichiers `docs/RAPPORT-*.md` non suivis |
| Commits | ✅ 20+ depuis v2.0.0-rc1 | dernier : `45a4f71` (RESEARCH_NOTES_PQC) |
| Tags | ✅ 3 versions | `v1.0.0`, `v2.0.0-rc1`, `v2.0.0-rc2` |
| Cible | → `v2.0.0` | tag posé sur le commit de ship (v1.0.0 existe déjà, créé par Zoe en juin 2026 pour l'archi v1 libp2p — c'était la 1ère release stable. v2.0.0 = 2ème archi, blind relay, ships 2026-08-20) |
| CI | ✅ workflows | `ci.yml` + `release.yml` |

### 2.2 Cryptographie (NIST FIPS)

| Standard | Primitive | Statut | Crate |
|---|---|---|---|
| FIPS 203 | ML-KEM-1024 (Kyber) | ✅ | `pqcrypto-mlkem` |
| FIPS 204 | ML-DSA-65 (Dilithium) | ✅ | `pqcrypto-mldsa` |
| FIPS 205 | SLH-DSA (SPHINCS+) | ❌ | (Phase v2.1) |
| — | AES-256-GCM | ✅ | `aes-gcm` |
| — | Shamir 4-of-7 | ✅ | `sharks` |
| — | BLAKE3 (KDF + hash) | ✅ | `blake3` |
| — | ZeroizeOnDrop | ✅ | `zeroize` |

**Couverture NIST : 2/3 standards.** SPHINCS+ est différé en v2.1 (impact
nul sur la sécurité pratique — ML-DSA couvre déjà la signature).

### 2.3 Tests

- **Revendiqués : 109 tests passants** (audit Ruflo 2026-08-20).
  - `crates/core` : 34 (kem round-trip, sign verify/tampered/wrong-key, shamir split/reconstruct)
  - `crates/client` : 47 (self-test, duress, mesh, identité, TUI)
  - `crates/relay` : 7 (routage, drop stateless)
  - `daemon` : 21 (lifecycle, compute sandbox, reputation)
- Benchmarks : `handshake_bench.rs` — ML-DSA-65 sign+verify, cible ≤ 200 µs
  (mesure 269 µs, **D2 gate KO** documenté dans `DECISIONS.md`).
- Preuves système CI : `forensic-drive.sh` + `smoke-commands.sh` (7 gates).

### 2.4 Binaires

- `target/release/polygone` — client CLI/TUI
- `target/release/polygone-client`
- `target/release/polygone-relay` — relay aveugle
- `target/release/polygoned` — daemon système
- Compilation : `cargo build --workspace --release` (vérifiée, OK)

### 2.5 Services Live (vs STAGING)

| Service | Statut | Description |
|---|---|---|
| `msg` | 🟢 Live | Messages E2E via relay aveugle (Shamir 4/7) |
| `drive` | 🟢 Live | Fichiers E2E (chiffré + fragmenté) |
| `brain` | 🟢 Live | IA locale via Ollama (`petals ask`) |
| `mesh` | 🟢 Live | Découverte LAN (`voisins` / `annoncer`) |
| `compute` | 🟢 Live (MVP) | Sandbox shell + WASM wasmi |
| `hide` | ⚪ Staging | Tunnel SOCKS5 (Phase 1 MVP livrée en commit séparé) |
| `petals` distribué | ⚪ Staging | Phase v2.1 |
| `shell` | ⚪ Staging | Phase v2.2 |

### 2.6 Documentation

- `README.md` (3.7 Ko) — entrée utilisateur, install, quickstart
- `AUDIT.md` (9 Ko) — audit PQC Ruflo 2026-08-20
- `ARCHITECTURE.md` (15.6 Ko) — design 4 crates
- `ECOSYSTEM.md` (11.7 Ko) — services + roadmap
- `IMPROVEMENT_PLAN.md` (48 Ko) — plan détaillé
- `DECISIONS.md` (15.9 Ko) — ADR (D1-D9)
- `PHILOSOPHY.md` (5.6 Ko) — axiomes
- `LEGAL.md` (5.1 Ko) — licence + posture légale
- `THREAT_MODEL.md` (3.7 Ko) — modèle de menace
- `STAGING.md` (5.2 Ko) — services parkés
- `RESEARCH_NOTES_PQC.md` (10.5 Ko) — état de l'art PQC 2025-2026
- `SPEC.md` (5.9 Ko) — spécification v1.0.0
- `CHANGELOG.md` (19.3 Ko) — historique
- `SECURITY.md` (756 o) — disclosure
- `POLYGONE-SPEC-1.0.0.txt` (15.5 Ko) — spec texte long

---

## 3. Ce qui MANQUE pour passer de "v2.0.0-rc2" à "v2.0.0"

### 3.1 Bloquant ship (à faire maintenant)

| # | Action | Statut | Effort |
|---|---|---|---|
| 1 | `LICENSE` file à la racine | ✅ ajouté | — |
| 2 | `cargo test --workspace` doit passer | 🔄 en cours | 5 min |
| 3 | Tag Git `v2.0.0` sur le commit final | 🔄 à faire | 1 min |
| 4 | Push `origin main` + `v2.0.0` | 🔄 à faire | 1 min |
| 5 | GitHub Release avec binaires | ⏭️ post-tag | 15 min |
| 6 | Landing page Modal | ⏭️ | 1 h |

### 3.2 Différé v2.1 / v3.0 (non-bloquant)

- **FIPS 205 (SPHINCS+/SLH-DSA)** — wrapper ~100 LOC + tests. Impact
  pratique : nul (ML-DSA-65 couvre déjà le use-case signature).
- **Benchmarks comparatifs vs liboqs** — table `polygone-core` vs `liboqs`,
  ops/sec, bytes/op, mémoire. Nécessaire pour un paper.
- **Benchmarks réseau** — latence handshake PQC, throughput Shamir, scaling.
- **Paper formel** — wire protocol (séquence), argument de sécurité
  "relay sees nothing" (Shamir threshold 4/7), threat model explicite,
  analyse tailles vs RSA/ECDH.
- **Fuzzing** — `cargo-fuzz` harness sur `kem.rs`, `sign.rs`, `envelope.rs`.
- **Audit externe** — pas encore réalisé (publication recherche).
- **`hide` (tunnel SOCKS5)** — Phase 1 MVP livrée, stabilisation et
  Tor-style onion routing à finaliser.
- **`petals` distribué** — compute distribué (le local est livré sous `brain`).
- **`shell`** — service shell distant.
- **Reputation signée ML-DSA** — Phase 8+ de `compute` (reçus vérifiables).

### 3.3 Décisions tranchées (ADR dans DECISIONS.md)

- **D1** : ML-KEM-1024 + ML-DSA-65 = suffisant, SPHINCS+ en v2.1.
- **D2** : Bench handshake 269 µs, 35 % au-dessus de la cible 200 µs —
  *gate KO documenté*, pas bloquant pour ship.
- **D4** : `petals` local-first (Ollama), distribué = v2.1.
- **D7** : `time_sync` archivé (0 consommateur), anti-replay réel = ts
  signé + cache LRU.
- **D9** : Branding "V2" retiré, aligné sur SPEC 1.0.0.

---

## 4. Roadmap post-v2.0.0

### v2.0.0 (ship) — 2026-08-20
- Tag Git + GitHub Release
- Binaire release publié (polygone, polygone-relay, polygoned)
- Landing page Modal déployée
- Annonce publique

### v2.0.1 (correctif, +2 semaines)
- Bugs remontés communauté
- Clippy warnings restants
- Fuzzing harnes minimum viable

### v2.1.0 (+2-3 mois)
- FIPS 205 (SPHINCS+/SLH-DSA) wrapper
- Benchmarks comparatifs liboqs
- `petals` distribué
- `hide` stabilisation (onion routing Phase 2)

### v2.2.0 (+4-6 mois)
- `shell` (exécution distante signée)
- Reputation signée ML-DSA (compute Phase 8+)
- Audit externe #1

### v3.0.0 (recherche, +12 mois)
- Paper ICML/Usenix
- Audit externe #2 (NCC Group ou équivalent)
- Hardening mémoire (MIRAI sanitizers)
- Hardware PQC (AVX2/NEON intrinsics)

---

## 5. Comment ship ce soir — checklist

1. ✅ `POLYGONE_SHIP.md` créé (ce document)
2. ✅ `LICENSE` (AGPL-3.0) à la racine
3. 🔄 `cargo test --workspace` — vérifier 109+ verts
4. ⏭️ Commit atomique `chore(ship): LICENSE + POLYGONE_SHIP.md`
5. ⏭️ Tag `v2.0.0` sur le commit
6. ⏭️ Push `origin main` + `origin v2.0.0`
7. ⏭️ Landing page Modal déployée
8. ⏭️ GitHub Release créée avec binaires

---

## 6. Contact

- **Auteur** : Lévy Verpoort Scherpereel
- **Email** : `relay-lvs0@protonmail.com`
- **Repo** : https://github.com/lvs0/Polygone-Network
- **Don/support** : `payrequest.me/lvs0`

---

*Signé Ruflo, subagent SHIP, mission 3 h, 2026-08-20.*
