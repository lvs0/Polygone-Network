# RESEARCH NOTES — PQC (Post-Quantum Cryptography)
## État de l'art 2025-2026 — niveau ingénieur/chercheur

**Date** : 2026-08-20
**Auteur** : Ruflo (mission SOE)
**Contexte** : Polygone-v2 — protocole P2P post-quantique avec ML-KEM-1024 + ML-DSA-65 + Shamir fragmentation

---

## 1. Le paysage PQC en 2026 : les standards sont là, la migration commence

Le 13 août 2024, le NIST a publié les trois premiers standards FIPS de cryptographie post-quantique, après 8 ans de compétition publique (69 soumissions initiales, 4 rounds d'évaluation). C'est l'équivalent crypto de ce que le Wi-Fi a été pour les réseaux : une standardisation qui débloque l'adoption de masse.

**Calendrier de migration** :
- 2024 : standards publiés (FIPS 203, 204, 205)
- 2025-2027 : adoption early (Signal, Apple iMessage, Google Chrome, Cloudflare)
- 2027-2030 : migration enterprise (banques, gouvernements, santé)
- 2030 : deadline NSA CNSA 2.0 pour les systèmes de sécurité nationale
- 2030-2035 : "Q-Day" estimé (ordinateur quantique capable de casser RSA-2048)

**La fenêtre pour Polygone** : 2026-2028. C'est le moment où les early adopters cherchent des solutions PQC open-source. Arriver après 2028 = marché saturé.

---

## 2. Les trois standards NIST FIPS (août 2024)

### 2.1 FIPS 203 — ML-KEM (Kyber)
- **Algorithme** : CRYSTALS-Kyber → ML-KEM (Module-Lattice Key Encapsulation Mechanism)
- **Type** : KEM (échange de clé)
- **Sécurité** : NIST Level 5 (le plus élevé) avec ML-KEM-1024
- **Tailles** (ML-KEM-1024) :
  - Clé publique : 1 568 bytes
  - Clé secrète : 3 168 bytes
  - Ciphertext : 1 568 bytes
  - Secret partagé : 32 bytes
- **Performance** : ~100K ops/sec/core (keygen), ~200K (encap), ~300K (decap)
- **Statut Polygone** : ✅ ML-KEM-1024 implémenté (`crates/core/src/crypto/kem.rs`), tests OK

### 2.2 FIPS 204 — ML-DSA (Dilithium)
- **Algorithme** : CRYSTALS-Dilithium → ML-DSA (Module-Lattice Digital Signature Algorithm)
- **Type** : Signature numérique
- **Niveau** : ML-DSA-65 (NIST Level 3) — bon compromis sécurité/performance
- **Tailles** (ML-DSA-65) :
  - Clé publique : 1 952 bytes
  - Clé secrète : 4 032 bytes
  - Signature : 3 309 bytes
- **Performance** : ~10K sign/sec, ~50K verify/sec (benchmark Polygone : 269 µs sign+verify)
- **Statut Polygone** : ✅ ML-DSA-65 implémenté (`crates/core/src/sign.rs`), tests OK, benchmark existe

### 2.3 FIPS 205 — SLH-DSA (SPHINCS+)
- **Algorithme** : SPHINCS+ → SLH-DSA (Stateless Hash-Based Digital Signature Algorithm)
- **Type** : Signature hash-based (pas de structure mathématique → pas d'attaque algébrique)
- **Avantage** : sécurité conservative (basée sur les fonctions de hachage uniquement)
- **Inconvénient** : signatures énormes (7-50 KB) et lentes
- **Usage** : backup pour ML-DSA, certification long-terme
- **Statut Polygone** : ❌ NON implémenté. À ajouter pour couverture NIST complète.

### 2.4 FIPS 206 — FN-DSA (Falcon) — en développement
- **Algorithme** : Falcon → FN-DSA (FFT-based NTRU Digital Signature Algorithm)
- **Statut** : standard attendu, pas encore finalisé
- **Avantage** : signatures plus petites que ML-DSA (~666 bytes vs 3 309)
- **Inconvénient** : implémentation complexe (FFT floating-point, side-channel sensible)
- **Statut Polygone** : ❌ Pas prioritaire. À considérer en v3.

---

## 3. Comparaison : classique vs PQC

| Métrique | RSA-2048 | ECDH P-256 | ML-KEM-1024 | ML-DSA-65 |
|---|---|---|---|---|
| **Clé publique** | 256 B | 64 B | **1 568 B** | 1 952 B |
| **Clé secrète** | 256 B | 32 B | **3 168 B** | 4 032 B |
| **Signature** | 256 B | 64 B | N/A | **3 309 B** |
| **Sécurité quantique** | ❌ (Shor) | ❌ (Shor) | ✅ | ✅ |
| **Sécurité classique** | ~112 bits | ~128 bits | ~256 bits | ~192 bits |

**Overhead PQC vs classique** : ~10-50× en taille. C'est le prix de la résistance quantique. Pour un protocole réseau, ça veut dire des handshakes plus lourds. C'est là que l'optimisation de Polygone (Shamir fragmentation pour éviter de tout envoyer au relay) devient pertinente.

---

## 4. Librairies de référence

### 4.1 liboqs (Open Quantum Safe)
- **Repo** : github.com/open-quantum-safe/liboqs
- **Langage** : C, bindings pour 10+ langages
- **Contenu** : implémentations de référence de TOUS les algorithmes NIST + candidats
- **Licence** : MIT
- **Statut** : utilisé par Google, AWS, Cloudflare pour leurs déploiements PQC
- **Pertinence Polygone** : c'est le standard de facto. Nos benchmarks doivent se comparer à liboqs.

### 4.2 pqcrypto (Rust)
- **Crates** : `pqcrypto-mlkem`, `pqcrypto-mldsa`, `pqcrypto-sphincsplus`
- **Statut Polygone** : ✅ déjà utilisé dans `crates/core/Cargo.toml`
- **Note** : ces crates wrappent les implémentations C de référence. Pas optimisés AVX2/NEON. Pour un benchmark honnête, il faut le mentionner.

### 4.3 Alternatives Rust (à explorer pour v3)
- **`rust-crypto`** : implémentations Rust natives, pas de FFI. Plus lent mais plus sûr (pas de unsafe).
- **`ml-kem`** : crate Rust pure pour ML-KEM, encore jeune.
- **`fips203`** : implémentation de référence en Rust par NIST.

---

## 5. Concurrents startups (2025-2026)

### 5.1 Nillion (mainnet mars 2025)
- **CEO** : Alex Page (ex-Uber, ex-Goldman)
- **Financement** : $25M+ (2024)
- **Techno** : "Blind Compute" — MPC + FHE, pas PQC pur
- **Positionnement** : stockage et calcul sur données chiffrées pour blockchain/AI
- **Token** : NIL (lancé mars 2025)
- **Différence Polygone** : Nillion est blockchain-first, on est P2P-first. Leur "blind compute" est du MPC (Multi-Party Computation), pas de la crypto post-quantique. Complémentaire, pas concurrent.

### 5.2 Zama (Paris)
- **CEO** : Rand Hindi (ex-Snips)
- **Financement** : $73M (Series A, 2024)
- **Techno** : Fully Homomorphic Encryption (FHE) — TFHE, Concrete
- **Positionnement** : calcul sur données chiffrées, ML privé
- **Open-source** : ✅ (BSD) — Concrete, Concrete-ML, TFHE-rs
- **Différence Polygone** : FHE ≠ PQC. Zama fait du calcul sur chiffré, pas du chiffrement post-quantique. Leurs primitives ne sont pas (encore) quantum-safe. Mais leur approche open-source + Rust est inspirante.

### 5.3 Sunscreen (San Francisco)
- **Techno** : FHE compiler (Rust → circuits FHE)
- **Statut** : early stage, moins visible que Zama
- **Différence Polygone** : niche différente (FHE compiler vs PQC protocol).

### 5.4 Positionnement Polygone

```
              Stockage/Calcul         Communication
              ─────────────           ─────────────
Classique     AWS KMS, Vault          TLS 1.3, Noise
              ─────────────────────────────────────
Post-quantique Nillion (MPC)          Polygone ← NOUS
              Zama (FHE)              (niche P2P PQC)
              Sunscreen (FHE)
```

**Notre niche** : Polygone est le SEUL protocole P2P open-source qui combine ML-KEM + ML-DSA + Shamir fragmentation pour un relay "zero-knowledge". Ni Nillion, ni Zama, ni Sunscreen ne font du P2P post-quantique.

---

## 6. Implications pour Polygone — décisions techniques

### Priorité 1 : Benchmarks comparatifs vs liboqs
- Installer liboqs et mesurer ML-KEM-1024, ML-DSA-65 avec les mêmes paramètres que Polygone
- Tableau comparatif : ops/sec, bytes/op, mémoire
- C'est le minimum pour un papier crédible

### Priorité 2 : Ajouter SPHINCS+ / SLH-DSA (FIPS 205)
- Ajouter `pqcrypto-sphincsplus` dans `Cargo.toml`
- Wrapper similaire à `kem.rs` et `sign.rs` (~150 LOC)
- Tests : sign/verify round-trip, tailles de clé
- Bénéfice : couverture NIST complète (3/3 FIPS) = argument de vente fort

### Priorité 3 : Audit de sécurité externe
- Contacter un chercheur PQC (ex: Thomas Prest, PQShield ; Peter Schwabe, MPI-SP)
- Proposer un audit informel du design Shamir fragmentation
- Même un "looks reasonable" par email est publiable dans un paper

### Priorité 4 : Fuzzing
- Ajouter `cargo-fuzz` sur `encapsulate()`, `decapsulate()`, `sign()`, `verify()`
- Tests de mutation sur les bytes de clé
- Essentiel pour la crédibilité d'un projet crypto

---

## 7. PQC + LLM : la synergie SOE-Orret + Polygone

C'est le positionnement unique de SOE : combiner dLLM + PQC dans un seul écosystème. Pourquoi c'est pertinent :

1. **Edge AI** : un dLLM qui tourne sur un laptop a besoin de sécurité. Si tu fais de l'inférence locale avec des données sensibles, le chiffrement PQC du stockage et des communications est un vrai besoin.
2. **Agent autonomy** : un agent IA qui signe des transactions, envoie des messages, ou accède à des APIs a besoin d'une identité cryptographique. ML-DSA-65 fournit ça.
3. **P2P model sharing** : si SOE-Orret devient un réseau de nœuds qui partagent des modèles ou des fine-tunes, le handshake PQC + Shamir fragmentation est la couche de transport idéale.

**Concept pour le papier** : "SOE-Orret: A Post-Quantum Symbiotic Operating Environment for Edge Language Models" — ça combine dLLM + agent architecture + PQC dans un seul système. Aucun autre projet ne fait ça.

---

## 8. Prochaines étapes recherche

1. [ ] Installer liboqs, benchmark ML-KEM-1024 + ML-DSA-65, table comparative
2. [ ] Ajouter SPHINCS+ wrapper (~150 LOC) → couverture FIPS 3/3
3. [ ] `cargo bench` sur toutes les primitives
4. [ ] `cargo-fuzz` harness sur kem + sign
5. [ ] Rédiger `SPEC.md` formel (wire protocol, security argument, threat model)
6. [ ] Contacter 2-3 chercheurs PQC pour feedback informel
7. [ ] Rédiger section "Related Work" pour le papier

---

## 9. Références clés (format BibTeX-ready)

```
@misc{nist2024fips203,
  title={FIPS 203: Module-Lattice-Based Key-Encapsulation Mechanism Standard},
  author={NIST},
  year={2024},
  howpublished={https://csrc.nist.gov/pubs/fips/203/final}
}

@misc{nist2024fips204,
  title={FIPS 204: Module-Lattice-Based Digital Signature Standard},
  author={NIST},
  year={2024},
  howpublished={https://csrc.nist.gov/pubs/fips/204/final}
}

@misc{nist2024fips205,
  title={FIPS 205: Stateless Hash-Based Digital Signature Standard},
  author={NIST},
  year={2024},
  howpublished={https://csrc.nist.gov/pubs/fips/205/final}
}

@misc{openquantumsafe2025,
  title={liboqs: C library for quantum-resistant cryptographic algorithms},
  author={Open Quantum Safe},
  year={2025},
  howpublished={https://github.com/open-quantum-safe/liboqs}
}
```

---

*Signé Ruflo, mission SOE, 2026-08-20.*
