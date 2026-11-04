# 06 — Confiance, sécurité, observabilité, vérité documentaire

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

## Épique SEC — sécurité prouvée

### SEC.1 — Écrire le modèle de menace de chaque service
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : `THREAT_MODEL.md` couvre le réseau entier ; `docs/threat-commodity.md` et `docs/threat-high-value.md` couvrent deux personae. Aucun document ne dit, service par service, ce qui est protégé : `msg`, `drive`, `hide` et `daemon` n'ont pas de tableau « protège / ne protège pas / preuve ».
- **Travail** : quatre fiches, chacune avec une ligne par propriété et une référence de preuve obligatoire.
- **Preuve** : `docs/threat-msg.md`, `threat-drive.md`, `threat-hide.md`, `threat-daemon.md` ; la CI échoue si une ligne « protège » n'a pas de référence (voir DOC.1).
- **Refus** : une propriété non prouvée va dans « ne protège pas ». Ce n'est pas un document commercial.
- **GATE-LÉVY** : publier ces fiches expose des faiblesses réelles (métadonnées du relais, mono-saut de Hide). Public ou interne ?

### SEC.2 — Fuzzer le décodeur NDJSON du socket
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : `daemon/src/socket.rs:445` documente l'exigence de symétrie entre les deux directions ; `decode`/`decode_reply` valident type, version et `MAX_LINE_BYTES`. Aucune occurrence de `cargo-fuzz`, `libfuzzer`, `proptest` ou `arbitrary` dans le dépôt : la robustesse est affirmée, pas éprouvée.
- **Travail** : cible `cargo-fuzz` sur les deux décodeurs, corpus initial tiré des tests, exécution 10 min en CI nocturne.
- **Preuve** : `cargo fuzz run decode_ndjson -- -max_total_time=600` sans panic ; corpus commité sous `fuzz/corpus/`, graine citée dans le rapport.
- **Refus** : 30 secondes de fuzzing ne prouvent rien. La durée est dans la preuve, pas dans une intention.

### SEC.3 — Prouver l'anti-rejeu à travers le relais
- **Phase** : P2 · **Dépend de** : SEC.2
- **Constat** : la protection existe en unitaire — `MAX_TS_SKEW_SECS = 300` (`crates/client/src/net/mod.rs:187`), `replayed_session_is_rejected` (`net/tests.rs:967`), cache fail-closed (`:1088`), horodatage de 10 h refusé (`:1053`). Aucun test ne rejoue une trame **capturée sur le fil** via le relais.
- **Travail** : scénario d'intégration : capture d'une trame valide, rejeu immédiat, puis rejeu décalé de 10 minutes.
- **Preuve** : `cargo test -p polygone-client --test replay_through_relay` — premier envoi accepté, rejeu rejeté, rejeu décalé rejeté.
- **Refus** : ne couvre pas un adversaire qui contrôle l'horloge des deux pairs ; à écrire dans `threat-msg.md`.

### SEC.4 — Figer l'usurpation de `from` en test de régression
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : `ETAT-REEL.md` §2 (ligne 2) déclare le relais protégé contre l'usurpation de `from`, preuve `crates/relay/src/relay.rs:165-230`. C'est un test unitaire du relais, pas un client malveillant réel.
- **Travail** : client de test qui se déclare `bob` en prétendant envoyer pour `alice`.
- **Preuve** : `cargo test -p polygone-relay --test spoofing` — rejet explicite, zéro fragment livré.
- **Refus** : ne couvre pas l'usurpation au niveau du transport chiffré (déjà tenue par ML-DSA-65, testée en unitaire).

### SEC.5 — Rendre les permissions de fichiers fail-closed
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : le socket impose `0600` et le vérifie (`daemon/src/socket.rs:1051`, test `:2738`). Mais l'identité et la socket réseau ignorent l'échec : `let _ = std::fs::set_permissions(...)` (`crates/client/src/identity.rs:80-81`, `crates/client/src/net/mod.rs:59-60`). `reputation.rs:127` fait l'inverse et propage l'erreur ; `:262-263` teste déjà `mode & 0o777 == 0o600`.
- **Travail** : propager l'erreur ; si le fichier contient une clé privée, refuser de continuer.
- **Preuve** : test qui force un `chmod` impossible (répertoire en lecture seule) et attend une erreur, pas un silence.
- **Refus** : durcissement local, pas une garantie globale — un `umask` permissif ailleurs reste possible.

### SEC.6 — Trancher la politique de logs du relais
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : le relais journalise l'adresse source à chaque événement — `relay.rs:250`, `:277`, `:288`, `:303`, `:311`, `:317`, `:323`, `:357`, `:381`, `:423` — alors que `crates/relay/src/main.rs:38` s'annonce « stateless, blind forwarder ». `README.md` affirme « pas de métadonnées » ; `LEGAL.md` §4.1 promet des logs IP « agrégés ». Le seul audit existant (`scripts/hide-smoke.sh:78-81`) vérifie l'absence de **contenu** en clair, pas de métadonnées.
- **Travail** : deux modes explicites — `--log-metadata=off` (compteurs seulement) et `=on` (IP, avec avertissement au démarrage) — défaut aligné sur `LEGAL.md`.
- **Preuve** : `bash scripts/relay-log-audit.sh` : en `off`, aucune occurrence d'adresse IP dans le journal (regex sur le fichier entier) ; en `on`, l'avertissement apparaît.
- **Refus** : ne rend pas le relais anonyme — il voit toujours qui parle à qui pendant la session.
- **GATE-LÉVY** : « ne garde rien » (débogage plus dur) ou « garde les IP » (assumé et écrit). Le silence n'est pas défendable.

### SEC.7 — Auditer les dépendances en CI
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `.github/workflows/ci.yml:25-32` exécute `fmt`, `clippy`, `test`, `build`. Aucune occurrence de `cargo audit`, `cargo deny`, `syft` ou `cyclonedx` : les avis de sécurité des dépendances ne sont jamais lus.
- **Travail** : `cargo deny check advisories licenses bans sources` en job séparé, avec `deny.toml` autorisant AGPL-3.0.
- **Preuve** : job vert sur l'arbre, et **rouge** sur une version épinglée artificiellement vulnérable — la porte est testée dans les deux sens.
- **Refus** : un vert ponctuel ne dit rien de demain ; la valeur est la répétition.

### SEC.8 — Empêcher un secret d'entrer dans l'arbre
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `.gitignore` exclut les identités, mais rien ne bloque mécaniquement un `git add` d'un fichier de clé ; l'audit de secrets existant est manuel.
- **Travail** : `gitleaks` en pre-commit et CI, plus un test qui échoue si un chemin suivi correspond à `identity.json`, `*.key` ou `.env`.
- **Preuve** : `gitleaks detect --no-banner` vert sur l'historique complet + `cargo test --test no_tracked_secrets`.
- **Refus** : ne nettoie pas l'historique — un secret déjà commité exige une rotation de clé, pas une réécriture.

### SEC.9 — Éprouver le socket sous charge et sous entrée hostile
- **Phase** : P2 · **Dépend de** : SEC.2
- **Constat** : `MAX_LINE_BYTES` et les codes `ERR_*` existent (`daemon/src/socket.rs:130-146`) ; la ligne trop longue est testée. Aucun test ne mélange connexions concurrentes, client qui disparaît au milieu d'une ligne, et réservation en cours.
- **Travail** : trois clients — un ferme brutalement au milieu d'un `alloc_request`, un envoie 10 Mo de bruit, un demande normalement.
- **Preuve** : `cargo test -p polygoned --test socket_abuse` : le client sain obtient son allocation, aucune réservation fantôme après `release`, daemon toujours joignable.
- **Refus** : test de cohérence du ledger sous interruption, pas un test de déni de service à l'échelle.

### SEC.10 — Mesurer ce que le relais voit réellement
- **Phase** : P2 · **Dépend de** : SEC.6
- **Constat** : « on voit rien » est prouvé pour le contenu (`scripts/hide-smoke.sh:78-81`). Aucun document n'inventorie les métadonnées visibles : pair source, destination, taille, cadence, horodatage.
- **Travail** : script qui capture un flux réel pendant un envoi et produit le tableau de ce qui est lisible dans la trame et dans le journal.
- **Preuve** : `bash scripts/relay-metadata-audit.sh` produit un rapport daté, cité par `docs/threat-msg.md`.
- **Refus** : ne quantifie pas une attaque par corrélation de cadence — listée hors périmètre.

**Ce qui peut faire échouer cet épique** : (1) fuzzer sans budget de temps donne une fausse assurance — durée et graine citées, exécution nocturne ; (2) le modèle de menace devient un exercice de style sans test adossé — la CI refuse une preuve manquante ; (3) SEC.6 contredit la communication actuelle et peut être repoussée indéfiniment — c'est un `GATE-LÉVY`, pas un choix technique ; (4) SEC.7 produit du bruit tant que la licence n'est pas tranchée (`NEEDS-LEVY.md`) — le faire après la décision.

---

## Épique OPS — observabilité sans télémétrie

### OPS.1 — Exposer `Capacity` dans `polygoned doctor`
- **Phase** : P1 · **Dépend de** : P1.1 (fait)
- **Constat** : `doctor` affiche les capacités de plateforme, l'activité utilisateur, CPU, mémoire et interfaces (`daemon/src/cli.rs:432-494`, appelé par `main.rs:308-309`, testé par `scripts/smoke-commands.sh:181-184`). Il n'affiche **pas** les trois grandeurs de P1.1 (`measured` / `reservable` / `enforced`).
- **Travail** : section « Capacité » avec les trois valeurs côte à côte et la mention « licence » quand `enforced == 0`.
- **Preuve** : `polygoned doctor | grep -A4 Capacité` montre trois lignes ; test unitaire sur `doctor_report`.
- **Refus** : `doctor` ne joint ni le daemon ni le réseau — il lit l'état local (voir OPS.6).

### OPS.2 — Compter les refus d'allocation par motif
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : les motifs existent (`daemon/src/socket.rs:130-146`) et partent sur le fil dans `AllocDenied.reason`, mais **aucun compteur** n'est tenu — aucune occurrence d'agrégat dans `socket.rs`. On ne peut pas répondre à « pourquoi mes nœuds sont refusés ? ».
- **Travail** : `HashMap<&'static str, u64>` dans le serveur — jamais dans `DaemonState`, même contrainte d'écrivain unique que le `Ledger` — exposé dans `status` et `doctor`.
- **Preuve** : test qui provoque chaque motif puis vérifie la répartition ; `polygoned status` l'affiche.
- **Refus** : métriques locales, non exportées, perdues à l'arrêt — cohérent avec l'absence de télémétrie.

### OPS.3 — Journal d'événements local et borné
- **Phase** : P2 · **Dépend de** : OPS.2
- **Constat** : daemon et relais écrivent sur stderr/`log::` sans rétention (`relay.rs:250-423`) ; rien ne reconstitue « la dernière heure » sans garder tous les logs — et les garder tous est précisément ce que le produit promet de ne pas faire.
- **Travail** : tampon circulaire en mémoire typé (tick, alloc, refus, bail perdu, erreur), exposé par `polygone journal --lignes 50`.
- **Preuve** : après scénario provoqué, `polygone journal` affiche les événements dans l'ordre ; test de taille maximale du tampon.
- **Refus** : non persistant par défaut. `--fichier` est explicite : un journal disque devient une preuve contre l'utilisateur.

### OPS.4 — Exposer les compteurs du nœud en JSON
- **Phase** : P3 · **Dépend de** : OPS.2
- **Constat** : `node.rs:463-506` tient `served_blocks`, `served_bytes`, `connections_accepted`, `connections_refused`, affichés en texte (`:588-589`). Aucune sortie machine ni code de retour distinct selon l'état.
- **Travail** : `polygone node status --json` au schéma versionné, codes de sortie documentés (0 sain, 2 bail perdu, 3 daemon absent).
- **Preuve** : `polygone node status --json | jq -e '.schema_version and .served_blocks and .lease.state'` ; test du code de sortie daemon arrêté.
- **Refus** : un état inconnu s'écrit `"unknown"`, jamais `null` ambigu.

### OPS.5 — Rapport de diagnostic copiable, sans secret, sans réseau
- **Phase** : P3 · **Dépend de** : OPS.1, OPS.4
- **Constat** : `doctor` imprime pour l'écran. Rien ne produit un artefact collable dans un rapport de bug sans fuiter clé, `NodeId` complet ou chemin personnel. Aucun test ne vérifie non plus qu'une commande de diagnostic n'ouvre pas de socket, alors que `README.md` et `THREAT_MODEL.md:130` promettent « zéro télémétrie ».
- **Travail** : `polygone diagnose` produit un bloc Markdown (versions, OS, capacité, motifs de refus, 20 derniers événements) avec `NodeId` tronqué et chemins en `~` ; test d'intégration qui compte les connexions sortantes.
- **Preuve** : test de rédaction (ni clé, ni `NodeId` complet, ni `/home/`) + `cargo test --test no_telemetry` : zéro connexion sortante pour `doctor`, `diagnose`, `journal`.
- **Refus** : ne collecte rien à distance ; le rapport est copié par l'utilisateur. Ne couvre pas un binaire tiers (Ollama) lancé par une autre commande.

### OPS.6 — Rendre le statut Gateway honnête sur l'état réseau
- **Phase** : P2 · **Dépend de** : P2.5
- **Constat** : `ETAT-REEL.md` §3 ligne 9 : le statut reflète une table en mémoire, sans heartbeat du relais ; `crates/gateway/src/status.rs` publie donc un état qui ne prouve pas la connectivité. `latency_ms` reste `None` : c'est honnête (`status.rs:6`, `routes.rs:199`, test `routes.rs:381`) mais non câblé (§3 ligne 10).
- **Travail** : soit heartbeat réel, soit champ d'état `inconnu` — jamais « connecté » sans preuve.
- **Preuve** : relais coupé → `inconnu` en moins de N secondes ; relais joignable → `connecte` avec latence mesurée ; latence absente → champ omis, jamais `0`.
- **Refus** : ne promet pas de haute disponibilité — trois états suffisent. Une latence locale ne dit rien de celle d'un tiers.

**Ce qui peut faire échouer cet épique** : (1) l'observabilité dérive vers la télémétrie — OPS.5 est un test, pas une intention ; (2) les compteurs créent un second écrivain et cassent l'invariant « `publish_shared` seul écrivain » — ils vivent dans le serveur ; (3) `doctor` grossit en tableau de bord illisible — trois sections, le reste en `--json` ; (4) OPS.6 force une dépendance réseau dans un binaire local — trois états dont `inconnu`, aucune obligation de joindre.

---

## Épique DOC — la vérité documentaire comme livrable

### DOC.1 — Test de cohérence documentaire (livrable central)
- **Phase** : P1 · **Dépend de** : P0.3 (fait)
- **Constat** : `P0.3` a corrigé `ARCHITECTURE.md` une fois (« 4 crates » → 7 membres, 109 → 348 tests). Rien n'empêche la rechute : aucun test ne lit un chiffre de la documentation. `docs/` compte 34 fichiers plus deux dossiers, plusieurs annonçant des chiffres périmés (`CORE-COMPLETE.md`, `RAPPORT-MISSION-2026-08-12.md:16`, `ARCHITECTURE_RESEAU.md:161`).
- **Travail** : `tests/doc_consistency.rs`, deux contrôles. **A — chiffres du workspace** : lire `Cargo.toml` (`members`, `[[bin]]`) et vérifier que le nombre de membres et de binaires annoncé dans `ARCHITECTURE.md` §1 est exact (binaires = `[[bin]]` + `src/main.rs` sans déclaration). **B — comptes de tests** : refuser tout `.md` de `docs/` contenant `\d+\s+tests` sans bandeau `Instantané daté` ni ligne `Status`. Lecture par système de fichiers, pas `git ls-files` (indisponible en test).
- **Preuve** : `cargo test --test doc_consistency` vert, **puis preuve de la porte** : retirer un membre de `Cargo.toml` doit faire échouer le test en nommant fichier et ligne. Un test de cohérence qui ne peut pas échouer ne prouve rien.
- **Refus** : ne vérifie ni la prose, ni les opinions, ni les dates de rédaction — seulement les chiffres contredits par le code.

### DOC.2 — Inventorier les documents avec date et statut — LIVRÉ 2026-11-04
- **Phase** : P1 · **Dépend de** : DOC.1 (**fait**)
- **Constat initial** : `docs/` n'avait pas d'index. Le chiffre de 34 documents était antérieur au portage Atlas ; à l'exécution, `find docs -type f -name '*.md'` a trouvé 61 fichiers, avant la création de l'index.
- **Travail livré** : `docs/INDEX.md` référence les 62 fichiers Markdown récursifs, index compris, avec chemin, titre, date (ou « Non indiquée »), statut (`autorité` / `daté` / `brouillon` / `à supprimer`), thème et justification. Les dates d'import Atlas sont explicitement distinguées des dates de rédaction inconnues.
- **Preuve** : `cargo test -p polygone-client --test doc_index` passe (1/1) ; retrait temporaire de `docs/BUDGET.md` → échec du test qui nomme le fichier ; `cargo test --workspace -j 2` passe (372 tests) ; `node scripts/check-docs.mjs --tests 372` passe.
- **Refus** : l'index ne juge pas la qualité ; il empêche l'oubli. Aucun statut « à supprimer » n'est attribué sans preuve d'un doublon exact.

### DOC.3 — Bandeauter ou retirer les documents faux
- **Phase** : P1 · **Dépend de** : DOC.2
- **Constat** : `CORE-COMPLETE.md` annonce 109 tests et contient la substitution non développée `$(git rev-parse HEAD)` — preuve qu'il n'a jamais été relu. `HERMES-FICHE-REPRISE-2026-08-12.md:27` annonce « 4 crates ». `RAPPORT-MISSION-2026-08-12.md:16` annonce 162 tests. Pire, un document qui ne se présente pas comme historique :  `ARCHITECTURE_RESEAU.md:161` affirme que `crates/gateway/src/lib.rs:107`
  renvoie `latency_ms_median: 42` en dur — le code ne le fait plus (`status.rs:6`,
  `routes.rs:381`). Le même passage relève trois affirmations de
  `VISION-COMPLETE.md` : `docs/site/` qui n'existe pas (`:28`), un background
  sync iOS que la plateforme ne permet pas (`:33`), et un statut réseau avec
  latence (`:39`).
- **Travail** : bandeau `Instantané daté — ne pas lire comme l'état courant` avec lien vers `ETAT-REEL.md` ; suppression réelle seulement pour les doublons exacts.
- **Preuve** : `grep -L "Instantané daté" docs/*.md` ne renvoie aucun fichier classé `daté` dans `INDEX.md` ; DOC.1 couvre le reste.
- **Refus** : on ne réécrit pas l'histoire. Un rapport daté qui se trompait reste sur disque avec son bandeau : c'est la trace du défaut.

### DOC.4 — Refuser un document sans date ni statut
- **Phase** : P1 · **Dépend de** : DOC.2
- **Constat** : rien n'empêche un nouveau `.md` d'arriver sans date — c'est ainsi que 61 fichiers Markdown cohabitaient avant la création de l'index (62 avec `INDEX.md`).
- **Travail** : hook `pre-commit` + étape CI exigeant, pour tout `.md` ajouté sous `docs/`, un en-tête contenant `Date` **et** `Statut`.
- **Preuve** : commit de test avec un `.md` sans en-tête refusé, message nommant le fichier ; même fichier avec en-tête accepté.
- **Refus** : ne s'applique pas aux fichiers d'outillage, listés en exclusion explicite.

### DOC.5 — Faire du README une vue dérivée d'`ETAT-REEL`
- **Phase** : P2 · **Dépend de** : DOC.1
- **Constat** : `README.md` est déjà prudent (« la suite fait foi, elle n'est pas recopiée ici ») et son tableau « Ce que ça ne fait pas » recoupe partiellement `ETAT-REEL.md` §3. Les deux peuvent diverger sans que rien ne le signale.
- **Travail** : le tableau des limites est généré depuis une section balisée d'`ETAT-REEL.md`, ou vérifié ligne à ligne.
- **Preuve** : test qui échoue si une des 18 lignes « faux / non câblé » n'a pas de contrepartie dans le README, ou l'inverse.
- **Refus** : les lignes peuvent être regroupées ; aucune ne disparaît sans justification écrite.

### DOC.6 — Imposer le lexique « licence / contrainte / capacité »
- **Phase** : P2 · **Dépend de** : P1.1 (fait)
- **Constat** : `P1.1` a introduit `Capacity { measured, reservable, enforced }` et `AllocGranted.enforced`. Les documents continuent d'employer « capacité » seul — le mot exact dont la polysémie a produit la confiance mal placée.
- **Travail** : passe sur `DAEMON_STATE.md`, `cli.md`, `PLAN_CONSOMMATEUR.md` et `PLAN_CONSOMMATEUR_MAJ.md` pour remplacer « capacité » par l'un des trois termes, plus un encadré de définition unique.
- **Preuve** : test qui échoue si « capacité » apparaît sans qualificatif dans ces quatre fichiers.
- **Refus** : ne touche pas aux documents historiques (bandéaurés en DOC.3).

### DOC.7 — Tenir la divulgation responsable à jour
- **Phase** : P3 · **Dépend de** : SEC.1
- **Constat** : `SECURITY.md` et `.well-known/security.txt` existent (`ETAT-REEL.md` §2 ligne 11), mais `LEGAL.md:79` dit qu'il n'y a **pas** de prime, alors que §6 annonce un hall of fame sans page correspondante — et `ETAT-REEL.md` §3 ligne 18 note qu'une promesse de prime circule.
- **Travail** : aligner les trois textes (pas de prime, hall of fame réel ou retiré) et faire pointer `security.txt` vers une adresse qui existe.
- **Preuve** : `bash scripts/check-security-txt.sh` valide RFC 9116 et l'absence de contradiction (`grep -i bounty` sur `LEGAL.md` et `SECURITY.md`).
- **Refus** : ne crée pas de programme de prime — engagement financier.
- **GATE-LÉVY** : hall of fame public souhaité ? prime, même symbolique, un jour envisagée ?

### DOC.8 — Statuer sur `docs/index.html`
- **Phase** : P1 · **Dépend de** : DOC.2
- **Constat** : `docs/index.html` (24 746 o) est **référencé** par trois documents — `AUDIT_VERACITE_20261012.md:92`, `PLAN_CONSOMMATEUR_MAJ.md:120`, `ARCHITECTURE_RESEAU.md:109-116` — qui le décrivent comme « le site ». Or le produit sert désormais son propre `web/` via `polygone-gateway` (`POLYGONE_SITE_DIR`). Deux sites cohabitent et aucun document ne dit lequel fait foi.
- **Travail** : statuer (archive déplacée vers `site/archive/`, ou suppression) et mettre à jour les trois références.
- **Preuve** : `grep -rn "docs/index.html"` ne renvoie que des mentions historiques datées, et `INDEX.md` explique le choix.
- **Refus** : pas un travail de design ; on ne tranche pas ici quelle interface fait autorité — c'est `VISION.md` §2.

**Ce qui peut faire échouer cet épique** : (1) DOC.1 peut être écrit si permissif qu'il ne casse jamais — la preuve de la porte est obligatoire ; (2) le travail documentaire passe pour du temps perdu — DOC.1 et DOC.2 sont en P1 parce qu'ils protègent tout le reste ; (3) DOC.6 peut produire une réécriture massive — quatre fichiers seulement ; (4) DOC.7 dépend d'un `GATE-LÉVY` — livrer d'abord l'alignement des textes, qui ne dépend de personne.