# DAEMON_STATE — état réel du daemon `polygoned`

> Rapport initial du 29 septembre 2026, conservé comme trace historique.
> **Ne pas lire les sections historiques ci-dessous comme l'état courant sans
> tenir compte de la mise à jour du 2 novembre 2026.**

## 0. État courant vérifié — 2 novembre 2026

- `GlowUpEngine::apply()` applique uniquement une priorité `nice`. Aucun quota
  CPU, aucune limite RAM et aucune affinité CPU ne sont imposés à un consommateur.
- Le serveur IPC Unix v1 est maintenant démarré par le binaire `polygoned` sur
  Linux/macOS (hors `--dry-run`). Il publie l'état via `runtime::run_tick`,
  accepte `status`, `ping`, `alloc` et `release`, et ferme son socket proprement.
- Le socket est créé avec les permissions `0600`. Le ledger est isolé par chemin
  explicite dans les tests ; ceux-ci ne mutent plus `POLYGONE_LEDGER_PATH`.
- `crates/client/src/node.rs` est exposé par la bibliothèque et branché aux
  commandes `polygone node servir/status`. Le nœud demande un bail avant le bind,
  sert les blocs avec une limite de transferts auto-imposée, puis rend le bail à
  Ctrl+C, à la perte du daemon ou au signal de retrait de politique.
- `GlowUpEngine::new` borne son allocation de démarrage à la RAM mesurée et au
  plafond configuré ; si la mesure échoue, il démarre à zéro plutôt que
  d'inventer une capacité.
- `GlowUpEngine::smooth_transition` compte les cycles consécutifs où la cible
  RAM descend sous l'allocation actuelle. Après l'hystérésis, `shrinking=true`
  reste publié pendant toute la contraction, jusqu'à la cible.
- Pendant cette contraction, les nouvelles allocations sont refusées avec
  `policy_shrinking`. Les requêtes de statut et de libération restent actives.
- Sans option d'isolation, un bail reste une licence et le nœud réduit lui-même
  ses transferts simultanés : aucun quota CPU/RAM OS n'est alors appliqué.
- Sous Linux, `polygone node servir --isoler` relance le client dans un scope
  systemd utilisateur dédié sous `app.slice` ; le client vérifie que son chemin
  cgroup est distinct de `session-*.scope` et que `memory.max` / `cpu.max`
  correspondent au bail demandé avant de lancer le service. Si le mécanisme n'est
  pas disponible ou ne correspond pas, aucun nœud n'est démarré.
- Ce mode est vérifié en E2E sous Linux. macOS n'a pas encore d'équivalent.
- Linux et macOS rapportent `UserActivity::Unknown` faute de source fiable.
- Windows reste explicitement non supporté par le daemon à ce stade.
- Vérifications du 2 novembre : daemon **95 tests** ; workspace complet
  **333 tests** réussis ; `cargo fmt --all -- --check`, Clippy strict et
  `git diff --check` réussis. Les tests client couvrent refus, service, perte
  du daemon, retrait de politique et libération.
- Test systemd/cgroup réel : `polygone node servir --isoler` s'exécute sous
  `/user.slice/user-1000.slice/user@1000.service/app.slice/polygone-node-….scope`,
  avec `MemoryMax=268435456` et `CPUQuota=100000/100000`. La session est restée
  dans `/user.slice/user-1000.slice/session-19.scope` avec `memory.max=max`,
  `cpu.max=max 100000` et `pids.max=max`. Après SIGINT, systemd indique
  `LoadState=not-found`, `ActiveState=inactive` et aucun scope Polygone résiduel.
- Test de processus complet avec configuration temporaire de pression mémoire :
  le daemon réel publie `shrinking`, refuse un bail de remplacement, le client
  réel arrête son service et rend son bail (`retrait_politique`, `released=true`).
- Test IPC réel : hello, status, allocation, libération, socket `0600` et
  nettoyage après SIGINT. Test CLI réel : démarrage du nœud, réponse `MISS`,
  `node status` frais, arrêt et bail rendu. Aucun fichier de configuration
  personnel ni aucune donnée du HOME réel n'a été modifié par ces tests.

Les sections 1 à 5 sont le rapport détaillé de septembre et doivent être lues
comme historique lorsque leurs affirmations contredisent ce bloc courant.

---

## 1. Localisation

### Trois emplacements portent le nom « daemon / polygoned »

| Emplacement | Taille | Nature | Verdict |
|---|---|---|---|
| `~/Projects/Polygone-real/daemon/` | **3 769 lignes**, 14 fichiers Rust | Le vrai daemon d'allocation de ressources (crate `polygoned`, membre du workspace) | ✅ **c'est celui-ci** |
| `~/Projects/Polygone-Network/crates/polygoned/` | 95 lignes | Daemon de **relais** : lance le relais TCP et affiche l'hôte. Aucune allocation de ressources. | ⚪ homonyme, autre métier |
| `~/Projects/Polygone-v2/crates/polygoned/` | 5 lignes | Coquille qui délègue au relais. | ⚪ quasi vide |

Le présent rapport ne porte que sur **`~/Projects/Polygone-real/daemon/`**.

### Fichier principal (point d'entrée)

- **`~/Projects/Polygone-real/daemon/src/main.rs` — 525 lignes.**
  C'est le point d'entrée du binaire `polygoned` (version 2.0.0), déclaré dans
  `daemon/Cargo.toml` comme `[[bin]] name = "polygoned"`.
  Il contient : lecture de la ligne de commande, chargement de la configuration,
  création de la plateforme système, la boucle de fond, le serveur d'état HTTP et
  les commandes `status` / `stop` / `doctor`.

- **Cœur de la politique d'allocation : `daemon/src/policy/glow_up.rs` — 683 lignes**
  (c'est là que vit « Glow-Up »).

### Découpage complet du daemon

| Fichier | Lignes | Rôle |
|---|---|---|
| `src/main.rs` | 525 | Boucle, CLI, endpoint HTTP |
| `src/policy/glow_up.rs` | 683 | Politique d'allocation Glow-Up, paliers, sécurité |
| `src/resources/linux.rs` | 400 | Plateforme Linux (CPU, RAM, réseau, GPU, IPC, service) |
| `src/allocator.rs` | 365 | **Ancien** allocateur (« Wozniak ») — **plus utilisé par la boucle** |
| `src/resources/macos.rs` | 348 | Plateforme macOS (partiellement stubée) |
| `src/resources/mod.rs` | 283 | Types partagés + trait `Platform` + fabrique |
| `src/cpu.rs` | 283 | Détection/affinité/nice CPU |
| `src/system.rs` | 268 | Capture d'instantané système (`SystemSnapshot`) |
| `src/bandwidth.rs` | 260 | Mesure de bande passante |
| `src/gpu.rs` | 149 | Détection GPU via `nvidia-smi` |
| `src/socket.rs` | 140 | Écriture des décisions vers le socket IPC |
| `src/lib.rs` | 45 | Exports de la bibliothèque |
| `src/resources/windows.rs` | 11 | **Placeholder** (Windows non implémenté) |
| `src/policy/mod.rs` | 9 | Module de politique |
| **Total** | **3 769** | |

---

## 2. Ce qui fonctionne

Chaque point est adossé à une preuve : nom de fonction ou nom de test réellement présent.

### 2.1 Boucle temps réel et politique Glow-Up

- **Boucle périodique** : `SystemSnapshot::capture` → `GlowUpEngine::tick` → `apply`,
  cadencée par `behavior.tick_interval_secs` (5 s par défaut), avec arrêt propre sur signal.
- **Paliers utilisateur** : `eco`, `balanced`, `performance`, `max` (+ `custom`), définis
  dans `TIER_PRESETS`, avec des plafonds CPU / RAM / bande passante / GPU.
  *Preuve : test `policy::glow_up::tests::test_tier_presets`.*
- **Planchers de sécurité** : `SafetyMargins::default()` = 4 Go de RAM libre minimum et
  plafond dur CPU à 85 % (une config partielle ne peut pas les baisser).
  *Preuve : test `policy::glow_up::tests::test_safety_application`.*
- **Lissage anti-oscillation** : montée rapide, descente lente, avec hystérésis
  (`grow_step_pct` 10, `shrink_step_pct` 5, `shrink_hysteresis_ticks` 5).
- **Frein en cas d'activité utilisateur** : le code existe (`throttle_on_user_activity`,
  division par 2 de l'allocation) — voir §3 pour la raison pour laquelle il ne se
  déclenche jamais.

### 2.2 Ressources mesurées

- **CPU** : détection des cœurs réellement autorisés au processus via `sched_getaffinity`,
  et choix des cœurs en évitant le cœur 0 (réservé aux interruptions).
  *Preuves : `cpu::detect_cores`, `cpu::pick_cores` ; tests `detect_cores_is_one_or_more`,
  `pick_cores_never_zero_and_within_bounds`, `allocate_reserves_one_for_host`,
  `allocate_returns_at_least_one`, `ratio_is_correct`.*
- **RAM** : lecture de la mémoire totale / libre / disponible.
  *Preuve : `LinuxPlatform::memory_info` (lecture de `/proc/meminfo`).*
- **GPU** : détection du VRAM total et utilisé via `nvidia-smi`.
  *Preuves : `LinuxPlatform::gpu_info`, `gpu::allocate` ; test
  `gpu::tests::allocate_returns_something_when_nvidia_smi_available` (passe aussi sans GPU).*
- **Bande passante** : mesure des compteurs réseau et moyenne glissante de 60 s.
  *Preuves : `bandwidth::Monitor::tick`, `read_counters`, `detect_primary_interface` ;
  tests `read_counters_on_lo`, `monitor_creates_and_ticks`,
  `detect_primary_interface_skips_loopback`, `bandwidth_allocation_saturated`.*

### 2.3 Application réelle sur le système

- **Priorité CPU (nice)** : `GlowUpEngine::apply` appelle `set_cpu_priority`, qui invoque
  réellement `setpriority` sous Linux (niveau 0 si plus de 512 Mo alloués, sinon 5).
- **Limite mémoire** : aucune. `set_memory_limit` a été retiré du trait `Platform` ;
  `GlowUpEngine::apply()` n'écrit aucun `memory.max` et ne pose aucun quota RAM.
- **Décroissance propre à l'arrêt** : la boucle remet l'allocation à zéro et envoie un
  signal `shrink` avant de quitter.

### 2.4 Outils utilisateur

- **`polygoned status`** : palier, limites effectives, RAM totale/libre/utilisée, cœurs,
  charge, GPU détectés, chemin et existence du socket.
- **`polygoned doctor`** : capacités de la plateforme (affinité, priorité, limite mémoire,
  moniteur réseau, moniteur GPU, sockets Unix, pipes nommés), CPU, mémoire, interfaces, GPU.
  *Preuve : `run_doctor` dans `main.rs` ; entrée dédiée dans `CHANGELOG.md` (« 4ᵉ binaire
  prouvé — `polygoned doctor`, exit 0 »).*
- **`polygoned stop`** : envoie le signal de décroissance.
- **`--dry-run`** : calcule et journalise sans rien appliquer.
- **`--gen-config`** : écrit un fichier de configuration relisible par le daemon lui-même.
- **`--tier <eco|balanced|performance|max>`** : changement de palier en ligne de commande.
- **`--expose <adresse>`** : petit serveur HTTP sans dépendance externe exposant `/status`
  et `/health` en JSON.

### 2.5 Robustesse de configuration

- Le daemon sait lire **l'ancien format** de configuration (`[tier]` sous forme de table,
  y compris le `[tier] name = …` écrit par les versions fautives) et retombe sur des
  valeurs par défaut saines quand des champs manquent.
  *Preuves : tests `test_config_legacy_tier_table`, `test_config_legacy_name_key`,
  `test_config_minimal_defaults`, `test_config_current_format_roundtrip`.*
  C'est une décision consignée (`DECISIONS.md` — D8).

### 2.6 Serveur IPC local (mise à jour du 2 novembre)

- Le binaire démarre maintenant le serveur Unix v1 sur `socket::socket_path()`
  hors `--dry-run`. Il sert `status`, `ping`, `alloc_request` et `release`.
- Les instantanés sont publiés par `runtime::run_tick`, le chemin de publication
  unique et testé. À l'arrêt, l'état nul est publié avant la fermeture du serveur.
- Le socket est propriétaire uniquement (`0600`). Le serveur refuse de remplacer
  une instance vivante ou un fichier qui n'est pas un socket.
- Ce serveur ne constitue pas encore un consommateur de ressources : voir §3.1.

### 2.7 Installation de service (code présent)

- Génération d'une unité `systemd --user` et activation, ainsi que la désinstallation.
  *Preuve : `LinuxPlatform::install_service` / `uninstall_service`.*
  ⚠️ **Jamais appelée** par le daemon ni par la ligne de commande — voir §3.

---

## 3. Ce qui manque

### 3.1 Le protocole IPC est servi, mais aucun consommateur produit n'est branché

- Le binaire `polygoned` démarre désormais le serveur Unix v1 ; le protocole est
  joignable et le test manuel du 2 novembre a vérifié `hello`, `status`,
  `alloc_granted`, `released` et le nettoyage à l'arrêt.
- `crates/client/src/node.rs` contient un consommateur expérimental, mais il
  n'est pas monté dans `crates/client/src/lib.rs` et le crate client ne dépend
  pas encore du crate `polygoned`.
- Il n'existe donc pas encore de parcours produit `polygone node start` qui
  obtient un bail, sert des blocs et restitue le bail de bout en bout.
- Le bail demeure une licence que le consommateur s'engage à respecter ; le
  daemon n'impose pas de quota OS au processus consommateur.

#### Les trois grandeurs, désormais séparées (2026-11-03)

Le défaut tenait dans un seul mot : « capacité ». Un client lisait un chiffre et
croyait à une garantie. Trois notions distinctes existent maintenant dans le
type `Capacity` (`daemon/src/socket.rs`) et dans `Reply::AllocGranted.enforced` :

| Grandeur | Sens | Aujourd'hui |
|---|---|---|
| `measured` | ce que la machine possède, mesuré au tick | total physique réel |
| `reservable` | ce qui peut encore être promis (mesuré − plafond de palier − marges − réservations en cours) | ~7 Gio sur une machine de 16 Gio à 50 % |
| `enforced` | ce que l'OS **impose** au consommateur | **0** — `apply()` ne pose qu'un `nice` |

Conséquences visibles sur le protocole :

- `alloc_granted` porte `enforced: bool`, **calculé** (jamais écrit en dur) :
  `false` signifie licence, `true` signifie qu'un quota couvre réellement la
  demande. Aujourd'hui il vaut `false` et c'est la vérité.
- Le refus distingué : `exceeds_measured` (définitif : la demande dépasse la
  machine, réessayer ne servira à rien) vs `insufficient_headroom` (temporaire :
  un pair qui rend son bail le débloque). Les confondre poussait un client à
  réessayer indéfiniment contre un mur.

### 3.2 Le CPU n'est jamais réellement alloué

- `set_cpu_affinity` (Linux) et `cpu::apply_thread_affinity` **n'ont aucun appelant** :
  seule la priorité (nice) est appliquée. Malgré les journaux d'époque (« would set CPU
  affinity to N cores »), l'affinité n'est pas posée par la boucle actuelle.
- Le mode d'affinité (`cpu_affinity_mode` : auto / spread / compact / performance / off)
  est **lu dans la configuration mais jamais utilisé**.

### 3.3 Le GPU n'est jamais réellement alloué

- `suggest_gpu_allocation` et `gpu::allocate` **ne sont pas appelés** par la boucle.
- `GlowUpEngine::compute_ideal` ne calcule que la RAM et la bande passante : le VRAM est
  **mesuré puis ignoré**.
- Le bouton `gpu_allocation_enabled` est **lu dans la configuration mais jamais utilisé**.

### 3.4 La détection d'activité utilisateur est morte

- Linux et macOS renvoient `UserActivity::Unknown` faute de source de vérité
  branchée à la session graphique. Ce n'est pas une mesure d'inactivité.
  *Preuve : `daemon/src/resources/linux.rs` et `macos.rs`.*
- Conséquence : le frein « utilisateur actif » (division par 2) ne se déclenche **jamais**,
  alors qu'il est décrit comme une protection produit.

### 3.5 Bande passante : mesurée, jamais imposée

- Aucune limitation réelle (pas de `tc`, pas de limitation de trafic) ; le code le reconnaît
  explicitement dans `bandwidth.rs`.
- Le bouton `bandwidth_shaping` est **lu dans la configuration mais jamais utilisé**.
- La valeur affichée est dérivée de **compteurs cumulés** (`/proc/net/dev`, `/sys/class/net/*`)
  convertis en « Mbps » sans différence temporelle : ce nombre n'est donc pas un débit
  représentatif, seulement un ordre de grandeur cumulé.

### 3.6 Planchers et interrupteurs déclarés mais non consommés

- `min_free_cpu_cores` et `min_free_vram_mb` sont déclarés et affichés, mais **jamais lus**
  par le calcul.
- `apply_safety` ne borne que le pourcentage CPU : il ne borne pas la RAM, le GPU ni le
  réseau malgré ce que le nom suggère.
- `memory_limit_enabled` et `service_integration` sont **lus dans la configuration mais
  jamais utilisés**.

### 3.7 Deux allocateurs coexistent

- `allocator.rs` (365 lignes, dit « Wozniak ») est **du code mort fonctionnel** : plus
  utilisé par la boucle (la politique active est `GlowUpEngine`), mais toujours compilé et
  testé. *Confirmé par `ARCHITECTURE.md` §7.* Cela brouille la lecture du projet.

### 3.8 Divergence de nommage des paliers

- La vision canonique parle de « invisible / basic / recommended / performance » ; le code
  expose « eco / balanced / performance / max ». Le concept est le même, les noms non.

### 3.9 Portages incomplets

- **Windows** : non implémenté. Un `compile_error!` explicite interrompt la compilation
  Windows (choix assumé et honnête : jamais un binaire cassé qui prétend marcher) ; le
  fichier `windows.rs` est un placeholder. La promesse produit affichée est Linux + macOS.
- **macOS** : `set_cpu_affinity` est un simple journal (« stub »), `uptime()` renvoie 0,
  `user_active()` renvoie `false`. Sous macOS le daemon **mesure** mais **n'applique
  presque rien**.

### 3.10 Tests manquants

- Aucun dossier `daemon/tests/` : pas de test d'intégration.
- Le binaire (`main.rs`) a **0 test** : la boucle, le serveur HTTP `--expose`, les
  commandes `status` / `stop` / `doctor` et l'écriture socket ne sont pas couverts.
- Aucun test de bout en bout « un tick produit une modification observable sur le système ».

### 3.11 Points annexes

- L'endpoint `--expose` n'a **ni authentification ni restriction d'interface** : exposé sur
  `0.0.0.0`, il publie l'état du daemon à tout le réseau.
- L'installateur `install.sh` mentionne le binaire et une unité `polygoned.service`, mais
  le daemon n'appelle jamais `install_service` : le service est **déclaré, pas posé par ce code**.
- Aucun quota mémoire n'est actuellement appliqué, avec ou sans cgroups v2.

---

## 4. État de compilation

Dépôt testé : `~/Projects/Polygone-real` (espace de travail contenant `daemon`).

| Commande | Résultat |
|---|---|
| `cargo check -p polygoned` | ✅ **OK** — `Finished dev profile … in 58.61s`, code de sortie 0, aucune erreur, aucun avertissement bloquant |
| `cargo test -p polygoned` | Snapshot historique : 25 tests au 29 septembre. Vérification du 2 novembre : **83 tests passés, 0 échec**. |

Aucune erreur à signaler sous Linux.

**Réserves :**
- `ARCHITECTURE.md` annonce « daemon 21 » tests (total workspace 109) ; la suite réelle du
  daemon en compte aujourd'hui **25** — la documentation est en retard sur le code.
- Un build **Windows** ne compile pas, volontairement (`compile_error!` explicite).
- La commande demandée portait sur le paquet `polygoned` ; le reste du workspace n'a pas été
  recompilé dans le cadre de cette inspection.

---

## 5. Les 3 prochaines tâches

1. **Donner un consommateur réel au socket de décision** : brancher le nœud client sur
   `~/.polygone/daemon.sock` en alignant le chemin et le protocole avec le client Petals
   existant, afin que l'allocation calculée soit effectivement lue par quelqu'un.
   *Preuve du besoin : `ARCHITECTURE.md` §11 (« no consumer », décision D5 en attente) +
   les trois incompatibilités relevées en §3.1.*

2. **Appliquer réellement le CPU et le GPU** : appeler l'affinité CPU (via `cpu::allocate`
   et `set_cpu_affinity`) depuis `apply()`, et calculer une part de VRAM dans la politique
   quand `gpu_allocation_enabled` est actif, afin que le daemon ne se contente plus de
   mesurer ces deux ressources.
   *Preuve du besoin : `set_cpu_affinity`, `cpu::apply_thread_affinity` et
   `suggest_gpu_allocation` n'ont aucun appelant en dehors de leur définition (§3.2, §3.3).*

3. **Rendre la détection d'activité utilisateur vraie et consommer les interrupteurs
   morts** : implémenter `user_active()` (Linux et macOS) et lire réellement
   `min_free_cpu_cores`, `min_free_vram_mb`, `memory_limit_enabled` et
   `bandwidth_shaping`, avec au moins un test d'intégration « tick → application ». 
   *Preuve du besoin : `user_active()` renvoie `false` en dur (§3.4) et quatre champs de
   configuration ne sont jamais lus (§3.6).*

---

*Portée : inspection en lecture seule de `~/Projects/Polygone-real/daemon/`, plus deux
commandes de vérification (`cargo check`, `cargo test`). Les dépôts `Polygone-v2` et
`Polygone-Network` n'ont été consultés que pour localiser leurs homonymes de daemon.*
