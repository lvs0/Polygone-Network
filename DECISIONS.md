# DECISIONS.md — Les 3 blocages en attente de Lévy

> *Le Conseil des Sages produit des recommandations. Lévy décide.*
> *Ce document liste explicitement les 3 décisions qui bloquent le calendrier.*

---

## D1 — Refonte TUI 2 onglets (P-V1)

### Question
Refonte complète du TUI de `Menu + Dashboard + Favoris + Settings` vers **2 onglets** : `Envoyer` et `Quitter` (réglages derrière `:`).

### Options
- **GO** : S5–S7 alloués. Version 3 menu archivé (tag `phase3-archive`).
- **NO** : Phase 3 menu conservé. Pas de refonte.

### Effet
- **GO** — alignement Council (Jobs + Musk). Risque : sur-simplification tue l'utilité (Mitnick peut quand même pivoter l'attaque).
- **NO** — calendrier conservateur. Le Menu reste un produit acceptable.

### Quand trancher
**Avant fin S1** (29 juin → 6 juillet 2026).

### Statut
🟡 **PENDING**.

---

## D2 — Bench perf handshake Sybil-resistance (P-A2 / P-S2)

### Question
`proof_of_key` ML-DSA-65 sur `(PeerID || nonce)`. Bench doit être **≤ 200 µs**.

### Options
- **OK** — phase 4.3 install 1-clic publique procède.
- **KO** — ré-investigation ; possible retour à ML-DSA-87 sur handshake local-LAN.

### Effet
- **OK** — produit shippé, P2 Sybil-resistance active sur réseau public.
- **KO** — 1 sem. retard + ré-architecture keypath.

### Quand trancher
**Fin S2** (≈ 13 juillet 2026).

### Statut
🟡 **PENDING**.

---

## D3 — Lettre État (CNIL / ANSSI / EFF)

### Question
Envoyer 3 lettres ouvertes aux régulateurs sur la posture privacy de Polygone.

### Options
- **OK** — 3 accusés reçus avant S8 fin (≈ 24 août 2026).
- **KO** — ré-écriture + 2ᵉ tentative.

### Effet
- **OK** — posture « Anticipation État » assumée. Pas attaquable par *silence = suspicion*.
- **KO** — escalade Nuit de l'État. Rédaction plus politique.

### Quand trancher
**Pendant S2** (rédaction), **accusé attendu S4-S5**.

### Statut
🟡 **PENDING**.

---

## Convention

- Chaque décision est **binaire** GO/NO-GO.
- Lévy tranche explicitement. Pas d'interprétation flottante.
- Une fois tranchée pour ce cycle de release, irréversible jusqu'au cycle suivant.

---

*Hérite de : `POLYGONE_ROADMAP_v2.md` S1-S8, Conseil des Sages 2026-06-29.*

---

## D4 — Créer Polygone-Protocols sibling (Lévy-blocking)

**Déclencheur** : `~/Par contre tu dois faire.md` (29/06/2026) — la coupe 8→2 du Conseil v1 est infirmée. Lévy défend un **écosystème de petits protocoles** (Petals, Daemon, Browser, RES, Tor+++) bâti sur un Core stable.

**Décision proposée** :
| Champ | Valeur |
|-------|--------|
| **Action** | Créer `/home/l-vs/Projets/Polygone-Protocols/` à côté de `Polygone-Final/`. Chaque sous-dossier = 1 protocole avec son propre README, THREAT_MODEL, LEGAL-check, mais SANS dépendance monolithique sur Core. |
| **Effort** | 1 personne × 1 sem. (squelette + manifesto + 1 protocole-pilote = Petals) |
| **Fichiers à toucher** | `/Projets/Polygone-Protocols/{README.md, AXIOMS.md, petas/SPEC.md, petals/LEGAL.md-check, etc.}` |
| **Owner** | Lévy |
| **Acceptance** | (1) `cargo check` n'est PAS exécuté sur le sibling (pure docs + spec au MVP) ; (2) `pet als/SPEC.md` passe le test Wozniak (lisible par ingénieur extérieur en <30 min) ; (3) Le manifeste des axiomes du sibling est cohérent avec `PHILOSOPHY.md` du Core (Axiome 1 conservé, Axiome 4 inversé). |
| **Dépendance** | D1/D2/D3 levées ou skewées ; le sibling est indépendant du Core sur le plan compilation. |
| **Risque** | Dispersion cognitive — Lévy est seul. Mitigation : limiter le sibling à **1 seul** protocole-pilote (Petals) au MVP. |
| **Hard block** | **D4 — attendant GO de Lévy.** |

**Justification** :
- Conseil v1 (Axiome 4 « coupe 90% ») → erreur (bâti sur Jobs qui aurait été Hibernate par Bach, Orwell et Gödel — voir COUNCIL_V2_RECONSIDERED.md §3.3).
- Conseil v2 (Bach T2) → « ne pas couper, transposer dans une autre octave ».
- Socrate T2 → « la transparence structurelle exige que les pouvoirs soient distribués ».
- Hitchock T2 → « le suspense est meilleur que la promesse ; un sibling sans suspense = app à pre-ship, sans attente ». OK en pratique un sibling *peut* avoir du suspense si on documente clairement ce qui arrive après.

**Statut** : ⏳ PENDING. Lévy à trancher avant de lancer Polygone-Protocols.
