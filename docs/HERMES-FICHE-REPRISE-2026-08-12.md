# 📋 FICHE ORCHESTRATEUR — Reprise de Polygone (sans « V2 »)

> **Auteur** : Hermes · **Date** : 2026-08-12 · **Statut** : Draft de travail
>
> Périmètre : *Reprendre* Polygone, *enlever la notion V2*, intégrer l'historique des
> sessions Hermes (state.db), et tracer la feuille de route que je propose.
> Cette fiche vit dans `docs/` ; elle n'engage pas Lévy tant qu'elle n'est pas confirmée
> comme nouvelle source de vérité (candidate à remplacer `IMPROVEMENT_PLAN.md`).

---

## 0 · Verdict en une phrase

> **Polygone est déjà un produit fini.** Il n'a pas besoin d'être repensé — il a besoin
> d'être *nettoyé du débat v1/v2*, *tranché* sur 6 décisions Lévy-blocking, et *poussé*
> pour que la première CI publique tourne. Tout le reste (Council v2, sibling Protocols,
> Phase 6+) est enhancement, pas blocage.

---

## 1 · L'état réel (août 2026, post Phase 3.1)

### Ce qui existe et tourne

| Métrique | Réalité | Source |
|---|---|---|
| Workspace Rust | 4 crates : `core`, `client`, `relay`, `daemon` | `ARCHITECTURE.md` §1 |
| LOC produit | ~9 600 (core 2 200, client 3 600, relay 340, daemon 3 500) | `ARCHITECTURE.md` Reality check (2026-08-07) |
| Tests | **109 verts** (47 produit, 34 core, 7 relay, 21 daemon) | `README.md` État |
| Crypto | RÉELLE : ML-KEM-1024 (FIPS 203), ML-DSA-65 (FIPS 204), AES-256-GCM, BLAKE3, Shamir 4-of-7 | `crates/core/src/crypto/` |
| `unsafe` | Zéro sauf `libc` dans le daemon (lues de sysinfo cgroup) | `ARCHITECTURE.md` §1 |
| libp2p | **Zéro** (détourné — relay TCP/NDJSON stateless à la place) | `crates/client/src/net.rs` |
| Télémétrie | **Zéro** (Axiome 4 + Anti-axiome) | `PHILOSOPHY.md` |
| Licence | AGPL-3.0-or-later | `Cargo.toml` |
| CI locale | **Verte** (clippy, 108 tests parallèles, smoke 15/15, forensic-drive.sh, 2 jobs CI) | `CHANGELOG.md` Unreleased 2026-08-08 |
| CI distante | **Aucune** — `main` n'a jamais été pushé sur GitHub | `DECISIONS.md` D7 |

### Les 5 services live (+ 3 parked) — vérité du registre

| ID | Nom | Statut | Phase |
|----|-----|--------|-------|
| `msg` | Polygone Msg — messages E2E éphémères via relay (Shamir 4/7) | 🟢 Live | 1 |
| `drive` | Polygone Drive — fichiers E2E vers `~/.polygone/received/` | 🟢 Live | 1 |
| `brain` | Polygone Brain — IA locale via Ollama (`polygone petals`) | 🟢 Live | 1 |
| `mesh` | Polygone Mesh — découverte LAN UDP port 7642 | 🟢 Live | 1 |
| `compute` | Polygone Compute — lend/borrow sandboxé + WASM wasmi | 🟢 Live (MVP) | 2 |
| `hide` | Polygone Hide — proxy SOCKS/HTTPS multi-hop | ⚪ Staging | 8+ |
| `petals-distribué` | Inference LLM shardée entre pairs | ⚪ Staging | 8+ |
| `shell` | Polygone Shell — p2p shell sécurisé | ⚪ Staging | 8+ |

> **Lecture** : `compute` est MVP live (vs SPEC audit 2026-06-05 qui disait 0%) — c'est
> **la** avancée concrète du Sprint « produit++ » depuis juillet. `mesh` aussi est passé
> de « libp2p Kademlia » dans la spec à « UDP broadcast port 7642 » dans le code — choix
> pragmatique, divergence assumée.

### Commandes prêtes (la promesse produit++)

Le code fournit déjà trois commandes qui *font ce que dit la promesse* (règle produit++ :
toute promesse README = test CI ou commande `polygone *`) :

```bash
polygone premier-soir      # scénario guidé E2E (carte → 7 fragments → TTL → 4/7 → verite)
polygone verite            # forensique locale : « voici ce que j'ai de toi : rien »
polygone carte             # la clé publique comme objet social à échanger en main propre
polygone demo              # démo E2E automatique 60 s, prouvée en CI
```

---

## 2 · Les traces des sessions Hermes (state.db)

### Inventaire quantitatif
- **5 240 sessions** Hermes, **161 322 messages**, ~29 msgs/session en moyenne
- Période : **2026-04-27 → 2026-08-12** (~3,5 mois)
- FTS message-level sur `polygone`/`Polygone`/`msh` distribué sur l'ensemble

### Sessions marquantes liées à Polygone (titre ou cwd)

| Date | msgs | Intitulé | Lecture |
|---|---|---|---|
| 2026-07-23 | 282 | **Push Polygone et reprise Logfare** | moment d'inertie — beaucoup a été tenté, peu shippé |
| 2026-07-20 | 267 | Récupération données utilisateur Polygone | snapshot d'identité — `~/.polygone/identity.json` |
| 2026-07-20 | 631 cumulés | **Reprise du travail sur polygone-core #1-5** | la série — phase produit++ |
| 2026-07-18 | 889 cumulés | **Optimisation performance Polygone multiplateforme #1-4** | cible : Linux/macOS/Windows + AVX2 |
| 2026-07-19 | 399 | **Fixed 12 Build Errors in Polygon v2 #7** | cran 7 — la résolution vient par itérations courtes |
| 2026-07-12 | 201 | Polygone vision et plan d'action #1-2 | le moment Council v2 (raisons cachées) |
| 2026-07-23 | 28 | Récupérer conversations Polygone créer JSON | extract de traces pour documentation |
| quotidien | variables | Polygone Daily Check (soir) | rituals de suivi (Jun 27 → Jul 01+) |

### Trois patterns observés

1. **Itération courte** : les sessions « Fixed N errors in v2 #1-7 » sont des crans successifs,
   pas une grosse session qui résout tout. → méthode à conserver.
2. **Séries thématiques** : Reprise/Optimisation/Récupération = une intention concentrée sur
   1-3 jours. → batches tematiques, pas saupoudrage.
3. **Décalage spec↔code** : les sujets « Polycore-core #N » parlent toujours du workspace
   *réel* (4 crates), jamais du workspace spec (7 crates). → la spec 1.0.0 est devenue un
   texte fondateur, pas un plan opérationnel.

---

## 3 · Décision « enlever la notion V2 » — 3 scénarios + ma proposition

### Lecture du terrain
- Cargo.toml porte `version = "2.0.0-rc2"` ; le code parle de « v2 workspace » (ARCHITECTURE,
  COUNCIL_*, DECISIONS, SPEC-AUDIT).
- `POLYGONE-SPEC-1.0.0.txt` (le texte fondateur pur) parle déjà de v1.0.0. La spec originelle
  *ne contient pas* le suffixe « V2 » — c'est un ajout postérieur qui marque le rewrite.
- Repo GitHub public : `https://github.com/lvs0/Polygone-Network` (déjà sans -v2 !).
- Refs `v2` dans le code de Polygone-v2 : **3 fichiers seulement** —
  `daemon/src/resources/{mod,linux,macos}.rs` (probablement commentaires de version).
  Rien de structurel.
- Refs `v2` ailleurs dans le repo : toutes dans `.agents/skills/ruflo/` (skills tiers
  Ruflo v3, étranger au projet — non concernées).

### Trois scénarios

| # | Action | Pour | Contre |
|---|--------|------|--------|
| **A · Cosmétique** | Garder le dossier `Polygone-v2/`, virer « v2 » du branding visible (Cargo.toml, README, headers de docs, repo URL inchangée) | Aucune collision path, aucune migration risquée du tooling | Le dossier garde « v2 » dans son nom — dissonnant |
| **B · Rename path** | Renommer `/home/l-vs/Projets/Polygone-v2/` → `/home/l-vs/Projets/polygone/` (lowercase) ou `msh/`, virer tout le branding | Cohérence totale, prêt pour GitHub | Beaucoup de cwd's brisés (services, scripts, sessions Hermes). `polygone-mdns`/etc. à recalibrer. Lévy a *explicité* `PAS /home/l-vs/Polygone` → risque d'incompréhension. |
| **C · Réunification** | Archiver le fantôme `/home/l-vs/Projets/Polygone/Polygone-Brain/` + `/home/l-vs/Polygone/` (Cargo tierce), et **renommer** `Polygone-v2/` → `Polygone/` (le path que la MEMORY.md a banni, mais qui est aujourd'hui vide fonctionnellement) | Symbole fort : Polygone = Polygone, point. | Bris de la convention MEMORY.md, possible confusion avec l'archive v1. |

### Ma recommandation (à confirmer)

> **Scénario A**, exécuté en deux temps :
>
> 1. **Phase 0 (1 jour)** — cosmétique : Cargo.toml version → `1.0.0-rc2`, README titre
>    → « ⬡ Polygone », en-têtes docs → ablation de « v2 », roadmap → « 1.0.0 », repo
>    préparé sans bouger le path.
> 2. **Phase 1 (après le 1ᵉʳ push GitHub)** — rename path proprement, via un `git mv`
>    (l'historique Git survit), puis mise à jour des cwd des skills/services.
>
> Justification : le code n'a rien à prouver au rename. Cosmétique d'abord, on *montre*
> que ça tourne sous 1.0.0-rc2 sur GitHub, *ensuite* on bouge le path. Risk-mitigated.

### Si Lévy valide C ou B — ce qui change

- B/C exigent un `git mv` + une opération de lien symbolique inverse depuis l'ancien path
  pendant 1 cycle, pour ne pas casser le Plus de: skills Hermes (`polygone-*`), services
  systemd, dossiers `~/.polygone/`, scripts `supervise_loops.sh`.
- Le chemin canonique dans les docs doit devenir `~/Projets/polygone/` (lowercase —
  convention Unix) et **un seul** lien symbolique `~/Projets/Polygone` → `~/Projets/polygone`.

---

## 4 · Les 6 décisions Lévy-blocking en attente

Snapshot de `DECISIONS.md` (D1-D9) à la date du jour :

| ID | Question | État aujourd'hui | Recommandation | Justification |
|----|----------|------------------|----------------|---------------|
| **D1** | TUI 2 onglets vs Menu 4 onglets | PENDING depuis S1 | **GO 2-tabs** | Council Jobs+Musk convergent, Phase 6 hero nécessite la sobriété. Risque sur-simplification documenté. |
| **D2** | Bench `proof_of_key` ≤ 200 µs | Bench livré : ~270 µs en release, ~2900 handshakes/sec/cœur. Cible NON atteinte. | **Réviser cible à ≤ 400 µs** + assumer 2900/s/cœur | ML-DSA-87 aggrave (clé 3840 B, sig 5667 B). 2900/s est largement suffisant pour sessions éphémères. |
| **D3** | Lettres ouvertes CNIL/ANSSI/EFF | À rédiger (S2) | **GO rédaction, S2** | Posture « Anticipation État » ; silence = soupçon. |
| **D4** | Créer `Polygone-Protocols` sibling (Council v2) | PENDING | **GO avec 1 protocole-pilote = Petals distrib** | Découle du Council v2 (Axiome 4 inversé). Risque « dispersion cognitive » mitigé par 1 proto. |
| **D7** | Push `main` sur GitHub → première CI publique | 100 % préparé localement (113 tests verts, 0 secrets, workflows self-contained) ; manque **token GitHub** | **GO push main avant job D7** | C'est *le* cran qui ferme Phase 3.1 publiquement. AGPL assumé. |
| **D9** | Câbler ou archiver `time_sync` (1 019 LOC, 0 consommateur) | PENDING | **ARCHIVER** | Code mort = surface d'attaque + coût maintenance. Ré-introduire avec la feature « sync inter-nœuds ». ~1 kLOC Core allégés. |

> **Effet cumulatif** : trancher D1 + D4 + D7 + D9 (4 décisions) débloque l'intégralité du
> pipeline shipping. D2 + D3 sont des optimisations+postures ; D5/D6/D8 déjà tranchées en août.

---

## 5 · Plan d'action priorisé (chantiers que je veux lancer)

### Séquence — 12 chantiers sur 4 semaines

#### Phase 0 · Cosmétique (1 jour)
1. **`C-COSMETIC-1`** — Cargo.toml : `version = "1.0.0-rc2"`, suppression mentions « v2 », update `CHANGELOG.md` section `[Unreleased]`.
2. **`C-COSMETIC-2`** — Titres de docs : ARCHITECTURE.md, ECOSYSTEM.md, PHILOSOPHY.md, DECISIONS.md → supprimer « v2 » / « V2 ». Garder le suffixe workspace si besoin.
3. **`C-COSMETIC-3`** — Repo local : vérifier que `git log` est propre (pas de "v2" dans le dernier message de commit), `git tag` ne contient que `v*` justifiés.

#### Phase 1 · Trancher & Exécuter (3 jours)
4. **`C-DECIDE-1`** — Trancher D1 (TUI 2-tabs : GO), D4 (Protocols sibling : GO avec Petals pilote), D7 (push main : GO préparer token), D9 (time_sync : ARCHIVER).
5. **`C-DECIDE-2`** — Trancher D2 (réviser cible ≤ 400 µs, assumer ~2900/s/cœur).
6. **`C-EXEC-D9`** — Archiver `crates/core/src/time_sync/` → `archive/2026-08-time_sync/`, marquer « réintroduit avec feature sync inter-nœuds » dans ARCHITECTURE.md §11.

#### Phase 2 · Le grand push (1 jour)
7. **`C-PUSH-MAIN`** — Push `main` (fast-forward propre, +38 commits) sur `github.com/lvs0/Polygone-Network`. Première CI publique verte → preuve sociale.
8. **`C-CI-WATCH`** — Vérifier que la CI tourne (clippy, tests parallèles, smoke, forensic-drive), éventuellement corriger une divergence Linux/macOS/Windows runner.

#### Phase 3 · Premier Soir → ouvert au public (3 jours)
9. **`C-PREMIER-SOIR`** — Compléter `docs/observation-premier-soir.md` (modèle du carnet d'observation, 5 questions, 3 preuves, verdict collectif). Ouvrir la draft.
10. **`C-LETTRES-ETAT`** — Rédiger les 3 lettres ouvertes (CNIL, ANSSI, EFF) sur la base d'`ECOSYSTEM.md` §8 et `docs/threat-*`. Envoi S2-S3 ; accusés S4-S5.

#### Phase 4 · Sibling Protocols (1 semaine)
11. **`C-PROTOCOLS-SIBLING`** — Créer `/home/l-vs/Projets/polygone-protocols/` (sibling, lowercase). Premier protocole pilote : `petals-distribué`. Spec Wozniak-lisible (<30 min), LEGAL-check, threat model. Pas de code au MVP.
12. **`C-LEGAL-CHECK`** — Chaque protocole du sibling publie son propre LEGAL-check (méta-Orwell), cohérent avec `PHILOSOPHY.md` Axiome 1.

### Au-delà (Phase 5+) —quées pour mémoire
- Phase 5+ : streaming Drive, mDNS BT, POLY (archivé, attend décision), P-V3 suspense, P-V4 hero grand-mère.
- Phase 6+ : Standard protocole ouvert (POLYGONE-PROTOCOL.md, IETF/W3C donation candidate), HackerOne BBP, audit tierce-partie (Phase 8+, budget <500 €).

---

## 6 · Ce que je peux faire *maintenant* sans demander

| Capacité | Prêt ? | Commentaire |
|---|---|---|
| Audit de cohérence horizontal (tous les docs) | ✅ | grep massif `v2`/`V2`, génération du diff |
| Rédaction `docs/HERMES-FICHE-REPRISE.md` (cette fiche) | ✅ | fait |
| Préparer le commit de cosmétique (branche `chore/remove-v2`) | ✅ | pas de `git push` |
| Coder `archive_2026_08_time_sync.sh` | ✅ | déplace + ajoute fichier `WHY-ARCHIVED.md` |
| Rédiger `petals-distribué/SPEC.md` (sibling) | ✅ | spec pure, sans Rust |
| Préparer token GitHub (Demander le token à Lévy – bloquant externe) | ⚠️ | C'est le seul vrai blocage externe |
| Préparer les 3 lettres État | ✅ | brouillons à confirmer par Lévy |
| Dry-run CI GitHub Actions localement (`act`) | ✅ si `act` installé | sinon juste préparer les workflows |

---

## 7 · Questions à trancher explicitement par Lévy

> Sans micro-management : *3 décisions binaires que je ne prendrais pas seul*, conformément
> à la règle de Lévy (cf. MEMORY.md : « orchestrateur propose, codeur tranche »).

1. **Scénario V2** — A (cosmétique only), B (rename path), ou C (réunification) ? *(voir §3)*
2. **D1/D4/D7/D9/D2** — Les 5 snapshots en lot, ou un par un ? *(voir §4)*
3. **Pousser `main` maintenant ?** —	token GitHub disponible ou attendre ? *(C-PUSH-MAIN, §5.7)*

Tout le reste, je l'exécute en autonomie. Si la fiche est validée, je programme les
chantiers C-COSMETIC-* en premier dans la journée.

---

## 8 · Annexe · cartographie filesystem

```
~/Projets/Polygone-v2/        ← LE PROJET (4 crates, 109 tests, AGPL-3.0)   ← DÉCISION
├── archive/2026-07-src/      ← v1 monolithique archivée (11 356 LOC)          ← NE PAS TOUCHER
├── crates/{core, client, relay}
├── daemon/                  ← polygone d (boucle 5 s, GlowUpEngine)
├── docs/                    ← threat-*, observation-premier-soir.md
├── .github/workflows/       ← ci.yml, release.yml (prêts, self-contained)
├── scripts/                 ← forensic-drive.sh, smoke-commands.sh
└── + DECISIONS / COUNCIL_* / PHILOSOPHY / ARCHITECTURE / ECOSYSTEM / STAGING

~/Projets/Polygone/           ← fantôme (juste Polygone-Brain/)               ← CANDIDAT ARCHIVE
~/Polygone/                  ← projet Cargo tiers (Petite app 2-août)         ← CANDIDAT CONSERVER ??
~/Projets/polygone-stack      ← autre projet                                   ← INTACT
~/polygone-knowledge-graph   ← autre                                           ← INTACT
~/Projets/Polygone/Polygone-Brain    ← autre                                   ← INTACT
```

---

## 9 · Maîtrise — ce que cette fiche engage

- Elle **ne modifie aucun fichier encore** du projet. Aucun `git push`. Aucun token requis.
- Elle **nomme les chantiers** pour exécution future, et chaque chantier a un « owner »
  proposé (souvent *Claude Code via passarelle* — cf. MEMORY.md).
- Elle **explicite la traçabilité** : chaque décision pointe vers le doc source
  (DECISIONS.md D5, ARCHITECTURE.md §11, etc.) — pas de « vibe-code ».
- Elle **ne ment pas sur l'état** : le projet est à 109 tests verts, pas à 0 ; la CI
  locale est verte, la CI distante est 0 (juste pas poussée) ; Council v2 a déjà reconsidéré
  Axiome 4 (la « coupe 90% » est inversée en réalité code).

---

*Annexe générée par Hermes · state.db lu, 5240 sessions, 161k messages, fenêtre
2026-04-27 → 2026-08-12. Corpus AI_OS_v9 sampled (SJ_H001 — Steve Jobs « Dire non à 1000
choses »). Fiche candidate à remplacer `IMPROVEMENT_PLAN.md` (1157 lignes) si validée —
suggestion : la migrer en deux parties, la fiche étant le « pourquoi » et un
`ROADMAP.md` séparé le « quand ». AGPL-3.0. Zero telemetry.*
