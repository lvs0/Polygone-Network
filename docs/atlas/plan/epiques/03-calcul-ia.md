# Épiques 03 — Calcul et IA : `DAEMON`, `PETALS`, `SRV`

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

## DAEMON — le bail et l'isolation

### DAEMON.1 — Remplir `expires_at` et expirer les baux
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : `expires_at: None` est écrit en dur dans `daemon/src/socket.rs:920`, et le doc du variant l'assume (`:262`). `Ledger.grants` est un `HashMap<String,(u32,u64,u64)>` (`:545`) : aucune entrée ne porte d'horodatage, donc **rien dans le ledger ne peut expirer**. Un consommateur qui meurt sans `release` garde sa réservation jusqu'au redémarrage du daemon.
- **Travail** : ajouter `expires_at_ms` à chaque grant, le poser dans `reserve()`, et faire que `free_ram_bytes`/`free_cpu_cores`/`free_storage_bytes` ne comptent plus les grants échus. `dispatch_alloc` renvoie `expires_at` réel.
- **Preuve** : `cargo test -p polygoned --lib bail_expire` — un grant de 1 s, `std::thread::sleep(1100ms)`, puis `ledger.free_ram_bytes()` remonte de la taille réservée **sans** `release`. Et `alloc_granted` porte un `expires_at` non nul.
- **Refus** : ne rend pas l'allocation sûre face à un client vivant qui dépasse — c'est la tâche DAEMON.3, pas celle-ci.

### DAEMON.2 — Balayer les baux orphelins au démarrage
- **Phase** : P1
- **Dépend de** : DAEMON.1
- **Constat** : `Ledger::load()` relit `~/.polygone/ledger.json` sans filtrer (`socket.rs:531-546`). Après un `kill -9` du client **et** du daemon, les réservations reviennent intactes et bloquent la capacité.
- **Travail** : à la lecture, marquer tous les grants comme échus si leur `expires_at_ms` est dépassé, et persister le nettoyage une fois. Consigner le nombre de grants purgés au démarrage.
- **Preuve** : test `les_baux_anterieurs_ne_survivent_pas_au_redemarrage` — écrire un `ledger.json` avec un grant échu, `Ledger::load()`, vérifier `grants.is_empty()` et `free_ram_bytes == headroom`.
- **Refus** : ne prétend pas qu'un bail orphelin est un bail valide « racheté ». Il est purgé, un point c'est tout.

### DAEMON.3 — Ajouter un heartbeat et un renouvellement explicite
- **Phase** : P1
- **Dépend de** : DAEMON.2
- **Constat** : le client sonde son bail (`crates/client/src/node.rs`, `LEASE_POLL_INTERVAL`) mais **le daemon ne reçoit aucune preuve de vie** : `Request` ne porte que `Status`, `Ping`, `Alloc`, `Release` (`socket.rs:186-211`).
- **Travail** : ajouter `Request::Renew { allocation_id }` et `Reply::Renewed { expires_at }`. Un bail non renouvelé expire par DAEMON.1 ; un bail renouvelé repousse l'échéance. Refuser le renouvellement d'un bail inconnu, sans révéler s'il a existé.
- **Preuve** : test `le_renouvellement_repousse_l_echeance` (renouveler à mi-TTL, attendre le TTL initial, vérifier que la réservation tient) et `renouveler_un_bail_inconnu_est_refuse`.
- **Refus** : ne rend pas le bail contraignant — renouveler prolonge une **licence**, pas un quota OS.

### DAEMON.4 — Corriger le commentaire faux sur la persistance du stockage
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : le doc de `reserved_storage_bytes` (`socket.rs:554`) affirme que le compteur « vit en mémoire » et qu'« un redémarrage l'oublie ». C'est faux : le champ n'a pas de `#[serde(skip)]` et le test existant le prouve — `socket.rs:2173-2179` persiste puis relit 5 Gio identiques. Une documentation qui contredit son propre test est le défaut que ce projet combat.
- **Travail** : réécrire le commentaire pour dire ce que le code fait réellement, et signaler le vrai risque résiduel : le stockage est **durable**, donc un grant échu ne libère pas d'octets physiques tant que les blocs ne sont pas supprimés.
- **Preuve** : `rg "vit en mémoire" daemon/src/` ne rend plus rien, et `cargo test -p polygoned --lib storage` reste vert.
- **Refus** : ne change aucun comportement. Si un comportement doit changer, c'est DAEMON.1.

### DAEMON.5 — Mesurer la capacité de stockage au lieu de publier 0
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : `runtime.rs` publie `total_storage_bytes: 0` et le commentaire le justifie par l'absence de source fiable. Résultat : **toute** demande de stockage est refusée, et le service Drive ne peut pas obtenir de bail.
- **Travail** : lire `statvfs` (ou `statfs`) sur le chemin de stockage du nœud, appliquer le plafond de palier et la marge, publier le résultat. Si l'appel échoue, publier 0 **et** un motif lisible.
- **Preuve** : test `storage_headroom_reflete_statvfs` (valeur > 0 sur la machine de test, plafonnée) et un `polygone-gateway` status qui montre un chiffre non nul.
- **Refus** : ne prétend pas que l'espace est **libre** parce qu'il est **présent**. Le chiffre publié est ce que le daemon s'autorise à servir.

### DAEMON.6 — Refuser explicitement une ressource non mesurable
- **Phase** : P1
- **Dépend de** : DAEMON.5
- **Constat** : `REASON_INSUFFICIENT_STORAGE` (`socket.rs:135`) sert à la fois au « pas assez de place » et au « je ne sais pas mesurer ». Le client ne peut pas distinguer un refus temporaire d'un refus structurel.
- **Travail** : ajouter `REASON_RESOURCE_NOT_MEASURED`, à utiliser quand la capacité mesurée vaut 0 par absence de source. Le message doit dire quelle ressource et pourquoi.
- **Preuve** : test `storage_non_mesure_donne_un_motif_distinct` — état avec `total_storage_bytes: 0`, demande de 1 Gio, motif attendu.
- **Refus** : ne devine pas la capacité. Zéro reste zéro.

### DAEMON.7 — Mesurer l'activité utilisateur sous Linux
- **Phase** : P1
- **Dépend de** : aucune
- **Constat** : `resources/linux.rs:293` renvoie `UserActivity::Unknown` en dur, `macos.rs:250` de même (`ETAT-REEL.md` §5). Le frein ne peut donc jamais s'appliquer sur la seule information qui compte : quelqu'un est-il devant la machine.
- **Travail** : brancher une source réelle (logind `IdleHint`/`IdleSinceHint`, ou `XScreenSaver` en repli) avec un seuil d'inactivité configurable. En cas d'échec, garder `Unknown` et **le dire**.
- **Preuve** : test avec une source injectée (`Active`, `Idle`, erreur → `Unknown`) + une mesure manuelle documentée : 3 minutes sans toucher la machine, `polygoned doctor` doit passer de `Active` à `Idle`.
- **Refus** : `Unknown` reste acceptable et ne doit jamais être traduit par « inactif ». Le confondre est ce qui faisait croire au frein.

### DAEMON.8 — Traiter batterie, thermique et capot fermé
- **Phase** : P1
- **Dépend de** : DAEMON.7
- **Constat** : aucune occurrence de `battery`, `thermal` ou de détection de capot dans `daemon/src/` (recherche sur tout le crate). La roadmap P1 promet ce comportement, `VISION.md` aussi ; le code n'en a rien.
- **Travail** : lire `/sys/class/power_supply/*/capacity` et `/sys/class/thermal/thermal_zone*/temp`, définir trois états (secteur, batterie > seuil, batterie faible) et une règle explicite pour le capot fermé. La préférence utilisateur est un champ de config, jamais une supposition.
- **Preuve** : tests sur valeurs simulées (secteur/batterie/faible/thermique) + un test d'intégration qui vérifie qu'une capacité faible réduit l'allocation **et** journalise le motif.
- **Refus** : ne promet pas le fonctionnement capot fermé. Sur beaucoup de portables, la veille suspend tout ; le document doit le dire au lieu de l'espérer.

### DAEMON.9 — Appliquer ou refuser le shaping réseau
- **Phase** : P5
- **Dépend de** : DAEMON.1
- **Constat** : `bandwidth.rs:93` mesure (`_rx_bps`, `_tx_bps` préfixés d'un underscore, donc non consommés) et publie `allocated_mbps`, mais **aucun `tc`, aucun `netem`, aucune limite de socket** n'existe dans le crate. Le plafond est calculé et jamais appliqué.
- **Travail** : soit appliquer une limite réelle (cgroup `io`/`tc` ou limitation applicative au niveau du relay), soit **retirer** `allocated_mbps` de l'état publié et le remplacer par `measured_mbps` honnête. Trancher par écrit avant de coder.
- **Preuve** : selon la branche choisie — soit un test qui mesure un débit plafonné (chiffres avant/après), soit `rg "allocated_mbps"` vide et un status qui n'annonce plus de plafond.
- **Refus** : ne garde pas un champ « allocation » que rien n'applique. C'est exactement le défaut corrigé par `P1.1`.

### DAEMON.10 — Rendre `--isoler` le défaut sous Linux
- **Phase** : P1
- **Dépend de** : DAEMON.6
- **Constat** : `--isoler` est un drapeau opt-in (`crates/client/src/main.rs:443-447`, usage `:290`) qui n'est vrai que sous Linux avec systemd et cgroup v2. Sans lui, le consommateur s'auto-impose son plafond. `ETAT-REEL.md` §5 le dit : « licence » par défaut, « contraint » seulement avec le drapeau.
- **Travail** : inverser la valeur par défaut sous Linux quand `systemd-run --user` et cgroup v2 sont disponibles, avec `--sans-isolation` pour revenir à l'ancien comportement. **GATE-LÉVY** : cela change ce qui se passe au premier lancement sur la machine de l'utilisateur — accepter ou non.
- **Preuve** : `polygone node servir` sans drapeau crée un scope `polygone-node-*.scope`, `systemctl --user show` donne `MemoryMax` attendu, et le scope disparaît au SIGINT. Test : `cargo test -p polygone --lib isolation_par_defaut`.
- **Refus** : ne prétend rien pour macOS et Windows. `--isoler` y reste indisponible avec un message explicite (`main.rs:343`).

### DAEMON.11 — Détecter cgroup v2 et refuser de mentir
- **Phase** : P1
- **Dépend de** : DAEMON.10
- **Constat** : `node_isolation.rs:92-114` vérifie `MemoryMax`/`CPUQuota` après lancement, mais rien ne vérifie **avant** que cgroup v2 est monté. Sur un système hybride (v1 + v2 partiel), `systemd-run` peut réussir sans appliquer.
- **Travail** : contrôle préalable (`/sys/fs/cgroup/cgroup.controllers` ou `systemd --version`), et refus explicite si seul v1 est présent. Le refus doit nommer la cause.
- **Preuve** : test avec un chemin de cgroup simulé invalide → erreur explicite, aucun scope créé ; et un test de succès réel sous Linux.
- **Refus** : ne bascule pas silencieusement en mode licence. L'utilisateur doit savoir qu'il n'est pas contraint.

### DAEMON.12 — Plafonner dette et crédit de l'unité de réciprocité
- **Phase** : P4
- **Dépend de** : DAEMON.3, `P2.2` (deux machines physiques), `docs/atlas/plan/DECISION-POLY.md` §5
- **Constat** : le ledger compte des réservations, pas un solde de contribution. `DECISION-POLY.md` §5.1 pose la règle (solde borné, dette autorisée dans une limite, crédit plafonné, zéro intérêt) mais **aucun plafond n'est décidé**.
- **Travail** : ajouter `credit`/`debt_limit` au ledger, bornés, sans transfert possible, sans convertibilité. **GATE-LÉVY** : quelle valeur de dette autoriser un nouvel arrivant — le plafond décide si le réseau peut démarrer sans capital initial.
- **Preuve** : `cargo test -p polygoned --lib reciprocite` — solde négatif bloqué au plafond, crédit bloqué au plafond, aucune API de transfert n'existe (`rg "fn transfer" crates daemon` vide).
- **Refus** : ce n'est pas une monnaie. Pas de chaîne, pas de wallet, pas de conversion — voir `DECISION-POLY.md` §4.2.

### DAEMON.13 — Éprouver crash, veille et SIGKILL
- **Phase** : P1
- **Dépend de** : DAEMON.3
- **Constat** : la roadmap `P1.5` demande quatre scénarios ; aucun test ne couvre la mort brutale du consommateur, la perte réseau, la veille ou le `kill -9`. `ETAT-REEL.md` §7 le range explicitement dans le non-prouvé.
- **Travail** : quatre tests d'intégration : `kill -9` du client, coupure du socket, `SIGSTOP` puis `SIGCONT` prolongé au-delà du TTL, et redémarrage du daemon avec un ledger existant.
- **Preuve** : `cargo test -p polygoned --test resilience` — 4 tests, aucun ignoré, chacun nommant le scénario.
- **Refus** : ne simule pas la veille machine. La veille réelle demande un test manuel documenté (phase P3.6).

---

## PETALS — inférence distribuée

### PETALS.1 — Reprendre `MockBackend` avec un type de backend honnête
- **Phase** : P4
- **Dépend de** : aucune
- **Constat** : le backend mock du dépôt distant est refusé en l'état pour deux raisons vérifiées (`docs/RECONCILIATION.md` §2 A2) : ses 10 tests contiennent `assert!(true)` et un faux failover, et `MockBackend::backend_type()` renvoie `BackendType::Ollama` (`crates/petals`). Un mock qui se déclare Ollama peut tromper un sélecteur.
- **Travail** : porter le backend seul, ajouter `BackendType::Mock`, et écrire **un** test de routage qui échoue si le sélecteur prend le mock pour Ollama.
- **Preuve** : `cargo test -p polygone-petals mock` — le test `le_selecteur_ne_prend_pas_le_mock_pour_ollama` échoue si on remet `BackendType::Ollama`.
- **Refus** : ne reprend pas les tests du commit distant. Ils prouvent moins que leur titre.

### PETALS.2 — Pipeline à deux workers, sortie identique au local
- **Phase** : P4
- **Dépend de** : PETALS.1
- **Constat** : `plan()` et `plan_with_peers()` (`pipeline.rs:136`, `:149`) n'ont **aucun appelant de production** — seulement des tests et une ré-exportation (`lib.rs:290`). `ETAT-REEL.md` §3.3 : « PipelinePlan sans consommateur hors tests ».
- **Travail** : brancher un consommateur réel : un modèle jouet découpable en deux plages de couches, exécuté localement puis réparti sur deux workers en processus séparés. La sortie doit être **bit à bit identique** à l'inférence locale.
- **Preuve** : `cargo test -p polygone-petals --test pipeline_deux_workers` — même entrée, `hash(sortie_locale) == hash(sortie_distribuée)`. Une différence, même de 1e-6, fait échouer le test.
- **Refus** : ne prétend pas distribuer un vrai modèle de production. Le modèle jouet prouve la plomberie, pas la performance.

### PETALS.3 — Mesurer le coût réseau contre le calcul gagné
- **Phase** : P4
- **Dépend de** : PETALS.2
- **Constat** : `docs/ARCHITECTURE_RESEAU.md:24-33` établit que le transfert d'un octet coûte 2×10³ à 10⁶ fois une lecture RAM, et qu'« aucun cache ne rattrape 10³ ». Ce chiffre est un raisonnement, pas une mesure sur Polygone.
- **Travail** : benchmark chiffré : latence par couche transférée, débit utile, temps de calcul local, temps de calcul distribué, et le rapport des deux. Le résultat **peut** invalider le pipeline.
- **Preuve** : `polygone-bench petals` produit un tableau avec machine, modèle, taille de couche, débit, et un verdict explicite « distribuer gagne / perd ». Le chiffre est cité dans `ETAT-REEL.md`.
- **Refus** : n'extrapole pas d'un LAN à un WAN. Deux colonnes distinctes, ou pas de conclusion.

### PETALS.4 — Trois workers et tolérance à la perte
- **Phase** : P4
- **Dépend de** : PETALS.3
- **Constat** : aucun mécanisme de reprise. Si PETALS.3 conclut que la distribution perd, cette tâche est **annulée**, pas reportée.
- **Travail** : troisième worker, et un scénario où l'un meurt en cours d'inférence : reprise sur les autres ou échec explicite, jamais une sortie partielle présentée comme complète.
- **Preuve** : test `perte_d_un_worker_est_soit_reprise_soit_refusee` — vérifier qu'aucun résultat tronqué n'est retourné comme valide.
- **Refus** : pas de « tolérance » qui produirait une sortie fausse. Mieux vaut échouer.

### PETALS.5 — Refuser un modèle non découpable
- **Phase** : P4
- **Dépend de** : PETALS.2
- **Constat** : `validate(plan)` (`pipeline.rs:325`) vérifie la cohérence du plan, mais rien ne vérifie qu'un modèle **peut** être découpé (architecture non séquentielle, tenseurs partagés, MoE).
- **Travail** : détecter les architectures non découpables et refuser avec un motif nommé, avant tout transfert.
- **Preuve** : test avec un modèle à connexions résiduelles croisées → refus motivé, aucun octet envoyé (compteur réseau à zéro).
- **Refus** : ne tente pas un découpage « au mieux ». Un découpage approximatif produirait une sortie fausse.

---

## SRV — serverless

### SRV.1 — Reçu signé des deux côtés
- **Phase** : P4
- **Dépend de** : aucune
- **Constat** : `crates/core/src/receipt.rs:13` définit `Receipt { sender, receiver, ts, payload_hash, signature }` avec **une seule** signature (`sign`, `:34`). Le worker ne signe pas : rien ne prouve qu'il a exécuté. `DECISION-POLY.md` §5.2 demande une paire.
- **Travail** : introduire `DualReceipt` (ou une variante `SignedByBoth`) portant `signature_worker` et `signature_requester`, sans casser `Receipt` (utilisé par `blocks.rs:77`). Le crédit n'est appliqué que sur paire valide.
- **Preuve** : `cargo test -p polygone-core receipt` — un reçu à signature unique est **refusé** par la vérification de paire, un reçu à double signature accepté, et un reçu dont une signature a été altérée refusé.
- **Refus** : ne remplace pas `Receipt`. `blocks.rs` en dépend ; une rupture silencieuse casserait le store de blocs.

### SRV.2 — Brancher `verify_receipts` avant `tally`
- **Phase** : P4
- **Dépend de** : SRV.1
- **Constat** : `quorum.rs:86` (`tally`) et `:153` (`verify_receipts`) ne sont appelés que dans les tests (`serverless/src/lib.rs:80`). Le doc du module dit lui-même que `tally` ne vérifie aucune signature.
- **Travail** : composer les deux dans un seul point d'entrée, dans cet ordre, avec un `resolve` de clé publique obligatoire. Rendre impossible d'appeler `tally` sur des résultats non vérifiés dans le chemin produit.
- **Preuve** : test `un_noeud_non_signe_ne_compte_pas` — 3 nœuds signés + 4 anonymes, seuil 4 → refus ; 4 signés → acceptation.
- **Refus** : ne considère pas « pas de reçu » comme « reçu valide ». C'est le repli dangereux.

### SRV.3 — Rendre le CLI honnête sur l'absence de dispatch
- **Phase** : P4
- **Dépend de** : aucune
- **Constat** : `crates/client/src/serverless.rs:25` construit et vérifie la demande localement et l'assume (`:12-16`, « aucun nœud n'est contacté »), mais la commande retourne `Ok(true)` sur une simple vérification de fragmentation. Un script qui teste le code de sortie croit que l'exécution a eu lieu.
- **Travail** : retourner un statut distinct (code de sortie non nul ou `Result` typé) signifiant « préparé, non exécuté ». Afficher explicitement « 0 nœud contacté » et le nombre de nœuds requis.
- **Preuve** : test `serverless_sans_dispatch_sort_en_echec` — la commande retourne un échec typé et l'affichage contient « 0 nœud contacté ».
- **Refus** : ne transforme pas la vérification locale en succès d'exécution. Ce serait la faute originelle.

### SRV.4 — Dispatcher réellement sur des nœuds
- **Phase** : P4
- **Dépend de** : SRV.2, SRV.3, `P2.2`
- **Constat** : `ETAT-REEL.md` §3.1-3.2 : `serverless::run` reconstitue localement et le quorum n'est jamais appelé. Le transport vers `ecouter --compute` n'existe pas sur ce chemin.
- **Travail** : envoyer chaque part Shamir à un nœud distinct via le relais, collecter les sorties et les reçus, appliquer SRV.2, et retourner la sortie acceptée avec la liste des signataires.
- **Preuve** : smoke `scripts/serverless-dispatch.sh` — 4 nœuds en processus séparés, seuil 4, une sortie acceptée, 4 signatures distinctes vérifiées, log horodaté.
- **Refus** : ne s'appelle pas « décentralisé » avant d'être passé par `P2.2` sur deux machines physiques. En local, c'est un test d'intégration.

### SRV.5 — Timeouts et éviction d'un nœud lent
- **Phase** : P4
- **Dépend de** : SRV.4
- **Constat** : `NodeOutcome.elapsed_ms` existe (`quorum.rs:30`) mais aucun budget de temps n'est appliqué ; un nœud muet bloque indéfiniment.
- **Travail** : budget par nœud et budget global, éviction explicite, et distinction entre « a répondu faux » et « n'a pas répondu ».
- **Preuve** : test `un_noeud_muet_est_evince_sans_bloquer` — 3 nœuds rapides + 1 muet, seuil 4 → refus rapide avec motif `timeout`, pas une attente infinie.
- **Refus** : ne transforme pas un timeout en vote. Une absence n'est pas un accord.

### SRV.6 — Conserver les reçus pour l'audit
- **Phase** : P5
- **Dépend de** : SRV.4
- **Constat** : les reçus sont produits puis jetés ; rien ne permet à un utilisateur de prouver après coup ce qui a été exécuté et par qui.
- **Travail** : journal append-only local des reçus acceptés, avec taille bornée et rotation, lisible par une commande (`polygone serverless recus`).
- **Preuve** : test `les_recus_survivent_a_un_redemarrage` + une commande qui affiche les N derniers avec signatures vérifiables.
- **Refus** : ne publie rien sur le réseau. Journal local, comme le reste du produit.

---

## Ce qui peut faire échouer ces trois épiques

1. **Le bail sans expiration est plus profond qu'il n'y paraît.** `Ledger.grants` n'a pas d'horodatage, donc DAEMON.1 touche le format persisté : un `ledger.json` ancien doit rester lisible. *Contre-mesure* : versionner le fichier (`"v": 1`) et écrire un test de compatibilité ascendante avant de changer la structure.
2. **DAEMON.10 change le comportement par défaut.** Un utilisateur qui a l'habitude du mode licence verra ses charges se faire tuer par systemd. **GATE-LÉVY** obligatoire, et `--sans-isolation` documenté dans le message d'erreur lui-même.
3. **PETALS.3 peut invalider PETALS.4.** C'est un résultat acceptable, pas un échec : si distribuer coûte plus cher que calculer, il faut l'écrire et arrêter. *Contre-mesure* : la tâche 4 est conditionnée par écrit au verdict de la 3, et le plan doit prévoir cette sortie.
4. **SRV.4 dépend de P2.2, qui n'est pas fait.** Rien de ce qui touche au dispatch ne peut être déclaré « décentralisé » avant le test à deux machines physiques. *Contre-mesure* : SRV.4 reste marqué `partiel` dans `ETAT-REEL.md` même après succès local.
5. **Le risque transversal est le même que partout dans ce dépôt** : du code branché à moitié qui produit des chiffres constants, comme le ledger POLY figé à 100.00. *Contre-mesure* : chaque tâche ci-dessus exige un test qui **échoue** si la fonctionnalité est retirée — pas un test qui se contente de passer.