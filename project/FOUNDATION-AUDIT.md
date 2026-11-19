# FOUNDATION-AUDIT — audit intégral du socle Polygone

> Date de l'audit : **2026-11-09**. Dépôt : `~/Polygone-canonical`.
> Branche : `reconcile/2026-11`. HEAD audité : `e2936a3`
> (`feat(client,daemon): migrer peers.json v1->v2 et restaurer la mesure de stockage`).
> Méthode : lecture du code et des tests, exécution des suites, comparaison aux
> documents d'état. Une affirmation sans commande ou sans chemin de code n'entre
> pas dans ce document.
>
> Ce fichier décrit **ce qui existe réellement**, pas ce qui est souhaité. Les
> tâches qui en découlent sont dans [`FOUNDATION-ROADMAP.md`](./FOUNDATION-ROADMAP.md) ;
> le tableau de validation est dans [`FOUNDATION-STATUS.md`](./FOUNDATION-STATUS.md).

---

## 1. État du dépôt au début de l'audit

| Élément | Valeur mesurée |
|---|---|
| Branche | `reconcile/2026-11` |
| HEAD | `e2936a3e94ae2d7bd95afb9c21be8dd23c2641f2` |
| Commits dans l'histoire | 219 (`git rev-list --count HEAD`) |
| Entrées non commitées | 9 (2 fichiers Rust/Makefile/gitignore + 5 documents + 1 preuve non suivie) |
| Membres du workspace | 7 (`daemon`, `core`, `relay`, `client`, `gateway`, `petals`, `serverless`) |
| Binaires produits | 4 : `polygoned`, `polygone`, `polygone-relay`, `polygone-gateway` |
| Fichiers Rust du workspace | 88 |
| Lignes Rust du workspace | 35 577 (mesuré après le correctif de persistance, §5) |
| Tests workspace | 463 avant l'audit, **465** après l'ajout des deux tests de persistance |
| Formatage | `cargo fmt --all -- --check` : exit 0 |
| Disque | 17 Gio libres sur `/home` (93 % utilisés) — builds à éviter en parallèle |
| `gitleaks` | binaire `~/bin/gitleaks` 8.30.1 ; preuve antérieure dans `docs/proofs/gitleaks-2026-11-09.txt` |

### Ce que le dépôt contient hors workspace

`archive/2026-07-src/` (monolithe libp2p abandonné, non compilé),
`salvage/` (documents et un benchmark historique non compilé), `web/` (PWA
statique servie par la gateway), `landing/`, `site/`, `phase0/` (preuves et
script d'un test à deux hôtes), `brand/`, `docs/`, `reports/`.

**Règle appliquée** : ces répertoires ne sont ni compilés, ni testés, ni
considérés comme du produit. Ils sont conservés (règle « ne rien faire
disparaître ») mais aucune capacité n'est revendiquée à partir d'eux.

---

## 2. Cartographie réelle des composants

| Composant | Type | Rôle réel | Persistance | Tests |
|---|---|---|---|---|
| `crates/core` | lib | primitives sans réseau : ML-KEM-1024, ML-DSA-65, AES-256-GCM, Shamir 4/7, `NodeId`, enveloppes, reçus, erreurs | aucune | 32 |
| `daemon` (`polygoned`) | lib + binaire | mesure système, politique d'allocation, socket Unix local, ledger de réservations | `~/.polygone/ledger.json`, config TOML | 127 |
| `crates/client` (`polygone`) | binaire | produit CLI/TUI : identité, messages, réseau via relais, blocs, mesh, exécution, Hide, pause, diagnostic | `~/.polygone/` (identité, pairs, reçus, blocs), config | 195 |
| `crates/relay` (`polygone-relay`) | lib + binaire | routeur TCP aveugle, handshake authentifié par signature, shards en mémoire | aucune (amnésie au redémarrage) | 28 |
| `crates/gateway` (`polygone-gateway`) | lib + binaire | HTTP local, appairage, heartbeat signé, latence ping/pong, fichiers statiques PWA | `nodes.json` (écriture atomique) | 47 |
| `crates/petals` | lib | planificateur de backends d'inférence locale (Ollama, vLLM, llama.cpp) | aucun état runtime | 19 |
| `crates/serverless` | lib | découpage Shamir et quorum de votes — **non câblé au produit** | aucune | 17 |

Total mesuré : 465 tests (`cargo test --workspace`).

### Flux de dépendances

```
core  ←  relay
core  ←  serverless
core  ←  gateway
core  ←  daemon  ←  client
core  ←  petals  ←  (client, via Ollama HTTP seulement)
```

`daemon` est une bibliothèque **et** un binaire : `client` et `petals` dépendent
de `polygoned` pour parler le même protocole socket. C'est volontaire — une
copie locale du protocole avait déjà divergé (`crates/petals/src/daemon.rs`).

---

## 3. Architecture réelle

### 3.1 Topologie de processus

```
                 ┌──────────────────────────────┐
                 │  polygoned (1 par machine)   │
                 │  boucle 5 s : capture →      │
                 │  politique → publication     │
                 │  socket Unix 0600            │
                 └───────────┬──────────────────┘
                             │ JSON ligne / socket
                 ┌───────────┴──────────────────┐
                 │  polygone (CLI / TUI)        │
                 │  identité, messages, blocs,  │
                 │  Hide, compute, doctor       │
                 └───────┬───────────┬──────────┘
                         │           │
              TCP NDJSON │           │ HTTP loopback
                         ▼           ▼
                 ┌──────────────┐  ┌──────────────────┐
                 │ polygone-    │  │ polygone-gateway │
                 │ relay        │  │ PWA + API locale │
                 │ (aveugle)    │  └──────────────────┘
                 └──────────────┘
```

Le relais ne connaît ni le contenu, ni le nom des fichiers. Il voit
`from`, `to`, `session`, la taille et la cadence — cette limite est mesurée et
documentée, pas cachée.

### 3.2 Propriété des états

| État | Propriétaire unique | Support | Perte au redémarrage |
|---|---|---|---|
| identité (KEM + signature) | `crates/client/src/identity.rs` | `~/.polygone/identity.json` 0600 | non |
| ancre de confiance des pairs | `crates/client/src/net/mod.rs` | `~/.polygone/peers.json` 0600 | non (TOFU appris) |
| réservations de ressources | `daemon/src/socket.rs` (`Ledger`) | `~/.polygone/ledger.json` 0600 | non (écriture atomique depuis cet audit) |
| état publié du daemon | `daemon/src/runtime.rs` → `DaemonState` | mémoire (`RwLock`) | oui, recalculé au tick |
| routage des pairs connectés | `crates/relay/src/relay.rs` | mémoire | oui, par conception |
| nœuds vus par la gateway | `crates/gateway/src/lib.rs` | `nodes.json` | non |
| blocs adressés par contenu | `crates/client/src/blocks.rs` | `~/.polygone/blocks/` | non |
| cache de calcul | `crates/client/src/calc.rs` | répertoire de blocs | non |

Invariant structurel vérifié par test : `run_tick` est le **seul** écrivain de
l'état publié (`daemon/src/runtime.rs::test_tick_is_the_only_writer`), et aucun
fichier de `daemon/src/` n'écrit de plafond mémoire de cgroup
(`test_no_cgroup_write_anywhere`).

### 3.3 Frontières de confiance

1. **Hôte ↔ daemon** : socket Unix `0600`, propriétaire uniquement. Pas
   d'authentification par message : le contrôle d'accès est le système de
   fichiers.
2. **Client ↔ relais** : TCP, handshake `AUTH_INIT`/`CHALLENGE`/`AUTH_RESP`
   (preuve de possession de la clé KEM + signature ML-DSA-65). Le relais ne
   déchiffre rien.
3. **Pair ↔ pair** : signature ML-DSA-65 de bout en bout, vérifiée par le
   destinataire ; TOFU ancré dans `peers.json`.
4. **Gateway ↔ navigateur** : HTTP loopback, sans TLS ; appairage par jeton
   porteur à usage unique, heartbeat signé par la clé épinglée.
5. **Tâche ↔ hôte** : `systemd-run --user` (isolation contre les accidents) ou
   WASM `wasmi` avec carburant borné. Ce n'est **pas** un bac à sable
   cryptographique.

---

## 4. État réel des fonctionnalités

Légende : **OK** = implémentée, appelée et testée ; **PARTIEL** = existe mais
couverture ou intégration incomplète ; **NON CÂBLÉ** = code présent sans
consommateur de production ; **ABSENT** = documenté mais inexistant.

| # | Capacité | État | Preuve / limite |
|---:|---|---|---|
| 1 | Identité persistante (KEM + signature) | OK | `identity.rs`, permissions 0600, tests SEC.5 |
| 2 | Rotation / révocation de clé de signature | OK | `polygone clef-tourner`, 3 tests, `FV-SEC-05` |
| 3 | Message chiffré hors ligne (KEM + AES + Shamir) | OK | `msg.rs`, round-trip, mauvaise clé refusée |
| 4 | Message via relais, signé et anti-rejeu | OK | `net/mod.rs`, tests relais réels |
| 5 | Handshake de relais authentifié | OK | `relay.rs`, `docs/PROTOCOL-RELAY-AUTH.md` |
| 6 | Découverte LAN (mesh UDP) | OK | `mesh.rs`, allowlist d'interfaces, tests |
| 7 | Blocs adressés par BLAKE3, transfert TCP | OK (local) | `blocks.rs` ; **aucun test entre deux hôtes physiques** |
| 8 | Réservation CPU/RAM/stockage, expiration, reprise | OK | `socket.rs::Ledger`, 35 tests de module, persistance atomique |
| 9 | Mesure stockage réelle | OK | `resources::StorageInfo`, Linux + macOS |
| 10 | Politique d'allocation (paliers, marges, `shrinking`) | PARTIEL | décision calculée ; `apply` ne pose qu'un `nice` |
| 11 | Isolation OS CPU/RAM | PARTIEL | `--isoler` sous systemd/cgroup v2 uniquement, Linux |
| 12 | Exécution shell sandboxée | OK (licence) | `systemd-run`, `ProtectHome`, limites ; même UID |
| 13 | Exécution WASM bornée | OK | `wasmi` + carburant, timeout |
| 14 | Compute distribué (demande → nœud → résultat signé) | PARTIEL | `net/mod.rs` + `compute-smoke.sh` en local ; pas de placement multi-nœuds |
| 15 | Serverless en quorum | NON CÂBLÉ | `serverless::run` reconstitue localement, aucun nœud contacté |
| 16 | Petals : pipeline réparti | NON CÂBLÉ | `PipelinePlan` sans consommateur de production |
| 17 | Réputation comme sélecteur de pairs | NON CÂBLÉ | score affiché, aucun sélecteur ne l'utilise |
| 18 | Hide SOCKS5 mono-saut | PARTIEL | smoke réel local, pas de multi-hop ni de test inter-hôtes |
| 19 | Gateway HTTP locale + appairage | OK | 47 tests, socket TCP réel |
| 20 | Heartbeat signé + latence ping/pong | OK (loopback) | clé épinglée, anti-rejeu, `latency_ms` seulement après pong |
| 21 | PWA installable | PARTIEL | coque et cache ; installation iOS/Android non testée |
| 22 | Diagnostic `doctor` / `verite` / `ecosysteme` | OK | lit l'état réel, affiche « non mesuré » plutôt que zéro |
| 23 | Pause / reprise bornée | OK | `pause.rs`, intention persistée, bail rendu |
| 24 | Désinstallation sans résidu | OK | `uninstall.rs`, refus hors racines, test HOME temporaire |
| 25 | Duress (destruction locale) | OK (manuel) | `duress.rs` ; aucun déclencheur automatique |
| 26 | Métriques exportables / rapport de diagnostic | ABSENT | aucun endpoint ni fichier de diagnostic agrégé |
| 27 | API produit pour tâches, historique, consommation | ABSENT | aucune API ; le ledger n'expose que des capacités agrégées |
| 28 | Bus d'événements pour une UI | ABSENT | le client interroge, rien ne pousse |
| 29 | TLS / exposition réseau de la gateway | ABSENT | HTTP loopback uniquement |
| 30 | Support Windows | ABSENT (assumé) | `compile_error!` explicite, canari CI non bloquant |

---

## 5. Défauts trouvés et corrigés pendant l'audit

| ID | Défaut | Impact | Correctif | Preuve |
|---|---|---|---|---|
| FND-01 | `Ledger::persist` écrivait directement sur le fichier final (`File::create` + `BufWriter`) | Un crash en cours d'écriture laissait un JSON tronqué ; `load_from` le lisait comme un ledger **vide** : toutes les réservations disparaissaient silencieusement | Écriture atomique : temporaire dans le même répertoire, `sync_all`, `rename`, fsync du répertoire ; fichier en `0600`; temporaire supprimé en cas d'échec | `test_ledger_persist_is_atomic_and_private` |
| FND-02 | Un ledger illisible était remplacé en mémoire sans aucune trace, puis écrasé à la première écriture | Perte définitive de l'état de réservation, sans possibilité d'inspection | Quarantaine `<nom>.corrupt-<horodatage>-<pid>` avant de repartir d'un ledger vierge ; journalisation de l'erreur | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| FND-03 | Le commentaire de `Reply::AllocGranted` affirmait « `expires_at` est aujourd'hui toujours `None` » alors que le code écrit une échéance et que `renew` existe | Documentation interne fausse, contraire au code et aux tests | Commentaire aligné sur le comportement réel (bail expirant, renouvelable) | lecture de `socket.rs`, tests `renew` |

Aucun de ces correctifs ne change le format du protocole ni du fichier : un
ledger existant reste lisible, un client existant reçoit les mêmes champs.

---

## 6. Défauts identifiés, non corrigés dans ce lot

| ID | Défaut | Sévérité | Pourquoi non corrigé ici |
|---|---|---|---|
| FND-04 | `Ledger::free_cpu_cores` compare les cœurs réservés au **total physique** (`state.cpu_cores`) et ignore `max_cpu_percent` du palier | P1 | Changer la sémantique modifie des tests et la politique produit ; doit être tranché explicitement (voir roadmap `F1-RES-03`) |
| FND-05 | `Reply::AllocGranted.enforced` compare `enforced_ram_bytes` (0) à la RAM demandée : correct aujourd'hui, mais aucune place n'est prévue pour un `enforced` **par ressource** | P2 | Nécessite un modèle de contrainte par ressource, pas un booléen |
| FND-06 | Aucune version de protocole au-delà de `v: 1` ; pas de négociation de capacités pour le socket daemon | P1 | Un changement de protocole casserait tous les clients ; doit être conçu avant la première rupture |
| FND-07 | Le relais ne conserve pas les fragments d'un pair hors ligne (drop) ; une déconnexion perd le message en cours | P1 | Comportement assumé aujourd'hui ; une file bornée est un chantier avec son propre modèle de menace |
| FND-08 | Premier contact TOFU : un `from` inconnu est accepté puis appris | P1 | Conception existante ; l'empreinte est affichée pour vérification hors bande. Une politique « refuser l'inconnu » doit être configurable |
| FND-09 | `DIRTY-REVIEW.md` annonce 31 fichiers « restant à statuer » alors que 4 d'entre eux ont été réintégrés ; le décompte n'est pas recalculé | P2 (documentation) | Document historique ; correction de comptage à faire avec la revue des fichiers restants |
| FND-10 | Aucun test entre deux machines physiques | P0 (preuve) | L'environnement ne fournit qu'un hôte ; procédure prête dans `docs/P0-DEUX-MACHINES.md` |
| FND-11 | CI GitHub Actions jamais exécutée sur cet arbre | P0 (preuve) | Aucun push autorisé ; la structure est vérifiée localement par `check-ci-honesty.mjs` |
| FND-12 | `petals` et `serverless` exposent des plans et du quorum sans consommateur CLI | P1 | Ne pas les présenter comme livrés ; câblage = chantier produit |
| FND-13 | Pas de métriques ni de rapport de diagnostic exportable | P1 | Nécessite d'abord une définition stable des événements et des données |
| FND-14 | `time_sync` archivé (décision D9) ; ne pas le réintroduire sans besoin démontré | P3 | Décision existante, respectée |

---

## 7. Duplications et couplages

1. **Deux chemins d'exécution de tâches** : `compute` via `net/mod.rs` (grant +
   RES) et `serverless` (Shamir + quorum). Le second ne contacte aucun nœud.
   Ils ne doivent pas être présentés comme un seul moteur.
2. **Deux surfaces de statut** : le socket daemon (`status_response`) et la
   gateway (`/api/v1/network/status`). Elles ne décrivent pas le même périmètre :
   le second est une table de nœuds vus avec heartbeat, pas l'état des
   ressources locales.
3. **`daemon/src/allocator.rs`** est un allocateur historique non utilisé par
   `GlowUpEngine`. Il est compilé et testé, mais aucun chemin de production ne
   l'appelle. Ne pas le prendre pour la politique active.
4. **Trois générations de handshake** dans l'histoire (`HELLO`, `AUTH_*` v1,
   mode `--allow-insecure-hello`) : le mode permissif existe pour la
   compatibilité ; il doit rester explicitement opt-in.
5. **`client/src/brain.rs` et `drive.rs`** n'ont aucun appelant (documenté dans
   `ARCHITECTURE.md`). Ne pas les compter comme des fonctionnalités.

---

## 8. Risques de sécurité (état, pas promesse)

| Risque | Réalité mesurée | Réduction existante | Risque résiduel |
|---|---|---|---|
| Métadonnées visibles du relais | `from`, `to`, `session`, tailles, cadence | contenu chiffré, nom de fichier chiffré | graphe social et adresses IP visibles |
| Usurpation d'identité au relais | handshake signé, slot non volable | preuve de possession KEM + ML-DSA | rotation de clé = changement de `node_id` |
| Rejeu | horodatage signé, fenêtre ±300 s, cache | refus explicite testé | horloges très dérivées à surveiller |
| Exécution de code d'un pair | shell `systemd-run` ou WASM | `ProtectHome`, `PrivateNetwork`, carburant | même UID pour le shell : pas de frontière cryptographique |
| Données au repos | identité et reçus en clair dans `~/.polygone` | permissions 0600, `duress` | vol du disque non chiffré |
| Déni de service local | 16 connexions, 50 req/s, 60 s d'inactivité | limites testées | un client local peut saturer le ledger en écritures |
| Dépendances | `pqcrypto-*`, `wasmi`, `tokio`, etc. | versions verrouillées par `Cargo.lock` | audit de vulnérabilités non exécuté dans ce lot |

Le modèle de menace consolidé est dans
[`docs/security/THREAT-MODEL.md`](../docs/security/THREAT-MODEL.md).

---

## 9. Problèmes de documentation

1. **Chiffres contradictoires** dans `ARCHITECTURE.md` (462/463, puis 35441) :
   recalculés et alignés sur 465 tests et 35 577 lignes.
2. **`STATE.md` contient plusieurs instantanés** (451, 463 tests) sans que le
   lecteur sache toujours lequel fait foi. La section « Vérifications manuelles
   actualisées » est la plus récente ; les autres sont historiques.
3. **`ETAT-REEL.md`** porte un score 11/18/4 explicitement gelé au 2026-11-02,
   avec un addendum du 5 novembre. Il ne doit pas être présenté comme un
   recomptage courant.
4. **`DIRTY-REVIEW.md`** : décompte incohérent (FND-09).
5. **Documents de vision multiples** (`VISION.md`, `VISION_CANONIQUE.md`,
   `VISION-COMPLETE.md`, etc.) : `VISION.md` est la référence ; les autres sont
   des traces.

---

## 10. Revue des modifications historiques

Source : `~/Ops/backups/polygone-unification-2026-11-09/dirty/` et
`docs/proofs/dirty-2026-11-09.tsv` (classification ligne à ligne).

| Verdict | Nombre |
|---|---:|
| Identiques au canonique | 23 |
| Canonique sur-ensemble (rien de réel perdu) | 14 |
| `a_examiner` (lignes uniques côté réel) | 31 |
| Répertoires non versionnés (outillage) | 5 |
| Absents du canonique | 5 |
| **Total** | **78** |

Fichiers réintégrés parce qu'ils portaient une fonctionnalité réelle :
`daemon/src/resources/mod.rs` (`StorageInfo`, `mount_for_path`),
`daemon/src/system.rs` (mesure au capture), `daemon/src/resources/linux.rs` et
`macos.rs` (`storage_info`). Un test de publication mesurée a en outre été
ajouté dans `daemon/src/runtime.rs`.

**Portée honnête de cette revue** : les 31 fichiers `a_examiner` n'ont pas été
réexaminés un par un dans cette session. Ceux cités dans `DIRTY-REVIEW.md`
(relais, `net/mod.rs`, `mesh.rs`, `socket.rs`, `runtime.rs`, `install.sh`) ont
été arbitrés en faveur du canonique, avec justification. Les autres restent
ouverts et sont suivis par `F1-DOC-02`. Aucun fichier réel n'a été supprimé.

---

## 11. Dépendances entre chantiers

```
identité stable ─┬─> réseau authentifié ─┬─> multi-machine ─┬─> multi-hôte réel
                 │                       │                  │
                 │                       └─> protocole versionné
                 │                                          │
ressources mesurées ─> politique du propriétaire ─> réservations ─> exécution distribuée ─> résultats vérifiables
                                                                    │
                                                                    └─> API produit ─> UI
observabilité ──────────────────────────────────────────────────────┘
```

Une UI construite avant les contrats de données afficherait des états que le
moteur ne sait pas produire. C'est pourquoi `product/UI-API-MAPPING.md` marque
explicitement les écrans sans API.

---

## 12. Recommandations, par ordre d'impact

1. **P0 — preuve multi-machine** : exécuter `docs/P0-DEUX-MACHINES.md` sur deux
   hôtes réels ; sans cela, aucun « réseau décentralisé » ne peut être déclaré
   validé.
2. **P0 — CI réelle** : exécuter le workflow sur ce HEAD (nécessite une
   autorisation de push) et rapporter le résultat du runner.
3. **P1 — politique CPU honnête** : aligner `free_cpu_cores` sur
   `max_cpu_percent`, ou documenter explicitement que le CPU est une licence
   globale sans plafond par palier.
4. **P1 — versionnement du protocole** : déclarer une version et une
   négociation de capacités avant toute rupture.
5. **P1 — observabilité minimale** : un rapport de diagnostic agrégé, sans
   secrets, dérivé de `status`, `refusals`, ledger et identité.
6. **P1 — câbler ou retirer** : `serverless`, `petals::PipelinePlan`,
   `reputation` ; ne pas les laisser entretenir une promesse.
7. **P2 — comptage `DIRTY-REVIEW`** et revue des fichiers restants.
8. **P2 — audit de dépendances** (`cargo audit` ou équivalent) et SBOM.

---

## 13. Limites de cet audit

- Une seule machine physique : aucun test inter-hôtes, aucun test NAT/pare-feu.
- CI distante non exécutée ; seule la structure du workflow est vérifiée.
- Aucun audit externe de sécurité ; le modèle de menace est une auto-évaluation
  citant les tests existants.
- Les tests de charge et de fuzz n'ont pas été exécutés.
- macOS n'a pas été compilé ici ; Windows est explicitement refusé.
- Les documents historiques n'ont pas été réécrits ; leurs contradictions sont
  signalées, pas maquillées.
