# ECOSYSTEM.md — the mother file

> *Si tu ne sais pas où regarder, c'est ici.*
>
> **Mis à jour 2026-10-16 (audit de cohérence).** This document is the
> single source of truth for **what Polygone is, what services it ships
> with, and what each one does**. Every other document references back to
> this one. If a doc contradicts this file, this file wins — and the other
> doc is wrong.

---

## 1. The three planes (reality, v2)

Polygone is a **3-plane system**. Anything you do with Polygone lives in
exactly one of these:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   PLANE 1            PLANE 2             PLANE 3                │
│   ────────           ────────            ────────               │
│                                                                 │
│   YOUR               THE LAN             THE RELAY              │
│   COMPUTER           (MESH)              (polygone-relay)       │
│                                                                 │
│   • crypto pipeline  • UDP broadcast     • TCP, NDJSON          │
│     offline            port 7642           stateless            │
│   • identity.json    • announces         • routes on `to`       │
│   • received/          node+relay        • in-memory only       │
│   • TUI + CLI        • zero-config       • never persists       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

- **Plane 1 — Your Computer.** The `polygone` command: offline crypto
  (envoyer/recevoir), identity, received files, TUI, mesh announce,
  RES execution, duress.
- **Plane 2 — The LAN Mesh.** Peers you can reach directly. UDP
  broadcast discovery (port 7642) announces node_id + relay. Enables
  zero-config sends (`ecouter --annoncer` + `envoyer --a <node>`
  without `--via`).
- **Plane 3 — The Relay.** A `polygone-relay` you use when the peer is
  not on your LAN. It routes fragments to connected node_ids. It sees
  **routing metadata** (`from`, `to`, `session`, sizes). It does not see
  message/file plaintext or file names (E2E-encrypted); **Compute is an
  exception**: its request and grant JSON bodies are signed but not encrypted
  E2E, so the relay can read the task, WASM payload, and response; the
  current TCP path has no TLS, so on-path observers can read them too.
  It holds no state to disk: restart = full amnesia.

### Product boundary: Network, Compute and HYPER

**Polygone-Network** owns transport, peer identity and end-to-end
encryption. **Polygone Compute** is the application-integration point; its
current execution features do not make it the global resource scheduler.
**HYPER** remains a separate resource-orchestration roadmap. Any connection
between Compute and HYPER must go through a versioned API contract that is
still to be defined; neither roadmap is merged here. See the non-normative
[HYPER–Polygone boundary proposal](./docs/HYPER-INTEGRATION-BOUNDARY.md).

---

## 2. Service registry (workspace version 2.0.0)

The registry below lists 5 live services and 1 partial MVP. The 24-path
validation snapshot SHA-256 `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`
reports 206/206 workspace tests, strict Clippy, fmt, release build, CLI smoke
and forensic loopback passing; Tilleul independently confirms Client 134/134
and no remaining P1/P2 TOFU defect. The Windows-only first-bootstrap test is
present but not compiled/executed, so Windows behavior is not proven. Compute
JSON remains readable by the relay and TCP observers without TLS; Modal has
not run and would not prove physical-host interoperability. Two services remain
staging. The Ghost Node deployment draft is not counted as shipped: its
launcher invokes CLI commands and flags not exposed by the current client.

| ID | Name | Role | Statut |
|----|------|------|--------|
| `msg` | Polygone Msg | Messages E2E éphémères via relay (4/7) | 🟢 **Live** |
| `drive` | Polygone Drive | Fichiers E2E via relay (4/7) → `~/.polygone/received/` | 🟢 **Live** |
| `brain` | Polygone Brain | IA locale (`polygone petals` → Ollama, zéro cloud) | 🟢 **Live** |
| `mesh` | Polygone Mesh | Découverte LAN + envoi zéro-config (UDP 7642) | 🟢 **Live** |
| `compute` | Polygone Compute | Lend/borrow sandboxé (shell + WASM); demandes/grants lisibles au relay; grant ciblé clos; validation 24 chemins `33626ec…` rapporte workspace 206/206 et Client 134/134 revu; P1/P2 TOFU corrigés; test Windows parent absent présent mais non exécuté; Modal non lancé; E2E et interopérabilité physique non validés | 🟡 **MVP partiel — limites E2E/inter-hôtes** |
| `hide` | Polygone Hide | Proxy SOCKS5 via relay, mono-saut; aucune garantie d’anonymat comparable à Tor | 🟢 **Live (MVP)** |
| `ghost` | Ghost Node | Ancien brouillon de nœud permanent; le lanceur ne correspond pas au CLI courant | ⚪ **Non livré / guide à reprendre** |
| `petals-distribué` | Distributed LLM | Shards d'inférence sur les pairs | ⚪ Staging |
| `shell` | Polygone Shell | Shell sécurisé peer-to-peer | ⚪ Staging |

Conditions de ré-introduction des services ⚪ : voir
[`STAGING.md`](./STAGING.md). **Un service ⚪ n'est pas annoncé comme
existant** — la TUI ne l'affiche pas, le README ne le vend pas.

---

## 3. The offline pipeline (msg.rs)

```
plaintext
  │  ML-KEM-1024 encapsulate (clé publique du destinataire)
  ▼
kem_ct + shared secret
  │  KDF BLAKE3 (domain-separated "polygone session key v1")
  ▼
32-byte key
  │  AES-256-GCM (nonce 96 bits aléatoire)
  ▼
ciphertext
  │  Shamir 4-of-7
  ▼
7 fragments → wire text "KEM_CT:/SENDER_PK:/FRAG:"
```

`polygone envoyer -d <clef> "message"` produit le wire text ;
`polygone recevoir wire.txt` reconstruit (≥4/7) et déchiffre.
La clé de session est `ZeroizeOnDrop`.

---

## 4. The network pipeline (net.rs — plane 2/3)

Wire contract — NDJSON over TCP :

```json
{"kind":"fragment","from":"<node_id>","to":"<node_id>","session":"<hex>",
 "seq":0,"type":"kem"|"frag","idx":0,"threshold":4,"total":7,
 "payload":[...],
 "sig":"<ML-DSA-65 signature — KEM envelope>",
 "signer":"<ML-DSA pk hex — KEM envelope>",
 "name_ct":"<nom de fichier chiffré par la clé de session — enveloppe KEM d'un fichier>"}
```

Handshake : `HELLO <node_id>\n`. `node_id` = 16 premiers hex de la clé
KEM publique (stable — c'est ce qui permet d'être retrouvé).

- `envoyer --via <relay> --a <node> [--fichier]` : 1 enveloppe KEM
  (signée + nom chiffré) + 7 fragments → le relay route sur `to`.
- `ecouter <relay>` : buffer par session, ≥4/7 → vérification de la
  signature ML-DSA (**« c'est bien Alice »**) → reconstruction →
  déchiffrement → message affiché / fichier écrit dans
  `~/.polygone/received/`.

**Honnêteté du relay (assumée et documentée, pas cachée) :** le relay
voit les métadonnées de routage (from/to/session/tailles) mais **plus
les noms de fichiers** (hors-bande, chiffrés). Il ne voit jamais le
contenu. Le modèle de menace est dans `docs/threat-commodity.md` et
`docs/threat-high-value.md`. La promesse « zero-knowledge » porte sur le
**contenu** ; les métadonnées de routage sont le prix du routage —
réduites (noms hors-bande) et documentées.

---

## 5. The local product (plane 1)

### Identity

`~/.polygone/identity.json` (clés en clair au repos; chmod 600 est
best-effort) : pseudo + clés ML-KEM-1024 et ML-DSA-65. `polygone clef` =
clé publique à partager. `polygone duress --confirmer` = destruction identité
et fichiers reçus (Axiome 5). `peers.json` ne lie qu’un `node_id` réseau
tronqué à une clé de signature. Sur le snapshot `33626ec…`, Tilleul confirme la
correction des deux défauts initiaux du store : rollback après `replace` qui
modifie puis échoue, et détection du backup orphelin par `verite`. Le nouveau
test Windows parent absent est présent mais n’a pas été compilé/exécuté; la
portabilité Windows n’est pas démontrée. Modal reste suspendu et aucune
durabilité universelle après crash n’est établie (voir `RELEASE-STATUS.md`).
`Pair --qr` construit une charge utile JSON interne `polygone-pair` avec le
`node_id` et la clé publique KEM, pas la clé ML-DSA ni une liaison
authentifiée; aucun QR n’est rendu et aucun consommateur du JSON n’a été
trouvé. Cette charge utile sérialise le `node_id` réseau de 16 caractères en
32 caractères par double-hex; c’est un défaut interne confirmé, pas la preuve
qu’un flux utilisateur opérationnel est cassé. Elle n’authentifie donc pas le
bundle complet. Il n’existe pas encore de commande CLI
prise en charge pour vérifier une empreinte canonique complète hors bande; le
TOFU réseau seul ne prouve pas la liaison entre les clés.
Un bundle des deux clés complètes fait 3 520 octets binaires, au-delà de la
capacité V40-L d’un QR standard (2 953 octets avant métadonnées). Une carte
QR v2 unique n’est donc pas promise; deux QR compacts ou un transfert
multi-QR restent des options non arbitrées, avec empreinte complète affichée
séparément pour comparaison indépendante.
Les parcours CLI, TUI et Gateway n’utilisent pas encore un format d’identité
partagé. L’inventaire Gateway rapporte un token `poly_` de 8 lettres, conservé
en mémoire sans expiration ni consommation observées; `/pair/accept` n’utilise
pas ce token comme autorisation. Le flux paraît incomplet, mais aucune prise de
compte ni vulnérabilité n’est démontrée; la revue indépendante reste en attente
(voir `RELEASE-STATUS.md`).
L’audit en lecture seule signale aussi l’absence observée de framing/plafond
explicite du corps HTTP, de limite de concurrence et de bornes/purge des maps;
ce sont des constats à confirmer, pas des limites corrigées. Tests et seuils
restent à décider. Des seuils candidats figurent dans `RELEASE-STATUS.md` à
titre d’hypothèses seulement; ils ne sont ni mesurés ni adoptés.
Le binaire Gateway du dépôt fixe son bind à `127.0.0.1:8787`; aucune exposition
réseau directe n’est démontrée dans ce mode. L’API de bibliothèque accepte
toute adresse fournie par l’intégrateur et reste en HTTP clair; la portée
externe dépend alors de son choix et d’éventuels proxies hors dépôt.
Une sémantique v2 reste à décider; la proposition non normative recommande un
pin d’identité explicite après comparaison hors bande, sans accorder de droits
Compute par le pairing, et sans faire du token une autorisation à lui seul.

### TUI

Style vim, deux commandes de premier niveau : `:envoyer` / `:quitter`.
Le reste derrière `:` — `:recevoir :clef :voisins :compute :ia :demo
:executer :wasm :statut :aide` (Axiome 2 : deux tons, pas trois).

### Produit++ — la promesse comme commande

Trois commandes transforment la promesse centrale en expérience exécutable
(règle produit++ : toute promesse du README est un test CI ou une commande
`polygone *`) :

- **`polygone premier-soir`** — le scénario guidé : la carte → 7 fragments
  naissent → TTL réel qui tourne → 4/7 reconstruisent → `verite` → carnet
  d'observation. « Le message meurt. Regarde. »
- **`polygone verite`** — forensique locale : énumère tout ce que le nœud
  garde et rend le verdict (« voici ce que j'ai de toi : rien »).
- **`polygone carte`** — la clé comme objet social encadré, à échanger en
  personne. Le résidu social EST le produit.

### RES — exécution

`polygone compute --emprunter <node> --via <relay>` → grant du nœud
fantôme (`ecouter --compute`) → `--executer <tâche>` (shell sandboxé
`systemd-run --user`) ou `:wasm <fichier>` (wasmi/WASI).

### Ghost Node — brouillon de déploiement, non livré dans le workspace actuel

Cette section décrit un concept historique, pas un service opérationnel
de la version courante. Le script `docs/deployment/ghost-node.sh` invoque
`polygone keygen`, `polygone ecouter --port/--inbox` et
`polygone annoncer --bootstrap/--alias/--key/--addr`; ces sous-commandes
et options ne sont pas présentes dans `crates/client/src/main.rs`. Le
Dockerfile et le guide associé ne prouvent donc ni un Ghost Node déployable
ni une inbox persistante compatible avec le client actuel.

Le guide et le lanceur sous `docs/deployment/` sont conservés comme
références historiques uniquement; ne pas les exécuter comme procédure de
déploiement du client 2.0.0. Pour rétablir cette capacité, il faut d'abord
porter le lanceur vers les sous-commandes actuelles, établir le stockage
persistant réellement pris en charge, puis ajouter des tests d'intégration
et valider l'image Docker. Aucun déploiement Ghost Node ni inbox persistante
compatible avec ce workspace n'est établi par les sources actuelles.

**Sandbox durcie (Phase 2, 2026-08-07) :** `ProtectHome=yes`,
`InaccessiblePaths=~/.polygone` (l'identité est **illisible** depuis la
sandbox — plus de vol de clés en une requête), `PrivateDevices`,
`PrivateNetwork`, `RestrictAddressFamilies`, `SystemCallFilter`, CPU/RAM
bornés, **2 exécutions max en parallèle**, unité transitoire arrêtée au
timeout (`RuntimeMaxSec` + `stop --wait` — pas d'orphelins), et WASM sous
**fuel metering** (une boucle infinie trappe, le nœud ne gèle plus),
limites strictes de compilation, sortie bornée à 8 Ko **pendant**
l'écriture, exécution hors de l'event loop (`spawn_blocking`).

**Authentification (Phase 4, contre-attaque) :** les requêtes RES sont
**signées ML-DSA** (comme les messages) et vérifiées par le fantôme
(fraîcheur ±300 s, ancre `peers.json` TOFU; correctif fail-closed et
renforcements de persistance revus par Tilleul sur le snapshot `33626ec…`; la
validation native Windows du test parent absent reste ouverte) ; les **grants sont signés**
et vérifiés par l'emprunteur (un relay malveillant ne peut plus forger
un grant) ; la **réputation est réellement enregistrée** côté fantôme
(échec sur requête non authentifiée) — le portillon n'est plus mort.

**Honnêteté :** isolation contre les accidents et l'abus opportuniste —
pas une sandbox cryptographique (même UID). **Confidentialité du canal
RES : les req/grants (tâches, sorties) transitent en clair sur le relay
— contrairement aux messages ML-KEM/AES-GCM.** Un fantôme exécutant des
calculs sensibles expose leur contenu au relay : ne pas utiliser le RES
pour des données confidentielles (décision documentée).

---

## 6. The daemon (polygoned)

Boucle de 5 s : snapshot système → `GlowUpEngine::tick` → apply
(re-nice + `memory.max` cgroup). Socket de commande
(`~/.polygone/daemon.sock`) : `set_alloc / shrink / grow / status`.

**Honnêteté :** aucun process ne lit le socket aujourd'hui ; les
allocations bande passante/GPU sont *calculées, pas appliquées* ;
`user_active()` Linux renvoie `false` en dur. Le daemon s'auto-limite,
il ne pilote pas encore le réseau — décision D5 en cours.

---

## 7. The naming

| Symbol | What it is |
| ------ | ---------- |
| `Polygone` | The ecosystem. The name on the box. |
| `polygone` | The product binary (TUI + CLI). |
| `polygone-relay` | The relay. Stateless. |
| `polygoned` | The resource daemon. |
| `lvs0` | Example node ID (Lévy, single node). |

---

## 8. The non-goals

Polygone will **not** :

- replace your email
- store your photos in the cloud
- integrate with Slack / Discord / Twitter
- ask you for an account
- phone home
- be a token (le ledger POLY archivé attend une décision explicite —
  il n'est pas dans le produit)
- be a DAO
- be a "Web3" thing
- prétendre que le relay ne voit rien (il voit les métadonnées de routage)

Polygone **will** :

- run on your machine
- encrypt by default (ML-KEM-1024 + AES-256-GCM, testé)
- crash loudly
- refuse to run broken crypto (self-test au démarrage)
- be readable in one sitting
- have a TUI you can use over SSH on a 80x24 terminal
- document its threats in writing, including the ones it does not stop

---

## 9. Cross-document map

| Doc | Rôle | Statut |
|-----|------|--------|
| `README.md` | Manifeste + quickstart | ✅ aligné sur workspace 2.0.0; 176 + 1 doc-test local |
| `ARCHITECTURE.md` | Comment c'est construit (6 Cargo packages) | ✅ mis à jour 2026-10-16 |
| `PHILOSOPHY.md` | Les 5 axiomes + invariants exécutables | ✅ réparé 2026-08-07 |
| `STAGING.md` | Services parkés + conditions de retour | ⚠️ tableau daté 2026-08-06; réconcilier avec ce registre |
| `DECISIONS.md` | Décisions binaires | 🟡 D5 à trancher (Phase 1) |
| `docs/threat-*.md` | Modèles de menace | ✅ non-dits écrits |
| `docs/HYPER-INTEGRATION-BOUNDARY.md` | Proposition non normative de frontière API | 🟡 à valider par les responsables HYPER/Compute |
| `LEGAL.md` | Subpoena, kill-switch, AGPL-3.0 | 🟡 licence à aligner |
| `RELEASE-STATUS.md` | Version, tests, gates et risques du snapshot local | ✅ référence de validation courante |
| `POLYGONE_SHIP.md` | Bilan de livraison du 20 août 2026 | 📚 archive historique, ne pas traiter comme état courant |
