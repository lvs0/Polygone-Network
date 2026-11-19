# Épiques LIVRAISON — INST · SDK · CI

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

## INST — installation, mise à jour, désinstallation

**État constaté, avant d'écrire une ligne.** Le fail-closed est **déjà là et
testé** : `install.sh:150-190` refuse une release sans `SHA256SUMS`, et
`scripts/test-installer.sh:216-224` exerce cinq modes d'échec (`missing`,
`mismatch`, `extra`, `symlink`, `duplicate`). Donc `docs/RECONCILIATION.md` §2 A1
est **périmé** : la reprise du patch distant n'apporte rien, c'est le distant qui
est en retard. **Ne pas rejouer A1.** Manquent réellement : désinstallation,
rollback, paquet, signature, et un PATH non idempotent.

### INST.1 — Rendre l'écriture du PATH idempotente
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `install.sh:432` teste le `PATH` courant, pas le contenu du
  fichier de shell : deux installations ajoutent deux fois la même ligne.
- **Travail** : tester la ligne exacte dans le fichier, écrire entre marqueurs
  `# >>> polygone`.
- **Preuve** : `test-installer.sh` appelle `setup_path` deux fois →
  `grep -c polygone "$HOME/.bashrc"` == 1 ; après retrait == 0.
- **Refus** : ne crée pas un `.bashrc` absent.

### INST.2 — Ajouter `--uninstall` qui retire tout
- **Phase** : P3 · **Dépend de** : INST.1
- **Constat** : quatre destinations écrites (`$INSTALL_DIR`, `$CONFIG_DIR`,
  `$DATA_DIR`, `$SHARE_DIR`) + une ligne de shell ; aucun chemin de retrait.
  Le daemon a `uninstall_service` (`daemon/src/resources/mod.rs:270`),
  jamais appelé par l'installateur.
- **Travail** : `--uninstall` : service arrêté, quatre répertoires retirés,
  ligne de shell supprimée ; `$DATA_DIR` préservé sans `--purge`.
- **Preuve** : dans un `HOME` de fixture, les quatre répertoires absents,
  `$DATA_DIR` **présent** sans `--purge`, absent avec.
- **Refus** : ne touche pas aux identités hors `$DATA_DIR`.

### INST.3 — Prouver l'absence de résidu
- **Phase** : P3 · **Dépend de** : INST.2
- **Constat** : `ROADMAP.md` P3.1 l'exige ; aucun test ne compare l'arbre.
- **Travail** : inventaire de `$HOME` (chemins + tailles) avant installation et
  après désinstallation.
- **Preuve** : `diff <(find "$HOME"|sort)` vide hors cache de build toléré ;
  un seul chemin survivant échoue.
- **Refus** : ne nettoie pas `target/`, qui vit hors fixture.

### INST.4 — Installer sans root, vérifié hors fixture
- **Phase** : P3 · **Dépend de** : INST.2
- **Constat** : cible `$HOME/.local/bin` (`install.sh:17`), donc aucun `sudo`
  requis — mais aucun test ne le démontre : tout tourne sur binaires factices.
- **Travail** : installation complète dans un conteneur `--user 1000` sans `sudo`.
- **Preuve** : transcript : `id -u`=1000, `sudo` absent du `PATH`, installation
  terminée, `polygone --version` répond.
- **Refus** : ne couvre pas `/usr/local`.

### INST.5 — Mise à jour avec rollback
- **Phase** : P3 · **Dépend de** : INST.2
- **Constat** : `POLYGONE_VERSION` existe (`:26`) mais l'installation écrase les
  binaires en place (`:390`) : aucune version précédente, aucun retour arrière.
- **Travail** : binaires dans `versions/<v>/`, lien `polygone` ; `--rollback`
  repointe vers N-1 après vérification.
- **Preuve** : installer v2, v3, `--rollback` → `polygone --version` renvoie v2 ;
  `chmod 000` sur v2 fait échouer le rollback **sans** lien cassé.
- **Refus** : pas de migration de config entre versions.

### INST.6 — Décider signature ou phrase honnête
- **Phase** : P3 · **Dépend de** : INST.5 · `GATE-LÉVY`
- **Constat** : l'empreinte vient de la **même origine HTTPS** (`:172-180`) :
  protège d'une corruption, pas d'une release compromise.
- **Travail** : soit signer `SHA256SUMS` (minisign, clé épinglée), soit écrire
  dans le README que ce n'est pas une release signée. Le silence est interdit.
- **Preuve** : signature → `minisign -Vm` en CI + une archive modifiée d'un octet
  refusée. Sinon → la phrase existe et **aucun** fichier ne dit « signed ».
- **Refus** : ne remplace pas un audit de la chaîne GitHub.

### INST.7 — Publier un `.deb`, et seulement celui-là
- **Phase** : P3 · **Dépend de** : INST.5 · `GATE-LÉVY`
- **Constat** : `release.yml` ne produit que des `.tar.gz`.
- **Travail** : `cargo-deb` pour `linux-x86_64` seul, dépendances déclarées,
  `conffiles` pour ne pas écraser `daemon.toml`.
- **Preuve** : `dpkg -i` puis `dpkg -r` en conteneur Debian ; `dpkg -L` = chemins
  de l'installateur ; un `daemon.toml` édité survit à la réinstallation.
- **Refus** : pas de `.rpm`, `brew`, Windows — non testés, donc non promis.

### INST.8 — Écrire la phrase exacte de l'installateur
- **Phase** : P3 · **Dépend de** : INST.6
- **Constat** : `install.sh` affiche « installé avec succès » sans dire ce qui a
  été vérifié.
- **Travail** : un paragraphe README : origine des binaires, ce que couvre le
  SHA256, ce qu'il ne couvre pas, destinations, procédure de retrait.
- **Preuve** : les quatre affirmations présentes et **cohérentes** avec les
  variables de `install.sh` (relecture croisée).
- **Refus** : ne promet pas de mise à jour automatique.

### INST.9 — Installer sur une VM neuve
- **Phase** : P3 · **Dépend de** : INST.2, INST.7 · `GATE-LÉVY`
- **Constat** : tout tourne sur fixtures ; aucune VM n'a été utilisée.
- **Travail** : Debian minimale, réseau réel, release réelle (donc licence
  tranchée).
- **Preuve** : transcript horodaté `install` → `--version` → `polygoned` démarre
  → `uninstall` → `polygone` introuvable.
- **Refus** : une distribution, une architecture. Ne pas généraliser.

**Ce qui peut faire échouer INST.** (1) Le `PATH` déjà correct pendant le test
fait sauter `setup_path` : tester avec un `PATH` qui ne contient pas
`$INSTALL_DIR`. (2) Un `rm -rf` sur `$DATA_DIR` détruit l'identité ML-KEM,
irrécupérable : `--purge` obligatoire et testé. (3) Un rollback vers un binaire
corrompu laisse un lien mort : vérifier `--version` avant de repointer.
(4) Un paquet sans `conffiles` écrase la config : test d'édition manuelle.
(5) Publier sans licence tranchée : `GATE-LÉVY` bloque, et c'est voulu.

---

## SDK — surface d'intégration

### SDK.1 — Geler le contrat `/api/v1`
- **Phase** : P4 · **Dépend de** : P2
- **Constat** : six routes testées en HTTP réel (`crates/gateway/src/routes.rs:87-110`,
  `tests/serve.rs`), mais aucune description hors du code ; `latency_ms_median: 42`
  est une constante (`crates/gateway/src/lib.rs:107`).
- **Travail** : schéma machine-readable par route (entrée, sortie, erreurs) ;
  retirer la latence constante ou la publier `null`.
- **Preuve** : test comparant schéma et routeur ; une route non déclarée fait
  échouer la CI ; `latency_ms_median` vaut `null` sans mesure.
- **Refus** : ne fige pas le format binaire du relais.

### SDK.2 — Corriger les appels front orphelins
- **Phase** : P4 · **Dépend de** : SDK.1
- **Constat** : `web/plan.html:277` appelle `/api/plan`, `:298` `/api/events`,
  `web/drive.js:10` une base `/api` : aucune n'existe, `routes.rs:106` répond
  `route inconnue`. Ces pages échouent en silence.
- **Travail** : implémenter les routes **ou** retirer les pages du site servi.
- **Preuve** : `scripts/gateway-smoke.sh` étendu : chaque URL citée par `web/`
  répond autre chose que 404, extraite automatiquement.
- **Refus** : ne juge pas l'utilité produit des pages (`GATE-LÉVY`).

### SDK.3 — Authentifier l'API locale
- **Phase** : P4 · **Dépend de** : SDK.1
- **Constat** : loopback sans authentification (`crates/gateway/src/http.rs`) :
  tout processus local peut appairer un appareil (`ETAT-REEL.md` §3 #12).
- **Travail** : jeton `0600` dans `$DATA_DIR`, exigé sur les `POST`, facultatif
  en lecture.
- **Preuve** : `POST` sans jeton → 401 ; avec → 200 ; jeton d'un autre UID → 403 ;
  `GET /api/v1/services` sans jeton → 200.
- **Refus** : ne protège pas d'un processus du même UID.

### SDK.4 — Client Rust `polygone-sdk`
- **Phase** : P4 · **Dépend de** : SDK.1, SDK.3
- **Constat** : aucun crate client ; `tests/serve.rs` construit du HTTP à la main,
  ce qui prouve le serveur, pas l'ergonomie.
- **Travail** : crate typé sur le schéma, erreurs classées, `reqwest` hors API
  publique.
- **Preuve** : tests de gateway réécrits avec le SDK (mêmes assertions) ;
  `cargo doc` sans warning ; exemple d'appairage à deux nœuds factices.
- **Refus** : pas de client async **et** sync en parallèle.

### SDK.5 — Client dans un second langage
- **Phase** : P4 · **Dépend de** : SDK.4 · `GATE-LÉVY`
- **Constat** : aucun. Choix non tranché : Python (données/journalisme) ou
  JavaScript (web) — à décider selon l'usage réel, pas la préférence.
- **Travail** : trois appels (statut, appairage, envoi), généré si possible.
- **Preuve** : les exemples de SDK.6 tournent dans les deux langages et produisent
  la même sortie normalisée.
- **Refus** : pas de parité de fonctionnalités promise.

### SDK.6 — Exemples exécutés en CI
- **Phase** : P4 · **Dépend de** : SDK.5
- **Constat** : aucun exemple d'intégration.
- **Travail** : `examples/` par langage, lancés contre une gateway locale.
- **Preuve** : job `examples` vert, sortie citée ; un exemple qui n'appelle plus
  rien de réel échoue.
- **Refus** : ne teste pas le mobile.

### SDK.7 — Pont Matrix, et la vérité sur les autres
- **Phase** : P4 · **Dépend de** : SDK.4
- **Constat** : aucun pont. `STRATEGIE.md` §5 vise Proton ; `VISION.md` cite
  Matrix/Signal/Telegram.
- **Travail** : un seul pont, Matrix (protocole ouvert, serveur auto-hébergeable).
  Documenter pourquoi Signal (pas de serveur tiers) et Telegram (E2E absent)
  trahiraient leur modèle de menace : refus argumenté, pas oubli.
- **Preuve** : message Matrix → `polygone recevoir` local, et retour ; transcript
  des deux côtés.
- **Refus** : aucun pont Proton — pas d'API publique, rien à brancher.

### SDK.8 — Politique de compatibilité
- **Phase** : P4 · **Dépend de** : SDK.7
- **Constat** : `/api/v1` existe, aucune règle de dépréciation.
- **Travail** : `v1` gelé, ajouts compatibles seuls, `v2` avec migration et
  recouvrement.
- **Preuve** : test comparant le schéma au tag précédent ; tout retrait de champ
  échoue.
- **Refus** : ne garantit pas la compatibilité du protocole relais.

**Ce qui peut faire échouer SDK.** (1) Un schéma qui décrit l'intention et pas le
code : le comparer au routeur à chaque CI. (2) Un second langage qui pourrit :
seuls les exemples CI font foi. (3) Un pont qui déchiffre côté serveur annule la
promesse : document de menace par pont, refus écrit quand l'E2E est impossible.
(4) `GATE-LÉVY` sur le langage et les cibles bloque SDK.5 et SDK.7.

---

## CI — construction et publication vérifiables

### CI.1 — Rendre la matrice honnête sur Windows
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `ci.yml:14` met `windows-latest` dans la matrice bloquante alors
  que `daemon/src/resources/mod.rs:291` contient un `compile_error!` : le build
  Windows ne **peut pas** réussir. Le job est rouge en permanence ou n'a jamais
  tourné. `ETAT-REEL.md` §6 dit « non supporté ».
- **Travail** : retirer Windows du job bloquant ; ajouter `windows-canary` en
  `continue-on-error: true` qui documente le portage.
- **Preuve** : run CI : `test` vert sans Windows ; `windows-canary` présent,
  autorisé à échouer, avec la raison du `compile_error!` dans le log.
- **Refus** : ne supprime pas le `compile_error!` — c'est la seule chose honnête
  du portage.

### CI.2 — Compiler avec `--locked`
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `--locked` seulement dans `docs/deployment/Dockerfile:18` ;
  `ci.yml` et `release.yml` ne l'utilisent pas → une release peut embarquer un
  `Cargo.lock` mis à jour silencieusement.
- **Travail** : `--locked` sur toutes les commandes cargo des workflows.
- **Preuve** : `grep -c 'cargo .*--locked'` couvre chaque invocation ; un
  `Cargo.lock` modifié sans commit fait échouer la CI.
- **Refus** : ne rend pas le build reproductible (voir CI.7).

### CI.3 — Auditer les dépendances
- **Phase** : P3 · **Dépend de** : CI.2
- **Constat** : `LEGAL.md:17` affirme « auditable en une commande (`cargo
  audit`, `cargo clippy`, `cargo test`) » ; aucune CI ne lance `cargo audit`.
  L'affirmation est fausse aujourd'hui.
- **Travail** : job `audit` : `cargo audit` + `cargo deny` (licences, avis,
  doublons), base d'avis épinglée.
- **Preuve** : job vert ; une dépendance vulnérable ajoutée en branche de test
  fait échouer le job.
- **Refus** : ne remplace pas un audit de code.

### CI.4 — Mettre en cache la compilation
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `release.yml:56` utilise `Swatinem/rust-cache` ; `ci.yml` non :
  chaque run recompile tout l'arbre.
- **Travail** : même cache dans `ci.yml`, clé incluant `Cargo.lock`.
- **Preuve** : deux runs consécutifs, les deux durées citées ; le second
  nettement plus court.
- **Refus** : cache purgé chaque semaine, sinon il masque un build cassé.

### CI.5 — Mesurer et publier la durée de CI
- **Phase** : P3 · **Dépend de** : CI.4
- **Constat** : aucune mesure (`ETAT-REEL.md` §4 #4).
- **Travail** : tableau daté dans `docs/CI.md` avec commit et durées.
- **Preuve** : trois runs, chiffres réels — pas « rapide ».
- **Refus** : pas de comparaison inter-projets.

### CI.6 — Vérifier les artefacts après publication
- **Phase** : P3 · **Dépend de** : INST.6
- **Constat** : `release.yml:95` génère `SHA256SUMS` et **aucun job ne le
  vérifie** après dépôt : une archive corrompue à l'upload passerait.
- **Travail** : job final qui télécharge chaque asset publié et recalcule.
- **Preuve** : `verify-release` vert sur un tag réel ; archive altérée à la main
  → job rouge.
- **Refus** : ne détecte pas une release malveillante cohérente avec elle-même.

### CI.7 — Build reproductible, mesuré
- **Phase** : P5 · **Dépend de** : CI.2
- **Constat** : pas de `SOURCE_DATE_EPOCH`, pas de `--remap-path-prefix` : deux
  builds du même commit diffèrent.
- **Travail** : figer chemins et horodatage, comparer deux builds indépendants.
- **Preuve** : même SHA-256 sur deux répertoires ; à défaut, écrire le chemin
  résiduel de non-reproductibilité et le hash obtenu.
- **Refus** : ne promet pas la reproductibilité avant de l'avoir mesurée.

### CI.8 — SBOM attaché à chaque release
- **Phase** : P5 · **Dépend de** : CI.3
- **Constat** : aucun SBOM.
- **Travail** : `cargo-cyclonedx` en CI, joint à la release.
- **Preuve** : `jq '.components|length' sbom.json` > 0 et fichier présent dans
  les assets ; le job échoue si les quatre binaires n'y figurent pas.
- **Refus** : ne prouve pas l'absence de composant malveillant.

### CI.9 — Tests d'intégration à deux processus
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `ETAT-REEL.md` §7 : « deux processus locaux » prouvés, mais
  jamais en CI.
- **Travail** : job lançant `polygone-relay` en tâche de fond, attente du port,
  message transitant, arrêt propre.
- **Preuve** : job vert, port réellement ouvert (`ss -ltn`), message comparé,
  code de sortie 0.
- **Refus** : ce n'est pas deux machines, et ne le prétend pas.

### CI.10 — Relancer l'archive P0.1
- **Phase** : P3 · **Dépend de** : aucune
- **Constat** : `scripts/p0-1-archive.sh` a prouvé la restauration **une fois**,
  à la main. La preuve vieillit.
- **Travail** : job hebdomadaire : archive + extraction + `cmp`.
- **Preuve** : artefact conservé 90 jours, `cmp` vert dans le log ; un fichier
  supprimé du dépôt fait échouer la restauration.
- **Refus** : ne restaure pas une machine, seulement le dépôt.

**GATE-LÉVY — ce qui bloque la publication.** (1) **Licence** : `LEGAL.md` §7 dit
AGPL-3.0 (2026-08-07), `VISION_CANONIQUE.md` §4 dit MIT — sans arbitrage, aucune
release (INST.9, CI.6) ni paquet (INST.7). C'est le blocage le plus lourd.
(2) **Nom du paquet** : `polygone` est courant et entre en collision ; décider un
préfixe avant INST.7. (3) **Cible de distribution** : `release.yml` publie déjà
quatre cibles dont deux macOS — promesse de fait, pas décision écrite.

**Ce qui peut faire échouer CI.** (1) Une CI verte qui ne teste rien — le piège
du dépôt distant, où un test s'annote absent et passe : compter et publier le
nombre de tests exécutés. (2) Une cible dans la matrice sans runner qui l'a déjà
vue verte : Windows est exactement ce cas. (3) Un cache qui ment : `--locked` +
purge + rebuild à froid sur les tags. (4) La licence non tranchée bloque tout
`GATE-LÉVY` — ce n'est pas une tâche technique.