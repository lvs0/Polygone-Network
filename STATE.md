# STATE — état courant

> Mis à jour par l'orchestrateur après chaque bloc de travail.
> Ne jamais écrire ici une valeur non vérifiée.

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
