# VALIDATION-RESULTS — 2026-11-09

> **Instantané daté.** Ce document enregistre ce qui a été exécuté, quand, avec
> quel code de sortie, et ce que cela ne prouve pas. L'état courant est dans
> [`../../project/FOUNDATION-STATUS.md`](../../project/FOUNDATION-STATUS.md) et
> `STATE.md`.
>
> Machine : un seul hôte Linux, 4 cœurs, 7,4 Gio de RAM, disque à 93 %.
> Dépôt : `~/Polygone-canonical`, branche `reconcile/2026-11`, HEAD `e2936a3`
> plus les correctifs décrits ci-dessous.

---

## 1. Porte complète finale

Exécutée après tous les correctifs, dans cet ordre, sur le même arbre :

| Commande | Résultat | Code de sortie | Portée |
|---|---|---:|---|
| `cargo fmt --all -- --check` | aucun écart | **0** | tout le workspace |
| `cargo clippy --workspace --all-targets -- -D warnings` | aucun avertissement | **0** | tout le workspace |
| `cargo test --workspace` | **490 réussis, 0 échec** (doc-tests inclus) | **0** | L0–L2 local |
| `node scripts/check-docs.mjs --tests 490` | documents cohérents, 78 Markdown couverts | **0** | documentation |
| `node scripts/check-ci-honesty.mjs` | matrice honnête, 5 jobs, Windows hors du job bloquant | **0** | structure CI |
| `cargo test -p polygone-client --test no_tracked_secrets` | 2 réussis, 0 échec | **0** | secrets suivis |
| `git diff --check` | aucun problème d'espacement | **0** | diff |
| `cargo test -p polygoned socket::tests::` | 35 réussis, 0 échec | **0** | module socket |

Mesures associées : 90 fichiers Rust, **37 392** lignes, 4 binaires,
7 membres de workspace.

### 1.1 Incrément F1-TASK-01 (même arbre)

25 tests ajoutés : 15 dans `core::task` (cycle complet, transition illégale,
état terminal immuable, résultat altéré, producteur étranger, résultat futur,
bornes de spec, permissions refusées, sérialisation hexadécimale), 8 dans le
magasin gateway (publication, redémarrage, annulation durable, expiration
balayée, pagination, quarantaine, échec de persistance sans accusé) et 2 dans
le routeur (création/liste/lecture/annulation, refus d'un `state` glissé dans
le corps, requêtes malformées).

`latency_populated_after_pong` a été corrigé : il comparait `last_seen`, horodaté
par la gateway, au timestamp du client, et échouait à une frontière de seconde.

Ce qui reste non prouvé : aucun exécuteur n'est branché, donc aucune tâche ne
dépasse `published` par l'API ; les routes de tâches sont testées au niveau du
routeur en mémoire, pas par un test HTTP de bout en bout dédié.

---

## 2. Ce qui a été corrigé pendant ce lot

| Correctif | Fichier | Test / preuve |
|---|---|---|
| Écriture atomique du ledger, permissions `0600`, temporaire nettoyé | `daemon/src/socket.rs` | `test_ledger_persist_is_atomic_and_private` |
| Quarantaine d'un ledger illisible, aucune perte silencieuse | `daemon/src/socket.rs` | `test_corrupt_ledger_is_quarantined_not_silently_erased` |
| Commentaire `alloc_granted.expires_at` aligné sur le bail réel | `daemon/src/socket.rs` | lecture + tests `renew` |
| Test de heartbeat dépendant d'une frontière de seconde | `crates/gateway/tests/serve.rs` | `heartbeat_round_trip_over_real_http_is_persisted` : la valeur persistée est comparée à celle renvoyée par la gateway, pas au timestamp du client |
| Chiffres d'architecture recalculés | `ARCHITECTURE.md` | `scripts/check-docs.mjs --tests 465` |

### Le défaut de test, expliqué

`serve.rs` comparait `node.last_seen` (horodaté par la gateway) au `stamp`
envoyé par le client. Les deux valeurs ne coïncident que si la requête ne
traverse pas une frontière de seconde. Observé en échec :
`left: 1794251413`, `right: 1794251412`. Le test est désormais déterministe :
il relit `last_seen` dans la réponse du heartbeat et vérifie que la valeur
rechargée depuis le disque est identique, plus une borne `>= stamp`. Ce n'était
pas un défaut du produit.

---

## 3. Tests ciblés ajoutés par ce lot

| Test | Ce qu'il prouve |
|---|---|
| `test_ledger_persist_is_atomic_and_private` | le ledger est écrit par `rename`, en `0600`, sans temporaire résiduel, et reste un JSON complet |
| `test_corrupt_ledger_is_quarantined_not_silently_erased` | un fichier illisible est déplacé en quarantaine avec ses octets d'origine ; le daemon repart d'un ledger vierge sans effacer la trace |

---

## 4. Ce qui n'a pas été exécuté

| Preuve absente | Raison | Conséquence |
|---|---|---|
| Test entre deux machines physiques | un seul hôte disponible | aucune revendication « réseau validé » |
| CI GitHub Actions | aucun push autorisé | structure vérifiée, runner non prouvé |
| Gitleaks sur l'arbre final | non rejoué dans ce lot | la preuve du 2026-11-09 reste une preuve antérieure |
| Audit de dépendances / SBOM | hors périmètre de ce lot | risque de dépendance non évalué |
| Test de charge | machine limitée | comportement sous saturation inconnu |
| Compilation macOS | pas de runner macOS ici | support macOS non vérifié |
| Test sur VM neuve | pas de VM disponible | installation reproductible partielle |

---

## 5. Limites de lecture

1. Ces résultats sont **locaux**. Ils ne prouvent pas un fonctionnement entre
   deux hôtes.
2. Un test vert ne prouve pas l'absence de faille ; il prouve un comportement
   observé dans un cas donné.
3. Les tests de sécurité sont des contre-épreuves ciblées, pas un audit.
4. Les chiffres de lignes et de tests changent dès qu'un fichier est modifié ;
   `scripts/check-docs.mjs` refuse une documentation qui ne suit pas.
5. Aucun secret n'est reproduit dans ce document.

---

## 6. Incrément `F1-RES-03` / `F1-SEC-02` — 2026-11-13

Même hôte Linux unique, branche `reconcile/2026-11`, arbre de la session 6.

| Commande | Résultat | Code de sortie | Portée |
|---|---|---:|---|
| `cargo fmt --all -- --check` | aucun écart | **0** | tout le workspace |
| `cargo clippy --workspace --all-targets -- -D warnings` | aucun avertissement | **0** | tout le workspace |
| `cargo test --workspace` | **499 réussis, 0 échec** (doc-tests inclus) | **0** | L0–L2 local |
| `node scripts/check-docs.mjs --tests 499` | documents cohérents, 85 Markdown couverts | **0** | documentation |
| `cargo test -p polygone-client --test doc_index` | 1 réussi, 0 échec | **0** | index documentaire |
| `gitleaks git .` | 281 commits, aucune fuite | **0** | historique Git |
| `gitleaks dir` (copie hors `target/`, `.git/`) | aucune fuite | **0** | arbre de travail |

Mesures : 90 fichiers Rust, **37 929** lignes, 4 binaires, 499 tests.

### 6.1 Plafond CPU par palier (`F1-RES-03`)

`allowed_cpu_cores()` vaut `floor(cpu_cores × max_cpu_percent / 100)` et
`free_cpu_cores()` s'y limite. `cpu_cores` conserve sa signification de total
physique pour ne pas casser le protocole v1. Deux tests couvrent le plafond et
le refus `insufficient_cpu` au-delà. Les fixtures de test modélisent 6 cœurs
physiques (3 réservables à 50 %) pour préserver les scénarios de réservation.

Limite : le plafond est une politique de partage, pas un quota noyau ;
`enforced` reste `0`.

### 6.2 Politique de confiance stricte (`F1-SEC-02`)

`POLYGONE_TRUST_POLICY=strict` refuse un pair absent de `peers.json` avant toute
livraison, avec `AUTH.UNKNOWN_PEER`, et n'apprend pas son ancre. `tofu` reste le
défaut ; une valeur inconnue fait échouer le démarrage au lieu de retomber
silencieusement sur TOFU. Trois tests : analyse des valeurs documentées, refus
sans apprentissage, acceptation d'un pair ancré.

Limite : en mode strict, l'empreinte doit être vérifiée hors-ligne avant le
premier message ; aucun outil d'appairage guidé n'existe encore.
