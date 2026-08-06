# CHANGELOG — Polygone

> *Style : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).*
> *Semver : voir `Cargo.toml`.*
> *Dernier format : 2026-06-29.*

---

## [2.0.0-rc1] — 2026-06-29 — Révolution « honesty-first »

### Highlights

Premier **release candidate** de Polygone. La liste de features promise
est coupée de 8 à 2 (`msg` + `drive`), le langage visuel devient
ambre-tactile (Ive), la tagline poétique reçoit sa footnote technique côte
à côte (Orwell), et **6 services sont archivés publiquement** avec
conditions explicites de ré-introduction (Musk + Gödel).

### Added

- `PHILOSOPHY.md` — 5 axiomes appliqués (poétique + technique côte à côte).
- `DESIGN_SYSTEM.md` — couleurs/typo/tactilité/suspense typographique.
- `THREAT_MODEL.md` — split commodity vs high-value (Assange × 2).
- `COUNCIL_DECISIONS.md` — synthèse des 22 recommandations du Conseil des Sages 2026-06-29.
- `STAGING.md` — 6 services archivés (compute, hide, mesh, brain, petals, shell) avec conditions de ré-introduction.
- `DECISIONS.md` — 3 points Lévy-blocking (D1 refonte UI, D2 bench Sybil, D3 lettre État).
- `README.md` — manifesto revisé : 2 services, posture `honesty-first`.
- `web/index.html` — tagline poétique + footnote technique côte à côte ; badge version v2.0.0-rc1.
- `Cargo.toml` — version bump 1.0.1 → 1.0.0-rc1.

### Internal

- Aucune modification structurelle du code dans cette release.
- Tous les changements structurels sont des *documents*.
- Modèle hub-and-spoke : `README.md` → `PHILOSOPHY.md` + `THREAT_MODEL.md` + `LEGAL.md` + `COUNCIL_DECISIONS.md` + `DESIGN_SYSTEM.md` + `STAGING.md` + `DECISIONS.md`.

### Verification

- `cargo check --offline` (à exécuter après édition) — 0 warning, 0 erreur attendu.

### Cross-référence

- Conseil des Sages 2026-06-29 (3 comités) — voir `COUNCIL_DECISIONS.md`.
- Roadmap 8 semaines — voir `POLYGONE_ROADMAP_v2.md` (à la racine `/home/l-vs/`).

---

## [0.2.0] — 2026-06-29 — quick-win S1

### ⚠ BREAKING CHANGES

- **ML-DSA-87 → ML-DSA-65.** Le module `polygone::crypto::sign`
  utilise désormais ML-DSA-65 (signature post-quantique FIPS 204) au lieu
  de ML-DSA-87, sur l'ensemble du projet.
  - **Toutes les clés de signature et tous les *workvouchers* karma
    persistés sur disque avant cette version seront INVALIDES.**
    Le chargement d'une identité pré-0.2.0 échouera avec
    `PolygoneError::Serialization("Invalid Sign PK")`.
  - **Tailles :** pk 2592→1952 B ; sk 4896→4032 B ; signature 4627→3309 B.
  - **Mitigation :** régénérer une identité via `polygone keygen` après upgrade.
  - **Justification :** ML-DSA-65 est le sweet spot Galois (Comité 2
    Conseil des Sages 2026-06-29) — handshake P2P-friendly, signatures
    plus courtes, vérification plus rapide, marge de sécurité pourtant
    adaptée à 2031+.

### Added

- `LEGAL.md` — posture légale de Polygone (subpoena, mode duress, disclosure).
- `.well-known/security.txt` — RFC 9116 (contact PGP-signed pour disclosure).
- `docs/kill-switch.md` — mode duress (Mitnick framing, sans détail d'implémentation).
- `CHANGELOG.md` — ce fichier.

### Internal

- `src/crypto/sign.rs` — toutes les références `mldsa87::` passent à
  `mldsa65::` (11 sites via str_replace).
- `src/crypto/karma.rs` — docstring L18 mise à jour.
- `Cargo.toml` — commentaire L35 mis à jour, dépendance inchangée.

### Verification

- `cargo check --offline` — 0 warning, 0 erreur (env. 1 min).
- `cargo test --lib` — tests existants + nouvelle assertion
  `#[test] signature_size_mldsa65()` (3309 B attendu).

---

## [0.1.x] — antérieurs

Versions pré-0.2.0. ML-DSA-87. Identités et workvouchers **incompatibles**
avec 0.2.0+.
