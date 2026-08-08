# CHANGELOG — Polygone

> *Style : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).*
> *Semver : voir `Cargo.toml`.*
> *Dernier format : 2026-08-07.*

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
  commande `polygone *`), statut honnête mis à jour (108 tests uniques,
  ML-DSA branché, relay durci, anti-rejeu).
- **`docs/cli.md`** — les 3 nouvelles commandes + `peers.json` documentés.
- **`docs/PREMIER-SOIR.md`** — pointe vers `premier-soir` comme répétition
  solo du protocole, checklist alignée sur 108 tests.

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
