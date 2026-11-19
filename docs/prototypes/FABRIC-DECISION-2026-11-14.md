# DÉCISION — consolidation de Polygone Compute (2026-11-14)

> **Instantané daté — 2026-11-14.** Ce document engage une décision de
> consolidation, pas l'état courant du produit. Voir
> [`ETAT-REEL.md`](../../ETAT-REEL.md) et [`STATE.md`](../../STATE.md).

## 1. Décision

**C — conserver `polygone-fabric` uniquement comme laboratoire isolé, invisible
dans le produit et interdit de release.**

Le transport, l'identité et l'exécution restent ceux de `crates/client`
Compute. Le laboratoire ne doit pas devenir un second service visible, ni une
seconde chaîne de sécurité.

La cible de sortie est **B** : réutiliser plus tard la seule abstraction qui a
de la valeur — le **planificateur de coût local/distant** — dans Compute, après
avoir mesuré les métriques réelles (RTT, débit, charge). Le transport Fabric ne
sera pas intégré.

**Pourquoi pas A maintenant** : intégrer Fabric reviendrait à ajouter ou à
remplacer un chemin TCP en clair, sans authentification de la demande, sans
anti-rejeu, sans sandbox et sans mesures réelles, alors que Compute possède déjà
le relais authentifié, les requêtes signées, le cache anti-rejeu, les grants
signés, la sandbox systemd/wasmi et la réputation locale. Ce serait une
régression de sécurité et un deuxième produit.

**Pourquoi pas B immédiatement** : le planificateur actuel compare des coûts à
partir de valeurs **déclarées** (`rtt_ms`, `bandwidth_kbps`, `load_permille`).
Aucune mesure réelle de RTT/débit n'est encore collectée par Compute. Le porter
maintenant produirait une précision fictive.

## 2. Comparaison

| Dimension | Compute existant (`crates/client`) | `polygone-fabric` |
|---|---|---|
| Transport | Relais TCP + enveloppes authentifiées ; le relais route sans déchiffrer les messages | TCP NDJSON direct, **en clair** |
| Identité | `NodeId` dérivé de ML-DSA-65, identité persistante | `NodeId` dérivé de ML-DSA-65, clés **éphémères** par processus |
| Demande | Signée, horodatée ±300 s, liée à la session | **Non authentifiée** (`requester` informatif) |
| Anti-rejeu | Cache borné (4096), TTL 600 s, échec fermé ; testé à travers le vrai relais | **Absent** (pas de nonce) |
| Réponse | `grant` signé, lié à la session et au demandeur ; TOFU/strict | `TaskResult` signé ML-DSA-65, hash BLAKE3, producteur vérifié |
| Sandbox | `systemd-run --user` (mémoire/CPU/temps, `NoNewPrivileges`, réseau privé) + wasmi (fuel, limites, sortie plafonnée) | **Aucune** : exécution dans le processus serveur |
| Limites | Bail du daemon + limites sandbox ; concurrence 2 | Aucune limite OS ; seules les bornes de `TaskSpec` |
| Placement | Choix explicite d'un ghost ; pas de calcul de coût local/distant | **Planificateur de coût**, marge 900 ‰, échéance, local d'abord |
| Reprise | Repli manuel / réputation locale | Exclusion du nœud fautif, `max_attempts` |
| Réputation | `reputation.json`, atomique, bornée | EWMA locale non persistée |
| Métriques | RAM libre réelle, bail, réputation ; RTT/débit non mesurés | Valeurs déclarées : RTT, débit, charge, batterie, température |
| Découverte | Mesh UDP 7642, interfaces LAN filtrées | Aucune ; pairs explicites |
| Daemon | Bail avant `bind`, arrêt sur perte de bail | Aucun |
| Stockage | Blocs BLAKE3, CAS existant | CAS + manifeste + plan de réplication (plan seulement) |
| Dépendances | Nombreuses (tokio, wasmi, daemon, etc.) | `polygone-core` + bibliothèques génériques |
| Surface produit | `polygone compute` (CLI/TUI) | binaire séparé, désormais **opt-in** |
| Test physique | Non exécuté sur deux hôtes | Non exécuté sur deux hôtes |

### Ce qui est réellement réutilisable

1. **Le planificateur de coût** (`planner.rs`) : règles déterministes, testées,
   sans dépendance au transport. À porter dans Compute **quand** les métriques
   seront mesurées.
2. **La vérification `TaskResult`** : hash + signature + `NodeId` + `task_id`.
   Compute signe déjà ses grants ; l'apport est la liaison explicite au hash de
   sortie et l'échec fermé sur résultat altéré.
3. **Le harnais de panne** : exclusion d'un nœud, deuxième tentative, résultat
   vérifié. Utile comme test de robustesse, pas comme architecture parallèle.

### Ce qui ne doit pas être réutilisé

- Le transport TCP en clair et la demande non authentifiée.
- Le serveur d'exécution sans sandbox.
- Les clés éphémères comme identité de nœud.
- Le second binaire comme chemin utilisateur.

## 3. Garde-fous appliqués

Dans `crates/fabric/Cargo.toml` :

```toml
publish = false

[features]
default = []
lab = []

[[bin]]
name = "polygone-fabric"
required-features = ["lab"]
```

Conséquences vérifiables :

- `cargo build --release` ne construit **pas** `polygone-fabric` ;
- `cargo build -p polygone-fabric` construit la bibliothèque seule ;
- le binaire n'existe que via `--features lab` ;
- `cargo test -p polygone-fabric` continue de tester la bibliothèque ;
- aucun autre crate ne dépend de `polygone-fabric` (vérifié par `cargo tree`).

Le crate reste dans le workspace pour ne pas perdre le code et ses tests, mais
il n'est pas un produit, pas une release, pas une commande `polygone`.

## 4. Résultats de sécurité vérifiés sur Compute

Commandes exécutées le 2026-11-14 :

| Test | Résultat |
|---|---|
| `compute_grant_tests` (2 tests) | **2 réussis** — succès/refus signés correctement interprétés |
| `net::tests::compute_request_replay_cache_rejects_duplicates_and_expires` | **réussi** |
| `net::tests::compute_request_replay_cache_fails_closed_when_full` | **réussi** |
| `net::tests::grant_reply_routes_back_and_signs_the_final_output_payload` | **réussi** |
| `net::tests::replay_through_relay_is_refused_with_an_explicit_code` | **réussi** (vrai relais) |
| `exec::tests::sandbox_*` (3 tests) | **ok mais SKIPPED** : `systemd-run --user` indisponible sur cette machine ; **aucune preuve d'exécution sandboxée** |

Le code confirme en outre :

- `verify_req_with_policy` vérifie signature, `NodeId` dérivé, fraîcheur
  ±300 s et ancre de confiance ;
- le cache anti-rejeu est borné à 4096 entrées et **échoue fermé** quand il est
  plein ;
- un grant est lié à la session, au demandeur et au ghost, puis vérifié ;
- le sandbox applique `MemoryMax`, `CPUQuota`, `RuntimeMaxSec`,
  `NoNewPrivileges`, `ProtectSystem=strict`, `PrivateNetwork` ;
- wasmi applique un fuel, des limites de compilation et un plafond de sortie.

### Lacunes de sécurité encore ouvertes

1. **Confidentialité du calcul** : la demande et la sortie Compute sont signées
   mais **pas chiffrées** de bout en bout ; le relais peut lire la commande et
   le résultat. C'est la priorité sécurité avant tout calcul sensible.
2. **Pas de TLS/Noise** : le transport vers le relais est en clair ; l'identité
   et les signatures protègent l'authenticité, pas la confidentialité.
3. **Pas de vérification indépendante du résultat** : la signature prouve
   l'identité du ghost, pas la correction de l'exécution. Aucun quorum.
4. **Pas de révocation** d'appareil.
5. **Sandbox non prouvée sur cette machine** : tests skippés.
6. **Pas de limites de stockage** dans le sandbox.
7. **Pas de métriques réelles de RTT/débit** pour le placement.

## 5. Validation physique

**Non exécutée.** Cette machine (`Hope`, `wlp3s0` 192.168.1.104) est le seul
hôte Linux joignable avec un shell. Tailscale ne montre qu'un iPhone actif, qui
ne peut pas exécuter le binaire Rust et n'expose aucun shell. Aucun second hôte
Linux avec identifiants n'est disponible.

Le protocole `docs/P0-DEUX-MACHINES.md` reste donc **non validé** ; son tableau
de preuves est vide. Aucun résultat physique n'est revendiqué.

## 6. Critères de sortie du laboratoire

Le laboratoire ne sera archivé qu'après :

1. un test réel sur deux machines physiques avec coupure et redémarrage ;
2. une mesure réelle de RTT/débit entre pairs ;
3. le port du **planificateur seul** dans Compute, avec tests sur métriques
   réelles ;
4. la preuve que Compute reste l'unique chemin d'exécution et de sandbox ;
5. la suppression du binaire lab du workspace ou son archivage explicite.

Tant que ces critères ne sont pas remplis, le laboratoire reste opt-in et ne
doit apparaître ni dans la CLI `polygone`, ni dans une release, ni dans la
documentation comme une capacité produit.

## 7. Décisions humaines encore nécessaires

- **Babel (`DEC-028`)** : voir
  [`BABEL-RESEARCH-2026-11-14.md`](BABEL-RESEARCH-2026-11-14.md). Choisir
  l'interprétation (adressage total, CAS + calcul, reconstruction générative)
  ou acter que Babel reste une métaphore.
- **Confidentialité du calcul** : accepter ou refuser que le relais voie les
  commandes/résultats Compute ; si refus, financer le chiffrement de bout en
  bout avant tout usage réel.
- **Validation physique** : fournir un second hôte Linux avec accès, ou
  accepter que la validation reste bloquée.
