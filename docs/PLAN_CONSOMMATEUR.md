# PLAN_CONSOMMATEUR — qui consomme, et qui applique l'allocation

> Incrément 2b-5c, 29/09/2026. **Aucun code** : un seul fichier créé. Sorties citées
> brutes. État de départ : 2b-5b (71 tests verts) — `apply()` ne pose plus que son
> `nice` : le daemon mesure, décide et publie, et rien ne consomme.

## 1. Le consommateur prévu, tel qu'il est

- Paquet **`polygone-petals`** (`crates/petals/Cargo.toml:2`), lib `polygone_petals` (`:7`),
  **membre du workspace** (racine `Cargo.toml:9`).
- `$ cargo check -p polygone-petals` → `Finished dev profile [unoptimized] target(s) in 1m 25s`, `warning: polygone-petals (lib) generated 10 warnings`, **RC=0**, aucune erreur.
- Attentes (`crates/petals/src/daemon.rs`) : chemin `/tmp/polygoned.sock` + env `POLYGONE_DAEMON_SOCKET` (`:20-21`) ; **le client parle en premier** (`:34` connect, `:46-53` écrit puis lit **une** ligne), aucun `hello` attendu ; JSON par ligne **sans `v`**, requêtes
  taguées `type` avec les noms Rust (`AllocateResources`, `ReleaseAllocation`, `GetSystemResources`, `RegisterModel`, `Heartbeat`, `:187-208`) ; réponses `AllocationGranted|AllocationDenied|Released|SystemResources|ModelRegistered|Error|Pong` (`:210-218`) ;
  `ResourceAllocation` attend `id`, `cpu_cores`, `ram_gb`, `gpu_vram_gb`, **`cpu_cgroup`** (`:148`), **`memory_cgroup`** (`:150`), `expires_at: DateTime<Utc>` (`:152`).
- Appelants : `lib.rs:123-124` construit le client, `lib.rs:140-143` appelle `request_allocation(...).await?`, `lib.rs:255-259` libère dans `shutdown()` ; `get_system_resources` (`:97`) et `register_model` (`:107`) **n'ont aucun appelant**.
- Tests **du client** : `$ cargo test -p polygone-petals` → lib `running 0 tests`, `ok. 0 passed; 0 failed; 0 ignored`, doc-tests `1 passed; 0 failed` (`no_run`, `lib.rs:18`). `crates/petals/tests/` est **vide** ;
  aucun `#[test]`/`#[tokio::test]` dans le crate. `examples/integration_test.rs` n'utilise **jamais** `PetalsEngine` (il appelle `OllamaBackend` en direct) : le chemin daemon n'est exercé par aucun test.

## 2. Y a-t-il un autre consommateur ? Non — **personne**

`$ grep -rc -E "daemon\.sock|polygoned\.sock|POLYGONE_DAEMON_SOCKET" .` (hors `target/`, `.git/`) :

```
./crates/petals/src/daemon.rs:2  ./daemon/src/main.rs:1     ./daemon/src/socket.rs:7
./scripts/smoke-commands.sh:4    ./docs/DAEMON_STATE.md:4    ./ARCHITECTURE.md:2
./docs/PLAN_SOCKET.md:2  ./docs/PLAN_INTEGRATION.md:1  ./docs/PLAN_PRIORITES_2B5.md:1
./CHANGELOG.md:1  ./daemon/SPEC.md:1  ./ECOSYSTEM.md:1  ./docs/SESSION-REPORT-2026-08-08.md:1
./.claude/bundle-polygone-v2/cluster-00-67sessions.md:1
```

- Ces occurrences sont : le code du daemon lui-même, de la **documentation**, ou le client Petals mort (§1) ; aucun binaire, aucun script d'installation.
- `$ grep -rln -E "AF_UNIX|unix_socket|UnixStream|net\.connect|socket\.socket" --include=*.py --include=*.js --include=*.ts --include=*.sh .` → **sortie vide**.
- Seul script qui touche le chemin : `scripts/smoke-commands.sh:193-207` — il `touch` le fichier
  et vérifie que `polygoned status` affiche ❌/✅. **Il ne se connecte pas.**
- Verdict : **personne ne parle le protocole v1** ; `ARCHITECTURE.md:310` le dit déjà (« Daemon socket has no consumer … D5 »). C'est un résultat, pas un échec.

## 3. Les écarts client ↔ protocole v1 (cahier des charges)

| # | Écart | Client (`crates/petals/src/daemon.rs`) | Serveur v1 (`daemon/src/socket.rs`) |
|---|---|---|---|
| 1 | Chemin | `/tmp/polygoned.sock` + env (`:20-21`) | `~/.polygone/daemon.sock`, sans env (`:40-48`) |
| 2 | Version + nom | sans `v`, `"AllocateResources"` (`:187`) | `v:1` obligatoire (`:353`), `"alloc_request"` (`:79`,`:149`) |
| 3 | Salutation | inattendue | `hello` **avant** toute requête (`:915-921`) |
| 4 | Nom de réponse | `allocation_granted`, `released`, `error` | `alloc_granted` (`:197`), `released` (`:226`), `error` (`:170-255`) |
| 5 | Sans équivalent | `GetSystemResources`, `RegisterModel`, `Heartbeat` (`:195-206`) | `heartbeat` hors `KNOWN_REQUEST_TYPES` (`:75-82`) |
| 6 | Corrélation `id` | jamais émis, jamais lu | optionnel, écho dans chaque `Reply` (`:170-255`) |
| 7 | Jetés en silence | `concurrent_requests` (`:131`), `model_names` (`:133`) | absents de `Request::Alloc` (`:150-156`) |
| 8 | Champs réponse | `id`, `cpu_cgroup`, `memory_cgroup`, `expires_at` | `allocation_id` (`:199`), **pas de cgroup** (retiré en 2b-5b), `expires_at: None` (`:452`) |
| 9 | Cadence (abonnements) | un aller-retour, jamais d'écoute | `subscribe`/`unsubscribe` répondent (`:393-394`) mais `TickUpdate` **n'est jamais émis** (`:230-231`) |
| 10 | Comptabilité | croit `release_allocation` | `Release` n'efface rien (`:395`), `AllocGranted` ne réserve rien (`:407-455`) |

## 4. Qui applique l'allocation ?

**A — le consommateur se limite.** Le daemon accorde (`alloc_granted`), Petals refuse au-delà. *Gagné* : zéro privilège, marche sous macOS comme Linux, un seul lieu décide. *Perdu* : la contrainte n'est pas vérifiable — un client menteur est indistinguable.
*Échoue en silence* : le client travaille au-delà ; le daemon mesure la machine, pas son locataire, donc ses décisions deviennent de la fiction (2b-5a : son cgroup était celui de la session du lanceur). *L'utilisateur voit* : rien.

**B — le daemon impose via cgroup.** *Coût réel ici* (2b-5a/2b-5b) : lancé depuis un shell, le daemon partage le cgroup de son lanceur (10–12 processus : terminal, shell, python3, lui-même) ; y écrire `memory.max` plafonne le **terminal de l'utilisateur**
(ce qui a produit 322121728, sans sous-cgroup `polygoned*`). Le CPU **n'est pas délégué** sous `app.slice` (`subtree_control = [memory pids]`, `cpu.max` **absent**), où le daemon vit pourtant : il faudrait un drop-in `Delegate=` (ou déplacer l'unité), un sous-cgroup dédié, et un actionneur
**survivant au redémarrage** ; `install_service()` (`resources/linux.rs:320`) n'est jamais appelé et n'écrit ni `MemoryMax=` ni `CPUQuota=`.
*Gagné* : la limite tient même si le client ment. *Perdu* : la souveraineté du processus, ~2 changements systemd hors dépôt, non testables par les 71 tests actuels. *Échoue en silence* : limite orpheline après un crash. *L'utilisateur voit* : sa machine plafonnée sans explication.

**C — hybride.** Unité systemd pour le **plafond** (`MemoryMax=`, `CPUQuota=`), consommateur pour l'**allocation** sous ce plafond. *Gagné* : une borne dure existe enfin, sans écrire dans le cgroup d'autrui.
*Perdu* : deux endroits connaissent le nombre ; plafond statique (posé à l'installation) donc décision dynamique à moitié appliquée. *Échoue en silence* : unité jamais installée → pas de plafond. *L'utilisateur voit* : rien tant qu'aucun client ne consomme.

**Recommandation (A aujourd'hui, C ensuite, B pas en premier).** A est le seul chemin qui ferme
la boucle sans privilège ni changement hors dépôt, et il rend le contrat explicite : le daemon
accorde, le consommateur refuse. B coûte une vraie délégation `cpu` sur cette machine pour une
garantie qui n'a de valeur que face à un client de mauvaise foi — or il n'y a **aucun client**
aujourd'hui. Donc : (1) finir A et prouver la boucle ; (2) quand un second consommateur
arrivera, poser C comme plafond systemd ; (3) B seulement si un locataire non coopératif existe.

## 5. Cahier des charges — deux incréments (option A)

**Incrément A (2b-6) — côté daemon, ~90–130 lignes, `daemon/src/socket.rs`.**
1. `socket_path()` (`:40-48`) honore `POLYGONE_DAEMON_SOCKET` avant `~/.polygone` : les deux
   côtés visent le même chemin sans changer le défaut du client (~6 lignes).
2. Comptabilité : `Ledger{ outstanding: HashMap<String,(u32,u64)> }` ; `dispatch` (`:374-405`)
   le prend en paramètre, `dispatch_alloc` (`:407-455`) décide contre `marge − outstanding` ;
   `AllocGranted` réserve, `Release` (`:395`) libère (~40 lignes + ~15 de tests).
3. `TickUpdate` : diffusion réelle aux abonnés **ou** retrait de `Reply::TickUpdate` (`:230-231`)
   et de `subscribe`/`unsubscribe` (`:75-82`) — ne pas annoncer un message que rien n'émet (~10
   lignes, ou ~60 pour la diffusion).
*Test qui ferme la boucle* (`socket.rs::server_tests`, nouveau) : deux connexions ; la 1ʳᵉ
`alloc_request` est `alloc_granted`, la 2ᵉ identique est `alloc_denied/insufficient_headroom`
(aujourd'hui **les deux** passent) ; la 1ʳᵉ envoie `release`, la 2ᵉ redemande et obtient.

**Incrément B (2b-7) — côté consommateur, ~120–180 lignes, `crates/petals/`.**
1. `src/daemon.rs` : défaut `~/.polygone/daemon.sock` (même env), `v:1` par requête, noms v1
   (`alloc_request`, `release`…), lire et **jeter** le `hello` (échouer si `protocol_version
   != 1`), émettre un `id` et le vérifier, projeter `alloc_granted` → `ResourceAllocation`
   (cgroup = `None`) (~50 lignes).
2. `src/lib.rs:123-143` : `daemon_enabled` (défaut `true`, `:92`) ne doit plus rendre
   `PetalsEngine::new` **inconstructible** (le `?` de `:142`) — « fail open » : journaliser et
   continuer sans allocation (~15 lignes).
3. Le fond de A : l'engine retient le plafond accordé et **refuse** au-delà
   (`max_concurrent_requests`, `:83`) avec une erreur explicite au lieu de travailler (~30 lignes).
*Test qui ferme la boucle* : le daemon est une **lib** (`daemon/src/lib.rs:9` expose `socket`),
donc `crates/petals` peut l'ajouter en `dev-dependencies` et faire l'aller-retour réel —
`polygoned::socket::encode(&Reply::AllocGranted{…})` lu par le parseur du client, ligne du client
décodée par `polygoned::socket::decode` — plus un faux daemon sur socket temporaire (`tempfile`,
déjà en `dev-dependencies` côté daemon) rejouant un transcript v1, prouvant que le client refuse
de travailler sans accord.

## 6. Risques

- **Le consommateur ignore l'accord** : aucun effet système, rien à détecter ; le daemon publie
  une allocation que personne n'honore (c'est l'option A, assumée).
- **Le socket meurt en plein travail** : le serveur rend la main sur erreur d'E/S (`:985`,
  `:991-992`), mais le client ne voit rien tant qu'il ne relit pas (`read_line` ne renvoie
  `Ok(0)`/`Err` qu'au prochain aller-retour). Si le daemon **redémarre**, `bind_listener` supprime
  l'orphelin (`:561`) et re-bind : l'ancien descripteur pointe un inode mort, appris à l'écriture
  (`EPIPE`), sans message.
- **Deux consommateurs** : le protocole **le permet** (`MAX_CONNECTIONS = 16`, `:489`, un thread
  par connexion) mais il n'y a **qu'un** `DaemonState` ; sans comptabilité (§5-A.2), les deux
  reçoivent `alloc_granted` pour la même marge : **sur-allocation** silencieuse.
- **Sans message visible** : (1) `concurrent_requests`/`model_names` jetés par serde (écart 7) ; (2) `expires_at: None` (`:452`) lu comme « n'expire jamais » ; (3) un daemon en `--dry-run` **sert** le socket (`main.rs:153`) mais n'applique rien — le client reçoit un accord qui n'existera jamais ;
  (4) inactivité > 60 s (`:503`) : fermeture **sans message** (`:991-992`) ; (5) `status`/`doctor` affichent ✅ dès que le fichier existe, même pour un socket mort.

## Découvertes inattendues

1. **Le client Petals compile mais est mort.** `polygone-petals` n'a **aucun `[[bin]]`** ni
   `src/main.rs` : rien ne l'exécute dans le dépôt. Aucun appelant hors du crate :
   `$ grep -rn "polygone_petals|PetalsEngine" .` → une seule ligne, dans
   `archive/2026-07-src/bin/polygone-install.rs:72`.
2. **Le chemin daemon est inconstructible par défaut.** `daemon_enabled: true` (`lib.rs:92`) →
   `request_allocation(...).await?` (`:142`) → `UnixStream::connect("/tmp/polygoned.sock")` →
   socket absent → `PetalsEngine::new()` échoue **après** avoir créé le backend, et personne ne
   crée `/tmp/polygoned.sock`.
3. **`TickUpdate` n'est jamais émis** : `socket.rs:230-231` est sa seule occurrence dans
   `daemon/src`. `docs/PLAN_SOCKET.md:74` annonçait `broadcast(tick)` — jamais écrit.
4. **`Release` est un accusé de réception** (`:395`) : il n'efface aucun état, donc rien à
   libérer. La boucle `alloc → release` n'a jamais été fermée.
5. Périmètre : `docs/PLAN_CONSOMMATEUR.md` est le **seul** fichier créé ; `crates/petals/*`
   (mtime 2026-09-03), `daemon/src/*`, `Cargo.toml` et `docs/PLAN_PRIORITES_2B5.md` (mtimes du
   29/09, antérieurs à ce document) sont intacts — aucun code touché.
