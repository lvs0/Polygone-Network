# Audit de véracité — `docs/VISION-COMPLETE.md`

Date de l'audit : 2026-11-02  
Périmètre : affirmations de `docs/VISION-COMPLETE.md`, confrontées au code et aux workflows présents dans ce dépôt.  
Règle : une fonction définie, exportée ou testée ne vaut pas intégration produit ; il faut rechercher un appelant de production et, lorsque c'est une capacité réseau, un chemin d'exécution réel.

## Résultat quantifié

| Verdict | Nombre |
|---|---:|
| **VRAI** — capacité présente et appelée dans le produit | 11 |
| **FAUX OU NON CÂBLÉ** — promesse absente, seulement planifiée, ou formulation trompeuse | 18 |
| **INDÉTERMINÉ** — le dépôt ne permet pas de conclure | 4 |
| **Total** | **33** |

## VRAI

1. **Transport de messages chiffrés et authentifiés.** Le client envoie une enveloppe KEM, chiffre le contenu avec AES-GCM, le fragmente avec Shamir et signe les octets canoniques en ML-DSA-65 ; le récepteur reconstruit et vérifie avant livraison. Appelants : `send_network` et `process_line`. Preuves : `crates/client/src/net/mod.rs:198-290`, `:650-860`, `crates/core/src/crypto/kem.rs:1-28`, `crates/core/src/crypto/symmetric.rs:1-20`, `crates/core/src/crypto/shamir.rs:34-80`, `crates/core/src/sign.rs:1-12`.
2. **Relais TCP aveugle pour les fragments.** Le relais route selon `kind`, `to` et `session`, refuse l'usurpation de `from`, limite la taille et le débit, et ne déchiffre ni ne stocke le payload. Preuves : `crates/relay/src/relay.rs:14-27`, `:59-91`, `:165-230`; tests : `crates/relay/src/relay.rs` (module `tests`). Le relais voit néanmoins l'adresse IP source et l'identifiant annoncé — voir FAUX #16.
3. **Exécution shell distante sandboxée sous systemd.** Le chemin `receive_network` appelle `run_res_task`, qui appelle `run_sandboxed` dans `spawn_blocking`; le scope fixe mémoire, CPU, durée, privilèges, accès système, réseau et périphériques. Preuves : `crates/client/src/net/mod.rs:601-620`, `:877-978`, `crates/client/src/exec.rs:105-189`.
4. **Exécution WASM locale au nœud exécutant.** `run_res_task` appelle `run_wasm`; wasmi applique fuel, limites de compilation et plafond de sortie. Preuves : `crates/client/src/exec.rs:30-103`, `crates/client/src/net/mod.rs:601-612`.
5. **IA locale via Ollama.** `polygone petals` interroge le serveur Ollama local, liste les modèles et appelle `/api/generate`. Preuves : `crates/client/src/petals.rs:1-18`, `:87-144`; sous-commandes : `crates/client/src/main.rs:181-185`, `:376-390`.
6. **PWA installable avec coque hors ligne.** Le navigateur enregistre `sw.js`, propose l'installation lorsqu'il reçoit `beforeinstallprompt`, et le service worker met en cache les ressources de la coque et une page hors ligne. Preuves : `web/pwa.js:11-18`, `:62-70`, `web/sw.js:11-51`, `:74-105`. Cela ne fait pas du téléphone un nœud.
7. **Gateway HTTP réellement servi sur socket TCP.** Le binaire démarre `Gateway::run`, qui lie un `TcpListener`; les routes JSON et statiques sont appelées sur les connexions reçues. Preuves : `crates/gateway/src/main.rs:15-40`, `crates/gateway/src/lib.rs:84-109`, `crates/gateway/src/routes.rs:63-103`; preuve socket : `crates/gateway/tests/serve.rs:53-173`.
8. **Hide Phase 1, mono-saut.** Un listener SOCKS5 et un exit node communiquent via le relay ; le circuit multi-hop est explicitement un dry-run qui n'ouvre aucun transport. Preuves : `crates/client/src/hide.rs:38`, `:263-265`, `:526-...`; `crates/client/src/main.rs:193-212`, `:818-840`; `STAGING.md:55-80`.
9. **État local du nœud de stockage.** `polygone node status` lit un fichier d'état horodaté et affiche bail, blocs/octets servis et connexions acceptées/refusées ; les données périmées sont signalées. Preuves : `crates/client/src/node.rs:435-508`, `:541-628`.
10. **Mode de destruction locale explicitement manuel.** `polygone duress --confirmer` supprime l'identité locale, les fichiers reçus, la réputation et les ancres de confiance. C'est une commande volontaire, pas un détecteur automatique de compromission. Preuves : `crates/client/src/main.rs:417-429`, `crates/client/src/duress.rs:1-21`, `:37-80`.
11. **Licence et canal de divulgation sécurité.** Le dépôt contient `LICENSE`, `SECURITY.md` et `.well-known/security.txt`. Preuves : `LICENSE:1`, `SECURITY.md:1-20`, `.well-known/security.txt:1-10`.

## FAUX OU NON CÂBLÉ

1. **« Serverless » exécute une fonction sur plusieurs nœuds.** Faux actuellement : `serverless::run` crée les parts, reconstitue localement la charge utile et imprime explicitement qu'aucun nœud n'a été contacté. Preuves : `crates/client/src/serverless.rs:12-15`, `:25-78`; appel CLI : `crates/client/src/main.rs:751-761`.
2. **Le quorum Serverless est utilisé en production.** `tally` et `verify_receipts` existent comme fonctions pures, mais le chemin CLI ne les appelle pas ; le test de bout en bout du crate est local et sans réseau. Preuves : `crates/serverless/src/lib.rs:23-31`, `:49-90`, `crates/serverless/src/quorum.rs:86-173`; le seul appel de la commande est `crates/client/src/main.rs:757`.
3. **Petals distribue les couches d'un modèle et exécute l'inférence entre pairs.** Non : `pipeline.rs` calcule un plan, mais aucune utilisation de `PipelinePlan` hors du module et de ses tests n'a été trouvée. L'interface utilisateur active interroge seulement Ollama local. Preuves : `crates/petals/src/pipeline.rs:134-225`, export `crates/petals/src/lib.rs:291`, `crates/client/src/petals.rs:1-6`, `:87-144`.
4. **La réputation choisit automatiquement les meilleurs pairs.** Non : le client enregistre des résultats locaux et affiche un score dans la liste ; aucun sélecteur de pair n'utilise ce score. Preuves : `crates/client/src/reputation.rs:81-100`, `crates/client/src/main.rs:643-743`.
5. **Un kill-switch s'active dès qu'une compromission est détectée.** Non : aucun détecteur ou déclencheur automatique n'est câblé. Le seul mécanisme est la commande manuelle `duress --confirmer`. Preuves : `crates/client/src/main.rs:186-192`, `:417-429`, `crates/client/src/duress.rs:20-21`.
6. **Déclencheurs USB, clavier ou GPIO.** Non trouvés dans `crates/client/src` ou `daemon/src`. Le seul chemin de déclenchement documenté est la commande CLI manuelle.
7. **Le téléphone rejoint le réseau comme nœud.** Non : la PWA sert une interface et une coque hors ligne ; aucune implémentation d'un daemon mobile, d'un relay mobile ou d'une participation compute n'est présente dans `web/`.
8. **Pause automatique selon batterie ou réseau mobile.** Non : aucun appel à Battery Status API ni logique de seuil batterie / mobile data n'a été trouvé dans `web/` ou le client.
9. **Le statut Gateway représente l'état global du réseau.** Non : les nœuds sont conservés dans la mémoire du processus Gateway ; `pair_accept` en insère un, mais aucune connexion au relay ou flux de heartbeat n'alimente cette table. Preuves : `crates/gateway/src/routes.rs:127-147`, `crates/gateway/src/status.rs:43-77`.
10. **La latence du réseau est réellement mesurée en production.** Non démontré : le champ `latency_ms` est initialisé à `None`; les seules autres affectations trouvées sont dans les tests. La médiane de production reste donc `null` tant qu'aucun producteur de mesure n'est ajouté. Preuves : `crates/gateway/src/routes.rs:136-145`, `crates/gateway/src/status.rs:61-77`.
11. **Comptes, login et agrégation multi-appareils.** Non : aucune commande `login`, aucun service de compte ni stockage multi-appareils n'existe dans les sous-commandes CLI. L'identité actuelle est une identité locale persistée, pas un compte réseau. Preuves : `crates/client/src/main.rs:63-213`, `crates/client/src/identity.rs:1-...`.
12. **Appairage par jeton porteur.** Partiel mais plus correctement lié : `pair_request` crée un secret aléatoire de 256 bits, valable cinq minutes, et plafonne les demandes pendantes à 4 096 ; `pair_accept` exige ce token, le consomme sous verrou et refuse le rejeu/expiration ; `pair_qr` retourne un deep link et purge les expirés. Ce n'est toujours pas une authentification mutuelle : l'API n'exige pas de preuve de possession signée de la clé publique associée, le Gateway parle HTTP sans TLS intégré, et `pair_qr` retourne une chaîne, pas une image QR. Aucun gestionnaire iOS/Android ni WebAuthn/passkey n'a été trouvé. Preuves : `crates/gateway/src/routes.rs`, `crates/gateway/src/lib.rs`, `crates/gateway/src/http.rs`.
13. **Dashboard global des messages, fichiers et disponibilités.** Non : le statut du nœud compte les blocs servis et les octets du service de stockage ; il ne suit pas les messages, les fichiers E2E, ni un historique global. Preuves : `crates/client/src/node.rs:451-508`, `:556-628`.
14. **Gateway publique HTTPS intégrée.** Non dans le binaire : l'adresse par défaut est `127.0.0.1:8088` et le serveur parle HTTP, sans terminaison TLS. Un reverse proxy externe pourrait fournir HTTPS, mais il n'est pas configuré ici. Preuves : `crates/gateway/src/main.rs:3-9`, `:19-40`, `crates/gateway/src/lib.rs:84-109`.
15. **Déploiement automatique Pages/Vercel/Cloudflare.** Non trouvé : `.github/workflows/ci.yml` teste et construit ; `release.yml` construit et publie des binaires sur tags, sans étape de déploiement du site. Preuves : `.github/workflows/ci.yml:1-...`, `.github/workflows/release.yml:1-...`.
16. **Site généré automatiquement depuis README/STRATEGIE/docs.** Non trouvé : `web/index.html` et `web/landing.html` sont des fichiers statiques présents dans le dépôt ; aucun générateur de site ou pipeline de génération documentaire n'est câblé.
17. **« Logs locaux uniquement, pas de serveur central ».** Formulation trompeuse : le client n'a pas de télémétrie centrale identifiée, mais le relay est un serveur TCP et ses journaux contiennent les adresses IP des pairs (`peer_addr`) et les identifiants annoncés. Il ne lit pas le contenu des payloads. Preuves : `crates/relay/src/relay.rs:14-16`, `:94-135`, `:231-278`.
18. **Programme de bug bounty.** Faux : `LEGAL.md:79` indique explicitement qu'il n'existe pas de programme de bug bounty rémunéré ; `SECURITY.md` décrit un canal de divulgation responsable, ce qui n'est pas un bounty.

## INDÉTERMINÉ

1. **Déploiement public actuel du site et certificat TLS.** Aucun déploiement n'est défini dans les workflows du dépôt ; une opération manuelle ou un hébergement externe reste possible. À trancher avec l'inventaire de l'hébergeur/DNS et un test HTTPS externe autorisé.
2. **Interopérabilité sur deux machines physiques indépendantes.** Les tests démontrent des processus, sockets et smoke tests locaux ; ils ne prouvent pas la fiabilité sur deux réseaux physiques distincts, avec NAT, pertes et coupures. À trancher par une campagne d'intégration multi-hôtes.
3. **Installation PWA sur les versions iOS et Android visées.** Le manifeste et le service worker sont présents, mais le dépôt ne contient pas de résultats de tests réels sur les appareils et navigateurs ciblés. À trancher par une matrice de tests appareil/navigateur.
4. **Objectifs de disponibilité, de latence et de 100 000 utilisateurs simultanés.** Aucun benchmark ni test de charge correspondant n'a été trouvé ; ces chiffres ne peuvent pas être affirmés à partir du code actuel. À trancher par un protocole de charge reproductible et des résultats mesurés.

## Tableau récapitulatif

| # | Promesse / affirmation | Verdict | Preuve principale |
|---:|---|---|---|
| 1 | Transport chiffré et signé | VRAI | `net/mod.rs:198-290`, `:650-860` |
| 2 | Relay TCP aveugle | VRAI | `relay.rs:14-27`, `:165-230` |
| 3 | Exécution shell sandboxée | VRAI | `net/mod.rs:601-620`, `exec.rs:105-189` |
| 4 | Exécution WASM wasmi | VRAI | `exec.rs:36-103`, `net/mod.rs:601-612` |
| 5 | IA locale Ollama | VRAI | `petals.rs:1-18`, `:87-144` |
| 6 | PWA installable/hors ligne | VRAI | `web/pwa.js:11-18`, `sw.js:11-105` |
| 7 | Gateway HTTP sur socket | VRAI | `gateway/lib.rs:84-109`, `tests/serve.rs:53-173` |
| 8 | Hide mono-saut | VRAI | `hide.rs:263-265`, `STAGING.md:71-74` |
| 9 | Statut local du nœud de stockage | VRAI | `node.rs:451-508`, `:556-628` |
| 10 | Destruction locale manuelle | VRAI | `main.rs:417-429`, `duress.rs:37-80` |
| 11 | AGPL et divulgation sécurité | VRAI | `LICENSE:1`, `SECURITY.md:1-20`, `.well-known/security.txt:1-10` |
| 12 | Exécution Serverless distante | FAUX | `serverless.rs:12-15`, `:73-76` |
| 13 | Quorum Serverless câblé | FAUX | `serverless/lib.rs:49-90`, `main.rs:757` |
| 14 | Inférence Petals distribuée | FAUX | `pipeline.rs:134-225`, `petals.rs:87-144` |
| 15 | Réputation utilisée pour choisir les pairs | FAUX | `main.rs:731-743` |
| 16 | Kill-switch automatique sur compromission | FAUX | `main.rs:417-429`, `duress.rs:20-21` |
| 17 | Déclencheurs USB/clavier/GPIO | FAUX | recherche `rg` dans `crates/client/src` et `daemon/src` : aucune occurrence |
| 18 | Téléphone comme nœud | FAUX | `web/pwa.js`, `web/sw.js` : installation/cache seulement |
| 19 | Pause batterie/réseau mobile | FAUX | recherche `rg` dans `web/` : aucune API/condition correspondante |
| 20 | Statut réseau global alimenté par le relay | FAUX | `gateway/routes.rs:127-147`, `status.rs:43-77` |
| 21 | Latence réseau mesurée en production | FAUX | `gateway/routes.rs:136-145`, `status.rs:61-77` |
| 22 | Comptes/login/multi-appareils | FAUX | `main.rs:63-213` |
| 22a | Jeton d'appairage bearer 256 bits, TTL 5 min, usage unique | VRAI (MVP) | `gateway/src/routes.rs`, `gateway/src/lib.rs` |
| 23 | QR natif, deep links mobile et passkeys | FAUX | `pair_qr` renvoie un deep link texte ; pas d'image QR, d'intent mobile ni de WebAuthn |
| 24 | Dashboard global de métriques | FAUX | `node.rs:451-508`, `:556-628` |
| 25 | Gateway HTTPS publique intégrée | FAUX | `gateway/main.rs:19-40`, `gateway/lib.rs:84-109` |
| 26 | Déploiement web automatisé | FAUX | `.github/workflows/ci.yml`, `release.yml` |
| 27 | Site généré depuis la documentation | FAUX | `web/index.html`, `web/landing.html`; aucun générateur trouvé |
| 28 | Aucun serveur / logs exclusivement clients | FAUX | `relay.rs:94-135`, `:231-278` |
| 29 | Bug bounty rémunéré | FAUX | `LEGAL.md:79` |
| 30 | Déploiement public/TLS effectif | INDÉTERMINÉ | aucun workflow de déploiement ; hébergeur externe non inspecté |
| 31 | Réseau validé entre hôtes physiques distincts | INDÉTERMINÉ | tests locaux seulement ; campagne multi-hôtes absente |
| 32 | PWA testée sur iOS/Android | INDÉTERMINÉ | fichiers web présents ; résultats appareil absents |
| 33 | Disponibilité/latence/100k utilisateurs | INDÉTERMINÉ | benchmark de charge correspondant absent |

## Commandes de vérification utilisées

Les recherches ont été faites sur le dépôt courant, sans modifier de code Rust :

```bash
rg -n 'pub fn shards|pub fn reassemble|pub fn tally|pub fn verify_receipts|pub fn plan|PipelinePlan|ServerlessRequest::new|serverless::' crates/serverless crates/petals crates/client/src
rg -n 'pub fn run_wasm|run_wasm\(|wasmi::|systemd-run|MemoryMax|CPUQuota|NoNewPrivileges|ProtectSystem|PrivateNetwork' crates/client/src daemon/src
rg -n 'kill.?switch|kill_switch|gpio|keyboard|USB|usb|Duress|duress' crates/client/src daemon/src
rg -n 'pair_request|pair_accept|pair_qr|node_info|snapshot|latency_ms|last_seen' crates/gateway/src
rg -n 'serviceWorker|beforeinstallprompt|manifest|navigator\.getBattery|Battery|WebAuthn|passkey|polygone://' web
rg -n 'Login|login|enrollment|WebAuthn|passkey|qr|Qr|Pair|pair' crates/client/src/main.rs crates/client/src web
rg -n 'bytes_sent|bytes_received|messages_sent|messages_received|files_sent|files_received|served_bytes|served_blocks' crates/client/src daemon/src crates/gateway/src
rg -ni 'bug bounty|bounty program|prime de bug' . --glob '!target/**' --glob '!Cargo.lock'
```

## Point adjacent corrigé pendant cette passe

La commande `polygone id` créait un `NodeId::random()` à chaque exécution, alors que le `node_id` du relay est dérivé de la clé KEM persistante (`crates/client/src/net/mod.rs:105`). Elle affiche maintenant `net::node_id(&identity)` (`crates/client/src/main.rs`, branche `Commands::Id`). Le contrôle CLI avec un `HOME` temporaire est effectué après compilation : deux invocations doivent afficher exactement le même identifiant, puis une nouvelle identité doit en afficher un autre.
