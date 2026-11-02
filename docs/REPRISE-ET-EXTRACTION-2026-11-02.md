# Reprise Polygone — extraction historique et état réel (2026-11-02)

> Note de travail issue d'une lecture en mode SQLite read-only de
> `~/Downloads/Data Graph/state.db.bak`, de l'inspection du dépôt local
> `~/Projects/Polygone-real` et de ses références GitHub.
> Aucun historique brut de conversation, secret, clé ou donnée personnelle
> n'est recopié dans ce dépôt.

## 1. Source historique examinée

- Base SQLite : `Downloads/Data Graph/state.db.bak` (~2,24 Go).
- 5 391 sessions ; 179 273 messages ; 170 597 messages marqués actifs.
- Index plein texte : 44 042 lignes correspondant à la recherche « polygone ».
- Tables principales : `sessions`, `messages`, `messages_fts`, `session_model_usage`.
- Extraction effectuée en lecture seule ; aucun changement de la base source.

Les conversations les plus utiles retrouvées couvrent :
- 10 mai 2026 — vision initiale et exigence d'un produit simple, réellement utilisable et fidèle à la spécification.
- 24 juin 2026 — frustration face aux travaux annoncés comme terminés alors qu'ils ne l'étaient pas ; demande de livrables vérifiables plutôt que de cron jobs improductifs.
- 20 juillet 2026 — consigne de poursuivre et peaufiner, mais avec sauvegarde restaurable et prudence absolue sur les données.
- 23 juillet 2026 — demande explicite de pousser les changements locaux sur GitHub puis de reprendre le travail.
- 2 août 2026 — clarification de périmètre : l'IA biomimétique et l'interface neuronale sont reportées ; priorité à Polygone Network comme produit stable, attractif et cohérent.
- 2 août 2026 — demande de daemon qui mesure les ressources réellement libres, conserve une marge de sécurité, alloue dynamiquement CPU/RAM/réseau jusqu'à désactivation, avec préférence utilisateur pour le comportement lorsque le portable est fermé.
- 22 août 2026 — demande de reprendre les anciennes conversations Hermès et les fichiers, de respecter la vision et de ne pas mélanger les projets futurs au produit actuel.

## 2. Vision et contraintes consolidées

### Produit prioritaire
Polygone doit d'abord être un produit de confidentialité/réseau utilisable par des personnes non techniques, et pas une collection de prototypes. La commande et l'expérience principales doivent rester simples ; l'interface doit s'appuyer sur les visuels existants du projet, pas inventer une « interface neuronale ».

### Principes à préserver
1. Confidentialité et souveraineté des données par défaut.
2. Cryptographie post-quantique et fragmentation là où elles sont réellement intégrées et testées.
3. Pas de promesse de fonctionnalité non démontrée par un test ou un parcours E2E.
4. Priorité à la stabilité, la légèreté, la simplicité et la compatibilité multiplateforme.
5. Ressources prêtées uniquement si elles sont mesurées disponibles, avec marge de sécurité et retrait rapide.
6. Toute opération risquée doit avoir un plan de restauration ; aucune réparation de disque ni suppression de données ne fait partie de cette reprise.
7. Petals distribué, IA biomimétique, essaims cognitifs et OS spécialisé restent des pistes ultérieures, après stabilisation du produit réseau.

### Ressources / daemon
La demande historique n'est pas « afficher un chiffre d'allocation » : elle est de permettre à un nœud de prêter des ressources disponibles sans dégrader l'ordinateur hôte, puis de les reprendre rapidement. Le mode portable fermé doit être une option explicite, jamais une hypothèse silencieuse : veille, thermique, batterie, alimentation et réglages de l'OS doivent être traités avant toute promesse de fonctionnement capot fermé.

## 3. État constaté de Polygone-real

Dépôt : `/home/lvs0/Projects/Polygone-real`
Remote : `https://github.com/lvs0/Polygone-Network.git`
Branche locale : `main`
HEAD local au moment de l'audit : `f3d09c0` (29 octobre 2026).

Workspace Cargo détecté :
- `polygoned`
- `polygone-core`
- `polygone-relay`
- `polygone-client`
- `polygone-serverless`
- `polygone-gateway`
- `polygone-petals`

Le travail local contient notamment des modifications non commitées sur Hide, Gateway, CLI, Petals, Serverless, README et STAGING, ainsi que de nouveaux tests et modules. Ces changements sont conservés tels quels.

### Vérifications effectuées
- `cargo metadata --no-deps --format-version 1` : réussi.
- `cargo fmt --all -- --check` : réussi.
- `cargo test --workspace` : réussi, 266 tests exécutés au total, aucun échec.
- `cargo clippy --workspace --all-targets -- -D warnings` : réussi, zéro avertissement bloquant.

### Lacune fonctionnelle majeure à traiter
Le daemon sait mesurer l'état système, calculer une politique, publier un état et gérer des réservations via socket. Mais dans `GlowUpEngine::apply`, l'application réelle se limite actuellement à une priorité CPU (`nice`). Le code précise lui-même que le daemon n'impose pas de quota RAM/CPU aux consommateurs : l'allocation accordée est une licence que le consommateur doit respecter, pas une contrainte système appliquée.

Autres limites constatées :
- Linux et macOS renvoient actuellement `UserActivity::Unknown` faute de source fiable branchée ; l'activité utilisateur ne peut donc pas encore piloter réellement la réduction d'allocation.
- La plateforme macOS contient encore des opérations stubées (affinité CPU, uptime).
- La capacité de stockage exposée par le snapshot daemon vaut actuellement zéro.
- Windows dispose d'une implémentation en cours de portage, mais doit être vérifiée sur un runner Windows.
- Petals distribué et Serverless ont encore des étapes de transport/dispatch explicitement non branchées dans leurs spécifications.

Conséquence : ne pas annoncer « allocation dynamique réellement appliquée » avant d'avoir un consommateur isolé, une limite effective, un retrait testé et des mesures sur l'hôte.

## 4. Écart avec GitHub — ne pas synchroniser à l'aveugle

La référence distante `origin/main` pointe sur `23d27ac` (11 septembre 2026), tandis que le HEAD local est `f3d09c0` (29 octobre 2026). Au relevé :
- 6 commits locaux absents de `origin/main`.
- 17 commits distants absents du HEAD local.
- Le dépôt local contient en plus des changements non commités.

La branche distante contient notamment :
- des tests mock du backend Petals ;
- des durcissements Drive / Serverless / Petals ;
- le durcissement fail-closed du checksum de l'installateur ;
- les spécifications/gouvernance et le dossier de vision ;
- le portage Windows du daemon et les correctifs multiplateformes.

La branche locale contient des travaux plus récents sur le daemon, l'intégration Gateway/Petals/Client et le workspace. Les deux lignes ont divergé : un merge ou reset automatique risquerait de perdre du travail ou de réintroduire des modules supprimés volontairement.

**Décision de reprise :** garder le répertoire local intact, examiner les 17 commits distants un par un, comparer leurs changements avec les modifications locales, puis intégrer sélectivement les apports utiles sur une branche de sauvegarde. Ne pas push avant revue des différences et validation des tests.

## 5. Ordre de travail recommandé

1. **Réconciliation Git sûre** : snapshot local, inventaire des commits divergents, intégration sélective des apports distants utiles (en particulier checksums installateur, tests Petals, correctifs Windows).
2. **Contrat de ressources honnête** : séparer capacité mesurée, capacité réservable et capacité effectivement contrainte ; refuser toute réservation qui ne peut pas être garantie.
3. **Consommateur isolé** : définir un cgroup dédié / mécanisme OS qui ne touche jamais au cgroup de la session graphique ; tests de retrait, crash, redémarrage et surcharge.
4. **Mesure d'activité et de sûreté** : brancher une source fiable d'activité, température/alimentation/batterie et comportement portable fermé, avec état Unknown conservateur.
5. **Parcours produit E2E** : installation vérifiée, création d'identité, découverte, envoi/réception, fichiers, arrêt propre et récupération.
6. **Petals distribué / Serverless** : seulement après validation de la base réseau et du broker de ressources.

## 6. Règles de modification pour la suite

- Pas de `git reset --hard`, `clean -fd`, force-push ou remplacement du répertoire.
- Pas de commit/push automatique tant que les changements divergents ne sont pas compris.
- Un lot cohérent à la fois ; tests ciblés puis workspace complet.
- Une fonctionnalité n'est « terminée » que si le comportement réel est exercé, pas seulement simulé.
- Les promesses README, site et CLI doivent être alignées sur les capacités mesurées.


## Addendum — extraction Polygone préexistante découverte le 2 novembre 2026

Une extraction dédiée existait déjà dans `/home/lvs0/Downloads/Data Graph/extract-polygone` et doit être considérée comme une source de travail à part entière, en complément de `state.db.bak`. Elle n'avait pas été intégrée au premier inventaire du rapport.

Inventaire vérifié en lecture seule :
- `messages.jsonl` : 32 923 entrées, 157 523 238 octets ; champs `id`, `session_id`, `role`, `timestamp`, `tags`, `content`.
- `sessions.jsonl` : 3 696 entrées, 158 345 936 octets ; champs `session_id`, `title`, `source`, `cwd`, `started_at`, `message_count_total`, `matched`, `has_polygone`, `messages`.
- `sessions.csv` : index de sessions, 251 921 octets.
- Rôles des messages : 3 936 `user`, 6 897 `assistant`, 22 090 `tool`.
- Tags observés : `polygone` (27 642 occurrences) et `reseau` (15 247 occurrences).
- Période des messages : avril à août 2026.

Cette extraction contient des conversations de conception, d'implémentation et d'exploitation (par exemple les sessions intitulées « Analyse et réécriture du projet Polygone », « Reprise du travail sur polygone-core », « MVP Cube sécurisé avec agent sandboxé » et « Correction des crons trop fréquents »). Les fichiers sources n'ont pas été modifiés. Les messages bruts ne sont pas recopiés dans le dépôt afin de ne pas y introduire inutilement des données privées ou des secrets.

**Consigne de reprise :** exploiter `extract-polygone` en lecture seule pour établir une chronologie et une matrice des décisions/exigences, puis confronter celles-ci au code local et à GitHub. Ne pas supposer que l'extraction est exhaustive ni que les résumés de sessions remplacent la lecture des messages pertinents.


## 11. Incréments réalisés après l'audit — 2 novembre 2026

Ces changements sont présents dans le dépôt local, mais **aucun commit, merge,
reset ou push n'a été effectué**. Les modifications préexistantes et les
fichiers non suivis ont été conservés.

### IPC daemon et client nœud

- Le binaire `polygoned` démarre le serveur IPC Unix v1 hors `--dry-run` ; les
  publications proviennent de `runtime::run_tick`. Le socket est en mode `0600`
  et est retiré à l'arrêt propre.
- `crates/client/src/node.rs` est exposé par `polygone-client` et branché aux
  commandes `polygone node servir` et `polygone node status`.
- Le client obtient le bail avant d'ouvrir son port, impose sa propre limite de
  transferts, écrit un état local et rend le bail à Ctrl+C. Il ferme aussi le
  service si le daemon disparaît.
- Les chemins du ledger de test sont passés en paramètres explicites ; les tests
  ne se disputent plus une variable d'environnement globale.

### Retrait de ressources

- L'ancien retrait reposait sur une pente d'historique lissée qui pouvait rester
  plate indéfiniment tant que l'allocation était maintenue. Le premier retrait
  était donc bloqué dans ce scénario.
- L'état RAM initial est borné à la capacité mesurée et au plafond configuré ;
  en cas de mesure manquante, le moteur démarre à zéro.
- L'hystérésis utilise un compteur de cycles consécutifs où la cible RAM est
  inférieure à l'allocation actuelle.
- Après le seuil, `shrinking=true` reste visible pendant toute la contraction
  progressive, puis s'efface une fois la cible atteinte.
- Pendant cette phase, les nouvelles demandes de bail reçoivent
  `policy_shrinking`. Les statuts et libérations restent permis.
- Le consommateur s'arrête sur ce signal et rend son bail. Sans `--isoler`, le
  bail reste un engagement et le plafond de transferts est auto-imposé. Sous
  Linux, `--isoler` lance le processus dans un scope systemd dédié qui impose et
  vérifie `MemoryMax` et `CPUQuota` via cgroup v2. macOS n'a pas encore ce mode.

### Gateway et vérifications

- `Gateway::serve` accepte un `TcpListener` déjà lié ; les tests d'intégration
  n'ouvrent plus une fenêtre entre la réservation puis la reprise d'un port
  éphémère, source de réinitialisations intermittentes.
- `cargo fmt --all -- --check` : réussi.
- `cargo test --workspace` : **333 tests réussis**, aucun échec (après le test
  de régression sur le plafond RAM de démarrage).
- `cargo clippy --workspace --all-targets -- -D warnings` : réussi.
- `git diff --check` : réussi.
- Le chemin compute/relay a ensuite été durci : grants refusés comptés comme
  échecs, cache anti-rejeu fail-closed, trames bornées dans les deux sens,
  délais absolus, slots atomiques, handshake borné et nettoyage de route
  protégé contre les déconnexions tardives. `scripts/compute-smoke.sh` passe
  avec une tâche réussie, une tâche distante en échec, les deux réputations
  et l'ancrage TOFU vérifiés. L'installateur a été testé hors ligne : checksums
  absents/invalides, archive avec chemins inattendus, lien symbolique, build
  source simulé, alias `polygone-client` et nettoyage des dossiers temporaires.
  L'historique de réputation est maintenant écrit atomiquement et les tests
  de réputation utilisent des dossiers temporaires, sans modifier `~/.polygone`.
  Le workflow de release ne référence plus un binaire Cargo inexistant ; son
  job de checksum utilise désormais `gh release download` (l'ancienne option
  `download_assets` de softprops v2 n'existe pas) et publie le manifeste sur le tag.
  Le mesh a aussi été exercé en LAN réel local ;
  sous un scope de test `MemoryMax=256 Mio`, la RAM annoncée était 253 Mio,
  et non la mémoire libre totale de l'hôte. Le smoke `forensic-drive.sh` a aussi
  confirmé un transfert de fichier via relay avec comparaison octet à octet ;
  les envois dépassant le plafond d'une trame sont désormais refusés avant
  l'ouverture du relay, sans laisser de session KEM partielle.
- Test `--isoler` avec le binaire réel : le processus nœud a tourné dans
  `/user.slice/user-1000.slice/user@1000.service/app.slice/polygone-node-….scope`
  avec `memory.max=268435456` et `cpu.max=100000 100000`. Le lanceur est resté
  dans `session-19.scope`, dont les plafonds sont restés `max`; après SIGINT,
  le bail a été rendu, le scope a été déchargé (`LoadState=not-found`) et aucun
  cgroup Polygone n'a subsisté. Le CLI refuse aussi un scope dont les limites
  ne correspondent pas exactement au bail accordé.
- Test de processus réel du daemon et du client : IPC hello/status/alloc/release,
  socket `0600`, nettoyage SIGINT, service de blocs, statut frais, arrêt sur
  retrait RAM réel, libération du bail, et refus d'une nouvelle réservation
  pendant `shrinking`. Ce test a utilisé une configuration temporaire dans un
  répertoire jetable et n'a pas touché à la configuration personnelle.

### Sauvegardes vérifiées de cet état

Une archive intermédiaire antérieure au mode cgroup `--isoler` reste disponible :
`/home/lvs0/Backups/Polygone-real-retrait-et-IPC-2026-11-02.tar.gz`.

La sauvegarde finale précédente est conservée :
`/home/lvs0/Backups/Polygone-real-reprise-finale-2026-11-02.tar.gz`.

La sauvegarde vérifiée après les durcissements compute/mesh/relay et installateur
est `/home/lvs0/Backups/Polygone-real-reprise-finale-2026-11-02-v2.tar.gz` :
90 503 627 octets, SHA-256
`f7127f21c1a0c00816720ce75b7b9bb2c220251979bb24f2faac65182ef727d6`.
Elle comprend `.git`, les fichiers suivis, les changements locaux et les fichiers
non suivis ; seul `target/` est exclu. L'archive a été listée intégralement,
extraite dans un répertoire jetable, puis les fichiers critiques comparés avec
`cmp` : `RESTORE_CHECK=PASS`.

### Reste prioritaire

1. Réconcilier les 6 commits locaux et les 17 commits distants en comparant les
   patches un par un ; pas de merge/reset/push avant cette revue.
2. Étendre l'isolation OS au-delà de Linux/systemd (macOS), puis décider si le
   mode `--isoler` devient le comportement par défaut sur les plateformes où un
   contrôleur fiable est disponible. Sur Linux, le scope dédié `app.slice` est
   déjà vérifié et n'altère pas `session-*.scope`.
3. Brancher une source fiable pour l'activité utilisateur, vérifier les
   plateformes macOS/Windows sur leurs runners et mesurer les cas batterie,
   thermique et portable fermé.
4. Faire un échange réel entre deux machines physiques ; les tests précédents
   prouvent les comportements locaux et loopback, pas encore le réseau multi-hôte.
