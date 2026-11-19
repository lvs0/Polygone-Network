# STATE — état courant

> Mis à jour par l'orchestrateur après chaque bloc de travail.
> Ne jamais écrire ici une valeur non vérifiée.

## Réconciliation github/main + contrats F1-API-01 — 2026-11-19 (soir, CORVUS)

| Champ | Valeur vérifiée |
|---|---|
| Branche | `main` = `1956f36` (PR #6 prototype fusionnée le 2026-11-19, CI verte run 44582207237 ; `prototype-complet`, `f1-api-01-contrats`, `reconcile/2026-11` conservées) |
| Réconciliation | **FAITE** — merge `78e9447` (deux parents `c13cdf6` + `23d27ac`), PR #4 **MERGED** 2026-11-19T16:07:42Z, `github/main` = `8307094`, arbre identique à `78e9447` (`git diff` vide) |
| Arbitrage | ADR-0004 **accepté** : 20 conflits + 11 auto-fusions inspectées → version canonique partout ; 24 ajouts purs de septembre en couche historique (bandeaux + INDEX) ; 2 fichiers mock exclus ; **0 fichier Rust modifié** (`git diff c13cdf6 78e9447 -- '*.rs'` = 0) |
| Gates locaux | fmt 0 ; clippy workspace `-D warnings` 0 ; **612 tests / 25 suites / 0 échec** (`--no-fail-fast`, TMPDIR=/tmp, -j 2) ; check-docs 96 documents ; check-ci-honesty OK ; gitleaks avant push (voir §13 STATUS) |
| CI arbre fusionné | **VERTE** run 44558244332 (ubuntu ✓ macos ✓ smoke ✓ ; canari Windows échec attendu non bloquant) |
| F1-API-01 | **domaines locaux LIVRÉS** (2026-11-19, prototype) : `identity`, `daemon`, `resources`, `settings`, `usage`, `events` — contrats `API-CONTRACTS.md` §2.4 + §8 à jour ; restent nodes/network/tasks distribués (voir STATUS §13) |
| ARCHITECTURE | re-mesuré (prototype) : **105 fichiers, 45 853 lignes, 612 tests** (gateway 13/5 551/89, daemon 17/11 224/133, client 33/16 842/217) |
| Cue SSH | toujours intermittent (sonde arrière-plan active) ; mesures RTT/débit **non faites** ; ne pas ouvrir l'URL Tailscale SSH check |
| Retour arrière | `git revert -m 1 8307094` sur main (ADR-0004 §7) ; aucun force-push, histoires préservées |

## Production — 2026-11-19 (mandat opérateur, CORVUS)

| Champ | Valeur vérifiée |
|---|---|
| Branche | `reconcile/2026-11`, HEAD après commits du jour (voir `git log`) |
| Commits du jour | `8c6704f` core, `734a515` compute, `d3f98f6` gateway, `b341bfb` fabric, `d8d08f3` docs, `804d2ee` preuve L1, `42d8a6b` ADR-0002, `017a6f4` preuve deux hôtes, `22cc718` ci workflow_dispatch (+ correctifs CI et version 2.1.0 à suivre) |
| Tests | `TMPDIR=/tmp cargo test --workspace -j 2` : **586 passés, 0 échec**, 24 suites (2026-11-19) |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` : exit 0 |
| Deux hôtes | Hope (7.2.5-3-omarchy) ↔ cue-sandbox (6.18.38+, Ubuntu 24.04, 8 CPU, 24 Go) via SSH root + Tailscale ; horloges 0 s ; protocole P0 exécuté, preuve `docs/proofs/P0-DEUX-HOTES-2026-11-19.md` |
| D2 | tranchée : ADR-0002 (clé ML-DSA-65), DEC-SEC-03 SUPERSEDED dans le registre |
| Gitleaks | 292 commits, 0 fuite (avant push) |
| GitHub | branche poussée, PR #4 (conflictuelle avec main — réconciliation distincte) ; **CI VERTE run 44501139127** (ubuntu+macos+smoke, HEAD 26c52d9) après 9 runs ; bug produit macOS/BSD corrigé (socket accepté O_NONBLOCK hérité → bloquant) |
| Release | **v2.1.0 PUBLIÉE** — run 44505345176 vert, 4 cibles + SHA256SUMS ; macos-x86_64 cross-compilé vérifié x86_64 (macos-latest=arm64, macos-14 retiré, asset mal étiqueté supprimé) ; installation réelle testée en répertoire isolé (checksums + polygone id) |
| Cue SSH | intermittent : la sandbox se suspend, et le port 22 peut passer en « Tailscale SSH check » (authentification interactive) — ne pas ouvrir l'URL de login, réessayer plus tard |

## Réconciliation canonique — 2026-11-09

Décision `D1` tranchée : **option D1-b**, documentée dans
`docs/decisions/ADR-0001-base-canonique-et-reconciliation.md`.

| Champ | Valeur |
|---|---|
| Dépôt canonique de travail | `~/Polygone-canonical` (clone de `Polygone-real`, remote `realign` → `~/Polygone-Atlas/03-REALIGNED`) |
| Branche | `reconcile/2026-11` — base `main` (`f3d09c0`), fusion `realign/v2` (`967afdb`), puis `p0/first-secure-network` (`d6e7229`) |
| Commits créés | `3486e4b` (fusion des deux lignées), `9f5413c` (correctif P0/S1) |
| Conflits | 65, tous `add/add` (aucun ancêtre commun) ; version `realign/v2` retenue, liste exhaustive en annexe de l'ADR-0001 |
| Non-perte | `comm -23` sur les deux arbres : **0 fichier perdu** de chaque côté ; 391 fichiers dans l'arbre canonique |
| Histoires préservées | `main` et `realign/realign/v2` intactes, aucun force-push |
| Hors fusion | les 78 modifications non commitées de `Polygone-real` (jamais commitées) restent dans `~/Ops/backups/polygone-unification-2026-11-09/dirty/` |

## Vérifications de la première réconciliation — état historique avant e2936a3

| Commande | Résultat |
|---|---|
| `cargo test --workspace` | **451 passés, 0 échec**, 22 suites, exit 0 (mesuré sur `reconcile/2026-11`) |
| `node scripts/check-docs.mjs` | **documents cohérents avec le code**, 71 Markdown indexés, exit 0 |
| `comm -23` sur les deux arbres | aucun fichier de `main` ni de `realign/v2` perdu par la fusion |
| `test_node_stops_when_lease_is_lost` | échouait une fois sur deux passes complètes : collision de port de test (`free_port` relâche le port avant que le nœud le reprenne), pas un défaut du nœud. Le test borne désormais 2 s de réessai et nomme la cause. |
| `cargo build --workspace` | exit 0, 4 binaires produits, dont `polygone` et `polygone-relay` |
| `POLYGONE_SKIP_BUILD=1 ./scripts/p0-two-process.sh` | **PASS**, exit 0 — 3 processus réels : relais, émetteur A (`f4a1de26…`), récepteur B (`cfd38084…`) ; session `01c25dfb…`, 7 fragments + KEM_CT, ancre de confiance apprise, `HELLO_DENIED auth_required` sur identité non prouvée, aucun secret dans les journaux |

| `cargo test -p polygone-client --bin polygone net::peers_tests` | **5/5** — migration v1→v2 : double entrée, copie `peers.json.v1.bak`, idempotence, entrée illisible conservée, plafond respecté |
| `scripts/peers-migration-smoke.sh` | **PASS** — fichier v1 réel migré, 2 entrées, copie 0600, aucune perte |
| `scripts/cross-version-smoke.sh` | **PASS** — relais strict refuse un `HELLO` hérité, `--allow-insecure-hello` l'accepte, client du 2026-11-07 produit un identifiant de 16 caractères |
| `gitleaks 8.30.1` (`git` + `dir`) | **aucune fuite** — 280 commits, 247 Mo d'arbre |
| Mesure de stockage restaurée | `StorageInfo`, `mount_for_path`, `Platform::storage_info` ; `total_storage_bytes` publié depuis `snap.storage.available_bytes`, plus jamais une constante |
| Revue des 78 modifications de `Polygone-real` | 4 fichiers porteurs d'une fonctionnalité réelle **réintégrés** ; 31 restent à statuer (dont la réservation de stockage du ledger) — `project/DIRTY-REVIEW.md` |

**Non vérifié, et pourquoi** : `gitleaks` n'est pas installé sur cette machine — le scan d'historique exigé par l'étape 9 du plan n'a **pas** été exécuté. Aucune publication vers `origin` n'a été faite. Le test de migration `peers.json` v1 → v2 (AK-02) n'existe pas encore : l'identifiant `node_id` a changé de longueur avec S1, donc la migration est un prérequis de version stable, pas un détail.


## Vérifications manuelles actualisées — 2026-11-09

HEAD vérifié : `e2936a3e94ae2d7bd95afb9c21be8dd23c2641f2` sur `reconcile/2026-11`. Cet état remplace les
indications devenues obsolètes dans la première réconciliation ci-dessus.

| Commande / contrôle | Résultat |
|---|---|
| `cargo test --workspace` | **463 réussis, 0 échec**, doc-tests inclus, exit 0 (log temporaire `/tmp/polygone-workspace-test-2026-11-09.log`) |
| `cargo fmt --all -- --check` | PASS |
| `node scripts/check-docs.mjs --tests 463` | PASS |
| `make migrate-proof` | PASS — cinq tests de migration et smoke test sur fichier v1 réel |
| Test `runtime_state` stockage | PASS — valeur mesurée publiée, enforcement reste à zéro, absence de mesure fail-closed |
| Tests ledger stockage | PASS — réservation/libération et conservation après redémarrage ; preuve : docs/proofs/storage-ledger-2026-11-09.txt |
| Publication | **Aucune** ; aucun push ou commit créé pendant cette reprise |

La réservation de stockage n'est donc plus une tâche à porter : le code du ledger
et ses tests sont déjà présents dans le HEAD. Le suivi a été corrigé pour le refléter.
La CI GitHub Actions et les essais sur deux machines physiques restent non vérifiés.

## Dernière session

| Champ | Valeur |
|---|---|
| Date | 2026-11-05 |
| Branche | `realign/v2` — locale, non poussée ; `realign/v1` est la branche publiée |
| HEAD | voir `git log -1 --format='%h %s'` (jamais recopié à la main) |
| Arbre | propre après commits locaux SEC.5 (cb1b5ef) et audits DOC.4/FV-SEC-06/SEC.10 (5d04cd6) ; non poussé |
| Origine | `lvs0/Polygone-Network` ; `origin/realign/v1` existe, `origin/realign/v2` non |

## Vérifications exécutées

| Commande | Résultat | Date |
|---|---|---|
| `cargo test --workspace` | **348 tests, 0 échec** | 2026-11-02 |
| `cargo fmt --all -- --check` | OK (hérité du dépôt canonique) | 2026-11-02 |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK (hérité du dépôt canonique) | 2026-11-02 |
| Archive P0.1 : listing + extraction + `cmp` ×3 + HEAD | **PASS** — 6 204 entrées, `tar exit=0`, stderr vide, HEAD `f3d09c0` et 65 fichiers modifiés identiques | 2026-11-03 |
| Inventaire P0.2 : 6 commits locaux + 17 distants | **Fait** — décisions par commit dans `docs/RECONCILIATION.md` | 2026-11-03 |
| Les 65 fichiers non commités du canonique sont-ils perdables ? | **Non** — 60 fichiers byte-identiques ici, 3 dossiers identiques (`diff -rq`), les 2 restants sont `.freebuff-tasks/` et `.specify/` (outillage). Déjà commités sur `realign/v1`. | 2026-11-03 |
| P0.3 : documentation factuellement fausse | **Corrigé** — `ARCHITECTURE.md` passe de « 4 crates, ~9 600 LOC, 109 tests » à **7 membres, 28 456 LOC, 4 binaires, 348 tests** (mesuré). `threat-commodity.md` ne recopie plus de compte. `CORE-COMPLETE.md` reçoit un bandeau d'instantané daté. | 2026-11-03 |
| `cargo test --workspace` sur `realign/v1` | **348 tests, 17 suites, 0 échec** | 2026-11-03 |
| P1.1 : capacités séparées (`Capacity`, `enforced`, refus distingué) | **353 tests, 0 échec** · `cargo clippy --workspace --all-targets -- -D warnings` **exit 0** · `cargo fmt --all -- --check` **OK** | 2026-11-03 |
| Plan Atlas importé | 132 identifiants dans docs/atlas/plan/epiques/ ; backlog opérationnel : 18 ouvertes + 20 terminées dans docs/atlas/agent/backlog.json ; 103 spécifications épiques non promues | 2026-11-04 |
| Cohérence backlog ↔ épiques | Contrôle automatisé : 132 identifiants uniques ; 38 entrées opérationnelles (18 ouvertes, 20 terminées), 29 correspondances épiques et 9 tâches transversales ; 103 tâches épiques non promues | 2026-11-04 |
| DAEMON.1/DAEMON.3 : expiration et renouvellement des baux | **fait** — 5 tests ajoutés (échéance, adoption d'un bail hérité, relecture du format historique, renouvellement, refus d'un bail inconnu/échu) · `315f0eb` | 2026-11-03 |
| DOC.1 : contrôle de cohérence documentaire | **fait** — `node scripts/check-docs.mjs --tests 361` → « documents cohérents », **exit 0** ; chiffre cassé exprès → **exit 1** et l'écart nommé · `288d67e` | 2026-11-03 |
| UI.1/UI.2/UI.5 : plus de chiffres inventés, plus de fausse promesse | **fait** — `Math.random` → 0 dans `web/dashboard.js`, `node --check` OK, libp2p/Five tabs/Sparklines retirés des deux pages · `f2e0a59` | 2026-11-03 |
| `cargo test --workspace` après ce lot | **361 tests, 18 suites, 0 échec** (exit 0) | 2026-11-03 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-03 |
| `cargo fmt --all -- --check` | **OK** | 2026-11-03 |
| Orchestrateur : tâches bloquées (AGT.1/AGT.2) | **fait** — `node --test docs/atlas/agent/tests/backlog.test.mjs` → **5/5**, dont « une tâche bloquée est refusée en nommant son bloqueur » (exit 3) | 2026-11-03 |
| hope-gateway : suite et démarrage | **44 tests, 0 échec** ; `server.mjs` + `GET /healthz` → **200** au premier essai ; CI ajoutée (`83ed22a`) | 2026-11-03 |
| MSG.1 : TTL de session contre les documents | **fait** — constante 300 s → **30 s**, reliée à `PUBLISHED_SESSION_TTL_SECS` ; trois documents alignés et lus à la compilation par `test_session_ttl_matches_the_published_promise` · `ac71668` | 2026-11-03 |
| DAEMON.13 : crash réel (SIGKILL du consommateur, puis du daemon) | **fait** — `node scripts/crash-recovery.mjs` → **9/9 PASS**, exit 0 : bail durable après SIGKILL du daemon, marge rendue à l'échéance, redémarrage qui répond | 2026-11-03 |
| MESH.1 : aucune annonce sur une interface non autorisée | **fait** — `parse_ip_addr_output` + `select_interfaces` remplacent le broadcast depuis `0.0.0.0` ; loopback, link-local, VPN et conteneurs exclus, `POLYGONE_MESH_IFACE` ne peut que restreindre. 4 tests, dont « une allowlist ne ressuscite pas `tailscale0` » et le témoin positif d'un datagramme bien reçu | 2026-11-04 |
| SEC.4 : usurpation de `from` figée en régression nommée | **fait** — `refuse_from_spoofing_through_relay` : trame forgée refusée **et** trame légitime acceptée dans la même session ; `cargo test -p polygone-relay` → **17/17** | 2026-11-04 |
| `cargo test --workspace -j 2` après ce lot | **370 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo fmt --all -- --check` | **OK** (exit 0) | 2026-11-04 |
| `scripts/check-docs.mjs` : branche de `STATE.md` comparée au checkout | **ajouté** — `git rev-parse --abbrev-ref HEAD` contre la ligne `| Branche |` ; une divergence nomme les deux branches et sort en 1 | 2026-11-04 |
| GW.2 : latence mesurée par ping/pong signé | **fait** — challenge à usage unique (TTL 30 s), `latency_ms` écrit seulement après pong, RTT ≤ 60 s, pong sans ping et rejeu refusés 409, médiane sur nœuds mesurés uniquement ; `latency_populated_after_pong`, `pong_without_ping_is_refused`, `pong_replay_is_refused` + assertion socket réel `/api/v1/network/status` = 42 | 2026-11-05 |
| SEC.1 : modèle de menace par service | **fait** — `docs/THREAT-MODEL-SERVICES.md`, six services, tests cités, risques résiduels ; `FV-SEC-08` 4 → 5, audit externe absent ; écart assumé : un fichier de synthèse, pas un par service | 2026-11-05 |
| `cargo test --workspace -j 2` après GW.2 | **415 tests, 0 échec** (22 suites) | 2026-11-05 |
| CLI.6 : chaque refus → cause + action | **fait** — `KNOWN_REASONS` = 8 constantes du daemon, `NodeError::Denied` affiche code + cause + action ; `every_reason_code_has_an_action`, `unknown_denial_keeps_the_technical_code` ; 417 tests | 2026-11-05 |
| FV-SEC-10 : limite par identité | **fait** — `IdentityLimiter` 100 env/s partagées entre reconnexions, purge à 60 s d'inactivité, refus journalisé sans fermer la connexion ; `per_identity_limit_denies_only_the_abusive_peer`, `per_identity_limit_resets_after_window` ; 419 tests | 2026-11-05 |
| SEC.10 : métadonnées du relais mesurées | **fait** — `relay-metadata-audit.mjs` : relais + émetteur + récepteur réels, proxy TCP, refus sans 2 connexions/2 HELLO/1 fragment, échec si contenu en clair ; 16 fragments, 2 HELLO, 1 session, 21 640–47 566 octets, marqueur absent ; rapport daté + `docs/threat-msg.md` ; `hide-smoke.sh` vert | 2026-11-05 |
| DOC.4 : index documentaire fail-closed | **fait** — `check-docs.mjs` contrôle les 66 Markdown, date ISO ou « Non indiquée », statut autorisé, unicité et couverture ; `node --test scripts/document-index.test.mjs` **6/6** ; date vide et statut invalide → exit 1 nommé ; index restauré, SHA-256 identique | 2026-11-05 |
| FV-SEC-06 : coût de création d'identités mesuré | **fait, portée limitée** — 30/30 identités distinctes ; ML-KEM-1024 + ML-DSA-65 + écriture : médiane/moyenne 2,85 ms, P95 3,07 ms, 21 641 octets, mode 0600 ; HOME temporaires supprimés ; rapport avec SHA-256 binaire. Cela ne fournit aucune résistance Sybil globale. | 2026-11-05 |
| SEC.5 : permissions de fichiers fail-closed | **fait** — helper atomique, `.polygone` 0700, fichiers 0600, chmod propagé/vérifié, symlinks et chemins non sûrs refusés ; magasin de pairs corrompu ne réactive jamais TOFU ; 10 tests ciblés, dont PermissionDenied injecté et nettoyage du temporaire ; smoke Hide réel vert ; workspace 432/432 + Clippy + fmt | 2026-11-05 |
| Console PWA reliée à la gateway | **fait** — `node.html`/`dashboard.js` lisent `/api/v1/network/status` et `/api/v1/services` ; état hors-ligne = « — », aucun chiffre simulé ; coque `polygone-v2` couvre console, Drive, Mesh et assets ; manifeste `start_url` node.html + 3 raccourcis ; `node --check` dashboard/sw/pwa, manifeste parsé, gateway 47/47 | 2026-11-05 |
| CLI `polygone ecosysteme` | **fait** — sondes réelles PWA (fichiers), Hide (TCP relais), Petals (GET Ollama `/api/tags`), Compute (socket daemon) ; `--strict` sort 1 si absent ; exécution : PWA/Petals prêts, Hide/Compute absents ; 4 tests dédiés, 436 tests workspace | 2026-11-05 |
| Hide : smoke E2E réel | **fait** — `hide-smoke.sh` avec binaires debug et ports éphémères : contenu reçu via SOCKS5, audit relay « zéro contenu en clair », grant chiffré accordé, tunnel client ouvert, **exit 0** | 2026-11-05 |
| Compute : smoke E2E réel | **fait** — `compute-smoke.sh` : succès signé (sortie `POLYGONE_COMPUTE_SMOKE_OK`), échec signé code 23 refusé, deux registres de réputation locaux, persistance TOFU, via relais réel, **exit 0** | 2026-11-05 |
| Petals : suite dédiée | **fait** — `cargo test -p polygone-petals` : 13 + 5 + 1 tests, 0 échec, exit 0 | 2026-11-05 |
| Compute daemon : suite dédiée | **fait** — `cargo test -p polygoned` : 117 + 1 tests, 0 échec, exit 0 | 2026-11-05 |
| INST.1 + INST.2 : PATH idempotent et désinstallation | **fait** — bloc balisé dans le rc, deux appels = un seul bloc, rc absent non créé ; `--uninstall [--purge]` retire service utilisateur, cinq binaires/alias, web, config et bloc PATH (ancienne ligne incluse), conserve les données sans purge et refuse `/` ou `$HOME` ; fixture complète PASS, seconde exécution idempotente | 2026-11-06 |
| INST.3 : zéro résidu dans HOME | **fait** — instantané chemins/types + tailles avant réinstallation, puis `install_binaries` + `setup_path` + unité user + données, puis `main --uninstall --purge` ; les deux `diff` sont vides, `.bashrc`/`.profile` préexistants conservés | 2026-11-06 |
| INST.6 + INST.8 : phrase exacte de l'installateur | **fait** — README : origine GitHub Releases, SHA256SUMS obligatoire, portée limitée à la même origine HTTPS, aucune signature détachée, destinations et retrait `--uninstall`/`--purge`, identités `~/.polygone` hors périmètre ; `check-docs 436` cohérent | 2026-11-06 |
| MSG.6 : débit loopback mesuré (partiel) | **preuve partielle, tâche ouverte** — `scripts/msg-throughput-audit.py` : 10 envois de 12 Kio via relais réel, 10/10 SHA-256 identiques, médiane 0,027 Mio/s, écart-type 0,011 ; le loopback ne remplace pas la preuve inter-hôtes exigée, donc MSG.6 n'est pas fermée | 2026-11-07 |
| Vérification workspace après SEC.5 | `cargo fmt --all -- --check`, `cargo test --workspace -j 2` **432/432**, `cargo clippy --workspace --all-targets -- -D warnings` exit 0 ; backlog agent 6/6 ; Atlas PASS | 2026-11-05 |
| FV-SEC-05 : rotation et révocation de clé | **fait** — `polygone clef-tourner` signe la nouvelle ML-DSA-65 avec l'ancienne (`polygone-key-rotation-v1`), révoque l'ancienne, garde le node_id KEM, écrit `identity.json` atomiquement ; 3 tests de rotation/révocation/relecture ; 422 tests | 2026-11-05 |
| `scripts/check-docs.mjs` : somme des LOC du tableau vérifiée comme celle des tests | **ajouté** — c'est ce contrôle qui manquait quand `client` affichait 11 365 pour 11 677 mesurées | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 370` | **documents cohérents**, exit 0 ; deux négatifs vérifiés : branche forcée à `realign/v1` → exit 1 (`[state.branche]`), LOC volontairement faussées → exit 1 (`[architecture.somme.LOC]`) | 2026-11-04 |
| `node --test docs/atlas/agent/tests/backlog.test.mjs` | **5/5**, exit 0 — 19 tâches ouvertes, 15 fermées, JSON valide | 2026-11-04 |
| SEC.3 : rejeu capturé sur le fil, refusé par le **vrai** relais | **fait** — `replay_through_relay_is_refused_with_an_explicit_code` : session déchiffrée (témoin positif), rejeu immédiat → `REPLAY.SESSION_ALREADY_COMPLETED`, rejeu à +10 min contre un récepteur frais → `REPLAY.STALE_TIMESTAMP`. Le refus n'est plus un `None` muet : `Refusal::code()` le nomme | 2026-11-04 |
| `cargo test --workspace -j 2` après SEC.3 | **371 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 371` | **documents cohérents**, exit 0 | 2026-11-04 |
| DOC.2 : index complet de `docs/` | **fait** — `docs/INDEX.md` recense les 62 Markdown récursifs (date, statut, thème et justification) ; `cargo test -p polygone-client --test doc_index` **1/1** ; test négatif : retrait de `docs/BUDGET.md` → échec nommé, index restauré | 2026-11-04 |
| `cargo test --workspace -j 2` après DOC.2 | **372 tests, 0 échec** (exit 0) | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 372` | **documents cohérents avec le code**, exit 0 | 2026-11-04 |
| Validation DOC.2 | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `node scripts/check-atlas.mjs`, tests agent **6/6**, `git diff --check` : tous verts | 2026-11-04 |
| CLI.1 : `polygone doctor` lit l'isolation OS au lieu de la déclarer | **fait** — section « Isolation OS (ce processus) » alimentée par `/proc/self/cgroup`, `memory.max` et `cpu.max` : `max` → « illimitée », fichier illisible → « Non mesurée », jamais un `0.00 Gio` inventé. Sortie réelle contre le daemon du projet : `Cgroup : /user.slice/user-1000.slice/session-42.scope (scope Polygone : non)`, `MemoryMax : illimitée`, exit 0. 7 tests `doctor`, 7 tests `node_isolation` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.1 | **384 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.1 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 384` | **documents cohérents**, exit 0 | 2026-11-04 |
| CLI.2 : pause bornée, bail rendu, reprise refusée si expirée | **fait** — session réelle contre le daemon du projet : `pause --duree 10m` → exit 0, `pause.json` écrit (`resumes_at = paused_at + 600`) ; `reprendre` → `bail obtenu … 0.25 Gio · 1 cœur(s) — rendu aussitôt : oui`, exit 0, intention consommée ; sans pause → exit 1 `aucune pause en cours` ; `--duree 9m` → exit 1 `durée hors bornes` ; intention antidatée → exit 1 `pause expirée depuis 60 s`, aucun bail demandé. 7 tests `pause` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.2 | **391 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.2 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 391` | **documents cohérents**, exit 0 | 2026-11-04 |
| CLI.5 : désinstallation énumérée, confirmée, sans reste | **fait** — session dans un `HOME` temporaire : sans `--confirmer`, exit 1 `confirmation requise` et **arborescence identique** (5 fichiers intacts) ; avec `--confirmer`, exit 0, 6 chemins supprimés, il ne reste que `Documents/keep.txt` (jamais touché) ; un chemin hors racines est refusé par `remove`. 4 tests `uninstall` | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.5 | **395 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après CLI.5 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` | **documents cohérents**, exit 0 | 2026-11-04 |
| DOC.3 : bandeaux `Instantané daté` sur les documents faux | **fait** — les **29** documents classés `daté` dans `docs/INDEX.md` portent le bandeau et un lien vers `ETAT-REEL.md`/`STATE.md` ; `grep -L "Instantané daté"` sur ces 29 → **aucun fichier** ; `check-docs` gagne le contrôle `docs.instantané` : bandeau retiré de `RECONCILIATION.md` → **exit 1** nommant le fichier, restauré → **exit 0**. Aucun contenu historique réécrit | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` après DOC.3 | **documents cohérents**, exit 0 — « documents datés : 29 vérifiés, tous bandeautés » | 2026-11-04 |
| CI.1 : matrice honnête sur Windows | **fait** — `windows-latest` retiré du job bloquant `test` (il ne peut pas être vert : `daemon/src/resources/mod.rs` porte un `compile_error!`) ; job `windows-canary` en `continue-on-error: true` qui imprime la raison. Contrôle local `node scripts/check-ci-honesty.mjs` → exit 0 ; négatif : `windows-latest` réinjecté dans la matrice bloquante → **exit 1** nommant le job, fichier restauré → exit 0. **Limite : le run GitHub Actions n'a pas été exécuté ici (aucun push autorisé) — la structure est vérifiée, pas le runner.** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 395` après CI.1 | **documents cohérents**, exit 0 | 2026-11-04 |
| OPS.2 : compter les refus d'allocation par motif | **fait** — `RefusalCounters` vit dans le serveur (jamais `DaemonState`), incrémenté au seul point où une réponse `AllocDenied` passe ; nouvelle requête `refusals_request` (pas un champ de `Status`, qui est déjà au plafond de taille porté par `NodeError`). Test sur un VRAI serveur : les **8** motifs provoqués (`invalid_request`, `exceeds_measured`, `insufficient_headroom`, `insufficient_cpu`, `insufficient_vram`, `insufficient_storage`, `unknown_allocation`, `policy_shrinking`), chacun compté une fois. Session réelle : `alloc_request` de 100 000 Gio sur le socket → `alloc_denied` ; `polygoned status` → `Refus : exceeds_measured: 1` ; `polygoned doctor` → `exceeds_measured: 1` ; daemon arrêté → `non mesurés (daemon absent)`, jamais `0` | 2026-11-04 |
| `daemon/src/cli.rs` n'était compilé par aucune cible | **corrigé** — le module (et ses 5 tests, jamais exécutés) n'était référencé ni par `lib.rs` ni par `main.rs` ; `pub mod cli;` ajouté, `cargo test -p polygoned --lib` passe de 111 à **117 tests** | 2026-11-04 |
| `cargo test --workspace -j 2` après OPS.2 | **402 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après OPS.2 | **exit 0** — le champ a été retiré de `Status` précisément parce que 8 octets suffisaient à déclencher `result_large_err` | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 402` | **documents cohérents**, exit 0 | 2026-11-04 |
| SEC.8 : empêcher un secret d'entrer dans l'arbre | **fait** — `crates/client/tests/no_tracked_secrets.rs` lit ce que git **suit** (`git ls-files -z`, repli par parcours si git absent) et refuse `identity.json`, `*.key`, `.env`/`.env.*` (gabarits `.env.example|sample|template` permis). Positif **2/2** ; porte : `.env` réellement ajouté avec `git add -f` → **échec nommant `.env`**, dé-suivi puis retiré → vert. Hook `.githooks/pre-commit` (activation documentée : `git config core.hooksPath .githooks`) + étape CI | 2026-11-04 |
| `cargo test --workspace -j 2` après SEC.8 | **404 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après SEC.8 | **exit 0** | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 404` | **documents cohérents**, exit 0 | 2026-11-04 |
| CLI.3 : profils d'usage avec impact chiffré | **fait** — `polygone profil <sobre|equilibre|genereux>` écrit `tier = "Custom"` + les quatre seuils dans `~/.config/polygone/daemon.toml` (écriture atomique, `BehaviorConfig`/`SafetyMargins` intacts) ; sans argument, affiche le profil courant. Preuves : `three_profiles_write_distinct_limits` (3×CPU 25/50/75 lus après réécriture, tick et marge de sûreté conservés) et `profil_without_arg_prints_current`. Session réelle dans un `HOME` temporaire : `polygoned status` relit `CPU 25% | RAM 30% | BW 40%` puis `CPU 75% | RAM 70% | BW 80%` ; `profil turbo` → exit 1 nommant les trois choix ; chaque rendu dit « aucune limite OS n'est posée sans isolation » | 2026-11-04 |
| `cargo test --workspace -j 2` après CLI.3 | **408 tests, 0 échec** (exit 0) | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 408` | **documents cohérents**, exit 0 | 2026-11-04 |
| GW.1 : heartbeat signé, `last_seen` persisté | **fait** — `POST /api/v1/nodes/heartbeat` vérifie ML-DSA-65 sur le message canonique `polygone-heartbeat-v1\n{node_id}\n{timestamp}`, avec la clé **épinglée à l'appairage** (le corps ne peut pas substituer une identité) ; fenêtre anti-rejeu ±300 s ; écriture atomique (`nodes.json.tmp` + `rename`) sérialisée par un verrou ; rechargement au démarrage. Preuves : `heartbeat_requires_valid_signature` (403, `last_seen` inchangé, **aucun fichier écrit**), `heartbeat_survives_process_restart` (relecture disque), `heartbeat_outside_replay_window_is_refused` (400), et `heartbeat_round_trip_over_real_http_is_persisted` (vrai socket TCP : appairage, heartbeat, `/status` actif, fichier relu par une instance neuve) | 2026-11-04 |
| `cargo test --workspace -j 2` après GW.1 | **412 tests, 0 échec** (exit 0) | 2026-11-04 |
| `cargo clippy --workspace --all-targets -- -D warnings` après GW.1 | **exit 0** — un `if` imbriqué a été replié en let-chain, pas désactivé | 2026-11-04 |
| `node scripts/check-docs.mjs --tests 412` | **documents cohérents**, exit 0 | 2026-11-04 |
| Registre de la vision du fondateur | **créé** — `docs/FOUNDER_VISION.md` : **67 exigences** identifiées (infra, économie POLY, calcul, IA, sécurité, UX, ops, corrections historiques), chacune avec état 1–7, preuve ou réalité vérifiée, travail restant, critère d'acceptation et priorité. Sources lues : corpus `_work/vision-long.md` (26 798 lignes), `VISION-EXTRAITE.md`, `VISION.md`, `ECOSYSTEM.md`, `PLAN-MAITRE.md`, `PLAN-FINITION.md`, `DECISION-POLY.md`. Aucune exigence supprimée pour cause de difficulté | 2026-11-04 |
| Avancement calculé, pas estimé | `node scripts/vision-status.mjs` → **23 validées / 67**, **9 bloquées**, **23/58 = 39,7 %** des exigences réalisables ; le script refuse un identifiant ou un état hors 1–7. `docs/EXECUTION_STATUS.md` donne la prochaine tâche exacte (`GW.1`) et les commandes de reprise | 2026-11-04 |
| Documents de pilotage indexés | `docs/INDEX.md` passe à **64 fichiers Markdown** ; `cargo test -p polygone-client --test doc_index` → **1/1** ; `check-docs --tests 408` → exit 0 | 2026-11-04 |
| Régression de latence trouvée par la suite complète | **corrigée** — OPS.2 sondait refus puis status sur une connexion au délai de 1 s, soit jusqu'à 2 s pour un daemon muet ; `test_status_probe_does_not_hang_on_silent_daemon` a échoué à 2.29 s. Le budget est désormais partagé (`STATUS_PROBE_TIMEOUT / 2` par requête), la borne du test reste 2 s et repasse | 2026-11-04 |

## Décisions tranchées

| Date | Sujet | Décision | Document |
|---|---|---|---|
| 2026-11-03 | Monnaie `POLY` | **Pas de cryptomonnaie.** Unité de réciprocité non transférable, dans le `Ledger` du daemon ; reçu signé dans `crates/core` ; rien dans `hope-gateway`, rien sur une chaîne. | `docs/atlas/plan/DECISION-POLY.md` |

## Bloquants connus

1. **Divergence Git non réconciliée** : 6 commits locaux, 17 commits distants
   sur `Polygone-Network` (`main`). Inventoriée commit par commit et tranchée
   dans `docs/RECONCILIATION.md` ; **aucun merge effectué**, et c'est volontaire.
   Le distant est une ligne antérieure, pas un complément : 2 éléments à
   reprendre, 15 refusés avec motif. L'installateur, d'abord classé « à
   sauver », s'est révélé déjà fail-closed localement avec cinq tests
   d'échec — correction du 2026-11-03, vérifiée dans `install.sh:157-177`.
2. **Aucun test entre deux machines physiques** : les tests prouvent des
   processus locaux, pas un réseau réel.
3. **18 capacités non câblées** : listées dans `ETAT-REEL.md` §3.
4. **macOS/Windows** : compilation possible, isolation OS non prouvée.

## Ce qui n'est pas dans ce dépôt

- `target/`, `node_modules/` : artefacts.
- `archive/2026-07-src/` : monolithe libp2p abandonné.
- Sous-produits d'outillage (`graphify-out/`, `.claude-flow/`, `.agents/`).
- Secrets, identités, bases de données, conversations.

## Prochaine action

**Priorité du backlog : `BLOCKS.9`** — prouver un transfert de blocs entre deux machines physiques. Cette preuve ne peut pas être simulée par deux processus locaux ; elle nécessite un second hôte réel.

**`CLI.1`, `CLI.2` et `CLI.5` fermés le 2026-11-04.** `doctor` lit l'isolation OS au lieu de la déclarer ; `pause --duree` rend le bail et écrit une intention bornée, `reprendre` la consomme et redemande un bail ; `desinstaller` énumère, exige `--confirmer`, ne touche que ses racines et ne laisse aucun reste.

**Prochaine tâche : G0 — réconcilier les 132 identifiants des six épiques avec les preuves et le backlog opérationnel.** `check-atlas` indique encore 96 tâches non promues ; chacune doit être comparée au code avant d'être classée. `BLOCKS.9`, `MSG.6` et `FV-INF-04` restent dépendants de deux machines physiques ; aucune preuve loopback ne les clôt. **Limite SEC.8** : `gitleaks` n'est pas installé ; l'audit d'historique n'a pas été exécuté.

**L'autorité d'ordre est `docs/atlas/agent/backlog.json`** : 9 tâches opérationnelles non closes (6 ouvertes et 3 bloquées : DAEMON.10 par D2, DRIVE.1 par D3, RECIP.1 par BLOCKS.9 et D8) ; 39 tâches fermées avec leur preuve dans `completed`. DOC.2, DOC.3, CLI.1, CLI.2, CLI.3, CLI.5, CI.1, OPS.2, SEC.8 et GW.1 sont fermés depuis le 4 novembre 2026. `DOC.3` et `CLI.3` n'étaient pas promues dans les tâches opérationnelles : leurs preuves sont ajoutées directement à `completed`, sans toucher au compte des ouvertes. `DOC.3` n'était pas promue dans les tâches opérationnelles : sa preuve est ajoutée directement à `completed`. Les vagues W0…W5 de `docs/atlas/plan/PLAN-ECOSYSTEME.md` §8 placent le gateway, l'agent et le corpus sur le même calendrier.

> **Correction du 2026-11-03** : la note précédente demandait de committer les
> 65 fichiers non suivis du dépôt canonique. **C'était inutile.** Vérifié par
> `cmp` sur 60 fichiers et `diff -rq` sur 3 dossiers : ils sont **déjà
> byte-identiques** ici, donc déjà commités sur `realign/v1` et poussés. Le
> dépôt canonique reste volontairement fragile : c'est le poste de travail,
> pas la copie de référence.

Voir `ROADMAP.md`, `docs/P0-RAPPORT.md` et `docs/atlas/agent/backlog.json`.


## Synchronisation documentaire Atlas

| Contrôle | Résultat à vérifier | Date |
|---|---|---|
| node scripts/check-atlas.mjs | 6 épiques, 132 identifiants, backlog cohérent, aucun chemin privé | 2026-11-04 |
| node --test docs/atlas/agent/tests/backlog.test.mjs | tests du véritable orchestrateur, worker absent et tâches déjà terminées | 2026-11-04 |

## Audit de fondation — 2026-11-09 (session 4)

HEAD `e2936a3`, branche `reconcile/2026-11`. Audit intégral du socle, correctifs
et documentation de fondation. Détail complet : `project/FOUNDATION-AUDIT.md`,
`project/FOUNDATION-STATUS.md`, `docs/testing/VALIDATION-RESULTS.md`.

| Contrôle | Résultat |
|---|---|
| `cargo test --workspace` | **465 réussis, 0 échec**, doc-tests inclus, exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `node scripts/check-docs.mjs --tests 465` | documents cohérents, 78 Markdown couverts, exit 0 |
| `node scripts/check-ci-honesty.mjs` | matrice honnête, exit 0 |
| `cargo test -p polygone-client --test no_tracked_secrets` | 2/2, exit 0 |
| `git diff --check` | exit 0 |
| `cargo test -p polygoned socket::tests::` | 35/35, exit 0 |
| Persistance du ledger | **corrigée** — écriture atomique, `0600`, quarantaine d'un fichier illisible |
| Test de heartbeat gateway | **rendu déterministe** — comparait un timestamp client à `last_seen`, échouait à une frontière de seconde |
| Mesures | 88 fichiers Rust, 35 597 lignes, 4 binaires, 465 tests |

**Non vérifié** : deux machines physiques, CI GitHub Actions, Gitleaks sur
l'arbre final, audit de dépendances, macOS, VM neuve. Aucun commit ni push créé.

Verdict : Foundation 1.0 **non atteinte** ; socle local cohérent et documenté,
mais multi-machine, CI, dépendances et secrets finaux restent non prouvés, et
l'API produit de tâches/événements n'existe pas.

## File de tâches — 2026-11-09 (session 5)

Modèle, file et API produit de tâches ; aucun exécuteur branché.

| Contrôle | Résultat |
|---|---|
| `cargo test --workspace` | **490 réussis, 0 échec**, exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `node scripts/check-docs.mjs --tests 490` | documents cohérents, 78 Markdown couverts, exit 0 |
| `git diff --check` | exit 0 |

Livré : `crates/core/src/task.rs` (15 tests), `crates/gateway/src/tasks.rs`
(8 tests), routes `/api/v1/tasks` (2 tests). Cycle de vie explicite, résultat
BLAKE3 recalculé et producteur imposé, persistance atomique `0600`, quarantaine
d'un fichier illisible, expiration balayée à la lecture. L'API ne dépasse jamais
`published` et renvoie `executor_attached: false`.

Limites : aucun exécuteur, donc aucune tâche réellement exécutée ; pas de test
HTTP de bout en bout dédié aux routes de tâches ; multi-machine, CI, Gitleaks
final et audit de dépendances toujours non vérifiés. Aucun commit ni push.

## Plafond CPU et politique de confiance — 2026-11-13 (session 6)

Deux ambiguïtés de fondation fermées, sans prétendre à une preuve multi-machine.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | **499 réussis, 0 échec**, exit 0 |
| `node scripts/check-docs.mjs --tests 499` | documents cohérents, 85 Markdown couverts, exit 0 |
| `cargo test -p polygone-client --test doc_index` | 1/1, exit 0 |
| `gitleaks git .` | 281 commits, aucune fuite |
| `gitleaks dir` (arbre hors `target/`, `.git/`) | aucune fuite |

`F1-RES-03` : `allowed_cpu_cores()` = `floor(cpu_cores × max_cpu_percent / 100)`.
`cpu_cores` reste le total physique ; `free_cpu_cores` ne dépasse plus ce pool.
Tests `test_cpu_pool_is_capped_by_the_tier` et
`test_cpu_above_the_tier_pool_is_denied`. Les fixtures de test modélisent
désormais 6 cœurs physiques pour conserver 3 cœurs réservables à 50 %.

`F1-SEC-02` : `POLYGONE_TRUST_POLICY` accepte `tofu` (défaut) ou `strict`.
En strict, un pair non ancré est refusé avant livraison (`AUTH.UNKNOWN_PEER`)
et n'est jamais appris ; une valeur inconnue empêche le démarrage du client.
Trois tests : analyse des valeurs, refus sans apprentissage, pair ancré accepté.

Limites : aucune contrainte CPU noyau (`enforced = 0`), TOFU reste le défaut,
et multi-machine, CI, audit de dépendances et VM neuve restent non vérifiés.

## Négociation de protocole v1/v2 — 2026-11-13 (session 7)

`F1-PROTO-01` : la fenêtre de compatibilité est explicite et testée.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | **502 réussis, 0 échec**, exit 0 |
| `node scripts/check-docs.mjs --tests 502` | documents cohérents, 85 Markdown couverts, exit 0 |
| `cargo test -p polygone-client --test doc_index` | 1/1, exit 0 |
| `gitleaks git .` | historique complet, aucune fuite |
| `gitleaks dir` (arbre hors `target/`, `.git/`) | aucune fuite |

Le `hello` reste en `v: 1` pour les binaires déjà installés, et annonce
`max_protocol_version: 2`, `min_protocol_version: 1` et les capacités
(`refusals`, `renew`, `storage_capacity`, `cpu_tier_cap`). Le serveur répond
dans la version de la requête : `v1` reçoit du `v1`, `v2` reçoit du `v2`. Le
client retient `min(2, max daemon)` et encode toutes ses requêtes avec cette
version ; sans recouvrement, il refuse la connexion avant toute requête.

Preuves : `test_serve_accepts_v1_and_refuses_v3_without_closing`,
`test_client_negotiates_v1_with_an_old_daemon`,
`test_client_negotiates_v2_from_the_max_field`,
`test_client_rejects_wrong_protocol_version`, et le contrat petals qui vérifie
la version courante au lieu de figer `v: 1`.

Limite : une future v3 devra élargir la fenêtre et les tests, pas réinterpréter
les messages existants. Le relais garde son propre handshake `AUTH_INIT v1`,
non concerné par cette négociation.

## Exécuteur local de tâches — 2026-11-13 (session 8)

`F1-TASK-02` (volet local) : les tâches `echo` et `blake3` sont réellement
calculées, avec résultat vérifié et reprise après interruption.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | **507 réussis, 0 échec**, exit 0 |
| `node scripts/check-docs.mjs --tests 507` | documents cohérents, 85 Markdown couverts, exit 0 |
| `cargo test -p polygone-client --test doc_index` | 1/1, exit 0 |
| `gitleaks git .` | historique complet, aucune fuite |
| `gitleaks dir` (arbre hors `target/`, `.git/`) | aucune fuite |

`TaskKind::execute` est pur : `echo` rend l'entrée, `blake3` rend son empreinte
hexadécimale, et la borne `max_output_bytes` de la tâche est vérifiée. Un
magasin ouvert avec `TaskStore::with_executor` assigne, exécute, vérifie le
hash et le producteur, puis persiste `succeeded` ou `failed`. Un magasin
ouvert avec `TaskStore::new` reste inerte et laisse la tâche `published`.

La gateway attache l'exécuteur quand un fichier de tâches est configuré ;
`executor_attached` reflète ce mode au lieu d'être figé à `false`. Au
démarrage, `TaskStore::recover` reprend les tâches `published`, `assigned`,
`accepted` ou `running` laissées par un arrêt brutal.

Preuves : `echo_and_blake3_are_deterministic_and_pure`,
`execution_respects_the_task_output_limit`,
`executor_completes_an_echo_task_with_a_verified_result`,
`executor_computes_blake3_and_respects_the_output_limit`,
`executor_recovers_an_interrupted_task`.

Limites : l'exécution est locale et mono-nœud ; aucun reçu signé n'est encore
vérifié de bout en bout (`F1-TASK-03`), et le placement distribué reste absent.

## Reçu de résultat signé — 2026-11-13 (session 9)

`F1-TASK-03` (volet local) : un résultat signe sa tâche, son producteur, son
hash et son instant, et la gateway vérifie cette signature avant d'accepter
`complete`.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | **512 réussis, 0 échec**, exit 0 |
| `node scripts/check-docs.mjs --tests 512` | documents cohérents, 85 Markdown couverts, exit 0 |
| `cargo test -p polygone-client --test doc_index` | 1/1, exit 0 |
| `gitleaks git .` | historique complet, aucune fuite |
| `gitleaks dir` (arbre hors `target/`, `.git/`) | aucune fuite |

Le payload signé est
`polygone-task-result-v1\n<task_id>\n<producer>\n<output_hash>\n<produced_at>`.
`TaskResult::verify` recalcule le hash, exige `producer == NodeId(clé publique)`
et vérifie la signature ML-DSA-65. `TaskRecord::complete` refuse en plus un
`task_id` différent. Un résultat non signé, une clé étrangère, une tâche
étrangère, un résultat futur ou un hash altéré sont refusés ; l'état de la
tâche ne passe pas à `succeeded`.

La clé de l'exécuteur est persistée dans `gateway-signing.json`, à côté de
`tasks.json`, en `0600` atomique. Elle est rechargée au redémarrage, donc une
tâche reprise est signée par le même `NodeId`. Si la clé a changé, la reprise
échoue explicitement au lieu de signer sous une identité qui n'est plus la
sienne.

Preuves : `complete_rejects_an_unsigned_result`,
`complete_rejects_a_result_signed_by_another_key`,
`complete_rejects_a_result_for_another_task`,
`executor_completes_an_echo_task_with_a_verified_result`,
`recovery_refuses_to_sign_for_another_identity`,
`executor_key_is_created_private_and_reloaded`.

Limites : la vérification est locale et le reçu n'est pas encore consommé par
un tiers ; le placement multi-nœuds reste absent.

## Supervision agrégée et Device Reuse — 2026-11-15 (session 10)

`F1-OBS-01` (volet gateway) et le volet « proposition de rôles » de Device
Reuse. Aucun compteur simulé, aucun rôle activé.

| Contrôle | Résultat |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test -p polygone-core reuse::` | **8 réussis, 0 échec** |
| `cargo test -p polygone-gateway diagnostics` | **2 réussis, 0 échec** |
| `cargo test -p polygone-client --bin polygone doctor` | **8 réussis, 0 échec** |
| `cargo test --workspace --no-fail-fast` | **565 réussis, 0 échec**, exit 0 |

`GET /api/v1/diagnostics` agrège : nœuds connus, nœuds actifs (≤ 300 s),
latences mesurées et non mesurées, médiane `null` sans mesure, répartition des
10 états de tâche, `executor_attached`, persistance déclarée, et deux sections
`available: false` motivées (`daemon`, `refusals`). Un test vérifie qu'aucune
clé de signature (`signing_key_hex`) n'apparaît dans le corps, et qu'un nœud
sans mesure ne produit pas `0`.

`polygone_core::reuse::suggest_roles` rend quatre propositions motivées
(`relais`, `supervision`, `stockage`, `calcul léger`) ; `polygone doctor` les
affiche à partir des mesures du daemon. Une contrainte non mesurée est
« contrainte non évaluée » ; un stockage non mesuré refuse le rôle de stockage.

La console `web/node.html` affiche ce rapport dans un panneau « Diagnostic
agrégé » ; `node --check web/dashboard.js` sort 0, et la route a été appelée
sur un socket TCP réel (`curl http://127.0.0.1:18088/api/v1/diagnostics` →
réponse `polygone-diagnostics-v1`, `node.html` → 200). Une gateway sans cette
route laisse le panneau à « — ».

Limites : la gateway ne parle pas au socket du daemon (pas de réserves ni de
compteurs de refus dans ce rapport) ; aucun flux d'événements ni historique ;
aucun rôle distribué ; toujours un seul hôte Linux avec shell.
