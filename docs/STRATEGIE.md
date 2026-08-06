# STRATEGIE.md — Pourquoi Polygone existe, et par où il va

> *Consolidé le 2026-08-06 depuis les notes Bear (11/07) + la synthèse
> actionnable + l'état réel du produit v2.0.0-rc2.*
> *Posture : honesty-first. Ce document dit ce qu'on fait, et ce qu'on ne
> fait pas.*

---

## 1. L'insight fondateur

> Classical privacy tools hide what you say.
> **POLYGONE hides that you said anything at all.**

| | VPN | Tor | ⬡ POLYGONE |
|---|:-:|:-:|:-:|
| Cache le contenu | ✓ | ✓ | ✓ |
| Cache l'IP source | ✓ | ✓ | ✓ |
| Cache l'IP cible | ✗ | ✓ | ✓ |
| Post-quantique | ✗ | ✗ | ✓ |
| Pas de relay persistant | ✗ | ✗ | ✓ |
| Messages auto-évaporés | ✗ | ✗ | ✓ |
| Zero-log prouvable | ✗ | ~ | ✓ |
| Open source | ~ | ✓ | ✓ |

## 2. Le pitch universel (« Harvest Now, Decrypt Later »)

> « En ce moment, des gouvernements stockent tous tes messages chiffrés.
> Quand l'ordinateur quantique arrivera dans 5-10 ans, ils les liront tous.
> Polygone est le seul réseau P2P qui te protège contre ça aujourd'hui. »

Ce pitch parle à tout le monde sans être technique. C'est la porte d'entrée
marketing du projet — le reste est vérifiable dans le code.

## 3. Les 3 angles stratégiques (notes Bear, 11/07)

| Angle | Description | Faisabilité × Impact | Statut |
|---|---|---|---|
| **A. Intégrateur B2B PQC** | Kits de migration PQC en white-label Rust (banques, healthtech, régulé EU/US) | Rentable, court terme | pas démarré — demande du commercial |
| **B. Private AI OS** | LLM local + stockage chiffré fragmenté post-quantique, binaire unique, zéro cloud | Différenciation max, **personne ne fait ça** | **en cours** — msg + drive + brain/petals convergents |
| **C. DID PQC** | Identité décentralisée post-quantique (Nostr bridge, DID `polygone:`) | Niche, marché trop tôt | pas démarré |

**Recommandation (notes + état actuel)** : A quand le B2B sera mûr, B est
en train de se construire — les 3 services live (msg, drive, brain) SONT
le socle du « Private AI OS ».

## 4. Modèle économique

| Phase | Modèle | Cible |
|---|---|---|
| 0-6 mois | Open source + GitHub Sponsors | Communauté dev |
| 6-18 mois | Freemium + consulting B2B | Pilotes régulés |
| 18-36 mois | SaaS « PQC-as-a-Service » + module AI | Scale-up EU/US |

**Pas de token. Pas d'investisseurs.** (Distraction réglementaire MiCA/SEC,
pas nécessaire pour le B2B, contraire aux axiomes.)

## 5. Partenaire cible : Proton

- ADN européen, privacy-first → partenaire idéal.
- Tactique : projet « prêt à l'emploi » + base d'utilisateurs + SDK/Workers
  pour que n'importe quel dev ajoute une messagerie sécurisée à son app.
- Séquençage : buzz communautaire d'abord, approche après.

## 6. Nœuds « fantômes » (RES — idée Bear)

Emprunter les ressources des PC inactifs pour ajouter des nœuds au réseau
P2P décentralisé. Note Bear : « un vrai coup de génie ». Le daemon
(`polygoned`, allocateur CPU/RAM/GPU + policy GlowUp) est déjà le socle
technique — la couche de prêt P2P reste à construire (staging `compute`).

## 7. Ce qu'on NE fait PAS

- Token / ICO / equity crowdfunding — jamais (LEGAL.md).
- Télémétrie / analytics — jamais (trust via vérifiabilité).
- Persistance utilisateur (compte, profil, social graph) — privacy-by-default.
- Cloud-only inference — offline-first.
- Paywall / subscription — MIT, $0, forever.

## 8. Risques (documentés, pas ignorés)

1. **Crypto-agilité** — si ML-KEM-1024 se casse, pouvoir switcher. L'architecture
   doit rester crypto-abstractée (le core est le point d'échange).
2. **Taille des signatures PQC** — ML-DSA-65 = 3309 B (acceptable) ; SLH-DSA = 50 KB
   (rejeté pour la TUI). Bench D2 enregistré dans DECISIONS.md.
3. **Régulation B2B** — clearances ITAR/dual-use dès qu'on vend à des États.
4. **Adoption** — les devs n'aiment pas changer leurs libs crypto → exemples
   concrets + marketing technique (d'où `polygone demo` et `scripts/demo.sh`).

---

*Stratégie consolidée · v2.0.0-rc2 · « On voit rien. Et c'est comme ça que ça devrait être. »*
