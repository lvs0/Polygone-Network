# CHANGELOG — Polygone

## [Unreleased] — 2026-09-02 — Polygone Hide (Phase 1 MVP) livré

### Added
- **`polygone hide`** — SOCKS5 listener sur `127.0.0.1:9050`, négociation RFC 1928 (no-auth, CONNECT), encapsulation dans le pipeline crypto existant (ML-KEM-1024 + AES-256-GCM + ML-DSA).
- **`polygone ecouter --hide`** — exit node : reçoit la demande CONNECT chiffrée, établit la connexion TCP réelle vers la destination, renvoie le flux bidirectionnel via le relay.
- **Tests d'intégration réels** — `scripts/hide-smoke.sh` : 2 nœuds locaux (exit node + client), `curl --socks5` à travers le proxy → réponse reçue ; audit relay : zéro contenu en clair.
- **Documentation** — `docs/HIDE-SPEC.md` (spécification complète), `THREAT_MODEL.md` section Hide (tradeoffs vs Tor honnêtes), `STAGING.md` statut 🟢 Live.
- **README mis à jour** — commande `hide` documentée, exemple `ecouter --hide`, statut Hide 🟢.

### Changed
- **STAGING.md** : `hide` promu de ⚪ Staging à 🟢 Live (Phase 1 MVP).
- **THREAT_MODEL.md** : section Hide ajoutée (adversaire type, ce qui est protégé, ce qui ne l'est pas, table tradeoffs vs Tor).

### Verification
- `cargo test --workspace` → 113 tests verts
- `cargo clippy --workspace --all-targets -- -D warnings` → 0 warning
- `./scripts/smoke-commands.sh` → TOUT VERT
- `./scripts/hide-smoke.sh` → TOUT VERT

---

## [1.0.0-rc2] — 2026-08-12 — branding unifié, décisions tranchées

### Changed
- **Version alignée sur SPEC 1.0.0** : `2.0.0-rc2` → `1.0.0-rc2` (Cargo.toml, README, docs). La notion « V2 » est retirée du branding visible — le code est le produit, pas le numéro.
- **Décisions D1/D2/D4/D7/D9 tranchées** (voir DECISIONS.md).
- **Archivage `time_sync/`** (1 019 LOC, 0 consommateur) → `archive/2026-08-time_sync/`.

---

# CHANGELOG — Polygone

> *Style : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).*
> *Semver : voir `Cargo.toml`.*
> *Dernier format : 2026-08-08.*

---

## [Unreleased] — 2026-08-08 — déterminisme, preuves système, D8

### Added

- **`polygone envoyer --stdin`** — le message passe par le stdin (symétrique
  de `recevoir -`) : il n'apparaît jamais dans l'historique shell.
  Recommandé par `docs/kill-switch.md`, désormais réel — round-trip
  prouvé en CI (zéro argument en clair).
- **Quickstart corrigé** — `envoyer … > wire.txt` : la forme FICHIER du
  transport (`recevoir wire.txt`) était documentée sans la redirection
  qui la produit ; round-trip fichier désormais prouvé en CI (16e gate
  smoke).
- **Mesh prouvé** — `annoncer` + `voisins` (découverte LAN, UDP 7642) :
  la promesse README « Live » n'avait aucune preuve système. 17e gate
  smoke : fatal si crash, découverte réelle vérifiée localement
  (3/3 déterministe), avertissement documenté si réseau isolé (jamais
  un faux rouge CI).
- **`docs/observation-premier-soir.md`** — le carnet d'observation du
  Premier Soir : le modèle complet (identité du soir, grille des 5
  questions, 3 preuves, verdict collectif, résidu social, 4 métriques),
  zéro résultat fictif, prêt à remplir et commiter.
- **Preuves système en CI** — chaque promesse du README tourne sur les
  binaires du commit :
  - `scripts/forensic-drive.sh` — relay + 2 clients RÉELS, fichier
    chiffré + fragmenté Shamir 4/7, comparaison octet à octet, relay
    stateless (job CI `drive`).
  - `scripts/smoke-commands.sh` — test 7/7, verite, carte, premier-soir,
    demo, round-trip stdin, duress 4→0, versions ×3, doctor, config
    legacy (job CI `smoke`).

### Changed

- **La CI ne ment plus (Phase 3.1)** — `-A clippy::all` retiré (le gate
  clippy ne pouvait jamais rougir), mode parallèle `cargo test --workspace`
  (celui exact de la CI), check MIT réparé, ~26 lints corrigés, garde
  Axiome 4 honnête (D6 — le produit reste mesuré, la rigueur sort du
  compteur).
- **Supply-chain** — actions tierces épinglées au SHA (dtolnay,
  Swatinem/rust-cache, softprops/action-gh-release, upload-artifact) +
  commentaire de traçabilité (tag + date) : plus de tag mobile.
- **Axiomes exécutables en CI** — Axiome 2 (`parse_known_commands` : le
  TUI tient ses deux tons), Axiome 5 (duress détruit réellement 4 → 0
  fichiers dans le smoke).
- **Le 4ᵉ binaire prouvé** — `polygoned doctor` (diagnostics, exit 0,
  HOME éphémère) rejoint le smoke + job CI.
- **Versions honnêtes** — les 4 binaires affichent
  `env!("CARGO_PKG_VERSION")` (fini relay 0.1.0 / daemon 0.3.0 en dur) ;
  gate de version dans le smoke — il a attrapé un binaire stale qui
  mentait encore.
- **Compteurs alignés sur la réalité** — 109 tests partout (README,
  PREMIER-SOIR, ARCHITECTURE, threat-commodity, threat-high-value).
- **README TTL précisé** (audit des promesses) — les fragments meurent
  immédiatement au relay (stateless, drop) ; le TTL 30 s appartient au
  scénario `premier-soir`. Aligné dans `verite` et threat-high-value.

### Fixed

- **Config daemon lisible (D8)** — `polygoned --gen-config` écrit ce qu'il
  lit (sérialisation serde), ET les configs legacy (`[tier]` en table,
  `[platform]` ignoré) se lisent avec les defaults PRODUIT (planchers
  réels, toggles activés — jamais de zéros silencieux). `polygoned
  status` fonctionne sur la config réelle de la machine (avant : TOML
  parse error).

### Security

- **Duress étendu (Phase 2.5)** — `peers.json` (ancres TOFU : la trace
  relationnelle, qui on a contacté) détruit avec l'identité ; 4 cibles,
  test HOME éphémère, couverture documentée dans THREAT_MODEL.md +
  threat-high-value.md.

### Known (attente Lévy)

- **D7** — première CI réelle sur GitHub : `main` jamais poussé
  (53 commits post-rc2), en attente de décision + credentials.
- **Premier Soir réel** — 3 personnes de confiance, un soir, un carnet
  commité : le seul chiffre qui compte.

---

## [Unreleased] — 2026-08-07 — produit++ (la promesse devient une commande)

### Added
- **`polygone verite`** — forensique locale : énumère tout ce que le nœud
  garde (identité, ancres peers.json, fichiers reçus, scores), le classe,
  puis rend le verdict « voici ce que j'ai de toi : rien ». La confiance
  devient une interaction, pas une lecture de README.
- **`polygone premier-soir`** — le scénario guidé de 5 minutes : carte →
  7 fragments naissent → TTL réel qui tourne (défaut 30 s, `--ttl` pour
  réduire) → 4/7 reconstruisent → `verite` → carnet d'observation. La
  promesse « Le message meurt. Regarde. » devient une commande.
- **`polygone carte`** — la clé comme objet social : identité encadrée
  (pseudo, empreintes ML-KEM/ML-DSA, adresse ⬡), à échanger en personne.
- **`docs/BUDGET.md`** — la soutenabilité du relay noir sur blanc
  (€/mois réel, sources : poche, grants NLnet/Prototype Fund, lettres D3,
  donate-free sans tracker, plan d'arrêt propre).

### Changed
- **README réécrit autour de la promesse unique** — « Le message meurt.
  Regarde. » en tête, règle produit++ énoncée (toute promesse = test CI ou
  commande `polygone *`), statut honnête mis à jour (109 tests uniques,
  ML-DSA branché, relay durci, anti-rejeu).
- **`docs/cli.md`** — les 3 nouvelles commandes + `peers.json` documentés.
- **`docs/PREMIER-SOIR.md`** — pointe vers `premier-soir` comme répétition
  solo du protocole, checklist alignée sur 109 tests.

---

## [Unreleased] — 2026-08-07 — produit++ (Phase 0 : la vérité des docs)

### Added
- **RES — exécution WASM** (`:wasm <fichier>`) — module wasm32-wasi exécuté
  dans le sandbox `wasmi` natif, sortie capturée (Phase 8 de la SPEC livrée).
- **RES — couche de réputation** — le ledger de confiance des nœuds
  fantômes : `grant_for()` verrouillé par test, routage des grants.
- **`:executer` / `compute --executer`** — exécution shell sandboxée du
  nœud fantôme (`systemd-run --user`, MemoryMax/CPUQuota/PrivateNetwork).

### Changed
- **`src/` monolithe archivé** → `archive/2026-07-src/` (11 356 LOC mortes :
  libp2p, relay HTTP, ledger POLY, vieille TUI). Le workspace ne le compile
  plus. Rien n'est perdu — README d'archive inclus.
- **`ARCHITECTURE.md` réécrit** sur l'architecture réelle (4 crates) avec
  ses lacunes honnêtes (ML-DSA non branché, relay métadonnées, daemon.sock
  sans lecteur, time_sync sans consommateur).
- **Axiomes exécutables** — PHILOSOPHY.md : Axiome 2 → `cargo test
  parse_known_commands` ; Axiome 3 → `grep -ci` insensible à la casse ;
  Axiome 4 → coupe documentée + garde `wc -l crates/client ≤ 5000`.
- **Licence unifiée : AGPL-3.0** (README, PHILOSOPHY, docs alignés sur
  `Cargo.toml`). MIT supprimé des docs.

### Security (Phase 1 — le transport)
- **ML-DSA-65 branché au réseau** : chaque message est signé
  (`session‖from‖to‖kem_ct‖ciphertext`), vérifié avant déchiffrement,
  fail-closed. Ancre `known_peers` : une clé connue ne peut pas être
  usurpée (TOFU documenté au premier contact).
- **Nom de fichier hors-bande** : chiffré avec la clé de session
  (`name_ct`) — le relay ne voit plus les noms.
- **Relay durci** : cap 64 KiB/ligne, rate-limit 200 env/s, `from` doit
  égaler le HELLO (anti-usurpation), table shardée 16×.
- `process_line` : vérifie `to`, drop les idx dupliqués, session OsRng.

### Security (Phase 2 — la machine)
- **Sandbox RES verrouillée** : `ProtectHome`, `InaccessiblePaths=
  ~/.polygone` (identité illisible), `PrivateDevices`, `SystemCallFilter`,
  2 exécutions max en parallèle.
- **WASM fuel metering** : une boucle infinie trappe au lieu de geler le
  nœud.
- **Timeout réel** : l'unité transitoire `systemd-run` est arrêtée
  (`systemctl --user stop`) — plus d'orphelins.
- **Duress étendu** : détruit aussi `reputation.json` (traces RES).
- **Portillon de réputation** au ghost : refuse les demandeurs à mauvaise
  réputation locale.
- Zeroize : limitation pqcrypto documentée honnêtement (bytes non
  mutables), l'effacement réel passe par `duress`.

### Security (Phase 4 — contre-attaque : 6 failles trouvées par le loop)
- **Ancre de confiance réelle** : `~/.polygone/peers.json` chargé une
  fois, TOFU appris après le premier message vérifié, empreinte affichée
  pour vérification hors-ligne, clé différente pour un `from` connu =
  rejet. (known_peers n'était plus du code mort.)
- **combinations4 borné** : idx de fragment validé 1..=7, ≤7 fragments
  bufferisés → C(7,4)=35 max (fini les 8,8 Md de vérifs ML-DSA).
- **Sessions bornées** : MAX_SESSIONS=1024 fail-closed + purge TTL 300 s.
- **Grant vérifié** : to/session/from contrôlés avant d'accepter un grant
  (fini l'empoisonnement des résultats RES).
- **Anti-replay** : horodatage signé (canonical v2), fenêtre ±300 s,
  cache des sessions complétées.
- **Anti-confusion** : clé de session `from|session`, second KEM
  d'identité différente rejeté, fragments liés au `from` du KEM.
- **Relay anti-squatting** : un node_id déjà connecté n'est pas écrasé,
  ack `HELLO_OK`/`HELLO_DENIED`, plafond MAX_CONNECTIONS=1024.

### Security (Phase 4 — contre-attaque, 2e vague : findings exec perdus)
- **WASM ne gèle plus** : fuel réduit (1e8), `EnforcedLimits::strict`,
  sortie bornée à 8 Ko pendant l'écriture (CappedWriter), exécution hors
  de l'event loop (`spawn_blocking`), garde de concurrence partagée.
- **Canal RES authentifié** : requêtes signées ML-DSA + vérifiées par le
  fantôme (fraîcheur, ancre, TOFU) ; **grants signés** + vérifiés par
  l'emprunteur (un relay malveillant ne forge plus de grant) ;
  **réputation réellement enregistrée** côté fantôme (échec sur requête
  non authentifiée) — le portillon n'est plus du code mort.
- **Pas d'orphelins** : `RuntimeMaxSec` posé sur l'unité transitoire +
  `systemctl stop --wait` — même si le client meurt (SIGKILL, duress),
  la tâche non fiable est tuée par le manager.
- **Canal RES documenté non confidentiel** : les tâches/sorties RES
  transitent en clair sur le relay (contrairement aux messages).
- **README rc2** — version, statuts services, test count (89), commandes
  réelles.

### Known (assumé, documenté)
- ML-DSA-65 généré mais **pas encore signé/vérifié** au chemin réseau.
- Relay : voit les métadonnées de routage (from/to/tailles/name), HELLO
  non authentifié, pas de limites — Phase 1.
- CI GitHub : à valider (51 commits post-rc2 jamais vus par la CI).

---

## [2.0.0-rc2] — 2026-08-06 — le produit

### Highlights

La reconstruction v2 passe du workspace technique au **produit** :
une commande unique `polygone`, une TUI à deux commandes (D1 GO),
une démo E2E post-quantique qui *prouve* « on voit rien », un
installateur 1-clic et une landing page. Le core tient enfin la
promesse du SPEC : crypto post-quantique complète et testée.

### Added (produit)

- **D1 GO** — TUI 2 onglets (`:envoyer` / `:quitter`), style vim,
  événementielle (aucun polling), écran d'accueil avec identité/uptime.
- **`polygone`** — binaire produit unifié (TUI par défaut ; sous-commandes
  `demo`, `envoyer`, `recevoir`, `clef`, `ecouter`, `id`).
- **Réseau réel (plane 2)** — relay TCP aveugle qui route les fragments
  (`HELLO <node_id>`, NDJSON, ne lit que kind/to/session) + client
  `ecouter` / `envoyer --via <relay> --a <node>`. Validé en conditions
  réelles : Alice → relay → Bob, déchiffré avec 4/7 fragments.
- **Drive** (2ᵉ service livré) — `envoyer --fichier` : un fichier chiffré,
  fragmenté 4/7, traverse le relay ; le destinataire le reçoit dans
  `~/.polygone/received/` (contenu vérifié identique).
- **Kill-switch réel (Axiome 5)** — `polygone duress [--confirmer]` :
  détruit identité + fichiers reçus, régénération au prochain lancement.
- **Petals (D4 pilot)** — `polygone petals status/models/ask` : IA locale
  via Ollama (défaut 127.0.0.1:11434, `POLYGONE_OLLAMA_URL`), client HTTP
  minimal sans dépendance (décodage chunked), zéro cloud.
- **`polygone test`** — self-test crypto réel (7/7 : ML-KEM, AES-GCM,
  BLAKE3 KDF, Shamir 4/7 + 3/7, ML-DSA sign/verify + tamper), exit 0
  uniquement si tout est vert.
- **Mesh (Phase 4)** — `polygone voisins` / `annoncer` : découverte LAN par
  UDP broadcast (7642), PING avec port de réponse, zéro dépendance.
- **Envoi zéro configuration** — `ecouter --annoncer` + `envoyer --a <node>`
  sans `--via` : le relay du destinataire est trouvé sur le LAN.
- **RES — nœuds fantômes** — `polygone compute` : RAM libre locale + carte
  des nœuds du LAN qui annoncent leur compute (l'idée « coup de génie »
  des notes Bear, socle du prêt P2P).
- **RES — prêt de compute** — `compute --emprunter <node> --via <relay>` :
  requête via le relay aveugle, le fantôme (`ecouter --compute`) répond
  par un grant (RAM disponible). Protocole live, exécution staging.
- **TUI complète** — `:envoyer :recevoir :voisins :compute :ia :demo
  :clef :statut :quitter` — tout le produit est dans les 2 commandes
  + le `:` (Axiome 2).
- **Messagerie E2E réelle** (`crates/client/src/msg.rs`) — ML-KEM-1024 →
  BLAKE3 KDF → AES-256-GCM → Shamir 4/7 ; format filaire
  `KEM_CT/SENDER_PK/FRAG` interopérable.
- **Identité persistante** (`~/.polygone/identity.json`, chmod 600) —
  clés ML-KEM + ML-DSA générées au premier lancement, pseudo 3 syllabes.
- **Démo E2E** (`polygone demo`) — relay aveugle, audit « on voit rien »,
  simulation d'adversaire (3/7 et 7/7 sans clé).
- **Installateur 1-clic** (`scripts/install.sh`) — SPEC §5, détection
  OS/arch, binaire précompilé (GitHub release) + fallback build source.
- **Landing page** (`site/index.html`) — DESIGN_SYSTEM : slate + ambre,
  hexagone, suspense typographique, contraste honnête.
- **Workflow release** (`.github/workflows/release.yml`) — tarballs
  `polygone-<os>-<arch>.tar.gz` publiés par tag `v*`.

### Added (crypto — polygone-core)

- `crypto/kem.rs` — ML-KEM-1024 (FIPS 203) ; `crypto/shamir.rs` —
  4-of-7 ; `crypto/symmetric.rs` — AES-256-GCM ; `SharedSecret` + KDF
  BLAKE3 domain-séparée. Le SPEC §2 est enfin réel dans le workspace v2.
- `sign.rs` (ML-DSA-65) réparé — le build était rouge, il est vert.

### Added (docs / décisions)

- `docs/threat-commodity.md` + `docs/threat-high-value.md` — livrables S2.
- `docs/kill-switch.md` v1.0 — runbook opérateur (avant/pendant/après).
- `DECISIONS.md` D2 — données du bench enregistrées : sign ~265 µs
  (goulot), verify ~79 µs, ~2900 handshakes/sec/cœur ; gate 200 µs non
  atteint, capacité non-bloquante ; décision finale à Lévy.

### Internal

- Workspace version synchronisée sur `2.0.0-rc2`.
- 0 warning sur tout le workspace (`cargo check --all-targets`).
- `cargo test --workspace` → **71 tests**.

### Verification

- Boucle E2E vérifiée entre deux identités distinctes : Alice envoie à
  la clé publique de Bob, Bob reconstruit (4/7) et déchiffre.
- TUI testée en pseudo-TTY : démarre, rend l'accueil, sort sur `:q`.

---

### Highlights

Premier **release candidate** de Polygone. La liste de features promise
est coupée de 8 à 2 (`msg` + `drive`), le langage visuel devient
ambre-tactile (Ive), la tagline poétique reçoit sa footnote technique côte
à côte (Orwell), et **6 services sont archivés publiquement** avec
conditions explicites de ré-introduction (Musk + Gödel).

### Added

- `PHILOSOPHY.md` — 5 axiomes appliqués (poétique + technique côte à côte).
- `DESIGN_SYSTEM.md` — couleurs/typo/tactilité/suspense typographique.
- `THREAT_MODEL.md` — split commodity vs high-value (Assange × 2).
- `COUNCIL_DECISIONS.md` — synthèse des 22 recommandations du Conseil des Sages 2026-06-29.
- `STAGING.md` — 6 services archivés (compute, hide, mesh, brain, petals, shell) avec conditions de ré-introduction.
- `DECISIONS.md` — 3 points Lévy-blocking (D1 refonte UI, D2 bench Sybil, D3 lettre État).
- `README.md` — manifesto revisé : 2 services, posture `honesty-first`.
- `web/index.html` — tagline poétique + footnote technique côte à côte ; badge version v2.0.0-rc1.
- `Cargo.toml` — version bump 1.0.1 → 1.0.0-rc1.

### Internal

- Aucune modification structurelle du code dans cette release.
- Tous les changements structurels sont des *documents*.
- Modèle hub-and-spoke : `README.md` → `PHILOSOPHY.md` + `THREAT_MODEL.md` + `LEGAL.md` + `COUNCIL_DECISIONS.md` + `DESIGN_SYSTEM.md` + `STAGING.md` + `DECISIONS.md`.

### Verification

- `cargo check --offline` (à exécuter après édition) — 0 warning, 0 erreur attendu.

### Cross-référence

- Conseil des Sages 2026-06-29 (3 comités) — voir `COUNCIL_DECISIONS.md`.
- Roadmap 8 semaines — voir `POLYGONE_ROADMAP_v2.md` (à la racine `/home/l-vs/`).

---

## [0.2.0] — 2026-06-29 — quick-win S1

### ⚠ BREAKING CHANGES

- **ML-DSA-87 → ML-DSA-65.** Le module `polygone::crypto::sign`
  utilise désormais ML-DSA-65 (signature post-quantique FIPS 204) au lieu
  de ML-DSA-87, sur l'ensemble du projet.
  - **Toutes les clés de signature et tous les *workvouchers* karma
    persistés sur disque avant cette version seront INVALIDES.**
    Le chargement d'une identité pré-0.2.0 échouera avec
    `PolygoneError::Serialization("Invalid Sign PK")`.
  - **Tailles :** pk 2592→1952 B ; sk 4896→4032 B ; signature 4627→3309 B.
  - **Mitigation :** régénérer une identité via `polygone keygen` après upgrade.
  - **Justification :** ML-DSA-65 est le sweet spot Galois (Comité 2
    Conseil des Sages 2026-06-29) — handshake P2P-friendly, signatures
    plus courtes, vérification plus rapide, marge de sécurité pourtant
    adaptée à 2031+.

### Added

- `LEGAL.md` — posture légale de Polygone (subpoena, mode duress, disclosure).
- `.well-known/security.txt` — RFC 9116 (contact PGP-signed pour disclosure).
- `docs/kill-switch.md` — mode duress (Mitnick framing, sans détail d'implémentation).
- `CHANGELOG.md` — ce fichier.

### Internal

- `src/crypto/sign.rs` — toutes les références `mldsa87::` passent à
  `mldsa65::` (11 sites via str_replace).
- `src/crypto/karma.rs` — docstring L18 mise à jour.
- `Cargo.toml` — commentaire L35 mis à jour, dépendance inchangée.

### Verification

- `cargo check --offline` — 0 warning, 0 erreur (env. 1 min).
- `cargo test --lib` — tests existants + nouvelle assertion
  `#[test] signature_size_mldsa65()` (3309 B attendu).

---

## [0.1.x] — antérieurs

Versions pré-0.2.0. ML-DSA-87. Identités et workvouchers **incompatibles**
avec 0.2.0+.
