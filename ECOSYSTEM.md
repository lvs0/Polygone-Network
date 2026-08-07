# ECOSYSTEM.md — the mother file

> *Si tu ne sais pas où regarder, c'est ici.*
>
> **Mis à jour 2026-08-07 (produit++, Phase 0).** This document is the
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
  **routing metadata** (`from`, `to`, `session`, sizes, file name) but
  **never the content** — every payload is ML-KEM-1024 + AES-256-GCM.
  It holds no state to disk: restart = full amnesia.

---

## 2. The service registry (v2.0.0-rc2)

The real product is 5 live services + 3 parked. "Live" means the code
exists, compiles, and is tested in the workspace.

| ID | Name | Role | Statut |
|----|------|------|--------|
| `msg` | Polygone Msg | Messages E2E éphémères via relay (4/7) | 🟢 **Live** |
| `drive` | Polygone Drive | Fichiers E2E via relay (4/7) → `~/.polygone/received/` | 🟢 **Live** |
| `brain` | Polygone Brain | IA locale (`polygone petals` → Ollama, zéro cloud) | 🟢 **Live** |
| `mesh` | Polygone Mesh | Découverte LAN + envoi zéro-config (UDP 7642) | 🟢 **Live** |
| `compute` | Polygone Compute | Lend/borrow compute : visibilité + grant + exécution sandboxée (shell + WASM) | 🟢 **Live (MVP)** |
| `hide` | Polygone Hide | Proxy SOCKS5/HTTPS anonymisant à travers le mesh | ⚪ Staging |
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

`~/.polygone/identity.json` (chmod 600) : pseudo + clés ML-KEM-1024 et
ML-DSA-65. `polygone clef` = clé publique à partager. `polygone duress
--confirmer` = destruction identité + fichiers reçus (Axiome 5).

### TUI

Style vim, deux commandes de premier niveau : `:envoyer` / `:quitter`.
Le reste derrière `:` — `:recevoir :clef :voisins :compute :ia :demo
:executer :wasm :statut :aide` (Axiome 2 : deux tons, pas trois).

### RES — exécution

`polygone compute --emprunter <node> --via <relay>` → grant du nœud
fantôme (`ecouter --compute`) → `--executer <tâche>` (shell sandboxé
`systemd-run --user`) ou `:wasm <fichier>` (wasmi/WASI). Honnête : la
sandbox shell isole contre les accidents, pas contre un attaquant
local (même UID) — durcissement en cours (Phase 2).

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
| `README.md` | Manifeste + quickstart | ✅ aligné rc2 |
| `ARCHITECTURE.md` | Comment c'est construit (4 crates) | ✅ réécrit 2026-08-07 |
| `PHILOSOPHY.md` | Les 5 axiomes + invariants exécutables | ✅ réparé 2026-08-07 |
| `STAGING.md` | Services parkés + conditions de retour | ✅ à jour |
| `DECISIONS.md` | Décisions binaires | 🟡 D5 à trancher (Phase 1) |
| `docs/threat-*.md` | Modèles de menace | ✅ non-dits écrits |
| `LEGAL.md` | Subpoena, kill-switch, AGPL-3.0 | 🟡 licence à aligner |
