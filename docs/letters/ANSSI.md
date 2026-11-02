# Lettre à l'ANSSI — Projet Polygone

**Date** : 2026-09-02
**Objet** : Notification d'un produit de sécurité — Réseau P2P post-quantique Polygone

---

Madame, Monsieur,

Le projet **Polygone** est un réseau pair-à-pair post-quantique open-source (AGPL-3.0) conçu pour l'échange de messages et fichiers avec confidentialité, intégrité et authenticité, sans tiers de confiance.

**Architecture de sécurité** :
- **Chiffrement post-quantique** : ML-KEM-1024 (FIPS 203) pour l'échange de clés, ML-DSA-65 (FIPS 204) pour les signatures, AES-256-GCM pour le chiffrement symétrique.
- **Dérivation de clés** : HKDF-BLAKE3 avec séparation de domaines.
- **Fragmentation** : Shamir 4-of-7 secret sharing — 7 fragments, 4 requis pour reconstruire.
- **Relay aveugle** : Stateless, ne voit que des enveloppes chiffrées, ne peut pas déchiffrer, ne stocke rien.
- **Authentification mutuelle** : Proof-of-key (PeerID || nonce) signé ML-DSA-65, résistance Sybil ≤ 400 µs.

**Modèle de menace** : Documenté dans `docs/threat-commodity.md` (commodité) et `docs/threat-high-value.md` (haute valeur). Attaques couvertes : interception, injection, replay, analyse de trafic, compromission de nœud, quantum-harvest.

**Kill-switch** : Documenté dans `docs/kill-switch.md` — 3 déclencheurs (USB-watchdog, séquence clavier, GPIO), effacement cryptographique des clés en < 5 s.

**Code** : Rust 1.82+, `unsafe` minimisé, `zeroize` sur les secrets, `clippy` strict, 113 tests unitaires/intégration.

**Audit** : Aucun audit tiers à ce jour. Code ouvert à la revue publique. Bounty non encore ouvert.

**Distribution** : Binaires précompilés Linux/macOS/Windows (P8 en cours). Installateur `curl|bash` (`scripts/install.sh`).

**Contact sécurité** : `security@polygone.network` (PGP : `0x...`), `docs/.well-known/security.txt` (RFC 9116).

Nous sollicitons votre avis sur la posture de sécurité globale et restons à disposition pour une analyse approfondie.

Cordialement,

**Lévy Verpoort Scherpereel** — Auteur principal
`polygone.network` | `security@polygone.network`

---

*Pièces jointes : `docs/threat-commodity.md`, `docs/threat-high-value.md`, `docs/kill-switch.md`, `LEGAL.md`*