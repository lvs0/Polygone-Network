# LEGAL.md — Posture légale de Polygone

> *Document associé à `LICENSE` (MIT) et `ECOSYSTEM.md`.*
> *Lisible par tous · Citable dans une procédure judiciaire.*
> *Date : 2026-06-29 · Statut : v0.2 · Posture « honest-first » (Conseil des Sages 2026-06-29, S5).*

---

## 1. Ce que Polygone est

Polygone est :

- **Un logiciel libre** distribué sous **licence MIT**.
- **Un écosystème d'outils** de chiffrement post-quantique : ML-KEM-1024 (FIPS 203) · ML-DSA-65 (FIPS 204) · AES-256-GCM · Shamir 4-of-7 · BLAKE3.
- **Un réseau peer-to-peer décentralisé**. Aucun serveur central n'est opéré par Polygone.

Polygone est écrit en Rust (édition 2021), auditable en une seule commande (`cargo audit`, `cargo clippy`, `cargo test`).

## 2. Ce que Polygone n'est PAS

Polygone **n'est pas** :

- ❌ Un service cloud. Polygone n'opère aucun serveur central.
- ❌ Un produit commercial. Pas d'abonnement. Pas de token tradable. Pas de NFT.
- ❌ Un substitut à un conseil juridique.
- ❌ Une promesse d'anonymat absolu. Voir §3.

## 3. Modèle de menace — résumé

| Audience | Adversaire typique | Ce que Polygone protège | Ce que Polygone ne protège PAS |
|----------|--------------------|------------------------|-------------------------------|
| **Utilisateur quotidien** | FAI curieux, GAFA, colocataire, malware opportuniste | Lien message ↔ identité chiffré en clair (ML-KEM-1024) · Persistance des messages après TTL | Keylogger sur la machine endpoint · Malware persistant · Coercition physique |
| **Journaliste / activiste** | Adversaire étatique, FAI national, observateur réseau | Pattern de messagerie hors TTL (≤ 30 s sur relay) · Mécanisme de subpoena documenté (`docs/kill-switch.md`) | Reconnaissance physique · Ingénierie sociale (Mitnick) |
| **Dissident organisé** | Adversaire étatique avec subpoena et accès physique | Contenu chiffré de bout-en-bout (mathématiquement hors-atteinte) · Mode duress documenté (§5) | Divulgation humaine forcée (rubber-hose attack) |

Détail complet dans :

- `docs/threat-commodity.md` — utilisateur quotidien (P3, livrable S2)
- `docs/threat-high-value.md` — dissident (P3, livrable S2)

**Ce que Polygone ne prétend pas** : rendre l'information *ontologiquement* impossible à observer pour un adversaire global. La métaphore « l'information n'existe pas » du site signifie **« aucun fragment reconstructible sans 4-of-7 »** — claim vérifiable, narrow, tenable.

## 4. Politique face à une demande étatique (subpoena)

`polygone-server` (le relay public optionnel) peut recevoir une demande légale de gel et/ou de divulgation.

### 4.1 — Ce que Polygone peut produire

- Des logs de connexion IP **agrégés**, jamais par utilisateur identifiable.
- Un accusé de réception de la demande.

### 4.2 — Ce que Polygone ne peut PAS produire

- Le contenu d'un message chiffré de bout-en-bout. **Impossible par construction.**
- L'identité réelle d'un expéditeur. ML-DSA-65 contient un `PeerID` cryptographique, pas une identité civile.
- Une clé privée ou un secret post-quantique. Les fragments sont dispersés en 4-of-7 Shamir ; aucun nœud ne possède l'intégralité.

### 4.3 — Délai de réponse

**24 heures ouvrées** après réception d'une demande conforme au droit applicable.

Contact : voir `.well-known/security.txt` (RFC 9116).

## 5. Mode duress

Polygone peut être configuré pour autodétruire ses clés et fragments locaux
sur réception d'un signal matériel spécifique. Voir `docs/kill-switch.md`.

**L'implémentation n'est volontairement pas détaillée dans ce repo public.**

## 6. Disclosure responsable — vulnérabilités

Si vous découvrez une vulnérabilité dans Polygone, contactez-nous via :

- **E-mail** : voir `.well-known/security.txt`
- **Chiffrement** : clé PGP listée dans `security.txt`
- **Acknowledgment** : un hall of fame public est publié sur `https://polygone.network/security/halloffame`

Pas de programme de bug bounty avec финансовое-incitatif (pas de budget). Cohérent avec « pas de token, pas de collecte ».

## 7. Licence

MIT. Voir `LICENSE`.

```
MIT License

Copyright (c) 2026 Lévy & Polygone contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
```

## 8. Pas de garantie

```
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

*Fin de `LEGAL.md` v0.2.*
*Hérite de : `ECOSYSTEM.md`, `POLYGONE_EXECUTION_PLAN.md` (S5), `Conseil des Sages 2026-06-29` (R5).*
