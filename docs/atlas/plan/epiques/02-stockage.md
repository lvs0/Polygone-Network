# Épiques STOCKAGE — DRIVE et BLOCKS

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

> Périmètre : `crates/client/src/{drive,blocks,calc,node}.rs`,
> `crates/serverless/src/`. Constats vérifiés dans ``.

---

# ÉPIQUE DRIVE — stockage distribué de fichiers

### DRIVE.1 — Trancher le sort du module Drive *(GATE-LÉVY)*
- **Phase** : P4
- **Dépend de** : aucune
- **Constat** : `crates/client/src/drive.rs` (84 lignes) existe sur disque mais
  **n'est déclaré nulle part** : `main.rs:14-37` monte `demo, duress, exec, hide,
  identity, mesh, msg, net, petals, product, reputation, self_test, serverless,
  testutil, tui` ; `lib.rs:19-23` expose `blocks, calc, mesh, node, node_isolation`.
  Aucune occurrence de `mod drive`. Le commit local `a9be256` l'a débranché au
  motif documenté « `DriveFile` a zéro appelant ».
- **Travail** : décider, et écrire la décision dans `VISION.md` §8 et
  `STATE.md`. Deux options seulement : (a) Drive redevient un module monté, avec
  le niveau de service minimal défini par DRIVE.2 à DRIVE.8 ; (b) Drive disparaît
  définitivement et `net/mod.rs:1025-1031` (réception sous `~/.polygone/received/`)
  devient la seule réception de fichiers, documentée comme telle.
- **Preuve** : `grep -rn "mod drive" crates/client/src/` renvoie la décision
  (ligne présente si (a), absente et documentée si (b)) ; la phrase correspondante
  existe dans `VISION.md` §8.
- **Refus** : ne traite pas du chiffrement au repos. **Aucun épique ne le
  couvre aujourd'hui**, et c'est un trou réel : `blocks::put_in` range le
  contenu **en clair**, adressé par son hash — un pair qui lit le disque lit
  les fichiers. Chiffrer au repos contredit l'adressage par hash du clair et
  demande une décision de conception (hash du chiffré, ou chiffré + index
  séparé). À arbitrer avec DRIVE.3, pas après.

> **Question exacte à poser à Lévy** : « Drive doit-il redevenir un module monté,
> et avec quel niveau de service minimal — fichier unique, ou répertoire
> versionné ? » Tant que la réponse n'est pas donnée, DRIVE.2 à DRIVE.8 ne
> démarrent pas.

### DRIVE.2 — Restituer la provenance des fichiers reçus
- **Phase** : P4
- **Dépend de** : DRIVE.1 = (a)
- **Constat** : `drive.rs:61` écrit `sender: NodeId([0u8; 16])` en dur dans
  `list()` (`:42`). Les fichiers listés **perdent** l'expéditeur qui avait été
  fourni à `save()` (`:28`) : l'information existe à l'écriture et n'est jamais
  relue.
  L'interface affiche donc un nœud nul pour tout le monde.
- **Travail** : persister l'expéditeur à côté du contenu (fichier d'index JSON
  par dossier reçu, ou en-tête binaire versionné), et relire au listing. Un
  index illisible ne doit pas faire disparaître les fichiers : il les liste avec
  `sender: None`, jamais avec un zéro qui passe pour une identité.
- **Preuve** : `cargo test -p polygone-client drive::tests::listing_preserves_sender`
  — sauvegarde avec un `NodeId` connu, `list()`, assert l'égalité. Deuxième test :
  index corrompu → fichiers listés, `sender == None`.
- **Refus** : ne vérifie pas la **signature** de l'expéditeur ; DRIVE.2 rend
  l'information, il ne l'authentifie pas.

### DRIVE.3 — Découper un fichier en blocs avec manifeste versionné
- **Phase** : P4
- **Dépend de** : DRIVE.2
- **Constat** : `drive.rs:30` fait `std::fs::write(&path, data)` sur le fichier
  entier, en mémoire. `blocks.rs:51` plafonne un bloc à `MAX_BLOCK_BYTES = 1 Mio` :
  un fichier de 10 Mio n'a aujourd'hui aucun chemin de bout en bout —
  il n'est ni découpé, ni adressé, ni reprenable.
- **Travail** : découper en blocs de `blocks::MAX_BLOCK_BYTES`, ranger chaque
  bloc via `blocks::put`, et écrire un manifeste
  `{magic, version, total_len, entries:[{hash, len}]}`. Le manifeste est lui-même
  un bloc (même adressage, même intégrité) : un fichier = un hash de manifeste.
- **Preuve** : `cargo test -p polygone-client drive::tests::chunk_and_manifest_roundtrip`
  sur un fichier de 3 Mio (3 blocs + manifeste), contenu comparé octet à octet.
  Test d'échec : un bloc manquant doit produire `Missing(hash)`, pas un fichier
  tronqué.
- **Refus** : ne déduplique pas **entre** fichiers au-delà de ce que `blocks::put`
  fait déjà par hash ; ne compresse pas.

### DRIVE.4 — Appliquer un quota de stockage par utilisateur
- **Phase** : P4
- **Dépend de** : DRIVE.3, BLOCKS.2
- **Constat** : `NodeConfig` (`node.rs:99-101`) ne porte que `bail_ram_gb` et
  `bail_cpu_cores` ; `allocate()` (`:232`) ne transmet **aucun** stockage. Côté
  daemon, `runtime.rs` publie `total_storage_bytes: 0`, donc
  `free_storage_bytes()` vaut toujours 0 et toute demande `storage_gb` est
  refusée avec `insufficient_storage`.
- **Travail** :
  1. brancher le champ `storage_gb` de la demande de bail sur un quota configuré ;
  2. refuser un `put` qui dépasserait le quota **avant** d'écrire, pas après ;
  3. compter l'espace réel (`sum(len)` des blocs possédés), pas le déclaré.
- **Preuve** : `cargo test -p polygoned socket::tests::storage_quota_refuses_before_write`
  — état avec 1 Gio de stockage, demande de 2 Gio → `AllocDenied{insufficient_storage}`,
  et `ls ~/.polygone/blocks` inchangé. Mesure : `du -sb` du magasin avant/après
  un `put` refusé = identique.
- **Refus** : ne mesure pas le disque libre réel du système — DRIVE.4 tient un
  quota logique ; le disque plein est traité par BLOCKS.7.

### DRIVE.5 — Répliquer un fichier sur n pairs et survivre à la perte d'un nœud
- **Phase** : P4
- **Dépend de** : DRIVE.3, BLOCKS.4
- **Constat** : `blocks::put` écrit **une** copie locale (`blocks.rs:196-217`).
  Aucun code ne réplique. Si le pair qui détient un bloc disparaît, le manifeste
  reste valide et le fichier est irrécupérable — et rien ne le signale avant la
  lecture.
- **Travail** : après `put`, pousser chaque bloc vers `n-1` pairs de la liste
  locale (via une requête `PUT`, à ajouter au protocole de `blocks.rs`), et
  enregistrer dans le manifeste **où** chaque bloc a été déposé. Un pair
  injoignable est un avertissement listé dans le résultat, pas un échec silencieux.
- **Preuve** : `bash scripts/drive-replication-smoke.sh` — 3 processus locaux
  (1 demandeur, 2 serveurs), fichier de 3 Mio, on tue un serveur, `drive get`
  réussit et affiche le repli. Résultat attendu : « repli sur pair 2, 1 bloc
  récupéré à distance ».
- **Refus** : ne promet pas le facteur `n` — il est **tenté** ; le rapport dit
  combien de copies ont réellement été confirmées.

### DRIVE.6 — Scrubber et réparer les blocs manquants
- **Phase** : P4
- **Dépend de** : DRIVE.5
- **Constat** : `blocks::get_in` (`:225-238`)  supprime un bloc dont le contenu
  contredit le hash (`:195-215`). C'est correct pour ne pas servir un mensonge, mais la
  suppression n'est ni comptée ni réparée : un disque qui se dégrade perd des
  fichiers sans que personne ne le sache.
- **Travail** : une commande `drive doctor` qui parcourt les manifestes, vérifie
  présence + hash de chaque bloc, et re-télécharge depuis un pair ce qui manque.
  Rapport : blocs vérifiés / manquants / réparés / irrécupérables.
- **Preuve** : `bash scripts/drive-doctor-smoke.sh` — on supprime un fichier de
  bloc à la main, `doctor` le détecte (manquant), le répare depuis le second
  serveur, puis un second passage rapporte 0 manquant.
- **Refus** : ne répare pas un bloc dont **toutes** les copies sont perdues ;
  il le déclare `irrécupérable` avec le hash.

### DRIVE.7 — Lire en flux sans tout charger en mémoire
- **Phase** : P4
- **Dépend de** : DRIVE.3
- **Constat** : `drive.rs:30` reçoit `data: &[u8]` — le fichier entier est en
  RAM. `blocks::get_in` rend `Vec<u8>`. Un fichier de 2 Gio sur une machine à
  4 Gio ne passe pas, et la limite n'est nulle part annoncée.
- **Travail** : `drive::read_stream(manifest_hash) -> impl Read` qui résout les
  blocs un par un ; écriture en flux vers un fichier de sortie ou un socket.
- **Preuve** : `cargo test -p polygone-client drive::tests::stream_reads_block_by_block`
  — fichier de 50 Mio, pic de mémoire mesuré via un `Read` instrumenté ;
  attendu ≤ 2 × `MAX_BLOCK_BYTES`.
- **Refus** : ne fait pas de seek aléatoire au milieu d'un bloc ; l'accès est
  séquentiel.

### DRIVE.8 — Liens éphémères et ramassage des blocs orphelins
- **Phase** : P4
- **Dépend de** : DRIVE.3, DRIVE.4
- **Constat** : rien n'expire. `~/.polygone/blocks/` ne fait que croître :
  `put_in` n'a aucune suppression, et aucun compteur ne dit combien d'espace est
  occupé par des manifestes dont plus personne ne veut.
- **Travail** : un lien = `{manifest_hash, expires_at}` persisté ; une commande
  `drive gc` supprime les blocs non référencés par un manifeste vivant, et
  **refuse** de supprimer un bloc référencé par un lien non expiré.
- **Preuve** : test `gc_keeps_blocks_of_live_links` : 2 fichiers, un lien expiré
  et un vivant, `gc`, le vivant se relit, l'expiré renvoie `Missing`. Mesure :
  `du -sb` du magasin décroît du seul fichier expiré.
- **Refus** : `gc` ne touche pas aux blocs dont le manifeste est inconnu — il les
  liste comme orphelins sans les supprimer (dry-run par défaut).

### DRIVE.9 — Migrer le format de manifeste sans perdre de fichier
- **Phase** : P5
- **Dépend de** : DRIVE.3, DRIVE.8
- **Constat** : `blocks.rs` a `CALC_VERSION` pour `calc.rs` (`:69`) mais aucun
  équivalent pour le manifeste : la version est dans le format proposé en
  DRIVE.3 et rien ne la lit encore.
- **Travail** : refuser un manifeste de version inconnue avec un message
  explicite (jamais tenter de le lire « au mieux »), et fournir
  `drive migrate --v1-to-v2` idempotent.
- **Preuve** : test `unknown_manifest_version_is_refused` + test d'idempotence :
  deux exécutions de `migrate` produisent le même nombre de fichiers et les
  mêmes hashes.
- **Refus** : ne migre pas un fichier dont un bloc est manquant — il le signale
  et continue avec les autres.

---

# ÉPIQUE BLOCKS — le magasin de blocs comme primitive

### BLOCKS.1 — Corriger le commentaire faux sur la persistance du stockage
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : `daemon/src/socket.rs:548-553` affirme : « ce compteur vit en
  mémoire. Un redémarrage du daemon l'oublie ». C'est **faux** : le champ
  `reserved_storage_bytes` (`:554`) n'a pas de `#[serde(skip)]` — seul `path`
  en a un (`:561`) — et `persist()` sérialise l'objet entier (`:584`). Le test
  `test_ledger_persistence_preserves_counters` (`:2157-2181`) **prouve le
  contraire du commentaire** en asservissant `loaded.reserved_storage_bytes`.
  Un commentaire qui contredit un test du même fichier est pire qu'aucun
  commentaire : il fait douter du test.
- **Travail** : réécrire le commentaire pour dire ce qui est vrai — les trois
  compteurs survivent au redémarrage, et le risque réel est ailleurs
  (l'espace disque n'est jamais vérifié, cf. BLOCKS.7).
- **Preuve** : `sed -n '540,560p' daemon/src/socket.rs` ne contient plus
  « l'oublie » ; `cargo test -p polygoned socket::tests::test_ledger_persistence_preserves_counters`
  passe sans modification du test.
- **Refus** : ne change aucun comportement — c'est un correctif de véracité.

### BLOCKS.2 — Annoncer une capacité de stockage réelle au lieu de zéro
- **Phase** : P1
- **Dépend de** : BLOCKS.1
- **Constat** : `daemon/src/runtime.rs` publie `total_storage_bytes: 0` avec le
  commentaire « restera à 0 tant qu'aucune source fiable de quota disque n'est
  branchée ». Conséquence mesurable : `free_storage_bytes()` (`socket.rs:623-627`)
  vaut 0, donc **toute** demande `storage_gb` est refusée. Le chemin de stockage
  du ledger n'est donc jamais exercé en production, seulement dans les tests qui
  fabriquent un état à la main.
- **Travail** : lire un quota depuis `daemon.toml` (`[storage] max_gb`, défaut 0
  = désactivé, comportement actuel préservé), et publier
  `min(quota_configuré, espace_disque_libre − marge)`.
- **Preuve** : `cargo test -p polygoned runtime::tests::storage_capacity_follows_config`
  — quota 10 Gio → `total_storage_bytes == 10 Gio` ; quota absent → 0 ; quota
  supérieur au disque libre → valeur plafonnée, jamais supérieure.
- **Refus** : n'alloue pas d'espace disque ; il **déclare** une capacité que
  BLOCKS.7 devra faire respecter.

### BLOCKS.3 — Transporter le quota de stockage dans le bail
- **Phase** : P1
- **Dépend de** : BLOCKS.2
- **Constat** : `NodeConfig` (`crates/client/src/node.rs:99-101`) a
  `bail_ram_gb` et `bail_cpu_cores`, et `allocate()` (`:232`) appelle
  `.allocate(config.bail_ram_gb, config.bail_cpu_cores)`. Le troisième paramètre
  `storage_gb` du protocole n'est **jamais** utilisé par le client : un nœud
  obtient un bail pour servir des blocs sans avoir déclaré d'espace.
- **Travail** : ajouter `bail_storage_gb` à `NodeConfig`, le passer à
  `allocate()`, et l'afficher dans `NodeState` + `node status` — l'espace
  promis doit être visible par le propriétaire de la machine.
- **Preuve** : `cargo test -p polygone-client node::tests::lease_carries_storage`
  + `polygone node status` affiche une ligne « Stockage : N Gio (déclaré) ».
- **Refus** : ne garantit pas que le disque a l'espace — c'est une déclaration,
  et le mot « déclaré » doit rester dans l'affichage.

### BLOCKS.4 — Choisir un pair qui possède réellement le bloc
- **Phase** : P2
- **Dépend de** : BLOCKS.3
- **Constat** : `blocks::fetch(hash, peer)` (`:386`) prend **une** adresse
  fournie par l'appelant. Il n'existe aucun annuaire : `mesh.rs` annonce
  `POLYGONE v1 <node_id> <relay> [<ram_mb>]` — ni port de blocs, ni contenu.
  `DEFAULT_PORT = 7643` (`:48`) est fixe précisément pour ne pas toucher au
  format d'annonce (commentaire `:34-47`).
- **Travail** : requête `HAS <hash>` à un pair (réponse `YES`/`NO` sans
  transporter le bloc), et boucle de sélection sur la liste de pairs connus avec
  ordre stable (pas de hasard) et arrêt au premier `YES`.
- **Preuve** : `cargo test -p polygone-client blocks::tests::selects_peer_that_has_the_block`
  — 2 faux serveurs, seul le second a le bloc, la sélection doit l'atteindre en
  2 requêtes ; compteur de requêtes asservi. Et `HAS` doit être refusé si le
  hash est invalide (même règle que `GET`, `is_valid_hash`, `:89-95`).
- **Refus** : ne construit pas d'index global distribué — la sélection interroge
  des pairs, elle ne maintient pas une base de qui détient quoi.

### BLOCKS.5 — Basculer sur un second pair quand le premier échoue
- **Phase** : P4
- **Dépend de** : BLOCKS.4
- **Constat** : `fetch` (`:386-450`) rend une erreur et s'arrête. Un pair qui a
  le bloc mais tombe pendant le transfert fait échouer toute la lecture, alors
  que d'autres copies peuvent exister.
- **Travail** : politique de repli ordonnée (liste de pairs, pas de retry
  infini) : timeout 5 s déjà posé par `IO_TIMEOUT`, 2 tentatives maximum par
  pair, journal des tentatives rendu à l'appelant.
- **Preuve** : `cargo test -p polygone-client blocks::tests::falls_back_when_first_peer_drops`
  — premier serveur qui accepte puis coupe, second qui sert ; le bloc est obtenu
  et le journal liste les deux tentatives.
- **Refus** : ne réessaie pas indéfiniment ; ne masque pas l'échec total — si
  aucun pair ne sert, l'erreur liste chaque tentative avec sa raison.

### BLOCKS.6 — Borner la bande passante par pair, pas seulement les connexions
- **Phase** : P4
- **Dépend de** : BLOCKS.3
- **Constat** : le seul plafond est `MAX_CONNECTIONS = 16` (`:62`), observable
  par `Served::accepted/refused` (`:239-246`). Aucune limite de débit : 16
  transferts de 1 Mio en parallèle saturent un lien montant domestique, ce qui
  contredit la promesse « n'utilise que la bande passante inutilisée ».
- **Travail** : lire le débit autorisé depuis le bail (`state.bandwidth_mbps`
  existe dans le protocole daemon) et étrangler chaque connexion par un
  token-bucket simple ; compteur `throttled` exposé dans `Served`.
- **Preuve** : `bash scripts/blocks-throttle-smoke.sh` — plafond 1 Mbps, transfert
  de 4 Mio, chronomètre ≥ 30 s ; sans plafond < 5 s. Les deux chiffres sont
  cités dans la sortie.
- **Refus** : ne prétend pas façonner le réseau au niveau OS — c'est un
  étranglement applicatif, et le document doit le dire ainsi.

### BLOCKS.7 — Faire respecter l'espace disque et évincer proprement
- **Phase** : P4
- **Dépend de** : BLOCKS.6, DRIVE.4
- **Constat** : `put_in` (`:156-190`) écrit sans jamais regarder l'espace libre.
  Un disque plein donne une écriture partielle : `std::fs::write` échoue, le
  temporaire est supprimé, mais un `put` réussi de peu peut remplir la partition
  et casser le reste du système — exactement l'incident du SSD à 99 % de
  septembre 2026.
- **Travail** : refuser un `put` sous un seuil de disque libre (garde, pas
  avertissement), et évincer les blocs les moins récemment lus quand le magasin
  dépasse son quota — jamais un bloc référencé par un lien vivant.
- **Preuve** : test `put_refuses_below_free_space_floor` avec un seuil injecté
  (pas de vrai disque plein) + `put_evicts_lru_within_quota`. Mesure :
  `du -sb` après éviction ≤ quota.
- **Refus** : ne récupère pas l'espace des fichiers ouverts ; ne promet pas
  l'absence de perte — l'éviction **est** une perte, et elle est annoncée.

### BLOCKS.8 — Rendre l'écriture durable avant de la déclarer finie
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : `put_in` fait `std::fs::write(&tmp, bytes)` puis
  `std::fs::rename` (`:156-180`). Ni `File::sync_all()` sur le fichier, ni
  `fsync` sur le dossier : après une coupure de courant, le renommage peut
  survivre alors que le contenu n'est pas sur le disque, donnant un fichier de
  longueur nulle sous un hash valide. `get_in` le supprimerait alors — au mieux
  une perte silencieuse, au pire une perte de la seule copie.
- **Travail** : ouvrir avec `File::create`, écrire, `sync_all()`, fermer,
  renommer, puis `fsync` du dossier.
- **Preuve** : `cargo test -p polygone-client blocks::tests::put_syncs_before_rename`
  — vérifie l'ordre des opérations via un `#![cfg(test)]` compteur d'étapes
  injecté ; et un test qui simule un renommage réussi avec contenu vide pour
  prouver que `get_in` refuse et supprime.
- **Refus** : ne rend pas le système de fichiers transactionnel ; `fsync` coûte
  du débit et le compromis est documenté.

### BLOCKS.9 — Prouver un transfert de bloc entre deux machines physiques
- **Phase** : P2
- **Dépend de** : BLOCKS.5
- **Constat** : `ETAT-REEL.md` §7 : « prouvé : deux processus locaux ». Aucun
  transfert n'a jamais été mesuré entre deux hôtes. C'est le trou le plus ancien
  du projet et il concerne directement `blocks.rs`, seule voie P2P directe.
- **Travail** : deux hôtes sur le même LAN, pare-feu réel, `polygone bloc servir`
  sur l'un, `fetch` sur l'autre, bloc de 1 Mio, hash comparé.
- **Preuve** : journal horodaté avec les deux adresses IP distinctes, le hash
  source, le hash reçu, la durée et le débit mesuré. Un `MISS` n'est pas une
  preuve.
- **Refus** : ne prouve pas le WAN, le NAT, ni la perte de paquets — P2.3.

### BLOCKS.10 — Mesurer le débit réel et le publier dans `node status`
- **Phase** : P4
- **Dépend de** : BLOCKS.6
- **Constat** : `NodeState` (`node.rs:450-470`) expose `served_blocks`,
  `served_bytes`, `connections_accepted/refused`, mais aucun débit : on ne peut
  pas savoir si la limite de BLOCKS.6 mord, ni si le nœud sature le lien.
- **Travail** : fenêtre glissante de 60 s sur les octets servis, publiée en
  `served_mbps` et `throttled_ms` dans `NodeState`, affichée par `node status`.
- **Preuve** : `cargo test -p polygone-client node::tests::reports_served_mbps`
  avec des compteurs injectés ; et `node status` sur un nœud réel affiche une
  valeur non nulle après un transfert de 8 Mio.
- **Refus** : ce n'est pas une mesure réseau (pas de RTT, pas de perte) — c'est
  un débit applicatif observé côté serveur.

---

## Corrections à mon brief (vérifiées)

1. **« `DriveFile::receive()` échoue au-delà de 64 Kio » est faux pour le code
   local.** Cette limite appartient à la version **distante** de `drive.rs`
   (commit `57bb551`, refusé dans `docs/RECONCILIATION.md` §3). Le
   `drive.rs` local n'a pas de `receive()` du tout : il a `save`, `list`,
   `received_dir`. Le vrai défaut local est l'absence de découpage (DRIVE.3) et
   la perte de provenance (DRIVE.2) — plus grave, parce que silencieux.
2. **La persistance du stockage n'est pas cassée**, contrairement à ce que dit
   `socket.rs:548` : le test `test_ledger_persistence_preserves_counters` le
   prouve (BLOCKS.1). Le trou réel est que la capacité annoncée est 0
   (BLOCKS.2), donc le chemin n'est jamais exercé en production.
3. **`calc.rs` est en avance sur `blocks.rs`** : `serve` (`:387`), `fetch_ref`
   (`:441`) et un test de « reprise distante » (`:661-681`) existent déjà côté
   cache de calcul. Le modèle de sélection de pair de BLOCKS.4 doit s'en
   inspirer plutôt que d'inventer un second motif.

## Ce qui peut faire échouer ces épiques

- **Le gate DRIVE.1 reste sans réponse.** Toute la moitié DRIVE s'arrête, et
  écrire du code pour un module non monté reproduirait exactement l'erreur de
  `a9be256` : du code mort qui a l'air vivant. *Contre-mesure* : ne rien écrire
  avant la décision ; en attendant, BLOCKS.1 à BLOCKS.3 sont utiles quoi qu'il
  arrive.
- **La réplication sans mesure d'espace.** Promettre 3 copies sur des disques
  qu'on ne mesure pas (BLOCKS.2 absent) donne des manifestes qui mentent sur la
  durabilité. *Contre-mesure* : BLOCKS.2 et BLOCKS.3 sont des prérequis durs de
  DRIVE.5, pas des préalables facultatifs.
- **Le `fsync` tue le débit.** BLOCKS.8 coûte cher sur du petit bloc. *Contre-mesure* :
  mesurer avant/après avec `blocks-throttle-smoke.sh`, publier les deux chiffres,
  et ne pas activer le mode durable par défaut si l'écart est inacceptable.
- **Le test à deux machines ne sera pas fait.** C'est le facteur limitant
  historique de tout le projet ; sans lui, DRIVE.5 et BLOCKS.9 restent des
  affirmations locales. *Contre-mesure* : réserver deux machines physiques dès
  le début de P2 et refuser de cocher P2.2 sur un test loopback.
- **L'éviction et le `gc` détruisent des données.** Deux ramasse-miettes mal
  ordonnés peuvent se contredire (l'un supprime ce que l'autre répare).
  *Contre-mesure* : un seul verrou de magasin, `gc` en dry-run par défaut, et un
  test qui exécute les deux en séquence sur le même magasin.