# ⚠️ Conseil v1 gelé — voir COUNCIL_V2_RECONSIDERED.md (10 perspectives : 5 sages × 2 fenêtres de connaissance).
# Ce document est conservé comme trace historique de la méthodologie v1 (22 décisions fusion

ées) et reste valide techniquement, mais a été remplacé par la méthodologie v2 (séparation stricte par sage × fenêtre de connaissance) révélée par Lévy dans `~/Par contre tu dois faire.md`.
#

# COUNCIL_DECISIONS.md — Les 9 décisions du Conseil des Sages

> *Synthèse publique des recommandations issues du Conseil des Sages 2026-06-29 (3 comités, ~30 voix).*
> *Chaque P : (a) voix d'origine, (b) justification courte, (c) critères d'acceptance, (d) blockers, (e) statut.*
> *Cross-référence : `POLYGONE_ROADMAP_v2.md`, `PHILOSOPHY.md`, `DECISIONS.md`.*

---

## Comité 1 — Architecture & Code

### P-A1 — Module `polygone-proof` (Coq/Lean sur kernel crypto)
- **Voix** : Kurt Gödel · Alan Turing 2026
- **Justification** : Le `Service` trait ne peut pas exhiber sa propre complétude. Gödel : réflexivement incomplet est une *propriété*, pas un défaut, mais doit être documenté par une preuve formelle du kernel.
- **Acceptance** : Coq sur session ML-KEM + reconstruction Shamir 4-of-7. Tests property-based en attendant.
- **Blockers** : Expertise Coq/Lean non disponible en S1.
- **Statut** : ⚪ **PARKED Phase 8+** — décision du roadmap 2026-06-29.

### P-A2 — Lier `PeerID` à une `proof_of_key` (Sybil-resistance)
- **Voix** : Satoshi Nakamoto · Turing 2026
- **Justification** : Le mesh accepte n'importe quel pair. Sans Sybil-resistance, un adversaire avec 1000 nœuds Raspberry Pi peut polluer la DHT Kademlia et paralyser la découverte.
- **Acceptance** : signature ML-DSA-65 sur `(PeerID || nonce)` en < 200 µs.
- **Blockers** : Bench perf (voir **D2** dans `DECISIONS.md`).
- **Statut** : 🟢 **S2 planifié — Phase 4.1 du roadmap**.

### P-A3 — Réduire 5500 lignes → 2800 lignes utiles
- **Voix** : Steve Wozniak · Camille (dev Rust)
- **Justification** : Le code doit être lisible par un lecteur neuf. Une ABI doit se lire en un après-midi par un adolescent de 14 ans qui lit la presse spécialisée.
- **Acceptance** : Extraction de 3 sous-crates (`polygone-crypto`, `polygone-relay`, `polygone-mesh`).
- **Blockers** : aucun.
- **Statut** : ⚪ **Phase 8+** (refonte après MVP).

### P-A4 — Champ GF(2^16) pour fragments Shamir
- **Voix** : Évariste Galois
- **Justification** : Reed-Solomon gratuit. Correction d'erreur. Compactage.
- **Acceptance** : impl. Shamir sur GF(2^16) au lieu de GF(p).
- **Blockers** : aucun.
- **Statut** : ⚪ **parked stretch** (Phase 8+).

### P-A5 — Documenter chaque invariant Rust en doc-comment ≤ 1 ligne
- **Voix** : Camille · Steve Wozniak
- **Justification** : Un reader doit comprendre pourquoi `&mut self.menu && &mut self.persistent` est valide par disjoint borrow.
- **Acceptance** : chaque invariant documenté en commentaire inline.
- **Blockers** : aucun.
- **Statut** : 🟢 **continu** — appliqué sur `app.rs` Hexi 2026-06-21.

### P-A6 — Modèle d'écoulement formel (von Neumann bottleneck)
- **Voix** : John von Neumann
- **Justification** : Le scheduler Computer explosera à N=50 sans modèle formel du goulot d'étranglement.
- **Acceptance** : 1 figure Markov chain dans `ARCHITECTURE.md` §4.
- **Blockers** : aucun.
- **Statut** : 🟢 **Phase 4.1** (3 jours).

---

## Comité 2 — Sécurité & Subversion

### P-S1 — Split threat model commodity vs high-value
- **Voix** : Julian Assange (×2, 1990 + 2026)
- **Justification** : Une posture unifiée ment. Deux audiences, deux SLA.
- **Acceptance** : `docs/threat-commodity.md` + `docs/threat-high-value.md` séparés.
- **Blockers** : aucun.
- **Statut** : 🟢 **S2 livré Phase 4.1**.

### P-S2 — Sybil-resistance via `proof_of_key`
- **Voix** : Shayne Coplan · Satoshi Nakamoto (echo P-A2)
- **Acceptance** : voir P-A2.
- **Statut** : 🟢 lié à P-A2.

### P-S3 — Mode duress matériel (capteur / séquence / bouton)
- **Voix** : Kevin Mitnick
- **Justification** : « La cryptographie est inutile si la machine est saisie en marche. »
- **Acceptance** : détection USB-watchdog + séquence clavier secrète + GPIO panique.
- **Blockers** : hardware polymorphe (multi-OS).
- **Statut** : 🟢 `docs/kill-switch.md` documenté ; code Phase 6.

### P-S4 — Audit chainage BLAKE3 KDF
- **Voix** : Expert privacy · Alan Turing 2026 (cryptanalyse)
- **Acceptance** : revue publique tierce-partie ; remplacement HKDF-SHAKE256 si doute.
- **Statut** : ⚪ **post-MVP** (Phase 8+).

### P-S5 — `LEGAL.md` + kill-switch runbook + 24 h de gèle subpoena
- **Voix** : Shayne Coplan · Expert privacy
- **Acceptance** : posture légale complète + Kafka + counter.
- **Statut** : ✅ **livré v0.2** (cf. `LEGAL.md` + `docs/kill-switch.md`).

### P-S6 — Disclosure responsable (`security.txt` RFC 9116)
- **Voix** : Expert privacy
- **Acceptance** : `security.txt` + PGP-signed contact.
- **Statut** : ✅ **livré v0.1** (cf. `.well-known/security.txt`).

### P-S7 — ML-DSA-87 → ML-DSA-65 sur handshake
- **Voix** : Évariste Galois (algo)
- **Justification** : sweet spot handshake (pk 1952 B vs 2592 B ; signature 3309 vs 4627).
- **Acceptance** : migration complète + tests de taille + CHANGELOG BREAKING.
- **Statut** : ✅ **livré v0.2** (cf. `CHANGELOG.md` ⚠ BREAKING 0.2.0).

### P-S8 — Adversaire sur contrat (HackerOne + freelance Red Team)
- **Voix** : Irving Janis · Serge Moscovici (2026)
- **Acceptance** : budget annuel < 500 € + HackerOne BBP.
- **Statut** : ⚪ **Phase 8+** (post-revenu).

### P-S9 — Posture « non audité tierce-partie »
- **Voix** : Bill Gates (adversaire)
- **Justification** : Les audits coûtent 3 M$. Vous avez 0. Donc *tout ce que vous promettez est auto-déclaré*. Abaissez les promesses ou augmentez les promesses. L'intermédiaire n'existe pas.
- **Acceptance** : tagline explicite dans README + LEGAL.md.
- **Statut** : 🟢 **partiellement livré** (posture `honesty-first` déjà présente).

---

## Comité 3 — Vision & Utilisateur

### P-V1 — TUI 2 onglets (`Envoyer` / `Quitter`)
- **Voix** : Steve Jobs (1984 + 2007) · Elon Musk
- **Justification** : « Un bouton. Le multitouch est la surprise. » 2 onglets. Tout le reste est paramètre caché derrière `:`.
- **Acceptance** : `views::two_tab_layout` test + rendu Jobs.
- **Blockers** : **D1** (`DECISIONS.md`) — Lévy tranche GO/NO-GO à S1.
- **Statut** : 🟡 **DECISION PENDING**.

### P-V2 — Tagline + footnote côte à côte
- **Voix** : George Orwell · Philip K. Dick
- **Justification** : tagline sans footnote = doublespeak.
- **Acceptance** : tagline poétique + footnote technique **1:1 pixel ratio** sur la même baseline.
- **Statut** : 🟢 **partiellement livré** — italic Georgia tagline + mono footnote (web).

### P-V3 — Suspense typographique (Hitchcock)
- **Voix** : Alfred Hitchcock
- **Justification** : le suspense est ce qui se passe AVANT. L'utilisateur doit VOIR l'attente.
- **Acceptance** : transit visible 0-400 ms typographique.
- **Statut** : ⚪ **Phase 6** (post-refonte UI).

### P-V4 — Hero réécrit en une seule ligne lisible par la grand-mère
- **Voix** : Michael Jackson · Socrate
- **Acceptance** : « *Écris à quelqu'un. Personne d'autre ne sait.* » ou équivalent.
- **Statut** : ⚪ **Phase 6** (avec refonte UI).

### P-V5 — Install 1-clic `curl-bash`
- **Voix** : Elon Musk · Sam Altman
- **Acceptance** : `curl -fsSL polygone.network/install | bash` sans Rust préinstallé.
- **Blockers** : **D2** (P2 perf OK).
- **Statut** : ⚪ **Phase 4.3**.

### P-V6 — Tag `En construction` sur chaque feature non-MVP
- **Voix** : Sam Altman · George Orwell
- **Acceptance** : badges live / wip / plan sur chaque feature.
- **Statut** : ✅ **livré PHASE 2** (juin 2026).

### P-V7 — Test grand-mère (5 humains non-tech)
- **Voix** : Socrate (maïeutique)
- **Acceptance** : install + envoi ≤ 5 min pour 5/5 humains. Itérer sur obstacles.
- **Blockers** : **D1** (refonte UI doit précéder le test).
- **Statut** : 🟡 **Phase 6-7**.

### P-V8 — Standardiser le protocole ouvert (POLYGONE-PROTOCOL.md)
- **Voix** : Nostradamus (horizon 2031) · E. Macron (État)
- **Justification** : sans standard, le projet devient artefact culturel capté par un prédateur.
- **Acceptance** : `POLYGONE-PROTOCOL.md` ouvert + donation IETF/W3C.
- **Statut** : ⚪ **Phase 8+**.

### P-V9 — Lettre ouverte État (CNIL / ANSSI / EFF)
- **Voix** : E. Macron · Daniela Amodei (responsibility)
- **Justification** : « L'État n'est pas l'ennemi. L'État est l'infrastructure. »
- **Acceptance** : 3 lettres envoyées + accusées de réception.
- **Blockers** : **D3** (Lévy consent à rédiger).
- **Statut** : 🟡 **S4 planifié**.

### P-V10 — Material design amber
- **Voix** : Jony Ive · Albert Einstein
- **Acceptance** : palette 2-couleurs cyber-amber + cyber-slate.
- **Statut** : ✅ **livré** (cf. `DESIGN_SYSTEM.md` §1).

### P-V11 — KPI = installs Y1
- **Voix** : Analyste business · Serge Moscovici (2026)
- **Acceptance** : 10 000 installs cumulés Y1 (compteur anonymisé dans `polygone-server`).
- **Statut** : ⚪ **post-release**.

### P-V12 — Pas de monétisation, jamais
- **Voix** : Boursicoteur · Prédicteur 5 ans
- **Justification** : pas d'ICO, pas de token tradable. Polygone est libre forever.
- **Acceptance** : zéro transaction monétaire dans le repo.
- **Statut** : ✅ **actuel**.

---

## Synthèse chiffrée

| Comité | Livré | Pending | Parked Phase 8+ |
|--------|-------|---------|-----------------|
| 1 — Architecture | 0 | 1 (P-A2 gated par D2) | 4 (incl. Coq/Lean) |
| 2 — Sécurité | 3 | 2 | 3 |
| 3 — Vision | 4 | 3 (P-V1/V7 gated par D1, P-V9 gated par D3) | 2 |
| **Total** | **7** | **6** | **9** |

**22 décisions** au total. **~32% livré**. **~27% pending** Lévy. **~41% parked Phase 8+**.

---

*Hérite de : Conseils_Sages_2026-06-29 transcript, `POLYGONE_ROADMAP_v2.md`, `DECISIONS.md`.*
*MIT License · Pas de télémétrie.*
