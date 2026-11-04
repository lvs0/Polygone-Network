# Interfaces — épiques CLI, GW, UI

> NOTE DE PORTAGE — 4 novembre 2026 : constats techniques établis le 3 novembre 2026 ; vérifier l’état courant, les commits et les tests avant intervention.

Périmètre : `crates/client/src/{main.rs,tui.rs,product.rs}`, `crates/gateway/`, `web/`, `site/`, `landing/` — vérifié le 2026-11-03.

---

## Épique CLI — le produit en une commande

### CLI.1 — Ajouter `polygone doctor` qui dit les contraintes réelles
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `polygoned doctor` existe (`daemon/src/cli.rs:92`), mais `Commands` côté
  produit (`crates/client/src/main.rs:64-232`) n'a ni `doctor`, ni `statut`, ni `pause`.
- **Travail** : lire `Reply::Status`, afficher les trois grandeurs (`Capacity`) — mesuré,
  réservable, contraint — et conclure : « contraint par l'OS » ou « licence à respecter ».
  Cette sortie sert aussi de source à `polygone statut`.
- **Preuve** : `cargo test -p polygone --lib doctor` — `doctor_reports_licence_when_nothing_enforced`
  et `doctor_fails_with_one_cause_when_daemon_absent`.
- **Refus** : n'invente aucun chiffre sans daemon : erreur avec **une** cause, pas un tableau vide.

### CLI.2 — `polygone pause` / `reprendre` avec une durée réelle
- **Phase** : P3 · **Dépend de** : CLI.1
- **Constat** : le bail meurt avec le processus — « se reconnecter veut dire redemander, pas
  reprendre » (`daemon/src/cli.rs:21`). Aucune pause, alors que la maquette promet 10 min → 4 h.
- **Travail** : `pause --duree 30m` libère le bail et écrit une intention horodatée dans
  `~/.polygone/pause.json` ; `reprendre` la consomme et redemande un bail.
- **Preuve** : `pause_writes_expiring_intent` et `resume_clears_intent_and_reallocates`.
- **Refus** : ne survit pas à un reboot tant que CLI.8 n'existe pas — c'est une intention, pas un état.

### CLI.3 — choisir un profil d'usage, avec son impact chiffré
- **Phase** : P3 · **Dépend de** : CLI.1
- **Constat** : la politique Glow-Up relit une config à chaque tick, mais rien ne la choisit ;
  `node servir` n'expose que `--ram-gb` et `--coeurs` (`main.rs:273-290`).
- **Travail** : `polygone profil <sobre|equilibre|genereux>` écrit les trois seuils et imprime
  l'impact attendu, y compris « aucune limite OS sans isolation ».
- **Preuve** : `three_profiles_write_distinct_limits` et `profil_without_arg_prints_current`.
- **Refus** : ne modifie pas la politique elle-même ; n'annonce jamais une contrainte OS.

### CLI.4 — rendre `--isoler` le défaut quand il est possible
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `isoler` est un booléen opt-in (`main.rs:447`, utilisé `:290`, `:342`, `:361`,
  `:384`) ; hors Linux/systemd, échec explicite (`:343`).
- **Travail** : détecter systemd + cgroup v2, isoler par défaut, offrir `--sans-isolation` pour
  refuser explicitement ; si absent, continuer **en l'annonçant** sur stdout.
- **Preuve** : `isolation_is_default_when_systemd_present` + smoke réel montrant le scope systemd.
- **Refus** : ne rend pas l'isolation possible sur macOS — le message reste « non isolé ».

### CLI.5 — `polygone desinstaller` propre, énuméré, confirmé
- **Phase** : P3 · **Dépend de** : CLI.8
- **Constat** : `uninstall_service` est déclaré (`daemon/src/resources/mod.rs:270`) et n'a
  **aucun appelant** ; `install.sh` n'a pas de contrepartie.
- **Travail** : arrêter et retirer le service, énumérer ce qui sera supprimé (identité, pairs,
  réputation, reçus), exiger `--confirmer`, puis supprimer `~/.polygone`.
- **Preuve** : `uninstall_requires_confirmation` + smoke `install → uninstall → ! -d ~/.polygone`.
- **Refus** : ne touche à aucun fichier de l'utilisateur hors `~/.polygone`.

### CLI.6 — traduire chaque code de refus en action
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `NodeError::Denied { reason }` remonte le code brut ; l'affichage générique
  retombe sur `{other:?}` (`main.rs:266`).
- **Travail** : table exhaustive `REASON_*` → cause → action. `exceeds_measured` → « réduisez à
  N Gio » ; `insufficient_headroom` → « réessayez quand un pair rend son bail ».
- **Preuve** : `every_reason_code_has_an_action`, qui itère sur **tous** les codes.
- **Refus** : ne masque pas le code technique — il reste affiché pour un rapport de bug.

### CLI.7 — installer le service utilisateur au démarrage
- **Phase** : P3 · **Dépend de** : CLI.4
- **Constat** : `install_service` / `uninstall_service` (`resources/mod.rs:269-270`) sont
  définis et jamais appelés ; aucun « redémarrer au boot » n'existe.
- **Travail** : `polygone installer-service` écrit l'unité `systemd --user` (MemoryMax,
  CPUQuota) et l'active.
- **Preuve** : `systemctl --user is-enabled polygone.service` = `enabled`, puis
  `show -p MemoryMax` non vide.
- **Refus** : jamais de service **système** (root). Rien sous macOS ni Windows.

> **GATE-LÉVY** — une application de bureau est-elle dans le périmètre « non-technicien » ?
> Si oui c'est un épique de plusieurs semaines, à trancher avant P3.3.

**Ce qui peut faire échouer cet épique** — (1) le service `--user` n'a pas toujours accès au
GPU : mesurer avant de promettre. (2) `--isoler` par défaut casse conteneurs et WSL : tester
trois environnements. (3) une pause persistée devient fausse si le daemon réalloue. (4) un
désinstalleur partiel est pire que pas de désinstalleur : le smoke vérifie l'absence de résidu.

---

## Épique GW — la passerelle

### GW.1 — recevoir un heartbeat de nœud et le persister
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : `Gateway.nodes` n'est rempli que par `pair_accept` (`crates/gateway/src/routes.rs:191`) ;
  table en mémoire, perdue au redémarrage, aucun heartbeat de relay (`ETAT-REEL.md` §3 ligne 9).
- **Travail** : `POST /api/nodes/heartbeat` authentifiée ML-DSA-65, mise à jour de `last_seen`,
  écriture atomique de `nodes.json`, rechargement au démarrage.
- **Preuve** : `heartbeat_survives_process_restart` et `heartbeat_requires_valid_signature`.
- **Refus** : la gateway ne contacte pas le relay — c'est le nœud qui pousse. Aucun scraping.

### GW.2 — mesurer réellement la latence
- **Phase** : P2 · **Dépend de** : GW.1
- **Constat** : `latency_ms` (`status.rs:47`) n'est affecté nulle part hors tests ;
  `calculate_latency_median` retourne `None` en production.
- **Travail** : ping/pong signé périodique vers chaque pair appairé ; écrit `last_seen` et
  `latency_ms` au retour.
- **Preuve** : `latency_populated_after_pong`, `median_uses_measured_nodes_only`.
- **Refus** : un pair silencieux garde `None` — jamais `0`.

### GW.3 — distinguer pairé, actif et mesuré
- **Phase** : P2 · **Dépend de** : GW.1, GW.2
- **Constat** : `snapshot()` (`status.rs:63`) expose `nodes` (tous appairés) et `active`
  (vus < 300 s) ; rien ne dit combien sont mesurés.
- **Travail** : ajouter `measured`, exposer l'état de contrainte du daemon si joignable ;
  figer un snapshot JSON comme contrat pour `web/dashboard.js`.
- **Preuve** : `status_reports_zero_measured_when_none` + snapshot versionné dans `docs/`.
- **Refus** : ne fusionne jamais « pairé » et « actif » en un seul chiffre.

### GW.4 — authentifier mutuellement l'appairage
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : jeton porteur 256 bits, TTL 5 min, usage unique (`routes.rs:116-206`) ; pas
  d'authentification mutuelle (`ETAT-REEL.md` §3 ligne 12).
- **Travail** : échanger les clés ML-DSA-65 pendant l'appairage ; le jeton devient un canal,
  la signature devient la preuve.
- **Preuve** : `pair_rejects_unsigned_peer`, `pair_binds_signing_key_to_node_id`.
- **Refus** : ne remplace pas TLS — un jeton signé sur HTTP reste lisible par un tiers.

### GW.5 — décider du TLS local, et l'écrire
- **Phase** : P2 · **Dépend de** : GW.4
- **Constat** : service HTTP en loopback (`crates/gateway/src/http.rs`), aucun TLS
  (`ETAT-REEL.md` §3 ligne 14).
- **Travail** : soit certificat auto-signé épinglé derrière `--tls`, soit décision écrite
  « loopback non chiffré, assumé » avec le raisonnement.
- **Preuve** : `tls_handshake_with_pinned_certificate`, ou document daté **et** test que sans
  `--tls` le bind reste 127.0.0.1.
- **Refus** : pas de Let's Encrypt sur une adresse loopback ; pas de « TLS » sans poignée testée.

### GW.6 — séparer l'administration du site public
- **Phase** : P2 · **Dépend de** : aucune
- **Constat** : routes d'administration et fichiers du site sortent du même `Gateway`
  (`routes.rs`, `static_files.rs`), sur la même origine.
- **Travail** : préfixe admin non servi sur l'origine publique, ou deux ports distincts.
- **Preuve** : `admin_routes_404_on_public_origin`.
- **Refus** : un mot de passe dans l'URL n'est pas une séparation.

### GW.7 — exposer publiquement : interrupteur, jamais défaut
- **Phase** : P4 · **Dépend de** : GW.4, GW.5
- **Constat** : rien n'expose hors loopback ; aucun workflow de déploiement
  (`ETAT-REEL.md` §3 ligne 15).
- **Travail** : `--exposer <adresse>` explicite, désactivé par défaut, avertissement nommant
  les métadonnées révélées (IP, horaires).
- **Preuve** : `binds_loopback_without_expose_flag` + modèle de menace mis à jour.
- **Refus** : **GATE-LÉVY** — exposer sur Internet change le modèle de menace et la charge du
  relay. Aucun travail de déploiement avant la décision.

**Ce qui peut faire échouer cet épique** — (1) un heartbeat non signé laisse gonfler le
compteur : GW.4 doit précéder GW.1 en production. (2) `nodes.json` introduit un état
persistant : tester la corruption au crash. (3) la latence entre deux nœuds appairés n'est pas
celle du réseau. (4) exposer avant P2.2 attire du trafic sans protocole prouvé.

---

## Épique UI — la vérité de l'affichage

### UI.1 — supprimer toute simulation du tableau de bord
- **Phase** : P1 · **Dépend de** : aucune
- **Constat** : `web/dashboard.js:31-37` fabrique un journal (« Test Shamir 4-of-7 : 35/35 ✓ »)
  et fixe `frag_ready: 3` ; `:130-160` fait dériver le trafic par `Math.random()`.
- **Travail** : état `null` explicite — bandeau « hors ligne », valeurs `—`, aucun journal
  inventé. Un écran vide et honnête vaut mieux qu'un écran plein et faux.
- **Preuve** : `scripts/no-simulation.sh` échoue si `Math.random` apparaît dans `web/*.js`,
  exécuté en CI.
- **Refus** : ne remplace pas la simulation par des zéros — `0` est une mesure, `—` une absence.

### UI.2 — réparer le polling, qui s'arrête après le premier succès
- **Phase** : P1 · **Dépend de** : UI.1
- **Constat** : `web/dashboard.js:170` — `if (online) return;` avec le commentaire « already
  polling endpoint ». Aucun autre appel ne rafraîchit : les données réelles sont figées puis
  écrasées par `tick()`, qui tourne chaque seconde.
- **Travail** : polling continu, backoff exponentiel avec jitter, bandeau daté après échecs
  répétés, reprise automatique.
- **Preuve** : `poll_continues_after_success`, `backoff_grows_then_resets_on_success`.
- **Refus** : ne masque pas une erreur réseau derrière un état « en ligne ».

### UI.3 — dériver l'état des modules du backend
- **Phase** : P2 · **Dépend de** : GW.3
- **Constat** : `web/dashboard.js:20-30` code en dur `Msg: running`, `Hide: running`,
  `Drive: off`, `Mesh: off`, `Brain: soon`.
- **Travail** : lire les modules depuis le contrat de statut ; un module absent est `inconnu`,
  pas `off`.
- **Preuve** : un statut sans `Drive` n'affiche pas « Off ».
- **Refus** : ne déduit pas l'état d'un module de la présence de son binaire.

### UI.4 — livrer les quatre onglets de la maquette
- **Phase** : P3 · **Dépend de** : CLI.2, CLI.3
- **Constat** : six vues techniques — `View { Home, Help, Output, SendPk, SendMsg,
  ReceivePaste }` (`crates/client/src/tui.rs:172`) — aucun onglet.
- **Travail** : Accueil / Favoris / Services / Paramètres, navigation aux flèches, chaque écran
  branché sur un état réel (contribution, services, pauses, chemins, profil).
- **Preuve** : test de machine à états (bascule, bornes) + capture justifiée par une commande
  reproductible.
- **Refus** : pas d'interrupteur qui ne coupe rien — chaque bascule a un effet mesuré.

### UI.5 — retirer la revendication fausse du site public
- **Phase** : P0 · **Dépend de** : aucune
- **Constat** : `web/index.html:741` et `web/landing.html:737` annoncent « ratatui v2. Five
  tabs. Sparklines. Real-time metrics » ; la TUI n'a ni cinq onglets ni sparklines.
- **Travail** : retirer la phrase maintenant ; la réintroduire quand UI.4 la rend vraie.
- **Preuve** : `rg -i "five tabs|sparklines" web/` retourne 0 occurrence.
- **Refus** : on ne corrige pas une phrase fausse en adoucissant l'adjectif.

### UI.6 — rendre l'interface utilisable sans souris
- **Phase** : P3 · **Dépend de** : UI.4
- **Constat** : `web/node.html` n'a qu'un `aria-hidden` sur le logo et un `role="status"` sur
  le toast ; aucun ordre de focus documenté.
- **Travail** : contraste AA, focus visible, tabulation logique, `aria-live` sur les erreurs,
  taille de police minimale.
- **Preuve** : audit automatisé sur `node.html`, `drive.html`, `mesh.html` — 0 violation
  critique, outil et version cités.
- **Refus** : un score d'audit ne prouve pas l'usage réel — le test manuel reste requis.

### UI.7 — tester la PWA sur un appareil réel et l'écrire
- **Phase** : P3 · **Dépend de** : UI.1, UI.2
- **Constat** : `web/sw.js` et `web/pwa.js` existent ; aucun test sur appareil
  (`ETAT-REEL.md` §4 point 3 : indéterminé).
- **Travail** : documenter un test iOS **ou** Android : version d'OS et de navigateur,
  installation, coque hors ligne, et ce qui a échoué.
- **Preuve** : `docs/pwa-test.md` daté, captures + versions.
- **Refus** : pas de « devrait fonctionner » — un appareil non testé reste indéterminé.

**Ce qui peut faire échouer cet épique** — (1) supprimer la simulation vide l'interface tant
que le backend n'a pas de statut réel : prévoir un message qui explique *pourquoi*.
(2) les onglets dépendent de CLI.2 et CLI.3 ; les livrer avant donne des interrupteurs
décoratifs. (3) un audit d'accessibilité peut passer alors que l'usage reste impossible.
(4) le cache PWA peut resservir une version qui contient encore la simulation.