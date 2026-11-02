# PLAN_INTEGRATION — brancher le serveur socket dans `main.rs`

Incrément 2b-3a : **plan seulement, aucun code écrit.** Décision acquise : l'état partagé devient `Arc<RwLock<DaemonState>>`.
Sources : `daemon/src/main.rs` (525 l.), `daemon/src/socket.rs` (1694 l.), `docs/PLAN_SOCKET.md`, `docs/DAEMON_STATE.md`.

---

## 1. Ordre de démarrage

Séquence actuelle (lignes de `main.rs`) :

| # | Étape | Ligne |
|---|---|---|
| 1 | `env_logger` puis `Args::parse` | 60-65 |
| 2 | `platform.init()?` | 72 |
| 3 | `load_or_create_config(&args)?` | 75 |
| 4 | Retours anticipés : `handle_command` / `gen_config` | 79-84 |
| 5 | `ctrlc::set_handler` → `RUNNING.store(false)` | 87-90 |
| 6 | `started_at_secs` | 97-99 |
| 7 | `GlowUpEngine::new(config, platform)` *(consomme `platform`)* | 105 |
| 8 | `status_state: Arc<Mutex<DaemonStatus>>` | 108-116 |
| 9 | `thread::spawn(run_status_http)` si `--expose` | 117-120 |
| 10 | `tick`, `tick_count`, `while RUNNING` | 122-126 |

**Insertion : entre (9) et (10), juste avant `let tick`.** Toutes les sorties anticipées sont passées (donc aucun nettoyage à prévoir sur le chemin 4) et `engine` existe déjà, ce qui permet de publier un état initial réel.

Dans cet ordre :

1. `let shared = Arc::new(RwLock::new(DaemonState::new(tier)))`. La valeur de `new()` n'est qu'une **valeur d'attente** (paliers plausibles, compteurs à zéro) : un client qui se connecte avant le premier tick reçoit une réponse bien formée plutôt qu'une erreur. Elle est remplacée dès la première publication.
2. `bind_listener(&socket_path())`, avec une **politique d'échec à trancher** :
   - `ErrorKind::AlreadyExists` → **quitter en erreur**. Ce cas signifie qu'une autre instance possède la surface de commande (ou qu'un fichier non-socket occupe le chemin) ; deux daemons qui allouent en parallèle se combattraient.
   - toute autre erreur (permissions, dossier) → `warn` et **continuer sans socket** : l'allocation est la mission première, la surface de commande est secondaire.
3. `let socket_shutdown = Arc::new(AtomicBool::new(false))`, puis `thread::spawn(move || serve_with_timeout(listener, state, socket_shutdown, CONNECTION_IDLE_TIMEOUT))`.
4. Un garde de nettoyage (§4), créé **après** un bind réussi seulement.

**Motif de `run_status_http` confirmé** (ligne 119) : `std::thread::spawn`, aucun runtime asynchrone. `serve_with_timeout` est bloquant et fait sa propre boucle `accept` — il est conçu pour un thread dédié (constat de 2a). On passe `CONNECTION_IDLE_TIMEOUT` explicitement ; `serve` reste le passe-partout.

⚠️ **Même en `--dry_run`, on bind et on sert.** `dry_run` veut dire « ne touche pas à l'OS », pas « ne publie pas tes décisions » — or c'est exactement ce qu'on veut observer. Conséquence directe : le retrait du fichier en sortie **ne doit pas** être placé dans le bloc `if !args.dry_run` (141-150, 176-183).

---

## 2. Publication de l'état

Dans la boucle, **après `engine.apply(&alloc)`** (143-145) et **à la place** du bloc `notify_allocation` (146-148), donc juste avant `log_alloc` (152).

Invariant à tenir : **le verrou n'est jamais tenu pendant un appel système, une écriture disque ou une écriture réseau.** Deux règles suffisent :

1. **Construire la valeur entière avant de verrouiller**, puis n'exécuter qu'un seul `*guard = snapshot`. La construction est pure : elle ne lit que `alloc` (une copie `Copy`), `snap`, `engine.config` et `engine.platform.name()`.
2. `engine.apply()` est appelé **avant** tout verrou : il fait des appels système (`set_cpu_priority`, `set_memory_limit`) mais personne ne tient le verrou à ce moment-là.

Règle symétrique côté serveur : `handle_client` lit l'état, **clone** l'instantané, relâche le garde, puis `dispatch(&snapshot)`. Jamais de verrou pendant `write_reply`. `dispatch` et `DaemonState` gardent leur signature (`&DaemonState`) — c'est tout l'intérêt du clonage.

**Où vit la conversion.** Recommandé : une fonction **pure** `runtime_state(engine: &GlowUpEngine, snap: &SystemSnapshot, started_at_secs: u64) -> DaemonState`, dans `lib.rs` et non dans `main.rs` — qui n'a aucun test. Pure ⇒ directement testable, et c'est exactement ce dont l'incrément suivant a besoin.

| Champ de `DaemonState` | Source |
|---|---|
| `tier` | `engine.config.tier` |
| `max_*_percent` | `engine.config.apply_safety(engine.config.effective_limits(&snap), &snap)` |
| `total_ram_bytes` | `snap.memory.total_bytes` (**pas** les 16 Gio codés en dur de `DaemonState::new`) |
| `ram_bytes` | `alloc.ram_bytes` |
| `bandwidth_mbps` | `alloc.bandwidth_mbps` |
| `cpu_cores` | ⚠️ aucune valeur honnête n'existe — voir Découvertes #5 |
| `gpu_free_vram_mb` | `Σ saturating(vram_total_mb − vram_used_mb)` sur `snap.gpu` |
| `shrinking` | `alloc.shrinking` |
| `uptime_secs` | `now − started_at_secs` |
| `daemon_version` | `env!("CARGO_PKG_VERSION")` |
| `platform` | `engine.platform.name()` |

La fréquence de publication est le tick : aucune limitation supplémentaire n'est utile, un `store` de structure coûte des microsecondes.

---

## 3. Retrait de l'auto-connexion — POINT CRITIQUE

Mécanisme : `send()` (socket.rs:102) se connecte à `~/.polygone/daemon.sock` et y écrit une ligne `{"cmd":…}`. Dès que le daemon écoute lui-même sur ce chemin, **il se connecte à lui-même** ; sa ligne n'a ni `v` ni `type`, donc elle est refusée en `UnknownType` à chaque tick. Le daemon se nourrit de ses propres erreurs.

Vérification par grep (tout le dépôt hors `target/`, `archive/`, `.agents/`) :

| Symbole | Appelants réels | Verdict |
|---|---|---|
| `send` (102) | les 4 `notify_*` uniquement | **Garder**, en `fn` privé, commentaire « écrit vers le nœud Polygone, jamais vers nous » |
| `notify_allocation` (127) | `main.rs:146` | **Supprimer** |
| `notify_shrink` (136) | `main.rs:180` et `main.rs:250` | **Supprimer les deux** |
| `notify_grow` (143) | **aucun** | **Supprimer** |
| `notify_status` (150) | **aucun** | **Supprimer** |
| `DaemonMsg` (48) | `send` + `notify_*` seulement | **Supprimer** avec eux |
| `ts_secs` / `chrono_lite_timestamp` (78-89) | `DaemonMsg` seulement | **Supprimer** avec lui |
| `socket_path()` (36) | `main.rs:240`, `main.rs:345`, interne | **Garder** — devient le chemin du serveur |
| `ensure_dir()` (93) | **aucun** (déjà constaté en 2b-1) | **Supprimer** (ménage) ; `bind_listener` crée le parent lui-même |

**Pourquoi supprimer plutôt que `#[allow(dead_code)]`** : ces fonctions n'ont **jamais servi** (Découvertes #1). Un `allow` ne protégerait pas un chemin éprouvé ; il laisserait un piège armé pour le prochain lecteur de `send()`, qui croira y voir le canal de sortie du daemon.

**Obligatoirement dans le même incrément.** Avant le bind, `send()` sortait en silence (`if !path.exists() { return Ok(()) }`, 104-107). Après, c'est une auto-connexion par tick. Séparer les deux changements ouvrirait une fenêtre où le daemon fonctionne *et* s'auto-alimente en erreurs — invisibles au niveau de journal par défaut (`log::debug!`).

**Un cas hors de la liste fournie, mais cassé par la même intégration** : `Commands::Stop` (`main.rs:250`) appelle `notify_shrink("user_requested")?` et imprime « shrink signal sent. ». Une fois le daemon serveur, ce message est refusé — **le CLI ment**. Recommandation : dans ce même incrément, retirer l'appel et faire dire la vérité à la sous-commande (« le protocole v1 n'a pas de requête d'arrêt »), avec un code de sortie non nul. Ajouter une vraie requête `shutdown` est un incrément à part, car cela touche la version du protocole — à planifier, pas à improviser ici.

---

## 4. Arrêt propre

- **Flag.** `shutdown: Arc<AtomicBool>` créé juste après `bind_listener`, cloné dans le thread serveur — même motif que `status_state` (108, 118). Deux drapeaux cohabitent : `RUNNING` (statique, cycle de vie du daemon) et `shutdown` (serveur). La boucle **ne lit jamais** `shutdown` ; c'est la sortie de boucle qui le pose.
- **Ordre de sortie** : boucle terminée → `shutdown.store(true)` → `join()` → retrait du fichier → « exited cleanly ».
- **Le vrai délai d'arrêt n'est pas le `join`, c'est la boucle.** Le `while RUNNING` (126) fait `sleep(tick)` **avant** de tester le drapeau : Ctrl+C peut donc mettre jusqu'à `tick_interval_secs` à être vu (**5 s** avec la config par défaut, `glow_up.rs:165`). L'arrêt du serveur, qui suit, hérite de ce délai. Si l'arrêt doit devenir rapide, il faut changer **la boucle** (dormir par tranches courtes et tester `RUNNING` entre chaque) — pas seulement le serveur.
- **Faut-il joindre le thread ? Oui**, avec une réserve : `JoinHandle::join` **n'a pas de délai d'attente** dans `std`. Ce n'est pas grave ici **parce que `serve` ne possède que la boucle `accept`** : les threads clients sont détachés (`thread::spawn`) et ne sont jamais attendus. Le seul blocage de `serve` est l'`accept` non bloquant suivi de `ACCEPT_IDLE_SLEEP` (50 ms), donc `join()` rend la main en ~50 ms. **Hypothèse à écrire dans le code** : la latence d'arrêt est bornée par `ACCEPT_IDLE_SLEEP` ; si cette constante grandit, l'arrêt ralentit d'autant. Un client bloqué en lecture jusqu'à `CONNECTION_IDLE_TIMEOUT` (60 s) n'est **pas** attendu, et c'est voulu.
- **Où retirer le fichier.** Recommandé : un **garde RAII** (struct privée, `Drop` → `remove_socket_file`), créé immédiatement après un bind réussi, dans l'esprit du `ConnectionGuard` de 2b-1. Il couvre d'un coup le chemin nominal *et* toute sortie anticipée par `?` ou panic, là où un appel explicite en fin de `main` finirait par être oublié. L'appel explicite reste acceptable si le garde semble superflu — mais alors il doit être **hors** du bloc `!args.dry_run`.
- **Ordre avec `join`** : retirer **après** le `join`, jamais avant. Supprimer le chemin pendant qu'un listener est encore lié laisse un socket vivant sur un inode sans nom : inoffensif, mais trompeur, et la sonde de `bind_listener` ne verrait plus personne.
- **Si le retrait échoue** : `warn`, puis **sortie normale** (code 0). Faire échouer un arrêt réussi pour un fichier résiduel serait disproportionné, d'autant que le démarrage suivant sait nettoyer un orphelin (`test_bind_listener_cleans_orphan`). Seule exception à signaler distinctement : « ce n'est plus un socket » — quelqu'un a remplacé le chemin ; le message de `remove_socket_file` le dit déjà, on le remonte tel quel.
- **Complémentarité à énoncer** : le garde couvre l'arrêt propre ; `bind_listener` (621) couvre `SIGKILL` et le double Ctrl+C, où aucun `Drop` ne s'exécute. Les deux sont nécessaires.
- **Sorties anticipées (79-84)** : toutes avant le bind ⇒ aucun nettoyage à prévoir.

---

## 5. Adaptation des tests

Trois signatures changent : `serve`, `serve_with_timeout` et `handle_client`, de `Arc<DaemonState>` vers `Arc<RwLock<DaemonState>>`. `dispatch` et `DaemonState` **ne changent pas** — c'est le but du clonage.

Bonne nouvelle : les 12 tests de `server_tests` passent tous par `start_server` / `start_server_with_timeout`, qui construisent l'état à **un seul endroit** (`socket.rs:1248`, seule occurrence de `Arc::new(DaemonState {`). Le reste de la suite est intact.

À modifier :
- `socket.rs:1248` : `Arc::new(RwLock::new(DaemonState { … }))`.
- Import de `std::sync::RwLock` dans le module de test.
- Helper à ajouter, à côté des autres : `fn shared_state(state: DaemonState) -> Arc<RwLock<DaemonState>>` — il servira aussi à l'incrément suivant.
- `fn state()` du module `tests` : **inchangé**, `dispatch` prend toujours `&DaemonState`.

À ajouter (la nouvelle logique n'a aucune couverture) :
- `test_runtime_state_maps_snapshot` : depuis un `SystemSnapshot` de référence, vérifier `tier`, `total_ram_bytes`, `ram_bytes`, `gpu_free_vram_mb`, `shrinking`, `platform`. C'est **la** fonction capable de publier un état faux sans qu'aucun test existant ne bronche.
- `test_shared_state_allows_concurrent_read` (facultatif) : un écrivain, deux lecteurs — verrouille l'invariant de publication.

---

## 6. Risques

`main.rs` a **zéro test** : le compilateur est la seule barrière, et il ne voit pas la sémantique. Les pannes sont donc silencieuses.

1. **Mauvais chemin.** Un littéral au lieu de `socket_path()` produirait un socket qui marche mais que personne ne regarde. Parade : n'utiliser que `socket_path()`, que `polygoned status` (240) et `doctor` (345) lisent déjà.
2. **Auto-connexion résiduelle.** Le daemon aurait l'air sain tout en s'envoyant un `UnknownType` par tick, journalisé en `debug` donc invisible au niveau `info` par défaut. Aucun test ne peut l'attraper sur un binaire. Parade recommandée : un garde-fou **mécanique** — un `grep -q "notify_" daemon/src` qui échoue si ces symboles réapparaissent. Peu élégant, mais c'est le seul contrôle qui fonctionne ici.
3. **Politique d'échec du bind.** Si `AlreadyExists` est traité en `warn` plutôt qu'en fatal, deux daemons allouent en parallèle sur la même machine : panne réelle, silencieuse, sans aucun test. C'est le point de sécurité de l'intégration.
4. **Verrou empoisonné.** Un panic en tenant le verrou d'écriture l'empoisonne et **toute publication ultérieure échoue**. Le code tolère déjà le poison en silence (`if let Ok(mut st) = status_state.lock()`, 155). À trancher explicitement : `unwrap_or_else(|e| e.into_inner())` plus un `warn` unique — sinon l'état du socket gèle sans le moindre signe.
5. **Correspondance sémantique fausse.** `cpu_cores` et `gpu_free_vram_mb` sont utilisés directement comme plafonds par `dispatch_alloc` (519, 527). Publier `0` (les valeurs par défaut) ferait refuser **toute** demande d'allocation sans qu'on comprenne pourquoi. C'est le risque le plus élevé de cette intégration.

**Forme du test « un tick → modification observable »** (incrément suivant) :
- Rendre la boucle atteignable : extraire le corps du `while` en une fonction de `lib.rs` (même geste que `runtime_state`) et permettre un `tick_interval_secs` court.
- Se donner **de quoi voir passer un tick** : aujourd'hui seul `uptime_secs` bouge (granularité 1 s) et `ram_bytes` reste stable sur une machine stable. Recommandation : ajouter un compteur `seq: u64` à `DaemonState` et à `status_response` — le test devient trivial (`seq` strictement croissant) et un vrai client y gagne un signal « quelque chose a changé ».
- Test : bind sur un chemin temporaire, `status_request` → relever `seq`, attendre plus que l'intervalle, `status_request` → `seq` a augmenté et `ram_bytes` correspond à l'allocation courante du moteur. Contre-épreuve : boucle arrêtée ⇒ `seq` ne bouge plus.

---

## Découvertes inattendues

1. **Les `notify_*` n'ont jamais fonctionné.** `send()` sort en silence si le socket n'existe pas (104-107) et rien ne l'a jamais créé : `~/.polygone/` ne contient aujourd'hui que `identity.json` et `reputation.json`. L'auto-connexion redoutée était donc précédée d'un silence total — ce qui renforce « supprimer » plutôt que « garder avec `#[allow(dead_code)]` » : on ne protège pas un chemin jamais emprunté.
2. **`Commands::Stop` devient un mensonge** (`main.rs:250`) : il imprime « shrink signal sent. » alors que le message sera refusé. La sous-commande n'est pas dans la liste des symboles à retirer, mais la même intégration la casse. À traiter dans le même incrément, sinon on remplace un no-op silencieux par un no-op qui ment.
3. **Le commentaire de `socket_path()` (32-35) devient faux** : il décrit un canal d'écriture du daemon vers le nœud, alors que le chemin est désormais celui du serveur. À corriger dans le même incrément, faute de quoi la doc contredit le code au moment précis où le sens change.
4. **L'indicateur socket de `status` / `doctor` change de sens** (240, 345) : il passe de ❌ à ✅ en permanence et signifie « le daemon écoute » au lieu de « le nœud Polygone écoute ». C'est plus juste, mais c'est un changement visible non annoncé.
5. **`DaemonState.cpu_cores` est documenté « cœurs encore disponibles »** (367) alors que le moteur n'alloue aucun CPU : `apply()` ne pose qu'un `nice` et une limite mémoire (497-508). Il n'existe donc **aucune valeur honnête** à publier. Publier le total de `snap.cpu.per_core.len()` est un plafond, pas un disponible — à assumer par écrit plutôt que de laisser le `3` codé en dur de `DaemonState::new` passer pour une mesure. `DAEMON_STATE.md` relevait déjà que `set_cpu_affinity` n'a aucun appelant ; ce point en est la conséquence côté état publié.
6. **Deux représentations de la même vérité** après intégration : `DaemonStatus` (pour `--expose`) et `DaemonState` (pour le socket). Elles diffèrent déjà (`started_at_secs` contre `uptime_secs`) et peuvent diverger. À unifier plus tard, à ne pas oublier.
