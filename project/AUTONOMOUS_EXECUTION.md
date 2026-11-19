# AUTONOMOUS_EXECUTION — état de reprise pour un agent qui n'a pas vu la session

> Fichier de continuité. Il dit **ce qui est fait, ce qui est prouvé, ce qui est
> bloqué, et la commande exacte pour reprendre**. Il ne remplace pas
> [`STATUS.md`](STATUS.md) (état de mission) ni `../STATE.md` (journal des
> vérifications) : il les oriente.
>
> Règle : aucune ligne « terminé » sans commande et sortie associées.
> Dernière mise à jour : **2026-11-19** (production sur mandat opérateur :
> WIP commité, D2 tranchée, preuve L1 persistée, test deux hôtes exécuté,
> push + PR #4, CI corrigée, version 2.1.0 — état complet dans `STATUS.md` §11).
> Le mandat du 2026-11-19 (« finir la production en toute autonomie ») lève
> l'interdiction de push/release pour cette mission ; la contrainte « aucun
> déploiement ni release automatique » ci-dessous vaut hors mandat explicite.

## 1. Objectif général

Faire progresser Polygone Compute vers des garanties P0 **démontrées** :
confidentialité de bout en bout, sandbox réelle, vérification indépendante,
validation sur deux machines, planificateur mesuré, puis supervision sans
compteur fictif. Babel reste gelé tant que `DEC-028` n'est pas tranchée par un
humain.

Contraintes non négociables : un seul Compute visible, aucun remplacement
architectural non justifié, aucun contournement de sécurité, aucun test déclaré
réussi sans exécution, aucun déploiement ni release automatique.

## 2. Où travailler

| Élément | Valeur |
|---|---|
| Dépôt | `~/Polygone-canonical` |
| Branche | `reconcile/2026-11` |
| Dernier commit | `f6382e6` (aucun commit créé pendant les sessions P0) |
| État worktree | modifications non commitées : Compute E2E, sandbox, L1, docs |

## 3. État actuel — 2026-11-15

### Terminé et vérifié dans cette session

| Tâche | Preuve |
|---|---|
| `MemorySwapMax=0` ajouté à la sandbox | Sans lui le swap absorbait l'allocation et `MemoryMax` semblait inopérant. |
| Cause réelle de la « limite mémoire non appliquée » identifiée | Mesure discriminante : 16 Mo survivent sous 32 Mo, 96 Mo touchés sont tués (exit 1), 96 Mo sans plafond survivent. |
| Vérification **L1** par recalcul local, opt-in `--verifier-local` | `net::annotate_local_recomputation` + 3 tests ; divergence ⇒ niveau L0 conservé et commande en échec. |
| Effet réel de `CPUQuota` mesuré | 48 Mio SHA-256 : 3,11 s à 100 % → 13,04 s à 10 % (**4,2x** ; seconde mesure 2,63 s → 13,06 s, **5,0x**). |
| Tests sandbox portés à 12 | `cargo test -p polygone-client --bin polygone exec::tests::` → **12 réussis**. |
| Rotation/révocation testée | `node_id` dérive de la clé de signature : la rotation change l'adresse ; un grant signé par la nouvelle clé ne peut pas revendiquer l'ancien `node_id`. La révocation reste **locale** (limite documentée). |
| Suite workspace | `cargo test --workspace --no-fail-fast` → **565 réussis, 0 échec**. |
| Documentation alignée | P0, `STATUS.md`, `ARCHITECTURE.md` (100 fichiers / 43 350 lignes / 565 tests), `docs/INDEX.md`. |
| Supervision agrégée — `GET /api/v1/diagnostics` | `crates/gateway/src/diagnostics.rs` ; 2 tests ; réseau mesuré, 10 états de tâche, `daemon`/`refusals` `available: false` motivés, aucune clé recopiée. |
| Device Reuse — proposition de rôles | `crates/core/src/reuse.rs` (8 tests) + section « Rôles proposés » de `polygone doctor` (1 test) ; aucune activation, mesure absente = « contrainte non évaluée ». |
| Console PWA de supervision | Panneau « Diagnostic agrégé » dans `web/node.html` + `web/dashboard.js` ; `node --check` 0 ; appel HTTP réel de `/api/v1/diagnostics`, `node.html` 200 ; « — » si la route manque. |

### En cours

Aucun travail à moitié fait. Le prochain agent peut attaquer directement le §6.

Device Reuse (volet proposition) est livré : `polygone doctor` affiche quatre
rôles motivés à partir des mesures du daemon. Aucun rôle n'est activé, aucune
API ne l'expose.

### Bloqué (ressource ou décision humaine)

| Blocage | Motif exact |
|---|---|
| Validation physique deux hôtes | Un seul hôte Linux avec shell (`Hope`). L'iPhone actif sur Tailscale n'exécute pas le binaire. |
| Capture `tcpdump` du relais réel | Exige la validation deux hôtes. Le test E2E prouve la construction cryptographique, pas l'absence de fuite sur un déploiement physique. |
| Quorum / second opérateur (L2) | Aucun opérateur indépendant disponible. Un quorum de nœuds du même opérateur ne prouve rien. |
| TEE / preuve d'exécution (L3) | Aucun TEE sur l'hôte. |
| Babel | `DEC-028` non tranchée — décision humaine. Aucun code Babel ajouté. |
| Planificateur | RTT, débit et disponibilité inter-hôtes non mesurables sans second hôte. |

## 4. Décisions prises

1. **L1 = recalcul local explicite.** Le demandeur déclare la tâche déterministe
   (`--verifier-local` avec `--executer`) ; il réexécute la même commande dans
   le même sandbox et compare les sorties. Le grant est annoté
   `recompute.{level,match,grant_output_blake3,local_output_blake3}`.
   Divergence ⇒ `verification` reste `producer-signed`, la commande échoue et le
   producteur est enregistré en échec de réputation.
   Pourquoi : c'est la seule vérification indépendante réalisable avec un hôte ;
   elle ne prétend pas remplacer un quorum.
2. **`MemorySwapMax=0` est obligatoire**, pas décoratif : sans lui le plafond
   mémoire fuit vers le swap.
3. **`RLIMIT_AS` reste** comme seconde barrière, pour les hôtes où le
   contrôleur mémoire n'est pas délégué au gestionnaire utilisateur.
4. **Aucun fallback plaintext** : version inconnue, `kem_ct` absent ou grant en
   clair sont refusés.
5. **Aucun compteur simulé** : la supervision n'est pas commencée plutôt que
   d'afficher une latence inventée.
6. **Fabric reste lab opt-in** (`publish=false`, `--features lab`).

## 5. Tests exécutés (commandes exactes)

```bash
cd ~/Polygone-canonical
cargo test -p polygone-client --bin polygone net::tests::        # 46 réussis
cargo test -p polygone-client --bin polygone exec::tests:: -- --nocapture
                                                                 # 12 réussis, mesure CPU affichée
cargo test -p polygone-client --bin polygone compute_grant_tests # 2 réussis
cargo test --workspace --no-fail-fast                            # 565 réussis, 0 échec
cargo test -p polygone-core reuse::                              # 8 réussis
cargo test -p polygone-gateway diagnostics                       # 2 réussis
cargo test -p polygone-client --bin polygone doctor               # 8 réussis
cargo clippy --workspace --all-targets -- -D warnings             # exit 0
cargo fmt --all -- --check                                       # exit 0
node scripts/check-docs.mjs --tests 565                          # exit 0
git diff --check                                                 # exit 0
```

Le test `wasm_runs_and_captures_stdout` s'auto-skippe (pas de `test.wasm`) ;
c'est un skip déclaré, pas un succès.

## 6. Prochaine tâche, dans l'ordre

1. **P1 (L2) — quorum vérifiable dès qu'un second hôte existe** : deux
   opérateurs distincts, même tâche déterministe, comparaison des sorties et
   refus si désaccord. Sans second opérateur, ne pas l'implémenter.
2. **P0 — exécuter `docs/P0-DEUX-MACHINES.md` §9** sur deux hôtes Linux
   distincts, en priorité la capture `tcpdump` du relais : aucune tâche ni
   sortie en clair.
3. **P2 — mesures inter-hôtes** (RTT, débit, disponibilité) avant tout
   planificateur : sans ces chiffres, un planificateur serait une précision
   fictive.
4. ~~**P5 — endpoint JSON de supervision**~~ **FAIT le 2026-11-15 (volet
   gateway)** : `GET /api/v1/diagnostics`. Restent absents : un flux
   d'événements, un historique de métriques, et l'exposition des compteurs de
   refus par la gateway (ils vivent dans le daemon, `refusals_request`).
   Prochaine tâche locale : un bus d'événements en lecture seule, ou un
   historique borné des mesures — sans inventer de chiffre.
5. **P0 restant** — tester l'interruption en cours de session (demandeur qui
   disparaît entre `kem_hello` et `req`) sur le déploiement deux hôtes.
   La rotation/révocation est testée ; la révocation distribuée exige une
   décision de protocole (aucune liste n'est diffusée aujourd'hui).

## 7. Procédure de reprise

```bash
cd ~/Polygone-canonical
git status --short && git branch --show-current
sed -n '1,200p' project/STATUS.md
sed -n '1,200p' docs/P0-COMPUTE-SECURITY-2026-11-14.md
cargo test --workspace --no-fail-fast
```

Si le contexte est presque épuisé : mettre à jour ce fichier et `STATUS.md`
**avant** de lancer une commande longue. Une session ne se relance pas
elle-même (`freebuff` n'a pas de mode non interactif) ; la continuité repose
uniquement sur ce fichier.

## 8. Ce que ce fichier n'autorise pas

- Publier, pousser, déployer ou créer une release.
- Trancher `DEC-028`.- Déclarer P0 satisfait : la validation physique, la capture du relais et le quorum restent absents.

## 9. Discipline d'ingénierie opposable (audit Akash, 2026-11-15)

> Ces règles ne remplacent ni `docs/atlas/agent/ORDRE.md` (constitution),
> ni `CONTRIBUTING.md`, ni `docs/testing/TEST-STRATEGY.md`. Elles fixent la
> **couche d'exécution** : ce qu'un agent doit lire, faire et prouver avant de
> toucher ce dépôt. Elles ne créent pas un second système d'instructions et ne
> modifient aucune décision enregistrée.

### 9.1 Avant d'écrire une ligne — lecture obligatoire

| # | Geste | Ce qu'il empêche |
|---|---|---|
| 1 | `git status --short && git branch --show-current && git log --oneline -5` | confondre ton travail avec des modifications non commitées laissées par un autre agent |
| 2 | lire le module touché : source, tests voisins, et `docs/architecture/API-CONTRACTS.md` pour la surface concernée | casser un contrat classé `stable` sans ADR |
| 3 | `rg -n '<nom de fonction|struct|route|fichier>'` sur tout le workspace | créer un doublon fonctionnel |
| 4 | lire `docs/decisions/`, `DECISIONS.md`, `NEEDS-LEVY.md` | renverser une décision canonique par un commit |
| 5 | lire `project/STATUS.md` §3 et §5, et la tâche en cours dans `backlog.json` | reprendre en parallèle une tâche déjà en vol |

**Interdits.** Créer un fichier, un type, une route ou un binaire dont
l'équivalent existe déjà. Créer une seconde implémentation « pour comparer ».
Éditer un fichier de secrets. Modifier `~/.polygone/identity.json`.

### 9.2 Preuve avant déclaration

- Aucun `✅`, `FAIT`, `TERMINÉ`, `RÉSOLU` sans **commande + code de sortie +
  sortie citée + date** (§5 de ce fichier est le modèle).
- Un test qui s'auto-skippe n'est pas un succès : `wasm_runs_and_captures_stdout`
  reste un **skip déclaré**.
- Un test d'un niveau inférieur ne remplace jamais un niveau supérieur
  (`docs/testing/TEST-STRATEGY.md` §1).
- Un échec se rapporte comme un échec, avec la sortie. Une commande non lancée
  s'écrit « non vérifié ».
- Toute affirmation sur un compteur (fichiers, lignes, tests) est recomputée par
  `node scripts/check-docs.mjs --tests <N>`, jamais recopiée d'un rapport ancien.

### 9.3 Tests : ciblé d'abord, régression ensuite

1. Test ciblé du crate touché :
   `cargo test -p <crate> <filtre> -- --nocapture`.
2. Régression complète : `cargo test --workspace --no-fail-fast`.
3. Portes statiques : `cargo fmt --all -- --check` puis
   `cargo clippy --workspace --all-targets -- -D warnings` (jamais contournées
   par un `#[allow]` ajouté pour faire passer la CI).
4. Un correctif de bug ajoute un test qui **échoue avant** et passe après ;
   un test de refus nomme le refus (pas seulement l'absence de résultat).
5. Compteurs ou documents touchés ⇒ `node scripts/check-docs.mjs --tests <N>`
   et `node scripts/check-ci-honesty.mjs`.

### 9.4 Multi-machine : le mot est réservé

- L2 = plusieurs processus sur une machine. L3 = **deux hôtes physiques**.
- Aucune formulation « validé multi-machine », « réseau réel » ou « deux
  machines » sans journal daté, hôtes identifiés, commande reproductible.
- Un conteneur, une VM sur le même noyau ou un tunnel loopback ne comptent pas
  comme second hôte.
- Tant que §3 (Bloqué) n'a pas changé, P0 et le quorum restent **non prouvés**,
  quel que soit le nombre de tests verts locaux.

### 9.5 Dépendances et dette

- Aucune dépendance nouvelle sans justification écrite : ce que la bibliothèque
  standard ne fait pas, licence, maintenance, taille, version minimale.
- Vérifier l'effet réel : `cargo tree -p <crate>` avant/après, et le poids du
  binaire si la dépendance est embarquée.
- Une dépendance ajoutée mais non appelée est de la dette : la retirer ou
  l'assumer explicitement dans ce fichier.
- Pas de `cargo update` global non demandé ; pas de changement de version de
  Rust ou d'édition sans décision enregistrée.

### 9.6 Décisions et traçabilité

- Toute modification de protocole filaire, de format persisté, de contrat
  d'API ou de modèle de menace produit un **ADR** dans `docs/decisions/` et une
  mise à jour de `docs/architecture/API-CONTRACTS.md` (avec sa classe de
  stabilité).
- Une décision existante ne se contredit pas dans le code : elle se remplace par
  une décision datée qui cite l'ancienne.
- Les documents historiques ne sont pas réécrits ; ils portent leur bandeau
  `Instantané daté` (DOC.3) et renvoient à `ETAT-REEL.md`/`STATE.md`.

### 9.7 Agents concurrents

- **Un seul écrivain par arbre de travail.** Avant de commencer, vérifier
  qu'aucune autre session n'a de tâche en cours (§9.1-5) ; sinon attendre ou
  choisir une tâche disjointe.
- Une tâche = un identifiant (`F1-*`, `P0`, `P1`) = un ensemble de fichiers
  annoncé avant édition. Pas de « et aussi j'ai corrigé ».
- Ne jamais commiter, formater ou supprimer le travail non commité d'un autre
  agent. `git diff --check` avant toute écriture de fichier.
- Une branche par sujet, jamais deux implémentations concurrentes du même
  contrat dans deux branches non fusionnées.

## 10. Provenance des règles (ce qui a été copié d'Akash, et ce qui ne l'a pas été)

Chaque règle du §9 vient d'un fichier Akash lisible aujourd'hui ; aucune n'est
une inférence de README marketing.

| Règle Polygone | Source Akash vérifiée |
|---|---|
| Lecture du module et des conventions avant modification | `akash-network/console/AGENTS.md` (description du monorepo + renvois `./.claude/instructions/*`), `console-air/CLAUDE.md` |
| Un seul fichier de règles, mis à jour au lieu d'être dupliqué | `console/AGENTS.md` agrège les RFC ; `console/CLAUDE.md` porte le même contenu (9 356 octets chacun) |
| Interdiction de contourner une porte CI | `console/AGENTS.md` : « NOT ALLOWED to add any `ignore-*` label » |
| Seuil mesuré sur le code modifié | `console/AGENTS.md` + `.github/workflows/reusable-validate-app.yml` : mutation ≥ 80 % sur les lignes du PR |
| Reprise après mise à jour | `provider/.github/workflows/tests.yaml` : `script/test-provider-recovery-upgrade.sh` |
| E2E découpé, image construite une fois, artefact réutilisé | `provider/.github/workflows/integration-tests.yaml` (`plan` → `build-image` → `e2e`, `retention-days: 7`) |
| Mises à jour et invariants de simulation testés avant merge | `node/.github/workflows/tests.yaml` : jobs `sims`, `network-upgrade-names`, `network-upgrade` |
| Release en essai à blanc avant tout déclenchement | `node`/`provider` `.github/workflows/tests.yaml` : `release-dry-run`, puis `dispatch-release` qui en dépend |
| Outil d'audit hors consensus, à blanc, artefacts canoniques | `auditor-cli/README.md` + `cmd/auditor-cli/evidence.schema.json` + `Makefile` (`test`, `vet`, `build`, snapshot goreleaser) |
| Enums typés au lieu de chaînes libres pour les états | AEP-86 §*Typed Reasons and Fault Attribution* (`FaultAttribution`, `AttestationRevocationReason`) |
| Périmètre différé déclaré, pas simulé | AEP-86 §*Signed Code Verification* : signature non appliquée en v1, reportée explicitement |
| Chaîne d'approvisionnement ralentie volontairement | `console-air/CLAUDE.md` : `min-release-age=7` dans `.npmrc` |
| Commits conventionnels avec périmètre fermé | `console-air/CLAUDE.md`, `chain-sdk/CONTRIBUTING.md` |
| Outils d'agent typés et testés un par un | `akash-network/mcp/src/tools/*` + `tests/tools/*.test.ts` |

**Ce que cet audit n'autorise pas** — et qui reste hors de Polygone :
Kubernetes, Cosmos SDK, blockchain, jeton, bonds économiques, marketplace,
module `x/verification`, attestations on-chain, sidecar d'attestation. Aucun de
ces éléments n'entre dans le dépôt par cette étude. Les seuls emprunts sont des
**gestes d'ingénierie** : lire avant d'écrire, prouver avant d'affirmer, réserver
le mot « multi-machine », typer les états, tracer les décisions.

**Constat de maturité à conserver** : AEP-86 est `status: Draft` ; l'`auditor-cli`
v0 vit sur la branche `aep-86/auditor-cli-foundation` (pas de `main`, dernier
travail 2026-06-26) ; le serveur MCP Akash se déclare lui-même « still under
development and not all tools are available ». Ces pratiques sont donc des
**modèles de méthode**, pas des composants à importer.

## 11. Preuve canonique et portes de fiabilité (intégration 2026-11-15)

### 11.1 `polygone.evidence.v1` — primitive pure, **non câblée**

`crates/core/src/evidence.rs` fixe la forme d'une preuve et la façon dont deux
preuves se comparent. Aucun binaire ne l'appelle, aucun fichier n'est écrit,
aucun champ n'entre dans une enveloppe.

| Élément | Contenu |
|---|---|
| Schéma | `polygone.evidence.v1`, `deny_unknown_fields`, JSON canonique (champs ordonnés) |
| Intégrité | `digest_hex()` = BLAKE3-256 des octets canoniques ; `digest_matches()` |
| Sujet | `subject_id` (tâche ou nœud), séparé de `provenance.producer` |
| Engagement | `committed_blake3` optionnel + `observed_blake3` obligatoire |
| Méthode | `local_recompute` (L1) · `signed_challenge` · `operator_attestation` (candidat L2) |
| Comparaison | `Match` / `Divergent` / `Inconclusive` + raison fermée (`output_differs`, `subject_differs`, `method_differs`, `committed_hash_disagrees`, `commitment_unavailable`, …) |
| Refus | schéma inconnu, champ inconnu, digest mal formé ou nul, horodatage nul, caractère de contrôle |

**Ce que la comparaison garantit** : deux producteurs différents mesurant le
même sujet avec la même méthode sont comparables (c'est le cas L2) ; deux
méthodes différentes ne le sont pas (`Inconclusive`) ; un désaccord d'engagement
est `Divergent`, jamais `Match`.

**Ce qui reste volontairement dehors** :

1. **Écrire un fichier de preuve** = format persisté ⇒ ADR au titre du §9.6.
   Aucun chemin, aucun nom de fichier, aucune rotation n'est décidé ici.
2. **Câbler L1** (`--verifier-local`) pour émettre cet artefact ⇒ même ADR,
   puis un test qui compare un artefact émis à un artefact relu.
3. **Quorum L2** ⇒ exige deux opérateurs réels (§3, bloqué). Le format est
   prêt, la validation distribuée ne l'est pas et ne sera pas simulée.
4. **Attribution de faute** (producteur / demandeur / partagée) ⇒ décision
   humaine ; le module nomme ce qui s'est passé, pas qui doit payer.

### 11.2 Release à blanc — `make release-dry-run`

`scripts/release-dry-run.sh` rassemble les binaires de release **déjà
construits** (ou les compile avec `--build`, explicite), écrit `SHA256SUMS`,
le vérifie, puis **prouve** qu'une altération est détectée sur une copie
temporaire. Il ne publie, ne tague et ne pousse rien.

Limite déclarée : l'archive `tar.gz` et le nommage attendu par `install.sh` ne
sont pas produits ni vérifiés ici. Le manifeste, lui, est prouvé.

### 11.3 Dépendances — `make audit`

Cible volontairement **hors** de `make check` : sans `cargo-audit` et
`cargo-deny`, elle sort en **code 2** avec un message explicite. Un outil absent
n'est pas un succès silencieux, et aucune installation n'a été faite.

### 11.4 Commandes et résultats de cette session

```bash
cargo test -p polygone-core evidence::                # 17 réussis
cargo test --workspace --no-fail-fast -j 2            # 582 réussis, 0 échec
cargo clippy -p polygone-core --all-targets -- -D warnings   # exit 0
cargo fmt --all -- --check                             # exit 0
node scripts/check-docs.mjs --tests 582                # exit 0
make audit                                             # exit 2 (outils absents, non exécuté)
POLYGONE_BIN_DIR=<factices> ./scripts/release-dry-run.sh  # exit 0, 4 entrées, altération détectée
cargo build -p polygone-relay                          # exit 0 (debug, 55,6 s)
./scripts/cross-version-smoke.sh                        # PASS — client antérieur réel :
                                                       #   strict → HELLO_DENIED auth_required
                                                       #   --allow-insecure-hello → HELLO_OK (transition)
```

Non exécuté et non revendiqué : `cargo clippy --workspace`, la CI GitHub
Actions, toute release réelle, tout test à deux hôtes.

## 12. État Git final (2026-11-15, intégration)

| Élément | Valeur |
|---|---|
| Branche | `reconcile/2026-11` |
| Dernier commit | `f6382e6` — **aucun commit créé** |
| Travail préexistant | 32 entrées `git status`, **non touchées** (Compute, gateway, docs) |
| Nouveaux fichiers (non suivis) | `crates/core/src/evidence.rs`, `scripts/release-dry-run.sh` |
| Fichiers modifiés par cette session | `crates/core/src/lib.rs`, `Makefile`, `CONTRIBUTING.md`, `ARCHITECTURE.md`, ce fichier |
| Ce fichier | **toujours non suivi (`??`)** — la continuité repose sur un fichier hors Git |
| Publication | aucune : ni commit, ni push, ni tag, ni release |

**Prochaine tâche concrète** : décider par ADR si `polygone.evidence.v1` devient
un fichier persisté, puis câbler `--verifier-local` pour l'émettre et le relire
(test de round-trip artefact). Tant que l'ADR n'existe pas, ne pas écrire de
preuve sur disque.
