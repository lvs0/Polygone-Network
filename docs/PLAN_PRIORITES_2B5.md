# PLAN_PRIORITES_2B5 — cgroup, réglages morts, allocation CPU

Incrément 2b-5a : plan de priorisation, **aucun code écrit, un seul fichier créé (ce
document)**. Références : `docs/PLAN_FREIN.md`, `docs/DAEMON_STATE.md`. Mesures faites avec
`target/debug/polygoned` du 29/09 01:54 (post-2b-4b) : linux, cgroup v2, 4 cœurs, 7,5 Gio.

## 1. Le cgroup : danger réel, confirmé

**Où écrit `set_memory_limit`** — `daemon/src/resources/linux.rs:168-179` :
> 171  `let cgroup_path = fs::read_to_string("/proc/self/cgroup")?` — **son propre cgroup**
> 178  `fs::write(cgroup_path.join("memory.max"), bytes.to_string())?;`

Le chemin écrit est `/sys/fs/cgroup` + **le cgroup du daemon lui-même**. Aucun sous-cgroup
dédié n'est créé. Lancé depuis ce terminal, le daemon partage le cgroup du lanceur :
```
$ cat /proc/self/cgroup
0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-graphical.slice/app-Hyprland-gtk\x2dlaunch-0b0bd2db.scope
$ cat /proc/258245/cgroup        # le daemon lancé depuis ce même shell -> la MÊME ligne
```
Avant tout lancement : `memory.max` = **`max`**, `cgroup.procs` = **10** (jusqu'à 12 en
cours de session), `memory.current` = **1402101760** (1,3 Gio).

**Un tick en `eco`**, dans un scope isolé de même nature mais vide (pour ne pas risquer la
session) : `t=+4 ms` → `memory.max = max` ; `t=+4566 ms` → **`memory.max = 322121728`**.
`memory.max` du cgroup de session : `max` avant, `max` après. Contrôle `--dry-run` (deux
ticks) : reste `max` — c'est donc bien `apply()` qui écrit, et `--dry-run` ne l'appelle
pas (`runtime.rs:134-142`). La valeur est **inventée** : `GlowUpEngine::new`
(`glow_up.rs:375-382`) part de `min(ram_pct × 1 Gio, 2 Gio)`, soit 30 % de 1 Gio =
322122547 o, arrondi par le noyau à 322121728 o (**307 Mio**) — sans rapport avec les
7,5 Gio de la machine.

**Qui est plafonné** : 10 à 12 processus, dont **un seul est le daemon** — `117708 foot`,
`117726 freebuff`, `117768 sh`, `117769 cat`, `120405 python3`, `256284 wl-copy`,
`258231 freebuff`, `258241 bash`, `258245 polygoned` (`cgroup.procs` + `ps -o pid=,comm=`)
— avec un plafond (307 Mio) **inférieur à ce que ce cgroup consomme déjà** (1,3 Gio). Le
noyau réclamera, puis **OOM-tuera** des processus du scope : navigateur et terminal avant.

**Sous-cgroup dédié** : `find /sys/fs/cgroup -maxdepth 6 -name "*polygoned*"` → **rien**.
Aucun n'existe — explicitement. **`cpu.max`** dans ce cgroup : **absent**
(`cpu.max=[ABSENT]`, `memory.max=[max]`), parce que le contrôleur `cpu` n'est pas délégué
sous `app.slice` (`user@1000.service` → `subtree_control=[cpu memory pids]`, `app.slice` →
`[memory pids]`). On ne peut donc pas y écrire — heureusement : cela aurait plafonné les
cœurs de toute la session graphique. `memory`, lui, est délégué : d'où le danger.

> **Conclusion.** Écrire `memory.max` dans le cgroup courant plafonne **10 à 12
> processus, dont 9 à 11 qui n'ont rien à voir avec le daemon** (terminal, shell,
> navigateur), avec une valeur (307 Mio) inférieure à ce que ce cgroup consomme déjà
> (1,3 Gio). Ce n'est pas un mécanisme d'allocation : c'est un OOM-kill en attente.

## 2. Les six réglages morts — confirmés (et un septième)

| Réglage | Défini | Défaut | Lu par la logique | Config générée |
|---|---|---|---|---|
| `min_free_cpu_cores` | `glow_up.rs:76` | `:87` | **nulle part** | oui |
| `min_free_vram_mb` | `:77` | `:88` | **nulle part** | oui |
| `memory_limit_enabled` | `:244` | `:268` | **nulle part** (`apply()` écrit sans le lire, `:517`) | oui |
| `bandwidth_shaping` | `:246` | `:269` | **nulle part** | oui |
| `gpu_allocation_enabled` | `:248` | `:270` | **nulle part** | oui |
| `service_integration` | `:250` | `:271` | **nulle part** | oui |
| `cpu_affinity_mode` *(7ᵉ, non demandé)* | `:242` | `:267` | **nulle part** | oui |
| `min_free_ram_gb` *(le seul vivant)* | `:75` | `:86` | **`glow_up.rs:451`** | oui |

Ailleurs, leurs seules occurrences sont des **fixtures TOML de tests**
(`glow_up.rs:780-796`) — jamais une lecture de décision (`grep -rn` sur `daemon/src`).
`--gen-config` les écrit pourtant tous (`toml::to_string_pretty(config)`, `main.rs:538`) :
`min_free_cpu_cores = 1`, `min_free_vram_mb = 512`, `memory_limit_enabled = true`, etc.

**Ce que croit l'utilisateur** avec `min_free_cpu_cores = 2` : que deux cœurs resteront
libres. Il relit sa valeur dans le fichier, `status` affiche `Limits : CPU …%`, et **rien
ne change, sans aucun message**. Un garde-fou affiché qui ne fait rien est pire qu'absent.

**Trois options** — *marquer* : doc + avertissement « champ ignoré » au chargement ; coût
faible, honnête, laisse du code mort. *Supprimer* : retirer du struct et de la config
générée ; coût faible, `serde` ignore déjà les clés inconnues donc les configs existantes
chargent encore ; le moins menteur. *Câbler* : implanter plancher CPU, VRAM et freins ;
coût élevé, car ni le CPU ni la VRAM ne sont alloués aujourd'hui (§3).

**Décision actée (2b-8, 29/09/2026)** — c'est *Supprimer qui parle et marquer qui
avertit* qui a été retenu : les **sept** clés ont quitté le schéma (`SafetyMargins`,
`DaemonConfig` dans `glow_up.rs`) et `--gen-config` ne les écrit plus (prouvé par
`test_generated_config_has_no_dead_keys`). Aucune config existante ne casse : `serde`
ignore les inconnues et **aucun** `deny_unknown_fields` n'existe dans `daemon/src`
(`test_legacy_config_with_dead_keys_still_parses`). Mais une clé avalée en silence reste
un mensonge plus discret : au chargement, `unknown_keys_in` (`glow_up.rs`) compare le
fichier au schéma et `main.rs` journalise **un avertissement par clé, avec son chemin
réel** — ce qui attrape du même coup les fautes de frappe (`tick_interval_sec`),
muettes jusqu'ici. Le type `CpuAffinityMode` (`resources/mod.rs`) est parti avec
`cpu_affinity_mode`, son unique lecteur.

> **Propagation terminée (2b-8b, 13/10/2026).** La suppression n'avait d'abord été
> appliquée qu'à `resources/mod.rs` : le champ `DaemonConfig::cpu_affinity_mode`, sa
> valeur par défaut, l'import de `glow_up.rs`, l'export de `lib.rs` et les fixtures de
> test pointaient encore vers un type qui n'existait plus — le crate ne compilait
> qu'à moitié. Même schéma pour `Platform::set_memory_limit`, appelé en
> best-effort dans `apply()` : le daemon n'écrit pas le cgroup qui le contient (il
> appartient à `systemd`, `MemoryMax=` de l'unité `--user`), l'appel est parti. Dans
> la même passe, `user_active: bool` est devenu `UserActivity` (Active/Idle/Unknown),
> parce qu'un booléen ne sait pas dire « je ne sais pas » — voir §4 et
> `docs/PLAN_FREIN.md`. Preuve : `cargo test --workspace` rc=0, `server_tests` inclus.

⚠️ Les numéros de ligne de ce §2 décrivent l'état **d'AVANT** 2b-8 : ils ne sont plus
valides. De même, les fixtures TOML de tests citées plus haut et le `main.rs:538` de
`--gen-config` ont bougé.

## 3. L'allocation CPU

1. **`set_cpu_affinity`** : déclaré `resources/mod.rs:242`, implémenté Linux en
   `resources/linux.rs:96-110` (`CPU_ZERO`/`CPU_SET` puis `sched_setaffinity(0, …)`), stub
   macOS `macos.rs:68`. `grep -rn set_cpu_affinity daemon/src` ne renvoie que la
   déclaration et les deux implémentations : **zéro appelant**, et `cpu_affinity_mode`
   n'est consulté nulle part.
2. **`apply()` aujourd'hui** (`glow_up.rs:508-519`), deux appels, dans l'ordre :
   `setpriority(PRIO_PROCESS, 0, nice)` (`linux.rs:113-120` ; `nice = 0` si
   `ram_bytes > 512 Mio`, sinon 5), puis `let _ = set_memory_limit(alloc.ram_bytes)`, dont
   **l'erreur est avalée**. Rien d'autre n'est posé.
3. **Un `nice` bas ne borne rien** — mesuré ici, 4 cœurs : travail fixe à `nice 0` →
   `wall=3,03 s  cpu=3,00 s` ; le même à `nice 19` → `wall=2,99 s  cpu=2,96 s` ; boucle
   infinie à `nice 19` → `ps` : **`%CPU 99,3`**. `nice` n'est qu'un **poids de partage** :
   sans tâche concurrente prête, le daemon prend 100 % des cœurs. Il ne protège que sous
   contention.
4. **`max_cpu_percent` (25 en `eco`) ne limite pas la consommation.** `compute_ideal`
   (`:405-419`) ne lit que `max_ram_percent` (`:409`) et `max_bandwidth_percent` (`:411`) :
   le pourcentage CPU n'est jamais converti en cœurs. Le seul usage d'un % CPU est
   `apply_constraints` (`:427-431`), qui compare le CPU **observé** à
   `safety.max_cpu_percent` (85) pour réduire la cible RAM et bande passante. Donc :
   **réduit la cible, ne limite jamais le daemon**.
5. **Mécanismes réels** : affinité (borne les cœurs, pas la part ; non câblée) / `nice`
   (mesuré : inefficace) / `cpu.max` du cgroup — **le seul vrai limiteur de débit**, mais
   il n'existe pas dans le cgroup du daemon (§1) : le câbler exige d'abord un cgroup où
   `cpu` est délégué, donc une unité `systemd --user`. Ce n'est pas un patch de 3 lignes.

## 4. Recommandation

1. **Le cgroup d'abord, tout de suite.** Tant que `apply()` écrit `memory.max` dans un
   cgroup qui n'est pas le sien, lancer le daemon depuis un terminal peut tuer le terminal
   et le navigateur. C'est le seul effet système réel du daemon, et c'est un piège. Le
   reste — réglages morts, CPU non alloué — est de la dette ; ceci est un incident en
   attente.
2. **Neutraliser avant d'installer** : retirer l'écriture, ou la conditionner à « ce
   cgroup est le mien » ; **puis** rendre le daemon propriétaire de son cgroup en
   installant l'unité `systemd --user` — `install_service` existe (`resources/linux.rs:320`)
   mais n'est **jamais appelée** — avec `MemoryMax=` et `CPUQuota=`, en laissant systemd
   gérer `memory.max`. À ce moment seulement l'écriture redevient légitime, et `cpu.max`
   devient disponible.
3. Ensuite : **supprimer** les six réglages morts (option la moins menteuse), puis traiter
   l'allocation CPU (`cpu_affinity_mode`, `cpu.max`) comme un incrément à part.

**Décision actée (2b-5b, 29/09/2026).** La limite mémoire appartient à `systemd`
(`MemoryMax=` de l'unité `--user`), pas au daemon : `set_memory_limit` a été retiré du trait
`Platform`, des implémentations Linux/macOS et de `apply()` — garde-fous : `runtime.rs:434`, `glow_up.rs:915`.

## 5. Découvertes qui contredisent les documents de référence

- `DAEMON_STATE.md` §2.3, §2.4 et §2.6 sont **périmés** : le daemon n'écrit plus vers le
  socket, n'envoie plus de `shrink` à l'arrêt, et `polygoned stop` **échoue
  volontairement** (`main.rs:370-377`). Ces passages décrivent l'état d'avant 2b-3.
- `DAEMON_STATE.md` §3.4 (« `user_active` renvoie `false` en dur ») est périmé depuis
  2b-4b (`UserActivity::Unknown`) ; §3.2 (affinité sans appelant, `nice` seul) est confirmé.
- `PLAN_FREIN.md` cite des lignes qui ont dérivé depuis 2b-4b (son `glow_up.rs:434` est
  aujourd'hui `:445`) ; ce document-ci donne les lignes **courantes**.
- Relevé au passage, hors sujet : `SIGTERM` (arrêt `systemctl`) laisse
  `~/.polygone/daemon.sock` derrière lui, et `SIGINT` (Ctrl+C) met jusqu'à ~15 s à sortir —
  un `status` peut donc afficher `✅` sur un socket mort.
