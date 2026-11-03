# RECONCILIATION — les 6 commits locaux et les 17 distants

> Rédigé le 2026-11-03. **Aucun merge, aucun reset, aucun push sur `main`.**
> Ce document décide *quoi* reprendre ; il ne modifie pas l'historique.
>
> Base commune : `060c11d`. Local : `f3d09c0` (6 commits d'avance). Distant :
> `23d27ac` (17 commits d'avance). Vérifications faites avec `git show`,
> `git grep` et lecture du diff — pas depuis les messages de commit.

---

## 0. Le fait qui change tout

Les 17 commits distants ne sont **pas** un complément du local : c'est une
**ligne de développement antérieure et parallèle**, alimentée depuis un autre
dossier (`PROJECT_DIR="${PROJECT_DIR:-~/Projets/Polygone-v2}"` dans
`scripts/company-loop.sh`, absent du local).

Preuve : `874f87f` **crée** `crates/gateway/` et `crates/petals/` de zéro
(6 681 insertions) le 2026-09-06 ; le local les a rebâtis plus tard, plus gros,
dans `f3d09c0` le 2026-10-29. Sur ces deux crates, le distant est **en amont**,
pas en aval.

Conséquence opérationnelle : **un `git pull` écraserait du travail local plus
récent**. Il n'y a pas de « mise à jour à récupérer ». Il y a **trois choses à
sauver** et **quatorze à refuser**.

---

## 1. Les 6 commits locaux — tous conservés

Aucun n'est à jeter. Deux portent 98 % de la valeur.

| hash | date | fichiers | +/− | ce qu'il apporte | décision |
|---|---|---:|---|---|---|
| `f3d09c0` | 2026-10-29 | 46 | +13 844 / −71 | `blocks.rs` (BLAKE3), `calc.rs` (cache de calcul), `node.rs` (bail daemon), gateway, 3 backends petals réels, daemon cli/client/runtime, 12 docs | **garder** |
| `48ef1d3` | 2026-10-13 | 1 | +2 949 / −108 | protocole socket v1 complet : `Request`/`Reply`, `DaemonState`, `DecodeError`, `Ledger` persistant | **garder** |
| `25f6cb3` | 2026-10-13 | 8 | +123 / −96 | `UserActivity` trois états, suppression de 7 clés de config mortes | **garder** |
| `6aa7f0d` | 2026-10-13 | 3 | +195 / −4 | `crates/client/src/lib.rs`, ledger tolérant, `PLAN_PRIORITES_2B5.md` | **garder** |
| `a9be256` | 2026-10-13 | 1 | +8 / −2 | débranche `mod drive` / `mod brain` (morts), fichiers conservés sur disque | **garder** |
| `3a3b775` | 2026-10-13 | 3 | +3 / −6 | `cargo fmt` | **garder** |

### Deux dettes d'hygiène, à ne pas régler par réécriture

1. `f3d09c0` embarque deux binaires : `phase0/__pycache__/modal_node.cpython-312.pyc`
   et `phase0/preuves/block-1MiB.bin` (1 Mio). Ils n'ont rien à faire dans
   l'historique, mais **on ne réécrit pas l'historique pour ça** : ils seront
   supprimés et `.gitignore`-és dans un commit d'hygiène ultérieur.
2. `48ef1d3` ne fait pas ce que son sujet annonce. « persist ledger allocations »
   décrit 3 % du diff ; les 97 % restants réécrivent le protocole socket et
   suppriment `DaemonMsg`, `send`, `notify_allocation`, `notify_shrink`.
   Le message est faux, le code est bon. À corriger dans le message d'un futur
   commit de réconciliation, pas par `rebase`.

### Ruptures d'API à assumer

`25f6cb3` supprime `CpuAffinityMode` et `Platform::set_memory_limit`, et change
`user_active: bool` en `enum UserActivity`. `48ef1d3` supprime `DaemonMsg`.
Ce ne sont pas des conflits de texte : c'est un **contrat** que le distant utilise
encore. Tout code distant qui s'y accroche devra être re-porté à la main.

---

## 2. Les 17 commits distants — ce qui est repris

### A1 · Installateur fail-closed — **À REPRENDRE** (`a796e83`, `8bb8654`)

`install.sh` passe de « vérification optionnelle » à **fail-closed** : si
`SHA256SUMS` est introuvable, l'installation est **refusée** ; l'empreinte est
comparée via `sha256sum` ou `shasum` ; chaque branche d'échec nettoie ses
fichiers temporaires. `release.yml` publie `SHA256SUMS` **et** un `.sha256` par
asset.

**Ce que ça garantit, exactement** : l'intégrité du téléchargement contre une
troncature, une corruption ou un miroir altéré.
**Ce que ça ne garantit pas** : une release compromise — l'empreinte vient de
la **même origine HTTPS**, sans signature hors-bande ni clé épinglée.

→ Reprendre le code, mais **ne jamais** l'appeler « release signée » dans le
README ni sur le site. C'est la règle `ETAT-REEL.md` appliquée à l'installateur.

### A2 · Backend mock de petals — **À REPRENDRE** (`57bb551`, `39a285d`)

`crates/petals/src/backends/mock.rs` (243 lignes) : `list_models`, `generate`,
`chat`, `stream`, `pull_model`, `health_check`, `info`, avec latence simulée,
échec pilotable (`MockBackend::failing`) et un flux qui cède le tour. C'est de
l'**infrastructure de test** — elle ne prouve pas l'inférence, elle permet de
tester le routage sans serveur externe.

**À reprendre : le backend. Pas les tests.** Trois défauts vérifiés dans le diff :

- `test_ollama_mock_stream` contient `assert!(true);` — il n'affirme rien.
- `test_backend_failover` n'exerce **aucun** failover : il appelle le mock en
  échec, constate l'erreur, puis appelle manuellement le mock qui marche.
- L'en-tête du fichier annonce « criterion benches (tokens/s, latency p50/p99,
  memory) » : **aucun de ces tests n'existe** dans le fichier.

De plus `MockBackend::backend_type()` renvoie `BackendType::Ollama` : un mock qui
se déclare Ollama peut tromper un sélecteur de backend. À renommer en
`BackendType::Mock` avant reprise.

### A3 · Gating Unix — **À REPORTER À LA MAIN** (`5126c19`)

Bon travail : `cfg(unix)` sur `tokio::net::UnixStream`, stub explicite hors Unix
pour `ensure_connected`/`send_request`. Mais il s'applique à l'**ancien**
`socket.rs` (`send`, `DaemonMsg`), que `48ef1d3` a supprimé. Le diff ne
s'appliquera pas : le contexte est introuvable.

→ Reporter le **principe** (`cfg` sur les API Unix, erreur explicite ailleurs)
dans le nouveau protocole. Ne pas tenter `git cherry-pick`.

Note : `read_counters_on_lo` y est assoupli — sur `ErrorKind::NotFound` il
affiche et **passe** au lieu d'échouer. Justifié hors Linux, mais c'est un test
qui ne teste plus. À documenter comme tel.

---

## 3. Ce qui est refusé, et pourquoi

| Commits | Verdict | Raison vérifiée |
|---|---|---|
| `46d19bc` « honest WindowsPlatform » | **refuser** | Supprime le `compile_error!` et renvoie une RAM **codée en dur** (8 Gio / 4 Gio) sous un commentaire affirmant « via sysinfo » — aucun appel `sysinfo` n'existe dans le fichier (`windows.rs:85-91`). `user_active → Ok(false)`, `uptime → Ok(0)`, `primary_interface → "Ethernet"`. Le module **mesure des ressources** : publier 4 Gio libres constants ferait accorder des allocations fantômes. Le refus de compiler était **plus honnête**. |
| `23d27ac` (tête distante) | **refuser** | Remplace `scripts/smoke-commands.sh` (245 lignes, 20 assertions réelles : `duress` 4→0 fichiers, round-trip `envoyer --stdin \| recevoir -`, `carte`, `verite`, démo E2E) par `cd ~/Projets/Polygone-v2` + `cargo run -- --help \| head`. Le `~` est entre guillemets donc **littéral** : le script ne trouve même pas le dossier. `--help` ne prouve rien du produit. |
| `57bb551` partie `serverless` | **refuser** | Le correctif Shamir **est déjà en place, en mieux** : `crates/serverless/src/request.rs:119` appelle `shamir::split`, et `reassemble` est testé en négatif (3 parts → erreur, parts mélangées → erreur). Redondant et inférieur. |
| `57bb551` partie `drive` | **refuser** | `crates/client/src/drive.rs` n'est **déclaré nulle part** — vérifié : aucune occurrence de `mod drive` dans `main.rs` ni `lib.rs`. Et `DriveFile::receive()` échoue volontairement au-delà de 64 Kio (`multi-chunk reassembly not yet implemented`). Code mort mono-fragment. |
| `874f87f` scaffold petals/gateway | **refuser** | Remplacé par `f3d09c0`, postérieur et plus complet. |
| `380c35b` fmt petals | **refuser** | 14 fichiers reformatés (417/206) sur le périmètre le plus disputé : bruit maximal, conflit garanti. `cargo fmt` sera rejoué une fois, à la fin. |
| `dd1c83b` « unbreak petals bench » | **refuser** | « Débloquer » signifie ici **supprimer `criterion`** au lieu de réparer le bench. |
| `e9045cc`, `178c7e6`, `a0b03e5`, `84c8e12` | **refuser** | Cosmétique (match au lieu de `let`, `Default` sur `MacOSPlatform`, fmt) ou sans objet (`84c8e12` corrige un import dans un fichier qui est un placeholder de 11 lignes). `cargo clippy` les redécouvrira si nécessaire. |
| `dff84d6`, `6e78b1a`, `f5f7831` | **discuter** | Documentation seule : `COMPANY.md`, `GOVERNANCE.md` (13 rôles), `DOSSIER-POLYGONE-DREAM.md`, réécriture de `SPEC.md` (405 lignes), polissage `index.html`. Aucune preuve d'exécution. Le dépôt compte déjà 1 511 `.md` : ajouter de la prose non vérifiée est exactement le défaut dénoncé par `AUDIT_VERACITE_VISION.md`. Reprendre seulement `SPEC.md` **s'il** est confronté au code. |

---

## 4. Une correction que je dois signaler

L'analyse initiale affirmait que `23d27ac` supprimait `guard_disk`, une garde
qui mettait la boucle en pause sous 1 Go libre (incident du 2026-09-10).
**C'est faux, et je l'ai vérifié** : `guard_disk` n'existe ni dans l'arbre local,
ni dans `origin/main`, ni dans la base commune (`git grep` sur `*.sh` = 0
occurrence). Et `scripts/company-loop.sh` est **absent du local** : c'est un
ajout distant, pas une modification.

Ce qui reste vrai, et suffit à refuser le commit : le remplacement de la suite
de smoke tests par des `--help`, et le `~` littéral. La raison fausse est
retirée ; la décision ne change pas, mais elle ne s'appuie plus que sur du
vérifié.

---

## 5. Procédure de réconciliation (à exécuter en P1, pas maintenant)

1. **Archive d'abord** — `P0.1`, déjà faite : `Backups/polygone-real-*.tar.gz`
   + `sha256` + rapport de restauration. Rien ne se fait avant.
2. **Commit des 65 fichiers non suivis** sur `realign/v1` (travail local réel,
   actuellement non protégé). Ne pas mélanger avec autre chose.
3. **A1** : appliquer l'installateur fail-closed à la main, puis vérifier qu'une
   installation sans `SHA256SUMS` **échoue** (test négatif obligatoire).
4. **A2** : porter `MockBackend` avec `BackendType::Mock`, écrire **un** test de
   routage qui échoue si le sélecteur prend le mock pour Ollama.
5. **A3** : ajouter les `cfg(unix)` au nouveau protocole, avec un test qui
   vérifie que la compilation hors Unix échoue proprement plutôt que de mentir.
6. **Ne pas toucher `main`.** Tout va sur `realign/v1`. Le distant reste tel
   quel : c'est un état historique, pas une branche à réparer.
7. Mettre à jour ce document avec les hashes des commits produits.

---

## 6. Ce que ce document ne fait pas

- Il ne fusionne rien, ne réécrit rien, ne pousse rien sur `main`.
- Il ne tranche pas la licence (AGPL vs MIT, cf. `NEEDS-LEVY.md`).
- Il ne décide pas du sort de `drive` / `brain` : `a9be256` les débranche à
  raison (zéro appelant), mais la vision les veut. **Décision produit, pas
  technique** — à arbitrer avec `VISION.md`, pas ici.
