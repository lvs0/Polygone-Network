# ADR-0004 — Réconciliation `github/main` ↔ `reconcile/2026-11` : arbitrage fichier par fichier

- **Statut** : proposé (2026-11-19) — sera « accepté » à la fusion de la PR #4
- **Décideurs** : LVS (mandat « continue en toute autonomie sur Polygone », 2026-11-19)
- **Exécution** : CORVUS, session du 19/11 (soir)
- **Prolonge** : ADR-0001 (base canonique, règle « plus récente, testée ») ; ADR-0002 (D2) ; ADR-0003 (L1)

## 1. Contexte

La PR #4 (`lvs0/Polygone-Network`, `reconcile/2026-11` → `main`) est `CONFLICTING` :
20 fichiers en conflit, mesurés par `git merge-tree --write-tree --name-only github/main HEAD`.

Les deux lignées ont un ancêtre commun (`060c11d`, 2026-09-02) :

| Lignée | Contenu | Commits hors de l'autre |
|---|---|---|
| `github/main` (`23d27ac`) | septembre 2026 : scaffold petals/gateway, SUPERPAW (scripts + 15 rôles), « Shamir 4/7 réel », port Windows, stubs web-pwa | 17 |
| `reconcile/2026-11` (`c13cdf6`) | base canonique `f3d09c0` (29 oct.) + realign/v2 + p0 + travail des 9–19 nov. : 586 tests, release v2.1.0, preuve deux hôtes | 119 |

`f3d09c0` (base canonique ADR-0001) **n'est pas** dans `github/main` : la lignée
GitHub a divergé le 2 septembre, avant la base canonique. Elle ne contient donc
aucun travail d'octobre–novembre.

## 2. Règle appliquée (rejeu d'ADR-0001, inspecté — pas de `-X ours` global)

> Pour tout fichier qui existe dans `reconcile/2026-11`, la version
> `reconcile/2026-11` est retenue : postérieure, testée (586), prouvée en
> production (release v2.1.0, deux hôtes). La lignée septembre n'apporte que ses
> **ajouts purs**, à l'exception des fichiers mock incompatibles (§4).

Chaque fichier conflictuel ou auto-fusionné dans du code vivant a été inspecté
avant arbitrage (§3). Les ajouts purs de `main` sont conservés tels quels comme
**couche historique de septembre** (§5) — précédent : `archive/2026-07-src/`
conservé par ADR-0001.

## 3. Table d'arbitrage — 20 conflits + 11 auto-fusions inspectées

### 3.1 Conflits (20) — tous retenus côté `reconcile/2026-11`

| Fichier | Justification inspectée |
|---|---|
| `.github/workflows/release.yml` | version HEAD prouvée par la release v2.1.0 (4 cibles, cross macOS, SHA256SUMS, dry-run) |
| `Cargo.lock`, `Cargo.toml` | workspace HEAD (10 crates, dont `polygone-serverless`) ; membres inchangés par la fusion → lock HEAD cohérent |
| `crates/client/src/serverless.rs` | HEAD = façade honnête vers `polygone-serverless` (vrai Shamir 4/7, quorum, « aucun nœud n'est contacté »). `main` (94 l., sept.) appelle `core::crypto::shamir` directement avec une API incompatible (`ServerlessRequest::new` vs `run(commande, payload, seuil, parts)`) |
| `crates/gateway/Cargo.toml`, `src/lib.rs`, `src/main.rs` | HEAD = gateway nov. (tasks, diagnostics, evidence) ; `main` = scaffold sept. |
| `crates/petals/Cargo.toml` | le bug `[[bench]] inference_bench` sans fichier est **déjà corrigé dans HEAD** (commentaire l.14, retrait documenté) ; le fix `dd1c83b` de main est subsumé |
| `crates/petals/examples/integration_test.rs`, `src/backends.rs`, `backends/ollama.rs`, `backends/vllm.rs`, `src/daemon.rs`, `src/lib.rs`, `src/models.rs`, `src/types.rs` | API petals nov. (pipeline, backends réels) ; la version sept. est l'ancêtre non testé |
| `daemon/src/socket.rs` | HEAD = ledger + negotiation (nov., prouvé deux hôtes) ; `main` = API sept. (`notify_allocation`, `chrono_lite_timestamp`…) |
| `install.sh` | HEAD contient déjà le fail-closed P8 (`a796e83`) **et plus** : refus si empreinte absente/ambiguë/malformée, vérification avant install (l.301-419) |
| `scripts/hide-smoke.sh`, `scripts/install.sh` | versions nov. associées à l'install vérifié |

### 3.2 Auto-fusions dans du code vivant (11) — reverties vers HEAD, inspection à l'appui

| Fichier | Ce que `main` apportait | Arbitrage |
|---|---|---|
| `crates/client/src/main.rs` | **conflit sémantique** : double bras `Serverless` (l.1438 `serverless::run` HEAD + l.1505 `ServerlessRequest::new` main → ne compile pas) ; résurrection de `mod brain`/`mod drive` démontés par `a9be256` | HEAD |
| `scripts/smoke-commands.sh` | stub 22 lignes remplaçant le contrat « produit++ » 245 lignes **exécuté par la CI** | HEAD |
| `daemon/src/resources/mod.rs` | supprime le `compile_error!` Windows et branche `WindowsPlatform` — le canari Windows « échec attendu et documenté » est une décision produit HEAD | HEAD |
| `daemon/src/resources/windows.rs`, `macos.rs` | port Windows sept. non vérifié | HEAD |
| `daemon/src/bandwidth.rs` | skip `NotFound` dans `read_counters_on_lo`, redondant avec le gate `#[cfg(target_os = "linux")]` de HEAD (`26c52d9`) | HEAD |
| `crates/client/src/duress.rs` | sanitisation des noms de thread pour chemins Windows (tests) — plateforme non supportée par décision produit | HEAD |
| `crates/client/src/brain.rs`, `drive.rs` | modules morts (aucune déclaration `mod` dans HEAD) | HEAD |
| `SPEC.md`, `index.html` | versions sept. de fichiers existants | HEAD |

### 3.3 Exclusions (2)

| Fichier | Raison |
|---|---|
| `crates/petals/src/backends/mock.rs` | orphelin : `backends.rs` retenu (HEAD) ne déclare pas `mod mock` → code mort |
| `crates/petals/tests/mock_tests.rs` | ne compilerait pas contre l'API petals retenue ; et des tests qui testent un mock sont une assurance fausse (audit de véracité) |

## 4. Résultat de la fusion

Arbre fusionné = **arbre `c13cdf6` à l'identique pour tout fichier préexistant**
+ 24 ajouts purs de la lignée septembre (aucun `.rs`) :

- `DOSSIER-POLYGONE-DREAM.md`, `GOVERNANCE.md`, `docs/COMPANY.md`, `docs/RAPPORT-PRE-SHIP-2026-09-10.md`
- `scripts/company-loop.sh` + `scripts/company/<15 rôles>/ROLE.md`
- `web-pwa/` (4 fichiers)

Aucun code Rust n'est modifié par la fusion → la suite de tests et les preuves
de `c13cdf6` s'appliquent telles quelles à l'arbre fusionné (revérifié : §6).

## 5. Statut de la couche septembre

Ces fichiers sont des **instantanés historiques** (2026-09-06 → 09-11), pas la
gouvernance ni le produit actuels :

- `GOVERNANCE.md` / `scripts/company/` (13→15 rôles, SUPERPAW) : vision
  d'organisation de septembre. L'état réel fait foi : `project/STATUS.md`.
- `web-pwa/sw.js` : stub pass-through (3 lignes, `fetch → fetch`, aucun cache) —
  **ne satisfait pas** la promesse PWA offline. La console produit est `web/`.
  `web-pwa/HOME-TV-BRIEF.md` est un brief hors sujet (HomeTV), conservé comme
  trace historique.
- Aucun de ces fichiers n'est référencé par la CI, le build ou les scripts
  produit de HEAD (vérifié : ils n'existaient pas dans HEAD).

## 6. Vérification (avant commit de fusion) — mesurée le 2026-11-19

- `git diff --name-only --diff-filter=U` → **0** conflit restant
- `git diff --name-status HEAD` → **24 `A`**, zéro `M` (tout préexistant = HEAD) ;
  `git diff --name-only HEAD -- '*.rs'` → **0**
- `cargo fmt --all -- --check` → **OK**
- `cargo clippy --workspace --all-targets -j 2 -- -D warnings` → **RC=0**
- `TMPDIR=/tmp cargo test --workspace --no-fail-fast -j 2` → **RC=0, 24 suites,
  586 passed, 0 failed, 0 ignored** (compte identique à `c13cdf6`)
- Premier passage : 1 échec `doc_index` — les deux nouveaux documents `docs/`
  n'étaient pas indexés. Corrigé : entrées `daté` dans `docs/INDEX.md` + bandeaux
  « Instantané daté » (DOC.3), puis suite complète verte.
- `scripts/check-docs.mjs --tests 586` : un écart préexistant révélé —
  ARCHITECTURE.md déclarait 44 271 lignes, mesure 44 332 (compteur non remis à
  jour après `8352c5c`). Corrigé avec la mesure (client 16 830, daemon 10 991).
- `scripts/check-ci-honesty.mjs` → **OK** (6 jobs, Windows canari non bloquant)
- `gitleaks git .` → **313 commits, aucune fuite, RC=0**
- Reste avant fusion de la PR : push + CI `workflow_dispatch` verte sur l'arbre
  fusionné (gate §9.5)

## 7. Retour arrière

- Avant fusion : `reconcile/2026-11` = `c13cdf6` = `github/reconcile/2026-11`
  (arbre propre, tout poussé). `git reset --hard c13cdf6` restaure l'état initial.
- La fusion **préserve les deux histoires** (merge commit, aucun force-push,
  aucun rewrite). `github/main` avant fusion = `23d27ac`.
- Après fusion de la PR #4 : retour arrière = `git revert -m 1 <merge-commit>`
  sur `main` (nouveau commit, pas de rewrite).

## 8. Conséquences

- La PR #4 devient mergeable ; « réconciliation github/main » sort du backlog.
- Le dépôt public porte enfin la lignée canonique auditée, sans perdre la
  couche septembre (histoire préservée des deux côtés).
- Les promesses non tenues de septembre (Windows, PWA offline, mocks) restent
  **non tenues** : cette fusion ne les promeut pas, elle les archive.
- Prochaines missions inchangées : RTT/débit inter-hôtes → ordonnanceur,
  F1-API-01, LAN/NAT, quorum L2 (second opérateur).