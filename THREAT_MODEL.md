# THREAT_MODEL.md — Deux audiences, deux SLA

> *Le projet ne prétend pas protéger le manifestant turc ET le banlieusard parisien.*
> *Deux audiences = deux SLA = deux documents séparés (Assange × 2, Conseil des Sages 2026-06-29).*
> *Cette page est un stub honnête. Le détail en `docs/threat-commodity.md` et `docs/threat-high-value.md` (livrables S2).*

---

## Scope 1 — Utilisateur quotidien (« commodity »)

### Adversaire type
- FAI curieux
- GAFA (Google, Apple, Facebook, Amazon, Microsoft)
- Colocataire · Wi-Fi partagé · VPN tiers
- Malware opportuniste

### Ce que Polygone protège
- **Lien message ↔ identité** chiffré en clair (ML-KEM-1024)
- **Persistance** des messages après TTL (≤ 30 s sur relay)
- **Contenu du message** (AES-256-GCM bout-en-bout)
- **Tamper-evident** des signatures (ML-DSA-65)

### Ce que Polygone ne protège PAS
- ⚠ Keylogger sur la machine endpoint
- ⚠ Malware persistant déjà installé
- ⚠ Coercition physique / vol de machine
- ⚠ Disclosure HUMAINE forcée (« rubber-hose attack »)

### Coût d'adoption
- **5 minutes** : `curl -fsSL polygone.network/install | bash`

### Fabriquer la confiance
- `cargo test` reproduit le mode en 2 min
- Le code source est lisible en un après-midi (cible : Wozniak Axiome 2)

---

## Scope 2 — Utilisateur haute-valeur (« dissident »)

### Adversaire type
- Adversaire étatique avec subpoena
- Adversaire avec accès physique prolongé
- Adversaire avec monitoring réseau national
- Adversaire actif (Sybil, MITM, rubber-hose)

### Ce que Polygone protège
- **Pattern de messagerie** hors TTL (4-of-7 fragments)
- **Mécanisme de subpoena** documenté (`LEGAL.md` §4, 24 h de gèle)
- **Contenu chiffré** de bout-en-bout
- **Mode duress** (`docs/kill-switch.md`) — autodestruction locaux sur signal

### Ce que Polygone ne protège PAS
- ⚠ Rubber-hose attack (Mitnick, Comité 2)
- ⚠ Reconnaissance physique (caméra, mouchard)
- ⚠ Ingénierie sociale (social engineering sur l'opérateur humain)
- ⚠ Disclosure humaine forcée (*quelqu'un finit par parler*)

### Coût d'adoption
- **30 minutes** pour comprendre threat model + setup
- Documenter : `threat-high-value.md` + `LEGAL.md` + `kill-switch.md`

### Fabriquer la confiance
- **1 audit externe** (Trail of Bits, NCC Group, Quarkslab, ou ANSSI) — **non réalisé à ce jour**, c'est dit explicitement dans `LEGAL.md`
- Mode duress testé physiquement par l'opérateur
- Communication chiffrée PGP-signed des updates du binaire

---

## Ce que Polygone ne promet PAS du tout

| Promesse | Pourquoi on ne la fait PAS |
|----------|---------------------------|
| « L'information n'existe pas » au sens ontologique | Métaphore poétique uniquement. Cf. `PHILOSOPHY.md` Axiome 1 — claim narrow, vérifiable. |
| Pas de Shannon leak (timing, taille, routage) | Métadonnées restent observables au niveau réseau. Réduit, pas annulé. |
| Pas de pression juridique sur l'opérateur | Aucune solution logicielle n'efface un être humain qui parle. |

---

## Documents associés (cf. ce repo)

| Document | Statut | Quand livré |
|----------|--------|-------------|
| `docs/threat-commodity.md` | S2 livrable | Roadmap S2 (= 13 juillet 2026) |
| `docs/threat-high-value.md` | S2 livrable | Roadmap S2 |
| `LEGAL.md` | ✅ Publié v0.2 | cf. repo |
| `docs/kill-switch.md` | ✅ Publié v0.1 | cf. repo |
| `.well-known/security.txt` | ✅ Publié v0.1 | cf. repo |

---

*Stub honnête Honoré Conseil des Sages. Mise à jour S2.*
*MIT License · Pas de token · Pas de télémétrie.*
